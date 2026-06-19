use rusqlite::{params, Connection};
use tauri::{AppHandle, State};

use crate::store::LiliaStore;

use super::events::emit_tasks_changed;
use super::relations::{task_project_id, update_descendant_projects, validate_parent};

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
    app: AppHandle,
    new_parent_id: Option<String>,
) -> Result<(), String> {
    let conn = store.conn()?;
    let old_project_id = task_project_id(&conn, &task_id, "task_reparent")?
        .ok_or_else(|| "task_reparent: 任务不存在".to_string())?;
    validate_parent(
        &conn,
        &task_id,
        new_project_id.as_deref(),
        new_parent_id.as_deref(),
        "task_reparent",
    )?;
    let sort_order = next_task_sort_order(&conn, new_project_id.as_deref(), "task_reparent")?;
    conn.execute(
        "UPDATE tasks SET project_id = ?1, parent_id = ?2, sort_order = ?3 WHERE id = ?4",
        params![
            new_project_id.as_deref(),
            new_parent_id.as_deref(),
            sort_order,
            task_id
        ],
    )
    .map_err(|e| format!("task_reparent: update 失败：{e}"))?;
    if old_project_id.as_deref() != new_project_id.as_deref() {
        update_descendant_projects(&conn, &task_id, new_project_id.as_deref(), "task_reparent")?;
        emit_tasks_changed(&app, old_project_id);
    }
    emit_tasks_changed(&app, new_project_id);
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
            CREATE TABLE task_dependencies (
              task_id       TEXT NOT NULL,
              depends_on_id TEXT NOT NULL,
              PRIMARY KEY (task_id, depends_on_id)
            );
            "#,
        )
        .unwrap();
    }

    fn insert_task(conn: &Connection, id: &str, project_id: Option<&str>, parent_id: Option<&str>) {
        conn.execute(
            r#"INSERT INTO tasks
               (id, project_id, session_id, title, status, created_at, parent_id, sort_order)
               VALUES (?1, ?2, ?1, ?1, 'waiting', 1, ?3, 0)"#,
            params![id, project_id, parent_id],
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

    #[test]
    fn validate_parent_rejects_self_cross_project_and_cycles() {
        let conn = Connection::open_in_memory().unwrap();
        create_tasks_schema(&conn);
        insert_task(&conn, "parent", Some("p1"), None);
        insert_task(&conn, "child", Some("p1"), Some("parent"));
        insert_task(&conn, "other", Some("p2"), None);

        assert!(
            validate_parent(&conn, "child", Some("p1"), Some("child"), "test")
                .unwrap_err()
                .contains("自己的父任务")
        );
        assert!(
            validate_parent(&conn, "child", Some("p1"), Some("other"), "test")
                .unwrap_err()
                .contains("同一项目")
        );
        assert!(
            validate_parent(&conn, "parent", Some("p1"), Some("child"), "test")
                .unwrap_err()
                .contains("循环")
        );
        assert!(validate_parent(&conn, "child", Some("p1"), Some("parent"), "test").is_ok());
    }

    #[test]
    fn update_descendant_projects_moves_subtree_project_scope() {
        let conn = Connection::open_in_memory().unwrap();
        create_tasks_schema(&conn);
        insert_task(&conn, "parent", Some("p1"), None);
        insert_task(&conn, "child", Some("p1"), Some("parent"));
        insert_task(&conn, "grandchild", Some("p1"), Some("child"));

        update_descendant_projects(&conn, "parent", Some("p2"), "test").unwrap();

        let child_project: Option<String> = conn
            .query_row(
                "SELECT project_id FROM tasks WHERE id = 'child'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let grandchild_project: Option<String> = conn
            .query_row(
                "SELECT project_id FROM tasks WHERE id = 'grandchild'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(child_project.as_deref(), Some("p2"));
        assert_eq!(grandchild_project.as_deref(), Some("p2"));
    }
}
