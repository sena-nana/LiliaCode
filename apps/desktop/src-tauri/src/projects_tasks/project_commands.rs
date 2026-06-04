use rusqlite::{params, OptionalExtension};
use tauri::State;
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

use super::queries::row_to_project;
use super::types::ProjectRow;

#[tauri::command]
pub fn project_list(store: State<'_, LiliaStore>) -> Result<Vec<ProjectRow>, String> {
    let conn = store.conn()?;
    let mut stmt = conn
        .prepare(
            r#"SELECT p.id, p.name, p.cwd,
                      COUNT(t.id) AS session_count,
                      p.sort_order,
                      p.pinned
               FROM projects p
               LEFT JOIN tasks t
                 ON t.project_id = p.id AND t.archived = 0
               GROUP BY p.id
               ORDER BY p.pinned DESC, p.sort_order ASC"#,
        )
        .map_err(|e| format!("project_list: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map([], row_to_project)
        .map_err(|e| format!("project_list: query 失败：{e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("project_list: row 失败：{e}"))?);
    }
    Ok(out)
}

#[tauri::command]
pub fn project_get(id: String, store: State<'_, LiliaStore>) -> Result<Option<ProjectRow>, String> {
    let conn = store.conn()?;
    let result = conn
        .query_row(
            r#"SELECT p.id, p.name, p.cwd,
                      (SELECT COUNT(*) FROM tasks t WHERE t.project_id = p.id AND t.archived = 0)
                      AS session_count,
                      p.sort_order,
                      p.pinned
               FROM projects p WHERE p.id = ?1"#,
            params![id],
            row_to_project,
        )
        .optional()
        .map_err(|e| format!("project_get: {e}"))?;
    Ok(result)
}

#[tauri::command]
pub fn project_create(
    name: String,
    cwd: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<ProjectRow, String> {
    let conn = store.conn()?;
    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    let trimmed = name.trim();
    let final_name = if trimmed.is_empty() {
        "未命名项目"
    } else {
        trimmed
    };
    let max_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM projects",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("project_create: max sort_order 失败：{e}"))?;
    let sort_order = max_order + 1;
    conn.execute(
        "INSERT INTO projects (id, name, cwd, created_at, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, final_name, cwd, now, sort_order],
    )
    .map_err(|e| format!("project_create: insert 失败：{e}"))?;
    Ok(ProjectRow {
        id,
        name: final_name.to_string(),
        cwd,
        session_count: 0,
        sort_order,
        pinned: false,
    })
}

#[tauri::command]
pub fn project_rename(
    id: String,
    next_name: String,
    store: State<'_, LiliaStore>,
) -> Result<bool, String> {
    let trimmed = next_name.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    let conn = store.conn()?;
    let changed = conn
        .execute(
            "UPDATE projects SET name = ?1 WHERE id = ?2 AND name != ?1",
            params![trimmed, id],
        )
        .map_err(|e| format!("project_rename: {e}"))?;
    Ok(changed > 0)
}

#[tauri::command]
pub fn project_remove(id: String, store: State<'_, LiliaStore>) -> Result<bool, String> {
    let conn = store.conn()?;
    conn.execute(
        "UPDATE tasks SET project_id = NULL WHERE project_id = ?1",
        params![id],
    )
    .map_err(|e| format!("project_remove: orphan tasks 失败：{e}"))?;
    let deleted = conn
        .execute("DELETE FROM projects WHERE id = ?1", params![id])
        .map_err(|e| format!("project_remove: {e}"))?;
    Ok(deleted > 0)
}

#[tauri::command]
pub fn project_toggle_pin(id: String, store: State<'_, LiliaStore>) -> Result<bool, String> {
    let conn = store.conn()?;
    let current: i64 = conn
        .query_row(
            "SELECT pinned FROM projects WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("project_toggle_pin: 查询失败：{e}"))?;
    let new_val = if current == 0 { 1i64 } else { 0i64 };
    conn.execute(
        "UPDATE projects SET pinned = ?1 WHERE id = ?2",
        params![new_val, id],
    )
    .map_err(|e| format!("project_toggle_pin: 更新失败：{e}"))?;
    Ok(new_val != 0)
}
