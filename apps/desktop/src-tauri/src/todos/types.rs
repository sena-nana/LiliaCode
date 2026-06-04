use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub(super) const PRIORITY_HIGH: &str = "high";
pub(super) const PRIORITY_NORMAL: &str = "normal";
pub(super) const PRIORITY_LOW: &str = "low";

pub(super) const GUIDE_STATUS_PENDING: &str = "pending";
pub(super) const GUIDE_STATUS_QUEUED: &str = "queued";
pub(super) const GUIDE_STATUS_SENT: &str = "sent";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskTodo {
    pub id: String,
    pub task_id: String,
    pub text: String,
    pub done: bool,
    pub order: i64,
    pub source: String,
    pub priority: String,
    pub guide_status: Option<String>,
    pub attachments: Vec<JsonValue>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTodoItem {
    #[serde(alias = "text", alias = "title", alias = "description")]
    pub content: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub completed: Option<bool>,
    #[serde(default)]
    pub done: Option<bool>,
    #[serde(default)]
    pub priority: Option<String>,
}

impl AgentTodoItem {
    pub(super) fn is_done(&self) -> bool {
        self.completed.unwrap_or(false)
            || self.done.unwrap_or(false)
            || self.status.eq_ignore_ascii_case("completed")
    }

    pub(super) fn normalized_priority(&self) -> String {
        normalize_priority(self.priority.as_deref())
    }
}

pub(super) fn normalize_priority(value: Option<&str>) -> String {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        PRIORITY_HIGH => PRIORITY_HIGH.to_string(),
        PRIORITY_LOW => PRIORITY_LOW.to_string(),
        _ => PRIORITY_NORMAL.to_string(),
    }
}

pub(super) fn normalize_guide_status(value: Option<&str>) -> Option<String> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        GUIDE_STATUS_PENDING => Some(GUIDE_STATUS_PENDING.to_string()),
        GUIDE_STATUS_QUEUED => Some(GUIDE_STATUS_QUEUED.to_string()),
        GUIDE_STATUS_SENT => Some(GUIDE_STATUS_SENT.to_string()),
        _ => None,
    }
}
