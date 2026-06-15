use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    WaitingUser,
}

impl AutomationRunStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::WaitingUser => "waiting_user",
        }
    }

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            "waiting_user" => Self::WaitingUser,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationScopeFilter {
    #[serde(default)]
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub include_inbox: bool,
    #[serde(default)]
    pub task_statuses: Vec<String>,
    #[serde(default)]
    pub backends: Vec<String>,
    #[serde(default)]
    pub event_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationNodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationNode {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub position: AutomationNodePosition,
    #[serde(default)]
    pub config: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub source_handle: Option<String>,
    #[serde(default)]
    pub target_handle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationDraft {
    #[serde(default)]
    pub nodes: Vec<AutomationNode>,
    #[serde(default)]
    pub edges: Vec<AutomationEdge>,
    #[serde(default)]
    pub scope: AutomationScopeFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationWorkflow {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub scope: AutomationScopeFilter,
    pub draft: AutomationDraft,
    pub published_version_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationWorkflowVersion {
    pub id: String,
    pub workflow_id: String,
    pub version: i64,
    pub snapshot: AutomationDraft,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationSignalEnvelope {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub event_kind: Option<String>,
    #[serde(default)]
    pub automation_run_id: Option<String>,
    #[serde(default)]
    pub payload: JsonValue,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRun {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub status: AutomationRunStatus,
    pub trigger: AutomationSignalEnvelope,
    pub scope: AutomationScopeFilter,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunSummary {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub status: AutomationRunStatus,
    pub trigger_kind: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub event_kind: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunNodeState {
    pub id: String,
    pub run_id: String,
    pub node_id: String,
    pub status: AutomationRunStatus,
    #[serde(default)]
    pub input: JsonValue,
    #[serde(default)]
    pub output: Option<JsonValue>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationSaveDraftInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub scope: AutomationScopeFilter,
    #[serde(default)]
    pub nodes: Vec<AutomationNode>,
    #[serde(default)]
    pub edges: Vec<AutomationEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunOnceInput {
    #[serde(default)]
    pub payload: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomationResumeRunInput {
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub payload: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunDetail {
    pub run: AutomationRun,
    pub nodes: Vec<AutomationRunNodeState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutomationChangedEvent {
    pub(crate) workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutomationRunEvent {
    pub(crate) run: AutomationRun,
}

pub(crate) enum GraphExecution {
    Finished,
    WaitingUser,
    WaitingAgent,
}
