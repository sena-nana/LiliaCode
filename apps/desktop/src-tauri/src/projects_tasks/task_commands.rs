use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

use super::events::emit_tasks_changed;
use super::ordering::next_task_sort_order;
use super::queries::{build_task, insert_task_with_deps, load_project_deps, load_task_deps};
use super::types::{NewTask, TaskRow};

#[tauri::command]
pub fn task_list(
    project_id: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<Vec<TaskRow>, String> {
    let conn = store.conn()?;
    let deps = load_project_deps(&conn, project_id.as_deref())?;
    let mut out = Vec::new();
    match &project_id {
        Some(pid) => {
            let mut stmt = conn
                .prepare(
                    r#"SELECT id, project_id, session_id, title, status, created_at, parent_id, sort_order, pinned
                       FROM tasks
                       WHERE project_id = ?1 AND archived = 0
                       ORDER BY pinned DESC, sort_order ASC"#,
                )
                .map_err(|e| format!("task_list: prepare 失败：{e}"))?;
            let rows = stmt
                .query_map(params![pid], |row| build_task(row, &deps))
                .map_err(|e| format!("task_list: query 失败：{e}"))?;
            for r in rows {
                out.push(r.map_err(|e| format!("task_list: row 失败：{e}"))?);
            }
        }
        None => {
            let mut stmt = conn
                .prepare(
                    r#"SELECT id, project_id, session_id, title, status, created_at, parent_id, sort_order, pinned
                       FROM tasks
                       WHERE project_id IS NULL AND archived = 0
                       ORDER BY pinned DESC, sort_order ASC"#,
                )
                .map_err(|e| format!("task_list: prepare 失败：{e}"))?;
            let rows = stmt
                .query_map([], |row| build_task(row, &deps))
                .map_err(|e| format!("task_list: query 失败：{e}"))?;
            for r in rows {
                out.push(r.map_err(|e| format!("task_list: row 失败：{e}"))?);
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn task_get(id: String, store: State<'_, LiliaStore>) -> Result<Option<TaskRow>, String> {
    let conn = store.conn()?;
    let deps_map = load_task_deps(&conn, &id)?;
    let result = conn
        .query_row(
            r#"SELECT id, project_id, session_id, title, status, created_at, parent_id, sort_order, pinned
               FROM tasks WHERE id = ?1 AND archived = 0"#,
            params![id],
            |row| build_task(row, &deps_map),
        )
        .optional()
        .map_err(|e| format!("task_get: {e}"))?;
    Ok(result)
}

#[tauri::command]
pub fn task_create(
    project_id: Option<String>,
    title: String,
    status: String,
    parent_id: Option<String>,
    depends_on: Vec<String>,
    store: State<'_, LiliaStore>,
    app: AppHandle,
) -> Result<TaskRow, String> {
    let conn = store.conn()?;
    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    let sort_order = next_task_sort_order(&conn, project_id.as_deref(), "task_create")?;
    insert_task_with_deps(
        &conn,
        NewTask {
            id: &id,
            project_id: project_id.as_deref(),
            session_id: &id,
            title: &title,
            status: &status,
            created_at: now,
            parent_id: parent_id.as_deref(),
            sort_order,
            depends_on: &depends_on,
        },
        "task_create",
    )?;
    let row = TaskRow {
        id: id.clone(),
        project_id: project_id.clone(),
        session_id: id,
        title,
        status,
        created_at: now,
        parent_id,
        depends_on,
        sort_order,
        pinned: false,
    };
    emit_tasks_changed(&app, project_id);
    Ok(row)
}

#[tauri::command]
pub fn task_update(
    id: String,
    title: Option<String>,
    status: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    if title.is_none() && status.is_none() {
        return Ok(());
    }
    let conn = store.conn()?;
    if let Some(t) = title {
        conn.execute("UPDATE tasks SET title = ?1 WHERE id = ?2", params![t, id])
            .map_err(|e| format!("task_update(title): {e}"))?;
    }
    if let Some(s) = status {
        conn.execute("UPDATE tasks SET status = ?1 WHERE id = ?2", params![s, id])
            .map_err(|e| format!("task_update(status): {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn task_delete(id: String, store: State<'_, LiliaStore>) -> Result<(), String> {
    let conn = store.conn()?;
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|e| format!("task_delete: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn task_promote(
    id: String,
    project_id: Option<String>,
    title: String,
    depends_on: Vec<String>,
    store: State<'_, LiliaStore>,
    app: AppHandle,
) -> Result<TaskRow, String> {
    let conn = store.conn()?;
    let now = now_millis();
    let sort_order = next_task_sort_order(&conn, project_id.as_deref(), "task_promote")?;
    insert_task_with_deps(
        &conn,
        NewTask {
            id: &id,
            project_id: project_id.as_deref(),
            session_id: &id,
            title: &title,
            status: "running",
            created_at: now,
            parent_id: None,
            sort_order,
            depends_on: &depends_on,
        },
        "task_promote",
    )?;
    let row = TaskRow {
        id: id.clone(),
        project_id: project_id.clone(),
        session_id: id,
        title,
        status: "running".to_string(),
        created_at: now,
        parent_id: None,
        depends_on,
        sort_order,
        pinned: false,
    };
    emit_tasks_changed(&app, project_id);
    Ok(row)
}

#[tauri::command]
pub fn task_archive_project(
    project_id: String,
    store: State<'_, LiliaStore>,
) -> Result<i64, String> {
    let conn = store.conn()?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND archived = 0",
            params![project_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("task_archive_project: count 失败：{e}"))?;
    conn.execute(
        "UPDATE tasks SET archived = 1 WHERE project_id = ?1 AND archived = 0",
        params![project_id],
    )
    .map_err(|e| format!("task_archive_project: update 失败：{e}"))?;
    Ok(count)
}

#[tauri::command]
pub fn task_archive(id: String, store: State<'_, LiliaStore>) -> Result<bool, String> {
    let conn = store.conn()?;
    let changed = conn
        .execute(
            "UPDATE tasks SET archived = 1 WHERE id = ?1 AND archived = 0",
            params![id],
        )
        .map_err(|e| format!("task_archive: {e}"))?;
    Ok(changed > 0)
}

#[tauri::command]
pub fn task_toggle_pin(id: String, store: State<'_, LiliaStore>) -> Result<bool, String> {
    let conn = store.conn()?;
    let current: i64 = conn
        .query_row(
            "SELECT pinned FROM tasks WHERE id = ?1 AND archived = 0",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("task_toggle_pin: 查询失败：{e}"))?;
    let new_val = if current == 0 { 1i64 } else { 0i64 };
    conn.execute(
        "UPDATE tasks SET pinned = ?1 WHERE id = ?2",
        params![new_val, id],
    )
    .map_err(|e| format!("task_toggle_pin: 更新失败：{e}"))?;
    Ok(new_val != 0)
}
