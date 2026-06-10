mod types;
mod utility;

pub use types::{
    ClaudeSessionAttachInput, ClaudeSessionAttachResult, ClaudeSessionPreview,
    ClaudeSessionPreviewInput, ClaudeSessionSearchInput, ClaudeSessionSearchResult,
    ClaudeSessionSummary,
};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::state::{default_composer, remember_agent_session, ChatStore};
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::codex_history::bulk::persist_history_events_batch;
use crate::codex_history::preview::preview_events_from_inputs;
use crate::projects_tasks::events::emit_tasks_changed;
use crate::projects_tasks::TaskRow;
use crate::store::LiliaStore;
use crate::util::now_millis;
use crate::{BACKEND_CLAUDE, RUNTIME_CHANNEL_BUILTIN};

use self::utility::run_claude_history_utility;

#[tauri::command]
pub async fn claude_session_search(
    app: AppHandle,
    input: ClaudeSessionSearchInput,
) -> Result<ClaudeSessionSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || claude_session_search_blocking(app, input))
        .await
        .map_err(|err| format!("Claude history search 任务执行失败：{err}"))?
}

fn claude_session_search_blocking(
    app: AppHandle,
    input: ClaudeSessionSearchInput,
) -> Result<ClaudeSessionSearchResult, String> {
    let result = run_claude_history_utility(
        &app,
        serde_json::json!({
            "action": "search",
            "input": input,
        }),
    )?;
    Ok(ClaudeSessionSearchResult {
        sessions: result.sessions,
        next_cursor: result.next_cursor,
    })
}

#[tauri::command]
pub async fn claude_session_preview(
    app: AppHandle,
    input: ClaudeSessionPreviewInput,
) -> Result<ClaudeSessionPreview, String> {
    tauri::async_runtime::spawn_blocking(move || claude_session_preview_blocking(app, input))
        .await
        .map_err(|err| format!("Claude history preview 任务执行失败：{err}"))?
}

fn claude_session_preview_blocking(
    app: AppHandle,
    input: ClaudeSessionPreviewInput,
) -> Result<ClaudeSessionPreview, String> {
    let detail = input.detail.as_deref().unwrap_or("lite");
    let result = run_claude_history_utility(
        &app,
        serde_json::json!({
            "action": "preview",
            "sessionId": input.session_id,
            "detail": detail,
        }),
    )?;
    let session = result
        .session
        .ok_or_else(|| "Claude session preview 缺少 session 信息".to_string())?;
    let is_full = detail == "full";
    let events = if is_full {
        preview_events_from_inputs(result.events)?
    } else {
        Vec::new()
    };
    Ok(ClaudeSessionPreview {
        session,
        event_count: if is_full {
            events.len()
        } else {
            result.event_count.unwrap_or(result.messages.len())
        },
        events,
        messages: result.messages,
        has_full_preview: result.has_full_preview,
    })
}

fn task_row_by_id(conn: &rusqlite::Connection, task_id: &str) -> Result<Option<TaskRow>, String> {
    conn.query_row(
        r#"SELECT id, project_id, session_id, title, title_source, status, created_at, parent_id, sort_order, pinned
           FROM tasks
           WHERE id = ?1 AND archived = 0"#,
        params![task_id],
        |row| {
            let pinned: i64 = row.get(9)?;
            Ok(TaskRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                session_id: row.get(2)?,
                title: row.get(3)?,
                title_source: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                parent_id: row.get(7)?,
                depends_on: Vec::new(),
                sort_order: row.get(8)?,
                pinned: pinned != 0,
            })
        },
    )
    .optional()
    .map_err(|e| format!("查询任务失败：{e}"))
}

fn next_task_sort_order(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) FROM tasks WHERE (project_id = ?1 OR (project_id IS NULL AND ?1 IS NULL)) AND archived = 0",
        params![project_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value + 1)
    .map_err(|e| format!("Claude history: 查询 sort_order 失败：{e}"))
}

fn create_task_for_session(
    app: &AppHandle,
    conn: &rusqlite::Connection,
    project_id: Option<String>,
    session: Option<&ClaudeSessionSummary>,
) -> Result<TaskRow, String> {
    let id = Uuid::new_v4().to_string();
    let title = session
        .map(|session| session.title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or("Claude 历史对话")
        .to_string();
    let now = now_millis() as i64;
    let sort_order = next_task_sort_order(conn, project_id.as_deref())?;
    conn.execute(
        r#"INSERT INTO tasks (id, project_id, session_id, title, status, created_at, sort_order)
           VALUES (?1, ?2, ?3, ?4, 'waiting', ?5, ?6)"#,
        params![id.as_str(), project_id, id.as_str(), title, now, sort_order],
    )
    .map_err(|e| format!("创建 Claude 历史对话失败：{e}"))?;
    emit_tasks_changed(app, project_id.clone());
    task_row_by_id(conn, &id)?.ok_or_else(|| "创建 Claude 历史对话后读取失败".to_string())
}

fn session_anchor_input(task_id: &str, session_id: &str, now: i64) -> AgentTimelineEventInput {
    AgentTimelineEventInput {
        id: Some(format!("{task_id}:claude-session-attach:{session_id}")),
        task_id: task_id.to_string(),
        turn_id: Some(format!("claude-session-attach:{session_id}")),
        backend: BACKEND_CLAUDE.to_string(),
        kind: "turn".to_string(),
        status: "success".to_string(),
        title: "Claude session attached".to_string(),
        summary: Some("已接入 Claude session".to_string()),
        payload: serde_json::json!({
            "backend": "claude",
            "sessionId": session_id,
            "subkind": "session_attach",
        }),
        created_at: Some(now),
        updated_at: Some(now),
    }
}

fn insert_session_anchor(app: &AppHandle, task_id: &str, session_id: &str) {
    persist_and_emit_input(
        app,
        session_anchor_input(task_id, session_id, now_millis() as i64),
    );
}

fn remember_claude_session(
    conn: &Connection,
    chat_store: &ChatStore,
    task_id: &str,
    session_id: &str,
) {
    remember_agent_session(
        conn,
        chat_store,
        task_id,
        BACKEND_CLAUDE,
        RUNTIME_CHANNEL_BUILTIN,
        session_id,
        "Claude attach",
    );
}

fn history_sync_error_input(
    task_id: &str,
    session_id: &str,
    message: String,
) -> AgentTimelineEventInput {
    let now = now_millis() as i64;
    AgentTimelineEventInput {
        id: Some(format!("{task_id}:claude-history-sync-error:{session_id}")),
        task_id: task_id.to_string(),
        turn_id: Some(format!("claude-history:{session_id}")),
        backend: BACKEND_CLAUDE.to_string(),
        kind: "diagnostic".to_string(),
        status: "error".to_string(),
        title: "Claude history sync failed".to_string(),
        summary: Some("Claude 历史后台同步失败，已保留 session 接入。".to_string()),
        payload: serde_json::json!({
            "backend": "claude",
            "subkind": "history_sync",
            "threadId": session_id,
            "sessionId": session_id,
            "error": message,
        }),
        created_at: Some(now),
        updated_at: Some(now),
    }
}

const HISTORY_SYNC_LIMIT: i64 = 120;

fn normalize_next_cursor(cursor: Option<String>) -> Option<String> {
    cursor.and_then(|cursor| {
        let trimmed = cursor.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn history_sync_success_input(
    task_id: &str,
    session_id: &str,
    event_count: usize,
    page_count: usize,
) -> AgentTimelineEventInput {
    let now = now_millis() as i64;
    AgentTimelineEventInput {
        id: Some(format!("{task_id}:claude-history-sync:{session_id}")),
        task_id: task_id.to_string(),
        turn_id: Some(format!("claude-history:{session_id}")),
        backend: BACKEND_CLAUDE.to_string(),
        kind: "diagnostic".to_string(),
        status: "success".to_string(),
        title: "Claude history synced".to_string(),
        summary: Some(if event_count > 0 {
            format!("已同步 {event_count} 条 Claude 历史事件")
        } else {
            "没有需要同步的 Claude 历史事件".to_string()
        }),
        payload: serde_json::json!({
            "backend": "claude",
            "subkind": "history_sync",
            "threadId": session_id,
            "sessionId": session_id,
            "eventCount": event_count,
            "pageCount": page_count,
        }),
        created_at: Some(now),
        updated_at: Some(now),
    }
}

fn spawn_claude_history_sync(app: AppHandle, task_id: String, session_id: String) {
    tauri::async_runtime::spawn_blocking(move || {
        let mut next_cursor: Option<String> = None;
        let mut page_count = 0usize;
        let mut event_count = 0usize;
        loop {
            let previous_cursor = next_cursor.clone();
            let result = run_claude_history_utility(
                &app,
                serde_json::json!({
                    "action": "sync",
                    "taskId": task_id,
                    "sessionId": session_id,
                    "limit": HISTORY_SYNC_LIMIT,
                    "cursor": next_cursor,
                }),
            );
            let sync = match result {
                Ok(sync) => sync,
                Err(err) => {
                    persist_and_emit_input(
                        &app,
                        history_sync_error_input(&task_id, &session_id, err),
                    );
                    return;
                }
            };
            let page_event_count = persist_history_events_batch(&app, &task_id, sync.events);
            event_count += page_event_count;
            page_count += 1;
            next_cursor = normalize_next_cursor(sync.next_cursor);
            if next_cursor.is_none() {
                persist_and_emit_input(
                    &app,
                    history_sync_success_input(&task_id, &session_id, event_count, page_count),
                );
                return;
            }
            if next_cursor == previous_cursor {
                persist_and_emit_input(
                    &app,
                    history_sync_error_input(
                        &task_id,
                        &session_id,
                        "Claude history sync 返回了重复 cursor，已停止后台同步。".to_string(),
                    ),
                );
                return;
            }
        }
    });
}

#[tauri::command]
pub async fn claude_session_attach(
    app: AppHandle,
    input: ClaudeSessionAttachInput,
) -> Result<ClaudeSessionAttachResult, String> {
    tauri::async_runtime::spawn_blocking(move || claude_session_attach_blocking(app, input))
        .await
        .map_err(|err| format!("Claude history attach 任务执行失败：{err}"))?
}

fn claude_session_attach_blocking(
    app: AppHandle,
    input: ClaudeSessionAttachInput,
) -> Result<ClaudeSessionAttachResult, String> {
    let session_id = input.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err("Claude sessionId 不能为空".to_string());
    }
    let mode = input.mode.as_str();
    if mode != "current" && mode != "new" {
        return Err(format!("未知 Claude session attach mode: {}", input.mode));
    }

    let store = app.state::<LiliaStore>();
    let chat_store = app.state::<ChatStore>();
    let conn = store.conn()?;
    let task = if mode == "new" {
        Some(create_task_for_session(
            &app,
            &conn,
            input.project_id.clone(),
            input.session.as_ref(),
        )?)
    } else {
        let task_id = input
            .task_id
            .as_ref()
            .ok_or_else(|| "接入当前对话需要 taskId".to_string())?;
        Some(task_row_by_id(&conn, task_id)?.ok_or_else(|| format!("未找到任务：{task_id}"))?)
    };
    let task = task.expect("task is always set");
    remember_claude_session(&conn, &chat_store, &task.id, &session_id);
    {
        let mut composers = chat_store.composers.lock().unwrap();
        let composer = composers
            .entry(task.id.clone())
            .or_insert_with(|| default_composer(&task.id));
        composer.backend = BACKEND_CLAUDE.to_string();
    }

    insert_session_anchor(&app, &task.id, &session_id);
    spawn_claude_history_sync(app.clone(), task.id.clone(), session_id.clone());
    Ok(ClaudeSessionAttachResult {
        task_id: task.id.clone(),
        project_id: task.project_id.clone(),
        session_id,
        task: Some(task),
        event_count: 0,
        history_sync: Some("queued".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::state::session_key;

    fn create_task_agent_sessions_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE task_agent_sessions (
              task_id         TEXT NOT NULL,
              backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                              CHECK (runtime_channel IN ('builtin','nanobot')),
              session_id      TEXT NOT NULL,
              updated_at      INTEGER NOT NULL,
              PRIMARY KEY (task_id, backend, runtime_channel)
            );
            "#,
        )
        .unwrap();
    }

    fn assert_agent_session_checkpoint(
        conn: &Connection,
        store: &ChatStore,
        task_id: &str,
        backend: &str,
        expected: &str,
    ) {
        assert_eq!(
            store
                .sdk_sessions
                .lock()
                .unwrap()
                .get(&session_key(RUNTIME_CHANNEL_BUILTIN, backend, task_id))
                .cloned(),
            Some(expected.to_string())
        );
        let session_id: String = conn
            .query_row(
                "SELECT session_id FROM task_agent_sessions WHERE task_id = ?1 AND backend = ?2 AND runtime_channel = 'builtin'",
                params![task_id, backend],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(session_id, expected);
    }

    #[test]
    fn session_anchor_records_claude_resume_session_id() {
        let input = session_anchor_input("task-1", "session-1", 1234);

        assert_eq!(
            input.id.as_deref(),
            Some("task-1:claude-session-attach:session-1")
        );
        assert_eq!(input.kind, "turn");
        assert_eq!(input.status, "success");
        assert_eq!(
            input
                .payload
                .get("sessionId")
                .and_then(|value| value.as_str()),
            Some("session-1")
        );
        assert_eq!(
            input
                .payload
                .get("subkind")
                .and_then(|value| value.as_str()),
            Some("session_attach")
        );
    }

    #[test]
    fn remember_claude_session_updates_chat_store() {
        let store = ChatStore::default();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_task_agent_sessions_schema(&conn);

        remember_claude_session(&conn, &store, "task-1", "session-1");

        assert_agent_session_checkpoint(&conn, &store, "task-1", BACKEND_CLAUDE, "session-1");
    }
}
