use rusqlite::{params, Connection};
use tauri::State;

use crate::store::LiliaStore;

pub(super) fn next_task_sort_order(
    conn: &Connection,
    project_id: Option<&str>,
    context: &str,
) -> Result<i64, String> {
    let max_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM tasks WHERE (project_id = ?1 OR (project_id IS NULL AND ?1 IS NULL)) AND archived = 0",
            params![project_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("{context}: max sort_order 失败：{e}"))?;
    Ok(max_order + 1)
}

#[tauri::command]
pub fn project_reorder(
    ordered_ids: Vec<String>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    let conn = store.conn()?;
    for (i, id) in ordered_ids.iter().enumerate() {
        conn.execute(
            "UPDATE projects SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )
        .map_err(|e| format!("project_reorder: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn task_reorder(
    _project_id: Option<String>,
    ordered_ids: Vec<String>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    let conn = store.conn()?;
    for (i, id) in ordered_ids.iter().enumerate() {
        conn.execute(
            "UPDATE tasks SET sort_order = ?1 WHERE id = ?2",
            params![i as i64, id],
        )
        .map_err(|e| format!("task_reorder: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn task_reparent(
    task_id: String,
    new_project_id: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    let conn = store.conn()?;
    let sort_order = next_task_sort_order(&conn, new_project_id.as_deref(), "task_reparent")?;
    conn.execute(
        "UPDATE tasks SET project_id = ?1, sort_order = ?2 WHERE id = ?3",
        params![new_project_id, sort_order, task_id],
    )
    .map_err(|e| format!("task_reparent: update 失败：{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_tasks_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE tasks (
              id          TEXT PRIMARY KEY,
              project_id  TEXT,
              session_id  TEXT NOT NULL,
              title       TEXT NOT NULL,
              status      TEXT NOT NULL DEFAULT 'waiting',
              created_at  INTEGER NOT NULL,
              parent_id   TEXT,
              archived    INTEGER NOT NULL DEFAULT 0,
              sort_order  INTEGER NOT NULL DEFAULT 0,
              pinned      INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
        .unwrap();
    }

    #[test]
    fn next_task_sort_order_is_scoped_by_project() {
        let conn = Connection::open_in_memory().unwrap();
        create_tasks_schema(&conn);
        conn.execute(
            "INSERT INTO tasks (id, project_id, session_id, title, status, created_at, sort_order) VALUES ('a', 'p1', 'a', 'A', 'waiting', 1, 4)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, project_id, session_id, title, status, created_at, sort_order) VALUES ('b', NULL, 'b', 'B', 'waiting', 1, 2)",
            [],
        )
        .unwrap();

        assert_eq!(next_task_sort_order(&conn, Some("p1"), "test").unwrap(), 5);
        assert_eq!(next_task_sort_order(&conn, None, "test").unwrap(), 3);
        assert_eq!(next_task_sort_order(&conn, Some("p2"), "test").unwrap(), 0);
    }
}
