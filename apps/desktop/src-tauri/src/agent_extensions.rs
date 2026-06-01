use rusqlite::Connection;
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::agent_events::{AgentEventEffect, AgentExtension, AgentRuntimeEvent, AgentTurnContext};
use crate::store::LiliaStore;
use crate::todos;

fn todo_event_items(event: &AgentRuntimeEvent) -> Option<&[JsonValue]> {
    match event {
        AgentRuntimeEvent::TodoList { items } => Some(items.as_slice()),
        AgentRuntimeEvent::ToolUse { name, input } if name == "TodoWrite" => input
            .get("todos")
            .and_then(|value| value.as_array())
            .map(Vec::as_slice),
        _ => None,
    }
}

#[cfg(test)]
fn apply_todo_runtime_event(
    conn: &Connection,
    task_id: &str,
    event: &AgentRuntimeEvent,
) -> Result<bool, String> {
    let Some(items) = todo_event_items(event) else {
        return Ok(false);
    };
    apply_todo_event_items(conn, task_id, items)?;
    Ok(true)
}

fn apply_todo_event_items(
    conn: &Connection,
    task_id: &str,
    items: &[JsonValue],
) -> Result<(), String> {
    let parsed = todos::parse_agent_todo_items(items);
    todos::apply_agent_event_impl(conn, task_id, &parsed).map(|_| ())
}

pub struct TodoMirrorExtension<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TodoMirrorExtension<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> AgentExtension for TodoMirrorExtension<R> {
    fn id(&self) -> &'static str {
        "todo-mirror"
    }

    fn on_event(
        &self,
        ctx: &AgentTurnContext,
        event: &AgentRuntimeEvent,
    ) -> Result<AgentEventEffect, String> {
        let Some(items) = todo_event_items(event) else {
            return Ok(AgentEventEffect::default());
        };

        let Some(store) = self.app.try_state::<LiliaStore>() else {
            return Err("LiliaStore is not available".to_string());
        };
        let conn = store.conn()?;
        apply_todo_event_items(&conn, &ctx.task_id, items)?;
        self.app
            .emit(
                "todo-changed",
                serde_json::json!({
                    "taskId": ctx.task_id,
                }),
            )
            .map_err(|err| format!("todo-changed emit failed: {err}"))?;

        Ok(AgentEventEffect::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use serde_json::json;

    fn create_task_todos_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE task_todos (
              id           TEXT PRIMARY KEY,
              task_id      TEXT NOT NULL,
              text         TEXT NOT NULL,
              done         INTEGER NOT NULL DEFAULT 0,
              "order"      INTEGER NOT NULL,
              source       TEXT NOT NULL CHECK (source IN ('lilia','agent')),
              priority     TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('high','normal','low')),
              guide_status TEXT CHECK (guide_status IS NULL OR guide_status IN ('pending','queued','sent')),
              attachments_json TEXT NOT NULL DEFAULT '[]',
              created_at   INTEGER NOT NULL,
              updated_at   INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();
    }

    #[test]
    fn todo_write_tool_event_updates_task_todos() {
        let conn = Connection::open_in_memory().unwrap();
        create_task_todos_schema(&conn);

        let handled = apply_todo_runtime_event(
            &conn,
            "task-1",
            &AgentRuntimeEvent::ToolUse {
                name: "TodoWrite".to_string(),
                input: json!({
                    "todos": [
                        { "content": "Draft event kernel", "status": "completed", "priority": "high" },
                        { "content": "Wire extension host", "status": "pending" }
                    ]
                }),
            },
        )
        .unwrap();

        assert!(handled);
        let rows: Vec<(String, i64, String, String, Option<String>)> = {
            let mut stmt = conn
                .prepare(
                    r#"SELECT text, done, source, priority, guide_status
                       FROM task_todos WHERE task_id = ?1 ORDER BY "order" ASC"#,
                )
                .unwrap();
            stmt.query_map(params!["task-1"], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };

        assert_eq!(
            rows,
            vec![
                (
                    "Draft event kernel".to_string(),
                    1,
                    "agent".to_string(),
                    "high".to_string(),
                    None
                ),
                (
                    "Wire extension host".to_string(),
                    0,
                    "agent".to_string(),
                    "normal".to_string(),
                    None
                ),
            ]
        );
    }

    #[test]
    fn provider_todo_list_event_updates_task_todos() {
        let conn = Connection::open_in_memory().unwrap();
        create_task_todos_schema(&conn);

        let handled = apply_todo_runtime_event(
            &conn,
            "task-1",
            &AgentRuntimeEvent::TodoList {
                items: vec![
                    json!({ "text": "Mirror provider todo", "completed": true, "priority": "low" }),
                    json!({ "content": "Keep Claude shape", "status": "pending" }),
                    json!({ "text": "Done alias", "done": true }),
                ],
            },
        )
        .unwrap();

        assert!(handled);
        let rows: Vec<(String, i64, String, String)> = {
            let mut stmt = conn
                .prepare(
                    r#"SELECT text, done, source, priority
                       FROM task_todos WHERE task_id = ?1 ORDER BY "order" ASC"#,
                )
                .unwrap();
            stmt.query_map(params!["task-1"], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };

        assert_eq!(
            rows,
            vec![
                (
                    "Mirror provider todo".to_string(),
                    1,
                    "agent".to_string(),
                    "low".to_string()
                ),
                (
                    "Keep Claude shape".to_string(),
                    0,
                    "agent".to_string(),
                    "normal".to_string()
                ),
                (
                    "Done alias".to_string(),
                    1,
                    "agent".to_string(),
                    "normal".to_string()
                ),
            ]
        );
    }

    #[test]
    fn non_todo_write_tool_event_does_not_update_task_todos() {
        let conn = Connection::open_in_memory().unwrap();
        create_task_todos_schema(&conn);

        let handled = apply_todo_runtime_event(
            &conn,
            "task-1",
            &AgentRuntimeEvent::ToolUse {
                name: "Read".to_string(),
                input: json!({ "file": "README.md" }),
            },
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task_todos", [], |row| row.get(0))
            .unwrap();

        assert!(!handled);
        assert_eq!(count, 0);
    }
}
