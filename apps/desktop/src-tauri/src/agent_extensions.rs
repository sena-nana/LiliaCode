use rusqlite::Connection;
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::agent_events::{AgentEventEffect, AgentExtension, AgentRuntimeEvent, AgentTurnContext};
use crate::store::LiliaStore;
use crate::todos::{self, AgentTodoItem};

pub fn apply_todo_tool_event(
    conn: &Connection,
    task_id: &str,
    name: &str,
    input: &JsonValue,
) -> Result<bool, String> {
    if name != "TodoWrite" {
        return Ok(false);
    }

    let Some(todos) = input.get("todos").and_then(|value| value.as_array()) else {
        return Ok(false);
    };

    let parsed: Vec<AgentTodoItem> = todos
        .iter()
        .filter_map(|value| serde_json::from_value::<AgentTodoItem>(value.clone()).ok())
        .collect();
    todos::apply_agent_event_impl(conn, task_id, &parsed)?;
    Ok(true)
}

pub fn apply_todo_tool_event_and_notify<F>(
    conn: &Connection,
    task_id: &str,
    name: &str,
    input: &JsonValue,
    mut notify: F,
) -> Result<bool, String>
where
    F: FnMut(&str) -> Result<(), String>,
{
    let handled = apply_todo_tool_event(conn, task_id, name, input)?;
    if handled {
        notify(task_id)?;
    }
    Ok(handled)
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
        let AgentRuntimeEvent::ToolUse { name, input } = event else {
            return Ok(AgentEventEffect::default());
        };
        if name != "TodoWrite" {
            return Ok(AgentEventEffect::default());
        }

        let Some(store) = self.app.try_state::<LiliaStore>() else {
            return Err("LiliaStore is not available".to_string());
        };
        let conn = store.conn()?;
        apply_todo_tool_event_and_notify(&conn, &ctx.task_id, name, input, |task_id| {
            self.app
                .emit(
                    "todo-changed",
                    serde_json::json!({
                        "taskId": task_id,
                    }),
                )
                .map_err(|err| format!("todo-changed emit failed: {err}"))
        })?;

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
              source       TEXT NOT NULL CHECK (source IN ('user','agent')),
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

        let handled = apply_todo_tool_event(
            &conn,
            "task-1",
            "TodoWrite",
            &json!({
                "todos": [
                    { "content": "Draft event kernel", "status": "completed" },
                    { "content": "Wire extension host", "status": "pending" }
                ]
            }),
        )
        .unwrap();

        assert!(handled);
        let rows: Vec<(String, i64, String)> = {
            let mut stmt = conn
                .prepare(
                    r#"SELECT text, done, source
                       FROM task_todos WHERE task_id = ?1 ORDER BY "order" ASC"#,
                )
                .unwrap();
            stmt.query_map(params!["task-1"], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };

        assert_eq!(
            rows,
            vec![
                ("Draft event kernel".to_string(), 1, "agent".to_string()),
                ("Wire extension host".to_string(), 0, "agent".to_string()),
            ]
        );
    }

    #[test]
    fn non_todo_write_tool_event_does_not_update_task_todos() {
        let conn = Connection::open_in_memory().unwrap();
        create_task_todos_schema(&conn);

        let handled = apply_todo_tool_event(
            &conn,
            "task-1",
            "Read",
            &json!({ "file": "README.md" }),
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task_todos", [], |row| row.get(0))
            .unwrap();

        assert!(!handled);
        assert_eq!(count, 0);
    }

    #[test]
    fn todo_write_tool_event_notifies_after_update() {
        let conn = Connection::open_in_memory().unwrap();
        create_task_todos_schema(&conn);
        let mut notifications = Vec::new();

        let handled = apply_todo_tool_event_and_notify(
            &conn,
            "task-1",
            "TodoWrite",
            &json!({
                "todos": [
                    { "content": "Mirror todo", "status": "pending" }
                ]
            }),
            |task_id| {
                notifications.push(task_id.to_string());
                Ok(())
            },
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task_todos", [], |row| row.get(0))
            .unwrap();
        assert!(handled);
        assert_eq!(count, 1);
        assert_eq!(notifications, vec!["task-1".to_string()]);
    }
}
