mod bulk;
mod preview;
mod types;
mod utility;

pub use types::{
    CodexThreadAttachInput, CodexThreadAttachResult, CodexThreadPreview, CodexThreadPreviewInput,
    CodexThreadSearchInput, CodexThreadSearchResult, CodexThreadSummary,
};

use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::state::{default_composer, session_key, ChatStore};
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::projects_tasks::events::emit_tasks_changed;
use crate::projects_tasks::TaskRow;
use crate::store::LiliaStore;
use crate::util::now_millis;
use crate::BACKEND_CODEX;

use self::bulk::persist_history_events_batch;
use self::preview::preview_events_from_inputs;
use self::utility::run_codex_history_utility;

#[tauri::command]
pub async fn codex_thread_search(
    app: AppHandle,
    input: CodexThreadSearchInput,
) -> Result<CodexThreadSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || codex_thread_search_blocking(app, input))
        .await
        .map_err(|err| format!("Codex history search 任务执行失败：{err}"))?
}

fn codex_thread_search_blocking(
    app: AppHandle,
    input: CodexThreadSearchInput,
) -> Result<CodexThreadSearchResult, String> {
    let result = run_codex_history_utility(
        &app,
        serde_json::json!({
            "action": "search",
            "input": input,
        }),
    )?;
    Ok(CodexThreadSearchResult {
        threads: result.threads,
        next_cursor: result.next_cursor,
    })
}

#[tauri::command]
pub async fn codex_thread_preview(
    app: AppHandle,
    input: CodexThreadPreviewInput,
) -> Result<CodexThreadPreview, String> {
    tauri::async_runtime::spawn_blocking(move || codex_thread_preview_blocking(app, input))
        .await
        .map_err(|err| format!("Codex history preview 任务执行失败：{err}"))?
}

fn codex_thread_preview_blocking(
    app: AppHandle,
    input: CodexThreadPreviewInput,
) -> Result<CodexThreadPreview, String> {
    let detail = input.detail.as_deref().unwrap_or("lite");
    let result = run_codex_history_utility(
        &app,
        serde_json::json!({
            "action": "preview",
            "threadId": input.thread_id,
            "detail": detail,
        }),
    )?;
    let thread = result
        .thread
        .ok_or_else(|| "Codex thread preview 缺少 thread 信息".to_string())?;
    let is_full = detail == "full";
    let events = if is_full {
        preview_events_from_inputs(result.events)?
    } else {
        Vec::new()
    };
    Ok(CodexThreadPreview {
        thread,
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
    .map_err(|e| format!("Codex history: 查询 sort_order 失败：{e}"))
}

fn create_task_for_thread(
    app: &AppHandle,
    conn: &rusqlite::Connection,
    project_id: Option<String>,
    thread: Option<&CodexThreadSummary>,
) -> Result<TaskRow, String> {
    let id = Uuid::new_v4().to_string();
    let title = thread
        .map(|thread| thread.title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or("Codex 历史对话")
        .to_string();
    let now = now_millis() as i64;
    let sort_order = next_task_sort_order(conn, project_id.as_deref())?;
    conn.execute(
        r#"INSERT INTO tasks (id, project_id, session_id, title, status, created_at, sort_order)
           VALUES (?1, ?2, ?3, ?4, 'waiting', ?5, ?6)"#,
        params![id.as_str(), project_id, id.as_str(), title, now, sort_order],
    )
    .map_err(|e| format!("创建 Codex 历史对话失败：{e}"))?;
    emit_tasks_changed(app, project_id.clone());
    task_row_by_id(conn, &id)?.ok_or_else(|| "创建 Codex 历史对话后读取失败".to_string())
}

fn insert_session_anchor(app: &AppHandle, task_id: &str, thread_id: &str) {
    persist_and_emit_input(
        app,
        session_anchor_input(task_id, thread_id, now_millis() as i64),
    );
}

fn session_anchor_input(task_id: &str, thread_id: &str, now: i64) -> AgentTimelineEventInput {
    AgentTimelineEventInput {
        id: Some(format!("{task_id}:codex-thread-attach:{thread_id}")),
        task_id: task_id.to_string(),
        turn_id: Some(format!("codex-thread-attach:{thread_id}")),
        backend: BACKEND_CODEX.to_string(),
        kind: "turn".to_string(),
        status: "success".to_string(),
        title: "Codex thread attached".to_string(),
        summary: Some("已接入 Codex thread".to_string()),
        payload: serde_json::json!({
            "backend": "codex",
            "sessionId": thread_id,
            "subkind": "thread_attach",
        }),
        created_at: Some(now),
        updated_at: Some(now),
    }
}

fn remember_codex_thread_session(chat_store: &ChatStore, task_id: &str, thread_id: &str) {
    let mut sessions = chat_store.sdk_sessions.lock().unwrap();
    sessions.insert(session_key(BACKEND_CODEX, task_id), thread_id.to_string());
}

fn history_sync_error_input(
    task_id: &str,
    thread_id: &str,
    message: String,
) -> AgentTimelineEventInput {
    let now = now_millis() as i64;
    AgentTimelineEventInput {
        id: Some(format!("{task_id}:codex-history-sync-error:{thread_id}")),
        task_id: task_id.to_string(),
        turn_id: Some(format!("codex-history:{thread_id}")),
        backend: BACKEND_CODEX.to_string(),
        kind: "diagnostic".to_string(),
        status: "error".to_string(),
        title: "Codex history sync failed".to_string(),
        summary: Some("Codex 历史后台同步失败，已保留 thread 接入。".to_string()),
        payload: serde_json::json!({
            "backend": "codex",
            "subkind": "history_sync",
            "threadId": thread_id,
            "error": message,
        }),
        created_at: Some(now),
        updated_at: Some(now),
    }
}

fn spawn_codex_history_sync(app: AppHandle, task_id: String, thread_id: String) {
    tauri::async_runtime::spawn_blocking(move || {
        let result = run_codex_history_utility(
            &app,
            serde_json::json!({
                "action": "sync",
                "taskId": task_id,
                "threadId": thread_id,
                "limit": 50,
            }),
        );
        match result {
            Ok(sync) => {
                let event_count = persist_history_events_batch(&app, &task_id, sync.events);
                let now = now_millis() as i64;
                persist_and_emit_input(
                    &app,
                    AgentTimelineEventInput {
                        id: Some(format!("{task_id}:codex-history-sync:{thread_id}")),
                        task_id: task_id.clone(),
                        turn_id: Some(format!("codex-history:{thread_id}")),
                        backend: BACKEND_CODEX.to_string(),
                        kind: "diagnostic".to_string(),
                        status: "success".to_string(),
                        title: "Codex history synced".to_string(),
                        summary: Some(if event_count > 0 {
                            format!("已同步 {event_count} 条 Codex 历史事件")
                        } else {
                            "没有需要同步的 Codex 历史事件".to_string()
                        }),
                        payload: serde_json::json!({
                            "backend": "codex",
                            "subkind": "history_sync",
                            "threadId": thread_id,
                            "eventCount": event_count,
                        }),
                        created_at: Some(now),
                        updated_at: Some(now),
                    },
                );
            }
            Err(err) => {
                persist_and_emit_input(&app, history_sync_error_input(&task_id, &thread_id, err))
            }
        }
    });
}

#[tauri::command]
pub async fn codex_thread_attach(
    app: AppHandle,
    input: CodexThreadAttachInput,
) -> Result<CodexThreadAttachResult, String> {
    tauri::async_runtime::spawn_blocking(move || codex_thread_attach_blocking(app, input))
        .await
        .map_err(|err| format!("Codex history attach 任务执行失败：{err}"))?
}

fn codex_thread_attach_blocking(
    app: AppHandle,
    input: CodexThreadAttachInput,
) -> Result<CodexThreadAttachResult, String> {
    let thread_id = input.thread_id.trim().to_string();
    if thread_id.is_empty() {
        return Err("Codex threadId 不能为空".to_string());
    }
    let mode = input.mode.as_str();
    if mode != "current" && mode != "new" {
        return Err(format!("未知 Codex thread attach mode: {}", input.mode));
    }

    let store = app.state::<LiliaStore>();
    let chat_store = app.state::<ChatStore>();
    let conn = store.conn()?;
    let task = if mode == "new" {
        Some(create_task_for_thread(
            &app,
            &conn,
            input.project_id.clone(),
            input.thread.as_ref(),
        )?)
    } else {
        let task_id = input
            .task_id
            .as_ref()
            .ok_or_else(|| "接入当前对话需要 taskId".to_string())?;
        Some(task_row_by_id(&conn, task_id)?.ok_or_else(|| format!("未找到任务：{task_id}"))?)
    };
    let task = task.expect("task is always set");
    remember_codex_thread_session(&chat_store, &task.id, &thread_id);
    {
        let mut composers = chat_store.composers.lock().unwrap();
        let composer = composers
            .entry(task.id.clone())
            .or_insert_with(|| default_composer(&task.id));
        composer.backend = BACKEND_CODEX.to_string();
    }

    insert_session_anchor(&app, &task.id, &thread_id);
    spawn_codex_history_sync(app.clone(), task.id.clone(), thread_id.clone());
    Ok(CodexThreadAttachResult {
        task_id: task.id.clone(),
        project_id: task.project_id.clone(),
        thread_id,
        task: Some(task),
        event_count: 0,
        history_sync: Some("queued".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_anchor_records_codex_resume_thread_id() {
        let input = session_anchor_input("task-1", "thread-1", 1234);

        assert_eq!(
            input.id.as_deref(),
            Some("task-1:codex-thread-attach:thread-1")
        );
        assert_eq!(input.kind, "turn");
        assert_eq!(input.status, "success");
        assert_eq!(
            input
                .payload
                .get("sessionId")
                .and_then(|value| value.as_str()),
            Some("thread-1")
        );
        assert_eq!(
            input
                .payload
                .get("subkind")
                .and_then(|value| value.as_str()),
            Some("thread_attach")
        );
    }

    #[test]
    fn remember_codex_thread_session_updates_chat_store() {
        let store = ChatStore::default();

        remember_codex_thread_session(&store, "task-1", "thread-1");

        assert_eq!(
            store
                .sdk_sessions
                .lock()
                .unwrap()
                .get(&session_key(BACKEND_CODEX, "task-1"))
                .cloned(),
            Some("thread-1".to_string())
        );
    }

    #[test]
    fn history_sync_error_input_records_thread_and_stable_id() {
        let input = history_sync_error_input("task-1", "thread-1", "network failed".to_string());

        assert_eq!(
            input.id.as_deref(),
            Some("task-1:codex-history-sync-error:thread-1")
        );
        assert_eq!(input.kind, "diagnostic");
        assert_eq!(input.status, "error");
        assert_eq!(
            input
                .payload
                .get("threadId")
                .and_then(|value| value.as_str()),
            Some("thread-1")
        );
        assert_eq!(
            input.payload.get("error").and_then(|value| value.as_str()),
            Some("network failed")
        );
    }
}
