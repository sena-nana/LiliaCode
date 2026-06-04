use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::util::now_millis;

use super::repository::select_by_task;
use super::types::{AgentTodoItem, TaskTodo};

pub(crate) fn parse_agent_todo_items(values: &[JsonValue]) -> Vec<AgentTodoItem> {
    values
        .iter()
        .filter_map(|value| {
            if let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                return Some(AgentTodoItem {
                    content: text.to_string(),
                    status: "pending".to_string(),
                    completed: None,
                    done: None,
                    priority: None,
                });
            }
            let mut item = serde_json::from_value::<AgentTodoItem>(value.clone()).ok()?;
            item.content = item.content.trim().to_string();
            if item.content.is_empty() {
                return None;
            }
            Some(item)
        })
        .collect()
}

pub(crate) fn apply_agent_event_impl(
    conn: &Connection,
    task_id: &str,
    todos: &[AgentTodoItem],
) -> Result<Vec<TaskTodo>, String> {
    let now = now_millis();

    let mut existing: HashMap<String, String> = HashMap::new();
    let mut duplicate_existing_ids = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                r#"SELECT id, text FROM task_todos
                   WHERE task_id = ?1 AND source = 'agent'
                   ORDER BY "order" ASC, created_at ASC"#,
            )
            .map_err(|e| format!("apply_agent_event: prepare 失败：{e}"))?;
        let rows = stmt
            .query_map(params![task_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("apply_agent_event: query 失败：{e}"))?;
        for r in rows {
            let (id, text) = r.map_err(|e| format!("apply_agent_event: row 失败：{e}"))?;
            if existing.contains_key(&text) {
                duplicate_existing_ids.push(id);
            } else {
                existing.insert(text, id);
            }
        }
    }

    let lilia_max: i64 = conn
        .query_row(
            r#"SELECT COALESCE(MAX("order"), -1) FROM task_todos
               WHERE task_id = ?1 AND source = 'lilia'"#,
            params![task_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("apply_agent_event: 查 lilia_max 失败：{e}"))?;

    let mut seen_texts: HashSet<String> = HashSet::new();
    let mut next_index = 0i64;

    for item in todos {
        let text = item.content.trim().to_string();
        if text.is_empty() {
            continue;
        }
        if !seen_texts.insert(text.clone()) {
            continue;
        }
        let done = item.is_done();
        let priority = item.normalized_priority();
        let order = lilia_max + 1 + next_index;
        next_index += 1;

        if let Some(id) = existing.get(&text) {
            conn.execute(
                r#"UPDATE task_todos
                   SET done = ?1, "order" = ?2, priority = ?3, guide_status = NULL, updated_at = ?4
                   WHERE id = ?5"#,
                params![if done { 1 } else { 0 }, order, priority, now, id],
            )
            .map_err(|e| format!("apply_agent_event: update 失败：{e}"))?;
        } else {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                r#"INSERT INTO task_todos
                   (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, 'agent', ?6, NULL, '[]', ?7, ?8)"#,
                params![id, task_id, text, if done { 1 } else { 0 }, order, priority, now, now],
            )
            .map_err(|e| format!("apply_agent_event: insert 失败：{e}"))?;
        }
    }

    for (text, id) in &existing {
        if !seen_texts.contains(text) {
            conn.execute("DELETE FROM task_todos WHERE id = ?1", params![id])
                .map_err(|e| format!("apply_agent_event: delete 失败：{e}"))?;
        }
    }
    for id in duplicate_existing_ids {
        conn.execute("DELETE FROM task_todos WHERE id = ?1", params![id])
            .map_err(|e| format!("apply_agent_event: delete duplicate 失败：{e}"))?;
    }

    select_by_task(conn, task_id)
}

#[cfg(test)]
mod tests {
    use super::super::types::PRIORITY_HIGH;
    use super::*;

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
            CREATE INDEX idx_task_todos_task_id_order
              ON task_todos(task_id, "order");
            "#,
        )
        .unwrap();
    }

    #[test]
    fn agent_mirror_dedupes_duplicate_text_items() {
        let conn = Connection::open_in_memory().unwrap();
        create_task_todos_schema(&conn);

        let updated = apply_agent_event_impl(
            &conn,
            "task-1",
            &[
                AgentTodoItem {
                    content: "重复任务".to_string(),
                    status: "pending".to_string(),
                    completed: None,
                    done: None,
                    priority: Some("high".to_string()),
                },
                AgentTodoItem {
                    content: "重复任务".to_string(),
                    status: "completed".to_string(),
                    completed: None,
                    done: None,
                    priority: Some("low".to_string()),
                },
            ],
        )
        .unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].text, "重复任务");
        assert!(!updated[0].done);
        assert_eq!(updated[0].priority, PRIORITY_HIGH);

        let row_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM task_todos", [], |row| row.get(0))
            .unwrap();
        assert_eq!(row_count, 1);
    }

    #[test]
    fn agent_mirror_removes_existing_duplicate_text_rows() {
        let conn = Connection::open_in_memory().unwrap();
        create_task_todos_schema(&conn);
        conn.execute(
            r#"INSERT INTO task_todos
               (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
               VALUES (?1, 'task-1', '重复任务', 0, ?2, 'agent', 'normal', NULL, '[]', ?3, ?4)"#,
            params!["first", 0, 1, 1],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO task_todos
               (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
               VALUES (?1, 'task-1', '重复任务', 1, ?2, 'agent', 'low', NULL, '[]', ?3, ?4)"#,
            params!["second", 1, 1, 1],
        )
        .unwrap();

        let updated = apply_agent_event_impl(
            &conn,
            "task-1",
            &[AgentTodoItem {
                content: "重复任务".to_string(),
                status: "completed".to_string(),
                completed: None,
                done: None,
                priority: None,
            }],
        )
        .unwrap();

        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].id, "first");
        assert!(updated[0].done);

        let second_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_todos WHERE id = 'second'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(second_exists, 0);
    }
}
