/*!
 * Project / Task 命令组：SQLite 持久化，替代前端内存 mock 数据。
 *
 * - `projects_*` 系列对应侧栏项目管理（创建 / 重命名 / 删除）。
 * - `tasks_*` 系列对应项目内会话任务（CRUD + 依赖 + 归档）。
 * - Orphan conversation = `project_id IS NULL` 的 task，前端按同一张表过滤。
 * - 草稿（draft）留在前端内存，不落库；`promote` 后才 INSERT。
 */

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::store::LiliaStore;
use crate::util::now_millis;

// ========== 数据结构 ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub cwd: Option<String>,
    pub session_count: i64,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: String,
    pub title: String,
    pub status: String,
    pub created_at: i64,
    pub parent_id: Option<String>,
    pub depends_on: Vec<String>,
    pub sort_order: i64,
}

// ========== 内部辅助 ==========

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: row.get(0)?,
        name: row.get(1)?,
        cwd: row.get(2)?,
        session_count: row.get(3)?,
        sort_order: row.get(4)?,
    })
}

fn build_task(row: &rusqlite::Row<'_>, deps: &std::collections::HashMap<String, Vec<String>>) -> rusqlite::Result<TaskRow> {
    let id: String = row.get(0)?;
    let depends_on = deps.get(&id).cloned().unwrap_or_default();
    Ok(TaskRow {
        id: id.clone(),
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        title: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
        parent_id: row.get(6)?,
        depends_on,
        sort_order: row.get(7)?,
    })
}

fn load_all_deps(conn: &Connection) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let mut stmt = conn
        .prepare("SELECT task_id, depends_on_id FROM task_dependencies")
        .map_err(|e| format!("load_all_deps: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| format!("load_all_deps: query 失败：{e}"))?;
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for r in rows {
        let (tid, did) = r.map_err(|e| format!("load_all_deps: row 失败：{e}"))?;
        map.entry(tid).or_default().push(did);
    }
    Ok(map)
}

// ========== Project 命令 ==========

#[tauri::command]
pub fn project_list(store: State<'_, LiliaStore>) -> Result<Vec<ProjectRow>, String> {
    let conn = store.conn()?;
    let mut stmt = conn
        .prepare(
            r#"SELECT p.id, p.name, p.cwd,
                      COUNT(t.id) AS session_count,
                      p.sort_order
               FROM projects p
               LEFT JOIN tasks t
                 ON t.project_id = p.id AND t.archived = 0
               GROUP BY p.id
               ORDER BY p.sort_order ASC"#,
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
                      p.sort_order
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
    let final_name = if trimmed.is_empty() { "未命名项目" } else { trimmed };
    let max_order: i64 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), -1) FROM projects", [], |r| r.get(0))
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
    // 脱离关联：把该项目下的 task project_id 置 NULL（变成孤儿）
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

// ========== Task 命令 ==========

#[tauri::command]
pub fn task_list(
    project_id: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<Vec<TaskRow>, String> {
    let conn = store.conn()?;
    let deps = load_all_deps(&conn)?;
    let mut out = Vec::new();
    match &project_id {
        Some(pid) => {
            let mut stmt = conn
                .prepare(
                    r#"SELECT id, project_id, session_id, title, status, created_at, parent_id, sort_order
                       FROM tasks
                       WHERE project_id = ?1 AND archived = 0
                       ORDER BY sort_order ASC"#,
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
                    r#"SELECT id, project_id, session_id, title, status, created_at, parent_id, sort_order
                       FROM tasks
                       WHERE project_id IS NULL AND archived = 0
                       ORDER BY sort_order ASC"#,
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
    let deps_map = load_all_deps(&conn)?;
    let result = conn
        .query_row(
            r#"SELECT id, project_id, session_id, title, status, created_at, parent_id, sort_order
               FROM tasks WHERE id = ?1"#,
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
) -> Result<TaskRow, String> {
    let conn = store.conn()?;
    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    let max_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM tasks WHERE (project_id = ?1 OR (project_id IS NULL AND ?1 IS NULL)) AND archived = 0",
            params![project_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("task_create: max sort_order 失败：{e}"))?;
    let sort_order = max_order + 1;
    conn.execute(
        r#"INSERT INTO tasks (id, project_id, session_id, title, status, created_at, parent_id, sort_order)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        params![id, project_id, id, title, status, now, parent_id, sort_order],
    )
    .map_err(|e| format!("task_create: insert 失败：{e}"))?;
    for dep in &depends_on {
        conn.execute(
            "INSERT OR IGNORE INTO task_dependencies (task_id, depends_on_id) VALUES (?1, ?2)",
            params![id, dep],
        )
        .map_err(|e| format!("task_create: insert dep 失败：{e}"))?;
    }
    Ok(TaskRow {
        id: id.clone(),
        project_id,
        session_id: id,
        title,
        status,
        created_at: now,
        parent_id,
        depends_on,
        sort_order,
    })
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
        conn.execute(
            "UPDATE tasks SET title = ?1 WHERE id = ?2",
            params![t, id],
        )
        .map_err(|e| format!("task_update(title): {e}"))?;
    }
    if let Some(s) = status {
        conn.execute(
            "UPDATE tasks SET status = ?1 WHERE id = ?2",
            params![s, id],
        )
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

/// 草稿 promote：前端内存草稿 → 落库。保留原 draft id 作为正式 task id。
#[tauri::command]
pub fn task_promote(
    id: String,
    project_id: Option<String>,
    title: String,
    depends_on: Vec<String>,
    store: State<'_, LiliaStore>,
) -> Result<TaskRow, String> {
    let conn = store.conn()?;
    let now = now_millis();
    let max_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM tasks WHERE (project_id = ?1 OR (project_id IS NULL AND ?1 IS NULL)) AND archived = 0",
            params![project_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("task_promote: max sort_order 失败：{e}"))?;
    let sort_order = max_order + 1;
    conn.execute(
        r#"INSERT INTO tasks (id, project_id, session_id, title, status, created_at, parent_id, sort_order)
           VALUES (?1, ?2, ?3, ?4, 'running', ?5, NULL, ?6)"#,
        params![id, project_id, id, title, now, sort_order],
    )
    .map_err(|e| format!("task_promote: insert 失败：{e}"))?;
    for dep in &depends_on {
        conn.execute(
            "INSERT OR IGNORE INTO task_dependencies (task_id, depends_on_id) VALUES (?1, ?2)",
            params![id, dep],
        )
        .map_err(|e| format!("task_promote: insert dep 失败：{e}"))?;
    }
    Ok(TaskRow {
        id: id.clone(),
        project_id,
        session_id: id,
        title,
        status: "running".to_string(),
        created_at: now,
        parent_id: None,
        depends_on,
        sort_order,
    })
}

/// 归档项目下所有对话：软删除（archived = 1）。返回影响行数。
#[tauri::command]
pub fn task_archive_project(project_id: String, store: State<'_, LiliaStore>) -> Result<i64, String> {
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

/// 归档单条对话：软删除（archived = 1）。返回是否成功。
#[tauri::command]
pub fn task_archive(id: String, store: State<'_, LiliaStore>) -> Result<bool, String> {
    let conn = store.conn()?;
    let changed = conn
        .execute("UPDATE tasks SET archived = 1 WHERE id = ?1 AND archived = 0", params![id])
        .map_err(|e| format!("task_archive: {e}"))?;
    Ok(changed > 0)
}

// ========== 排序 / 跨项目移动 ==========

/// 批量更新项目排序。`ordered_ids` 按显示顺序传入（index 0 = sort_order 0）。
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

/// 批量更新某个项目内 task 排序。`project_id = None` 表示孤儿。
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

/// 跨项目移动 task：更新 project_id。
/// `new_project_id = None` 表示移入孤儿收集箱。
/// session_id 保持不变（前端按 project.cwd 取工作目录）。
#[tauri::command]
pub fn task_reparent(
    task_id: String,
    new_project_id: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    let conn = store.conn()?;

    // 计算目标列表的新 sort_order（追加到末尾）
    let max_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM tasks WHERE (project_id = ?1 OR (project_id IS NULL AND ?1 IS NULL)) AND archived = 0",
            params![new_project_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("task_reparent: max sort_order 失败：{e}"))?;

    conn.execute(
        "UPDATE tasks SET project_id = ?1, sort_order = ?2 WHERE id = ?3",
        params![new_project_id, max_order + 1, task_id],
    )
    .map_err(|e| format!("task_reparent: update 失败：{e}"))?;

    Ok(())
}
