use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::chat::state::ChatStore;
use crate::claude_history::{
    claude_session_attach_blocking, claude_session_preview_blocking,
    claude_session_search_blocking, ClaudeSessionAttachInput, ClaudeSessionAttachResult,
    ClaudeSessionPreview, ClaudeSessionPreviewInput, ClaudeSessionSearchInput,
    ClaudeSessionSummary,
};
use crate::codex_history::{
    clean_codex_thread_background_terminals_blocking, codex_thread_attach_blocking,
    codex_thread_preview_blocking, codex_thread_search_blocking, query_codex_thread_runtime_states,
    CodexThreadAttachInput, CodexThreadAttachResult, CodexThreadPreview, CodexThreadPreviewInput,
    CodexThreadRuntimeState, CodexThreadSearchInput, CodexThreadSummary,
};
use crate::projects_tasks::TaskRow;
use crate::store::LiliaStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryImportProvider {
    Codex,
    Claude,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportSearchInput {
    pub provider: HistoryImportProvider,
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
pub struct HistoryImportRuntimeState {
    pub item_id: String,
    pub task_id: String,
    pub task_title: String,
    pub project_id: Option<String>,
    pub running: bool,
    pub queued: bool,
    pub pending: bool,
    pub queued_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportItem {
    pub id: String,
    pub provider: HistoryImportProvider,
    pub title: String,
    pub status: Option<String>,
    pub model: Option<String>,
    pub source_kind: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub archived: bool,
    pub preview: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub runtime: Option<HistoryImportRuntimeState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportSearchResult {
    pub items: Vec<HistoryImportItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportPreviewMessage {
    pub id: String,
    pub role: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportPreviewInput {
    pub provider: HistoryImportProvider,
    pub item_id: String,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportPreview {
    pub item: HistoryImportItem,
    #[serde(default)]
    pub events: Vec<crate::agent_timeline::AgentTimelineEvent>,
    pub event_count: usize,
    #[serde(default)]
    pub messages: Vec<HistoryImportPreviewMessage>,
    #[serde(default)]
    pub has_full_preview: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportAttachInput {
    pub provider: HistoryImportProvider,
    pub item_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub item: Option<HistoryImportItem>,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImportAttachResult {
    pub task_id: String,
    pub project_id: Option<String>,
    pub item_id: String,
    pub task: Option<TaskRow>,
    pub event_count: usize,
    pub history_sync: Option<String>,
}

#[tauri::command]
pub async fn history_import_search(
    app: AppHandle,
    input: HistoryImportSearchInput,
) -> Result<HistoryImportSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || history_import_search_blocking(app, input))
        .await
        .map_err(|err| format!("Lilia history import search 任务执行失败：{err}"))?
}

fn history_import_search_blocking(
    app: AppHandle,
    input: HistoryImportSearchInput,
) -> Result<HistoryImportSearchResult, String> {
    match input.provider {
        HistoryImportProvider::Codex => {
            let result = codex_thread_search_blocking(
                app,
                CodexThreadSearchInput {
                    search_term: input.search_term,
                    cursor: input.cursor,
                    limit: input.limit,
                    archived: input.archived,
                },
            )?;
            Ok(HistoryImportSearchResult {
                items: result
                    .threads
                    .into_iter()
                    .map(HistoryImportItem::from)
                    .collect(),
                next_cursor: result.next_cursor,
            })
        }
        HistoryImportProvider::Claude => {
            let result = claude_session_search_blocking(
                app,
                ClaudeSessionSearchInput {
                    search_term: input.search_term,
                    cursor: input.cursor,
                    limit: input.limit,
                    archived: input.archived,
                },
            )?;
            Ok(HistoryImportSearchResult {
                items: result
                    .sessions
                    .into_iter()
                    .map(HistoryImportItem::from)
                    .collect(),
                next_cursor: result.next_cursor,
            })
        }
    }
}

#[tauri::command]
pub fn history_import_runtime_states(
    app: AppHandle,
    chat_store: tauri::State<'_, ChatStore>,
) -> Result<Vec<HistoryImportRuntimeState>, String> {
    let Some(store) = app.try_state::<LiliaStore>() else {
        return Ok(Vec::new());
    };
    let conn = store.conn()?;
    query_codex_thread_runtime_states(&conn, &chat_store).map(|states| {
        states
            .into_iter()
            .map(HistoryImportRuntimeState::from)
            .collect()
    })
}

#[tauri::command]
pub async fn history_import_clean_background_terminals(
    app: AppHandle,
    item_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        clean_codex_thread_background_terminals_blocking(&app, &item_id)
    })
    .await
    .map_err(|err| format!("Lilia history import clean 任务执行失败：{err}"))?
}

#[tauri::command]
pub async fn history_import_preview(
    app: AppHandle,
    input: HistoryImportPreviewInput,
) -> Result<HistoryImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || history_import_preview_blocking(app, input))
        .await
        .map_err(|err| format!("Lilia history import preview 任务执行失败：{err}"))?
}

fn history_import_preview_blocking(
    app: AppHandle,
    input: HistoryImportPreviewInput,
) -> Result<HistoryImportPreview, String> {
    match input.provider {
        HistoryImportProvider::Codex => {
            let result = codex_thread_preview_blocking(
                app,
                CodexThreadPreviewInput {
                    thread_id: input.item_id,
                    detail: input.detail,
                },
            )?;
            Ok(result.into())
        }
        HistoryImportProvider::Claude => {
            let result = claude_session_preview_blocking(
                app,
                ClaudeSessionPreviewInput {
                    session_id: input.item_id,
                    detail: input.detail,
                },
            )?;
            Ok(result.into())
        }
    }
}

#[tauri::command]
pub async fn history_import_attach(
    app: AppHandle,
    input: HistoryImportAttachInput,
) -> Result<HistoryImportAttachResult, String> {
    tauri::async_runtime::spawn_blocking(move || history_import_attach_blocking(app, input))
        .await
        .map_err(|err| format!("Lilia history import attach 任务执行失败：{err}"))?
}

fn history_import_attach_blocking(
    app: AppHandle,
    input: HistoryImportAttachInput,
) -> Result<HistoryImportAttachResult, String> {
    match input.provider {
        HistoryImportProvider::Codex => {
            let item = input.item.map(CodexThreadSummary::from);
            let result = codex_thread_attach_blocking(
                app,
                CodexThreadAttachInput {
                    thread_id: input.item_id,
                    task_id: input.task_id,
                    project_id: input.project_id,
                    thread: item,
                    mode: input.mode,
                },
            )?;
            Ok(result.into())
        }
        HistoryImportProvider::Claude => {
            let item = input.item.map(ClaudeSessionSummary::from);
            let result = claude_session_attach_blocking(
                app,
                ClaudeSessionAttachInput {
                    session_id: input.item_id,
                    task_id: input.task_id,
                    project_id: input.project_id,
                    session: item,
                    mode: input.mode,
                },
            )?;
            Ok(result.into())
        }
    }
}

impl From<CodexThreadSummary> for HistoryImportItem {
    fn from(thread: CodexThreadSummary) -> Self {
        Self {
            id: thread.id,
            provider: HistoryImportProvider::Codex,
            title: thread.title,
            status: thread.status,
            model: thread.model,
            source_kind: thread.source_kind,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
            archived: thread.archived,
            preview: thread.preview,
            cwd: None,
            project: None,
            runtime: None,
        }
    }
}

impl From<ClaudeSessionSummary> for HistoryImportItem {
    fn from(session: ClaudeSessionSummary) -> Self {
        Self {
            id: session.id,
            provider: HistoryImportProvider::Claude,
            title: session.title,
            status: session.status,
            model: session.model,
            source_kind: session.source_kind,
            created_at: session.created_at,
            updated_at: session.updated_at,
            archived: session.archived,
            preview: session.preview,
            cwd: session.cwd,
            project: session.project,
            runtime: None,
        }
    }
}

impl From<HistoryImportItem> for CodexThreadSummary {
    fn from(item: HistoryImportItem) -> Self {
        Self {
            id: item.id,
            title: item.title,
            status: item.status,
            model: item.model,
            source_kind: item.source_kind,
            created_at: item.created_at,
            updated_at: item.updated_at,
            archived: item.archived,
            preview: item.preview,
        }
    }
}

impl From<HistoryImportItem> for ClaudeSessionSummary {
    fn from(item: HistoryImportItem) -> Self {
        Self {
            id: item.id,
            title: item.title,
            status: item.status,
            model: item.model,
            source_kind: item.source_kind,
            created_at: item.created_at,
            updated_at: item.updated_at,
            archived: item.archived,
            preview: item.preview,
            cwd: item.cwd,
            project: item.project,
        }
    }
}

impl From<CodexThreadRuntimeState> for HistoryImportRuntimeState {
    fn from(state: CodexThreadRuntimeState) -> Self {
        Self {
            item_id: state.thread_id,
            task_id: state.task_id,
            task_title: state.task_title,
            project_id: state.project_id,
            running: state.running,
            queued: state.queued,
            pending: state.pending,
            queued_count: state.queued_count,
        }
    }
}

impl From<CodexThreadPreview> for HistoryImportPreview {
    fn from(preview: CodexThreadPreview) -> Self {
        Self {
            item: preview.thread.into(),
            events: preview.events,
            event_count: preview.event_count,
            messages: preview
                .messages
                .into_iter()
                .map(HistoryImportPreviewMessage::from)
                .collect(),
            has_full_preview: preview.has_full_preview,
        }
    }
}

impl From<ClaudeSessionPreview> for HistoryImportPreview {
    fn from(preview: ClaudeSessionPreview) -> Self {
        Self {
            item: preview.session.into(),
            events: preview.events,
            event_count: preview.event_count,
            messages: preview
                .messages
                .into_iter()
                .map(HistoryImportPreviewMessage::from)
                .collect(),
            has_full_preview: preview.has_full_preview,
        }
    }
}

impl From<crate::codex_history::CodexThreadPreviewMessage> for HistoryImportPreviewMessage {
    fn from(message: crate::codex_history::CodexThreadPreviewMessage) -> Self {
        Self {
            id: message.id,
            role: message.role,
            summary: message.summary,
        }
    }
}

impl From<crate::claude_history::ClaudeSessionPreviewMessage> for HistoryImportPreviewMessage {
    fn from(message: crate::claude_history::ClaudeSessionPreviewMessage) -> Self {
        Self {
            id: message.id,
            role: message.role,
            summary: message.summary,
        }
    }
}

impl From<CodexThreadAttachResult> for HistoryImportAttachResult {
    fn from(result: CodexThreadAttachResult) -> Self {
        Self {
            task_id: result.task_id,
            project_id: result.project_id,
            item_id: result.thread_id,
            task: result.task,
            event_count: result.event_count,
            history_sync: result.history_sync,
        }
    }
}

impl From<ClaudeSessionAttachResult> for HistoryImportAttachResult {
    fn from(result: ClaudeSessionAttachResult) -> Self {
        Self {
            task_id: result.task_id,
            project_id: result.project_id,
            item_id: result.session_id,
            task: result.task,
            event_count: result.event_count,
            history_sync: result.history_sync,
        }
    }
}
