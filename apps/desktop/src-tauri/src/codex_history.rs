use std::io::Write;
use std::process::{Command, Stdio};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::agent_timeline::{self, AgentTimelineEvent, AgentTimelineEventInput};
use crate::chat::runner::locate_agent_runner;
use crate::chat::state::{default_composer, session_key, ChatStore};
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::projects_tasks::events::emit_tasks_changed;
use crate::projects_tasks::TaskRow;
use crate::provider::{
    build_codex_app_server_probe_status, resolve_connection_for, validate_backend_ready_for_send,
};
use crate::store::LiliaStore;
use crate::util::now_millis;
use crate::BACKEND_CODEX;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadSearchInput {
    #[serde(default)]
    pub search_term: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadSummary {
    pub id: String,
    pub title: String,
    pub status: Option<String>,
    pub model: Option<String>,
    pub source_kind: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub archived: bool,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadSearchResult {
    pub threads: Vec<CodexThreadSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadPreview {
    pub thread: CodexThreadSummary,
    pub events: Vec<AgentTimelineEvent>,
    pub event_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadAttachInput {
    pub thread_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadAttachResult {
    pub task_id: String,
    pub project_id: Option<String>,
    pub thread_id: String,
    pub task: Option<TaskRow>,
    pub event_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexHistoryUtilityOutput {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    threads: Vec<CodexThreadSummary>,
    #[serde(default)]
    next_cursor: Option<String>,
    #[serde(default)]
    thread: Option<CodexThreadSummary>,
    #[serde(default)]
    events: Vec<AgentTimelineEventInput>,
}

fn locate_codex_history_utility(app: &AppHandle) -> std::path::PathBuf {
    let runner = locate_agent_runner(app);
    runner
        .parent()
        .map(|dir| dir.join("codex-history.mjs"))
        .unwrap_or_else(|| std::path::PathBuf::from("codex-history.mjs"))
}

fn run_codex_history_utility(app: &AppHandle, payload: JsonValue) -> Result<CodexHistoryUtilityOutput, String> {
    validate_backend_ready_for_send(BACKEND_CODEX)?;
    let script = locate_codex_history_utility(app);
    let connection = resolve_connection_for(app, BACKEND_CODEX);
    let codex_app_server = build_codex_app_server_probe_status();
    let codex_path = codex_app_server
        .path
        .ok_or_else(|| "未找到满足要求的 Codex CLI，无法读取 Codex 历史".to_string())?;

    let mut cmd = Command::new("node");
    cmd.arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("LILIA_CODEX_CLI_PATH", codex_path);
    if let Some(url) = connection.base_url {
        cmd.env("OPENAI_BASE_URL", url);
    }
    if let Some(key) = connection.api_key {
        cmd.env("OPENAI_API_KEY", key);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动 Codex history utility：{e}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        let mut bytes = serde_json::to_vec(&payload)
            .map_err(|e| format!("Codex history payload 序列化失败：{e}"))?;
        bytes.push(b'\n');
        stdin
            .write_all(&bytes)
            .map_err(|e| format!("写入 Codex history utility 失败：{e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待 Codex history utility 失败：{e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| {
            let detail = stderr.trim();
            if detail.is_empty() {
                "Codex history utility 没有返回数据".to_string()
            } else {
                format!("Codex history utility 没有返回数据：{detail}")
            }
        })?;
    let result: CodexHistoryUtilityOutput = serde_json::from_str(line)
        .map_err(|e| format!("解析 Codex history utility 输出失败：{e}"))?;
    if let Some(error) = result.error.as_ref().filter(|s| !s.trim().is_empty()) {
        return Err(error.clone());
    }
    if !output.status.success() {
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            "Codex history utility 异常退出".to_string()
        } else {
            format!("Codex history utility 异常退出：{detail}")
        });
    }
    Ok(result)
}

#[tauri::command]
pub fn codex_thread_search(
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
pub fn codex_thread_preview(
    app: AppHandle,
    thread_id: String,
) -> Result<CodexThreadPreview, String> {
    let result = run_codex_history_utility(
        &app,
        serde_json::json!({
            "action": "preview",
            "threadId": thread_id,
        }),
    )?;
    let thread = result
        .thread
        .ok_or_else(|| "Codex thread preview 缺少 thread 信息".to_string())?;
    let events = preview_events_from_inputs(result.events)?;
    Ok(CodexThreadPreview {
        thread,
        event_count: events.len(),
        events,
    })
}

fn preview_events_from_inputs(inputs: Vec<AgentTimelineEventInput>) -> Result<Vec<AgentTimelineEvent>, String> {
    let conn = rusqlite::Connection::open_in_memory()
        .map_err(|e| format!("创建 preview timeline 内存库失败：{e}"))?;
    conn.execute_batch(
        r#"
        CREATE TABLE agent_timeline_events (
          id                TEXT PRIMARY KEY,
          task_id           TEXT NOT NULL,
          turn_id           TEXT,
          backend           TEXT NOT NULL CHECK (backend IN ('claude','codex')),
          kind              TEXT NOT NULL,
          status            TEXT NOT NULL,
          title             TEXT NOT NULL,
          summary           TEXT,
          payload           TEXT NOT NULL,
          created_at        INTEGER NOT NULL,
          updated_at        INTEGER NOT NULL,
          turn_seq          INTEGER NOT NULL,
          intra_turn_order  INTEGER NOT NULL
        );
        CREATE INDEX idx_agent_timeline_events_task_id_turn
          ON agent_timeline_events(task_id, turn_seq, intra_turn_order);
        "#,
    )
    .map_err(|e| format!("创建 preview timeline schema 失败：{e}"))?;
    for input in inputs {
        agent_timeline::insert(&conn, input)?;
    }
    agent_timeline::list(&conn, "preview")
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
    persist_and_emit_input(app, session_anchor_input(task_id, thread_id, now_millis() as i64));
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

fn stable_history_event_id(task_id: &str, event: &AgentTimelineEventInput) -> Option<String> {
    let payload = event.payload.as_object()?;
    let thread_id = payload.get("threadId")?.as_str()?;
    let turn_id = payload
        .get("turnId")
        .and_then(|value| value.as_str())
        .or(event.turn_id.as_deref())?;
    let item_id = payload.get("itemId")?.as_str()?;
    Some(format!(
        "{task_id}:{turn_id}:codex-history:{thread_id}:{turn_id}:{item_id}"
    ))
}

fn persist_history_events(app: &AppHandle, task_id: &str, events: Vec<AgentTimelineEventInput>) -> usize {
    let mut count = 0;
    for mut event in events {
        event.task_id = task_id.to_string();
        if let Some(id) = stable_history_event_id(task_id, &event) {
            event.id = Some(id);
        }
        persist_and_emit_input(app, event);
        count += 1;
    }
    count
}

#[tauri::command]
pub fn codex_thread_attach(
    app: AppHandle,
    store: State<'_, LiliaStore>,
    chat_store: State<'_, ChatStore>,
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

    let sync = run_codex_history_utility(
        &app,
        serde_json::json!({
            "action": "sync",
            "taskId": input.task_id.as_deref().unwrap_or("pending"),
            "threadId": thread_id,
            "limit": 50,
        }),
    )?;
    let conn = store.conn()?;
    let task = if mode == "new" {
        Some(create_task_for_thread(
            &app,
            &conn,
            input.project_id.clone(),
            sync.thread.as_ref(),
        )?)
    } else {
        let task_id = input
            .task_id
            .as_ref()
            .ok_or_else(|| "接入当前对话需要 taskId".to_string())?;
        Some(task_row_by_id(&conn, task_id)?
            .ok_or_else(|| format!("未找到任务：{task_id}"))?)
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

    let event_count = persist_history_events(&app, &task.id, sync.events);
    insert_session_anchor(&app, &task.id, &thread_id);
    Ok(CodexThreadAttachResult {
        task_id: task.id.clone(),
        project_id: task.project_id.clone(),
        thread_id,
        task: Some(task),
        event_count,
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
            input.payload.get("sessionId").and_then(|value| value.as_str()),
            Some("thread-1")
        );
        assert_eq!(
            input.payload.get("subkind").and_then(|value| value.as_str()),
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
    fn stable_history_event_id_uses_thread_turn_and_item() {
        let input = AgentTimelineEventInput {
            id: None,
            task_id: "temp".to_string(),
            turn_id: Some("turn-1".to_string()),
            backend: BACKEND_CODEX.to_string(),
            kind: "message".to_string(),
            status: "success".to_string(),
            title: "Assistant".to_string(),
            summary: Some("ok".to_string()),
            payload: serde_json::json!({
                "history": true,
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "msg-1",
            }),
            created_at: Some(1),
            updated_at: Some(1),
        };

        assert_eq!(
            stable_history_event_id("task-1", &input).as_deref(),
            Some("task-1:turn-1:codex-history:thread-1:turn-1:msg-1")
        );
    }
}
