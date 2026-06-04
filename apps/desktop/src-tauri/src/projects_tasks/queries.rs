use std::collections::HashMap;

use rusqlite::{params, Connection};

use super::types::{NewTask, ProjectRow, TaskRow};

pub(super) fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
    let pinned: i64 = row.get(5)?;
    Ok(ProjectRow {
        id: row.get(0)?,
        name: row.get(1)?,
        cwd: row.get(2)?,
        session_count: row.get(3)?,
        sort_order: row.get(4)?,
        pinned: pinned != 0,
    })
}

pub(super) fn build_task(
    row: &rusqlite::Row<'_>,
    deps: &HashMap<String, Vec<String>>,
) -> rusqlite::Result<TaskRow> {
    let id: String = row.get(0)?;
    let depends_on = deps.get(&id).cloned().unwrap_or_default();
    let pinned: i64 = row.get(8)?;
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
        pinned: pinned != 0,
    })
}

fn collect_deps<I>(rows: I) -> Result<HashMap<String, Vec<String>>, String>
where
    I: IntoIterator<Item = rusqlite::Result<(String, String)>>,
{
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for r in rows {
        let (tid, did) = r.map_err(|e| format!("load_scoped_deps: row 失败：{e}"))?;
        map.entry(tid).or_default().push(did);
    }
    Ok(map)
}

pub(super) fn load_project_deps(
    conn: &Connection,
    project_id: Option<&str>,
) -> Result<HashMap<String, Vec<String>>, String> {
    if let Some(project_id) = project_id {
        let mut stmt = conn
            .prepare(
                r#"SELECT d.task_id, d.depends_on_id
                   FROM task_dependencies d
                   INNER JOIN tasks t ON t.id = d.task_id
                   WHERE t.project_id = ?1 AND t.archived = 0"#,
            )
            .map_err(|e| format!("load_scoped_deps: prepare 失败：{e}"))?;
        let rows = stmt
            .query_map(params![project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("load_scoped_deps: query 失败：{e}"))?;
        return collect_deps(rows);
    }

    let mut stmt = conn
        .prepare(
            r#"SELECT d.task_id, d.depends_on_id
               FROM task_dependencies d
               INNER JOIN tasks t ON t.id = d.task_id
               WHERE t.project_id IS NULL AND t.archived = 0"#,
        )
        .map_err(|e| format!("load_scoped_deps: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("load_scoped_deps: query 失败：{e}"))?;
    collect_deps(rows)
}

pub(super) fn load_task_deps(
    conn: &Connection,
    task_id: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut stmt = conn
        .prepare("SELECT task_id, depends_on_id FROM task_dependencies WHERE task_id = ?1")
        .map_err(|e| format!("load_scoped_deps: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("load_scoped_deps: query 失败：{e}"))?;
    collect_deps(rows)
}

pub(super) fn insert_task_with_deps(
    conn: &Connection,
    task: NewTask<'_>,
    context: &str,
) -> Result<(), String> {
    conn.execute(
        r#"INSERT INTO tasks (id, project_id, session_id, title, status, created_at, parent_id, sort_order)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        params![
            task.id,
            task.project_id,
            task.session_id,
            task.title,
            task.status,
            task.created_at,
            task.parent_id,
            task.sort_order
        ],
    )
    .map_err(|e| format!("{context}: insert 失败：{e}"))?;
    for dep in task.depends_on {
        conn.execute(
            "INSERT OR IGNORE INTO task_dependencies (task_id, depends_on_id) VALUES (?1, ?2)",
            params![task.id, dep],
        )
        .map_err(|e| format!("{context}: insert dep 失败：{e}"))?;
    }
    Ok(())
}
