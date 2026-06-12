use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::codex_history::spawn_codex_thread_archive_sync;
use crate::store::LiliaStore;
use crate::util::now_millis;
use crate::{BACKEND_CODEX, RUNTIME_CHANNEL_BUILTIN};

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
                    r#"SELECT id, project_id, session_id, title, title_source, status, created_at, parent_id, sort_order, pinned
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
                    r#"SELECT id, project_id, session_id, title, title_source, status, created_at, parent_id, sort_order, pinned
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
            r#"SELECT id, project_id, session_id, title, title_source, status, created_at, parent_id, sort_order, pinned
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
        title_source: "auto".to_string(),
        status,
        created_at: now,
        parent_id,
        depends_on,
        sort_order,
        pinned: false,
    };
    emit_tasks_changed(&app, project_id);
    crate::automation::emit_task_changed_signal(
        &app,
        row.project_id.clone(),
        Some(row.id.clone()),
        Some(row.status.clone()),
        "task_created",
        None,
    );
    Ok(row)
}

#[tauri::command]
pub fn task_update(
    id: String,
    title: Option<String>,
    status: Option<String>,
    store: State<'_, LiliaStore>,
    app: AppHandle,
) -> Result<(), String> {
    if title.is_none() && status.is_none() {
        return Ok(());
    }
    let conn = store.conn()?;
    let (project_id, previous_status) = conn
        .query_row(
            "SELECT project_id, status FROM tasks WHERE id = ?1 AND archived = 0",
            params![id.as_str()],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| format!("task_update: 查询任务失败：{e}"))?
        .unwrap_or((None, String::new()));
    let mut next_status = previous_status;
    let status_changed = status.is_some();
    if let Some(t) = title {
        conn.execute(
            "UPDATE tasks SET title = ?1, title_source = 'manual' WHERE id = ?2",
            params![t, id.as_str()],
        )
        .map_err(|e| format!("task_update(title): {e}"))?;
    }
    if let Some(s) = status {
        conn.execute(
            "UPDATE tasks SET status = ?1 WHERE id = ?2",
            params![s, id.as_str()],
        )
        .map_err(|e| format!("task_update(status): {e}"))?;
        next_status = s;
    }
    emit_tasks_changed(&app, project_id.clone());
    crate::automation::emit_task_changed_signal(
        &app,
        project_id,
        Some(id),
        Some(next_status),
        if status_changed {
            "task_status_changed"
        } else {
            "task_updated"
        },
        None,
    );
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
    parent_id: Option<String>,
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
            parent_id: parent_id.as_deref(),
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
        title_source: "auto".to_string(),
        status: "running".to_string(),
        created_at: now,
        parent_id,
        depends_on,
        sort_order,
        pinned: false,
    };
    emit_tasks_changed(&app, project_id);
    crate::automation::emit_task_changed_signal(
        &app,
        row.project_id.clone(),
        Some(row.id.clone()),
        Some(row.status.clone()),
        "task_created",
        None,
    );
    Ok(row)
}

#[tauri::command]
pub fn task_archive_project(
    project_id: String,
    store: State<'_, LiliaStore>,
    app: AppHandle,
) -> Result<i64, String> {
    let conn = store.conn()?;
    let thread_ids = codex_thread_ids_for_project_archive(&conn, &project_id)?;
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
    if count > 0 {
        spawn_codex_thread_archive_sync(app, thread_ids);
    }
    Ok(count)
}

#[tauri::command]
pub fn task_archive(
    id: String,
    store: State<'_, LiliaStore>,
    app: AppHandle,
) -> Result<bool, String> {
    let conn = store.conn()?;
    let thread_ids = codex_thread_ids_for_task_archive(&conn, &id)?;
    let changed = conn
        .execute(
            "UPDATE tasks SET archived = 1 WHERE id = ?1 AND archived = 0",
            params![id],
        )
        .map_err(|e| format!("task_archive: {e}"))?;
    if changed > 0 {
        spawn_codex_thread_archive_sync(app, thread_ids);
    }
    Ok(changed > 0)
}

fn codex_thread_ids_for_task_archive(
    conn: &rusqlite::Connection,
    task_id: &str,
) -> Result<Vec<String>, String> {
    codex_thread_ids_for_archive(
        conn,
        r#"SELECT s.session_id
           FROM task_agent_sessions s
           JOIN tasks t ON t.id = s.task_id
           WHERE t.id = ?1
             AND t.archived = 0
             AND s.backend = ?2
             AND s.runtime_channel = ?3"#,
        task_id,
        "task_archive: 查询 Codex thread 失败",
    )
}

fn codex_thread_ids_for_project_archive(
    conn: &rusqlite::Connection,
    project_id: &str,
) -> Result<Vec<String>, String> {
    codex_thread_ids_for_archive(
        conn,
        r#"SELECT s.session_id
           FROM task_agent_sessions s
           JOIN tasks t ON t.id = s.task_id
           WHERE t.project_id = ?1
             AND t.archived = 0
             AND s.backend = ?2
             AND s.runtime_channel = ?3
           ORDER BY t.sort_order ASC, t.created_at ASC"#,
        project_id,
        "task_archive_project: 查询 Codex thread 失败",
    )
}

fn codex_thread_ids_for_archive(
    conn: &rusqlite::Connection,
    sql: &str,
    id: &str,
    context: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("{context}：prepare 失败：{e}"))?;
    let rows = stmt
        .query_map(params![id, BACKEND_CODEX, RUNTIME_CHANNEL_BUILTIN], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| format!("{context}：query 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let thread_id = row.map_err(|e| format!("{context}：row 失败：{e}"))?;
        let trimmed = thread_id.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_archive_schema(conn: &rusqlite::Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE tasks (
              id           TEXT PRIMARY KEY,
              project_id   TEXT,
              session_id   TEXT NOT NULL,
              title        TEXT NOT NULL,
              status       TEXT NOT NULL,
              created_at   INTEGER NOT NULL,
              archived     INTEGER NOT NULL DEFAULT 0,
              sort_order   INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE task_agent_sessions (
              task_id         TEXT NOT NULL,
              backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                              CHECK (runtime_channel IN ('builtin','mutsuki_core')),
              session_id      TEXT NOT NULL,
              updated_at      INTEGER NOT NULL,
              PRIMARY KEY (task_id, backend, runtime_channel)
            );
            "#,
        )
        .unwrap();
    }

    fn insert_task(conn: &rusqlite::Connection, id: &str, project_id: &str, archived: bool) {
        conn.execute(
            r#"INSERT INTO tasks
               (id, project_id, session_id, title, status, created_at, archived, sort_order)
               VALUES (?1, ?2, ?1, ?3, 'waiting', 1, ?4, ?5)"#,
            params![
                id,
                project_id,
                format!("任务 {id}"),
                if archived { 1 } else { 0 },
                match id {
                    "task-2" => 2,
                    "task-3" => 3,
                    _ => 1,
                },
            ],
        )
        .unwrap();
    }

    fn insert_session(
        conn: &rusqlite::Connection,
        task_id: &str,
        backend: &str,
        runtime_channel: &str,
        session_id: &str,
    ) {
        conn.execute(
            r#"INSERT INTO task_agent_sessions
               (task_id, backend, runtime_channel, session_id, updated_at)
               VALUES (?1, ?2, ?3, ?4, 1)"#,
            params![task_id, backend, runtime_channel, session_id],
        )
        .unwrap();
    }

    fn task_archived(conn: &rusqlite::Connection, id: &str) -> bool {
        conn.query_row(
            "SELECT archived FROM tasks WHERE id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
            != 0
    }

    #[test]
    fn archive_task_skips_non_codex_archived_and_missing_sessions() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_archive_schema(&conn);
        insert_task(&conn, "claude-task", "project-1", false);
        insert_task(&conn, "mutsuki-task", "project-1", false);
        insert_task(&conn, "archived-task", "project-1", true);
        insert_task(&conn, "missing-session", "project-1", false);
        insert_session(
            &conn,
            "claude-task",
            crate::BACKEND_CLAUDE,
            RUNTIME_CHANNEL_BUILTIN,
            "claude-session",
        );
        insert_session(
            &conn,
            "mutsuki-task",
            BACKEND_CODEX,
            crate::RUNTIME_CHANNEL_MUTSUKI_CORE,
            "thread-mutsuki",
        );
        insert_session(
            &conn,
            "archived-task",
            BACKEND_CODEX,
            RUNTIME_CHANNEL_BUILTIN,
            "thread-archived",
        );

        assert!(codex_thread_ids_for_task_archive(&conn, "claude-task")
            .unwrap()
            .is_empty());
        assert!(codex_thread_ids_for_task_archive(&conn, "mutsuki-task")
            .unwrap()
            .is_empty());
        assert!(codex_thread_ids_for_task_archive(&conn, "archived-task")
            .unwrap()
            .is_empty());
        assert!(codex_thread_ids_for_task_archive(&conn, "missing-session")
            .unwrap()
            .is_empty());

        let changed = conn
            .execute(
                "UPDATE tasks SET archived = 1 WHERE id = ?1 AND archived = 0",
                params!["archived-task"],
            )
            .unwrap();

        assert_eq!(changed, 0);
    }

    #[test]
    fn archive_queries_collect_builtin_codex_threads_before_local_update() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_archive_schema(&conn);
        insert_task(&conn, "task-1", "project-1", false);
        insert_task(&conn, "task-2", "project-1", false);
        insert_task(&conn, "task-3", "project-2", false);
        insert_task(&conn, "task-4", "project-1", true);
        insert_session(
            &conn,
            "task-1",
            BACKEND_CODEX,
            RUNTIME_CHANNEL_BUILTIN,
            "thread-1",
        );
        insert_session(
            &conn,
            "task-2",
            BACKEND_CODEX,
            RUNTIME_CHANNEL_BUILTIN,
            "thread-2",
        );
        insert_session(
            &conn,
            "task-3",
            BACKEND_CODEX,
            RUNTIME_CHANNEL_BUILTIN,
            "thread-other-project",
        );
        insert_session(
            &conn,
            "task-4",
            BACKEND_CODEX,
            RUNTIME_CHANNEL_BUILTIN,
            "thread-already-archived",
        );

        assert_eq!(
            codex_thread_ids_for_task_archive(&conn, "task-1").unwrap(),
            vec!["thread-1"]
        );
        assert_eq!(
            codex_thread_ids_for_project_archive(&conn, "project-1").unwrap(),
            vec!["thread-1", "thread-2"]
        );

        let count = conn
            .execute(
                "UPDATE tasks SET archived = 1 WHERE project_id = ?1 AND archived = 0",
                params!["project-1"],
            )
            .unwrap();

        assert_eq!(count, 2);
        assert!(task_archived(&conn, "task-1"));
        assert!(task_archived(&conn, "task-2"));
        assert!(!task_archived(&conn, "task-3"));
    }
}
