use lilia_feature_automation::AutomationSignalEnvelope;
use lilia_storage::Db;
use rusqlite::{params, OptionalExtension};

use super::DesktopApplicationError;

pub(super) struct AutomationInbox {
    db: Db,
}
pub(super) struct Delivery {
    pub signal: AutomationSignalEnvelope,
    pub workflow_id: String,
}

impl AutomationInbox {
    pub(super) fn new(db: Db) -> Result<Self, DesktopApplicationError> {
        db.lock()
            .execute_batch(
                r#"
            CREATE TABLE IF NOT EXISTS desktop_automation_signals (
                id TEXT PRIMARY KEY, signal_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS desktop_automation_deliveries (
                signal_id TEXT NOT NULL, workflow_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending', run_id TEXT, error TEXT,
                PRIMARY KEY(signal_id, workflow_id)
            );
            CREATE TABLE IF NOT EXISTS desktop_automation_sources (
                source_key TEXT PRIMARY KEY, value TEXT NOT NULL, revision INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS desktop_automation_origins (
                source_key TEXT PRIMARY KEY, run_id TEXT NOT NULL
            );
        "#,
            )
            .map_err(error)?;
        {
            let connection = db.lock();
            connection.execute_batch("CREATE TABLE IF NOT EXISTS desktop_automation_todo_events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL, todo_id TEXT NOT NULL, action TEXT NOT NULL, payload TEXT NOT NULL);").map_err(error)?;
            let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_todos')", [], |r|r.get(0)).map_err(error)?;
            if exists {
                connection.execute_batch(r#"
                    CREATE TRIGGER IF NOT EXISTS desktop_automation_todo_insert AFTER INSERT ON task_todos BEGIN
                      INSERT INTO desktop_automation_todo_events(task_id,todo_id,action,payload) VALUES (NEW.task_id,NEW.id,'created',json_object('text',NEW.text,'done',NEW.done,'priority',NEW.priority));
                    END;
                    CREATE TRIGGER IF NOT EXISTS desktop_automation_todo_update AFTER UPDATE ON task_todos
                    WHEN OLD.text IS NOT NEW.text OR OLD.done IS NOT NEW.done OR OLD.priority IS NOT NEW.priority OR OLD."order" IS NOT NEW."order" OR OLD.attachments_json IS NOT NEW.attachments_json OR OLD.conversation_references_json IS NOT NEW.conversation_references_json OR OLD.workflow_json IS NOT NEW.workflow_json
                    BEGIN
                      INSERT INTO desktop_automation_todo_events(task_id,todo_id,action,payload) VALUES (NEW.task_id,NEW.id,'updated',json_object('text',NEW.text,'done',NEW.done,'priority',NEW.priority));
                    END;
                    CREATE TRIGGER IF NOT EXISTS desktop_automation_todo_delete AFTER DELETE ON task_todos BEGIN
                      INSERT INTO desktop_automation_todo_events(task_id,todo_id,action,payload) VALUES (OLD.task_id,OLD.id,'deleted',json_object('text',OLD.text,'done',OLD.done,'priority',OLD.priority));
                    END;
                "#).map_err(error)?;
            }
        }
        Ok(Self { db })
    }
    pub(super) fn todo_events_after(
        &self,
        sequence: i64,
    ) -> Result<Vec<(i64, String, String, String, serde_json::Value)>, DesktopApplicationError>
    {
        let db = self.db.lock();
        let mut query = db.prepare("SELECT sequence,task_id,todo_id,action,payload FROM desktop_automation_todo_events WHERE sequence>?1 ORDER BY sequence LIMIT 200").map_err(error)?;
        let rows = query
            .query_map([sequence], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(error)?;
        rows.map(|row| {
            let (sequence, task, todo, action, payload) = row.map_err(error)?;
            Ok((
                sequence,
                task,
                todo,
                action,
                serde_json::from_str(&payload).map_err(error)?,
            ))
        })
        .collect()
    }
    pub(super) fn todo_cursor(&self) -> Result<i64, DesktopApplicationError> {
        self.db
            .lock()
            .query_row(
                "SELECT COALESCE(MAX(sequence),0) FROM desktop_automation_todo_events",
                [],
                |r| r.get(0),
            )
            .map_err(error)
    }
    pub(super) fn enqueue(
        &self,
        signal: &AutomationSignalEnvelope,
        workflows: &[String],
    ) -> Result<(), DesktopApplicationError> {
        self.enqueue_with_checkpoint(signal, workflows, None)
    }
    pub(super) fn enqueue_with_checkpoint(
        &self,
        signal: &AutomationSignalEnvelope,
        workflows: &[String],
        checkpoint: Option<(&str, &str, i64)>,
    ) -> Result<(), DesktopApplicationError> {
        let mut db = self.db.lock();
        let tx = db.transaction().map_err(error)?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO desktop_automation_signals(id, signal_json) VALUES (?1, ?2)",
                params![signal.id, serde_json::to_string(signal).map_err(error)?],
            )
            .map_err(error)?;
        if inserted != 0 {
            for workflow in workflows {
                tx.execute("INSERT INTO desktop_automation_deliveries(signal_id, workflow_id) VALUES (?1, ?2)", params![signal.id, workflow]).map_err(error)?;
            }
        }
        if let Some((key, value, revision)) = checkpoint {
            tx.execute("INSERT INTO desktop_automation_sources(source_key,value,revision) VALUES (?1,?2,?3) ON CONFLICT(source_key) DO UPDATE SET value=excluded.value, revision=excluded.revision", params![key,value,revision]).map_err(error)?;
        }
        tx.commit().map_err(error)
    }
    pub(super) fn pending(
        &self,
        signal_id: Option<&str>,
    ) -> Result<Vec<Delivery>, DesktopApplicationError> {
        let db = self.db.lock();
        let mut query = db.prepare("SELECT s.signal_json, d.workflow_id FROM desktop_automation_deliveries d JOIN desktop_automation_signals s ON s.id=d.signal_id WHERE d.status='pending' AND (?1 IS NULL OR d.signal_id=?1) ORDER BY s.rowid, d.workflow_id").map_err(error)?;
        let rows = query
            .query_map([signal_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(error)?;
        rows.map(|row| {
            let (json, workflow_id) = row.map_err(error)?;
            Ok(Delivery {
                signal: serde_json::from_str(&json).map_err(error)?,
                workflow_id,
            })
        })
        .collect()
    }
    pub(super) fn record(
        &self,
        signal: &str,
        workflow: &str,
        status: &str,
        run: Option<&str>,
        reason: Option<&str>,
    ) -> Result<(), DesktopApplicationError> {
        self.db.lock().execute("UPDATE desktop_automation_deliveries SET status=?3, run_id=?4, error=?5 WHERE signal_id=?1 AND workflow_id=?2", params![signal,workflow,status,run,reason]).map_err(error)?;
        Ok(())
    }
    pub(super) fn runs(&self, signal: &str) -> Result<Vec<String>, DesktopApplicationError> {
        let db = self.db.lock();
        let mut query=db.prepare("SELECT run_id FROM desktop_automation_deliveries WHERE signal_id=?1 AND run_id IS NOT NULL ORDER BY workflow_id").map_err(error)?;
        let rows = query.query_map([signal], |r| r.get(0)).map_err(error)?;
        rows.map(|r| r.map_err(error)).collect()
    }
    pub(super) fn source(
        &self,
        key: &str,
    ) -> Result<Option<(String, i64)>, DesktopApplicationError> {
        self.db
            .lock()
            .query_row(
                "SELECT value, revision FROM desktop_automation_sources WHERE source_key=?1",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(error)
    }
    pub(super) fn checkpoint(
        &self,
        key: &str,
        value: &str,
        revision: i64,
    ) -> Result<(), DesktopApplicationError> {
        self.db.lock().execute("INSERT INTO desktop_automation_sources(source_key,value,revision) VALUES (?1,?2,?3) ON CONFLICT(source_key) DO UPDATE SET value=excluded.value, revision=excluded.revision", params![key,value,revision]).map_err(error)?;
        Ok(())
    }
    pub(super) fn origin(&self, key: &str) -> Result<Option<String>, DesktopApplicationError> {
        self.db
            .lock()
            .query_row(
                "SELECT run_id FROM desktop_automation_origins WHERE source_key=?1",
                [key],
                |r| r.get(0),
            )
            .optional()
            .map_err(error)
    }
    pub(super) fn record_origin(
        &self,
        key: &str,
        run: &str,
    ) -> Result<(), DesktopApplicationError> {
        self.db.lock().execute("INSERT INTO desktop_automation_origins(source_key,run_id) VALUES (?1,?2) ON CONFLICT(source_key) DO UPDATE SET run_id=excluded.run_id WHERE desktop_automation_origins.run_id=''",params![key,run]).map_err(error)?;
        Ok(())
    }
}
fn error(value: impl std::fmt::Display) -> DesktopApplicationError {
    DesktopApplicationError::InvalidInput {
        field: "automation_inbox",
        message: value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reopening_preserves_pending_delivery_checkpoint_and_turn_origin() {
        let path = std::env::temp_dir().join(format!("lilia-inbox-{}.db", uuid::Uuid::new_v4()));
        let signal = AutomationSignalEnvelope {
            id: "durable-signal".into(),
            kind: "task_changed".into(),
            project_id: None,
            task_id: None,
            backend: None,
            event_kind: None,
            automation_run_id: None,
            payload: serde_json::json!({}),
            created_at: 1,
        };
        {
            let store = AutomationInbox::new(Db::open(&path).unwrap()).unwrap();
            store
                .enqueue(&signal, &["busy-workflow".into(), "ready-workflow".into()])
                .unwrap();
            store
                .record(
                    &signal.id,
                    "ready-workflow",
                    "delivered",
                    Some("ready-run"),
                    None,
                )
                .unwrap();
            store
                .record_origin("turn:task:finished-turn", "original-run")
                .unwrap();
            store.checkpoint("product", "19", 19).unwrap();
        }
        {
            let store = AutomationInbox::new(Db::open(&path).unwrap()).unwrap();
            store.enqueue(&signal, &["new-workflow".into()]).unwrap();
            let pending = store.pending(None).unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].workflow_id, "busy-workflow");
            assert_eq!(store.runs(&signal.id).unwrap(), ["ready-run"]);
            assert_eq!(
                store.origin("turn:task:finished-turn").unwrap().as_deref(),
                Some("original-run")
            );
            assert_eq!(store.source("product").unwrap(), Some(("19".into(), 19)));
        }
        std::fs::remove_file(path).unwrap();
    }
}
