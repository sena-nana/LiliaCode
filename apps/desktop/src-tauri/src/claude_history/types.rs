use serde::{Deserialize, Serialize};

use crate::agent_timeline::{AgentTimelineEvent, AgentTimelineEventInput};
use crate::projects_tasks::TaskRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionSearchInput {
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
pub struct ClaudeSessionSummary {
    pub id: String,
    pub title: String,
    pub status: Option<String>,
    pub model: Option<String>,
    pub source_kind: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub archived: bool,
    pub preview: Option<String>,
    pub cwd: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionSearchResult {
    pub sessions: Vec<ClaudeSessionSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionPreview {
    pub session: ClaudeSessionSummary,
    #[serde(default)]
    pub events: Vec<AgentTimelineEvent>,
    pub event_count: usize,
    #[serde(default)]
    pub messages: Vec<ClaudeSessionPreviewMessage>,
    #[serde(default)]
    pub has_full_preview: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionPreviewMessage {
    pub id: String,
    pub role: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionPreviewInput {
    pub session_id: String,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionAttachInput {
    pub session_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub session: Option<ClaudeSessionSummary>,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionAttachResult {
    pub task_id: String,
    pub project_id: Option<String>,
    pub session_id: String,
    pub task: Option<TaskRow>,
    pub event_count: usize,
    pub history_sync: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClaudeHistoryUtilityOutput {
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) sessions: Vec<ClaudeSessionSummary>,
    #[serde(default)]
    pub(super) next_cursor: Option<String>,
    #[serde(default)]
    pub(super) session: Option<ClaudeSessionSummary>,
    #[serde(default)]
    pub(super) events: Vec<AgentTimelineEventInput>,
    #[serde(default)]
    pub(super) event_count: Option<usize>,
    #[serde(default)]
    pub(super) messages: Vec<ClaudeSessionPreviewMessage>,
    #[serde(default)]
    pub(super) has_full_preview: bool,
}
