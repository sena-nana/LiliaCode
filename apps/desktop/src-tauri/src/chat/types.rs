use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatMessage {
    pub(crate) id: String,
    pub(crate) task_id: String,
    pub(crate) role: String, // "user" | "assistant" | "system"
    pub(crate) content: String,
    pub(crate) attachments: Vec<ChatAttachment>,
    pub(crate) created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatAttachment {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) size: Option<u64>,
    #[serde(default = "default_attachment_exists")]
    pub(crate) exists: bool,
    #[serde(default)]
    pub(crate) mime: Option<String>,
    #[serde(default)]
    pub(crate) directory: Option<ChatAttachmentDirectoryMeta>,
}

fn default_attachment_exists() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatAttachmentDirectoryMeta {
    pub(crate) file_count: u64,
    pub(crate) directory_count: u64,
    pub(crate) total_size: u64,
    pub(crate) truncated: bool,
    pub(crate) unreadable_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatContextSearchResult {
    pub(crate) attachment: ChatAttachment,
    pub(crate) relative_path: String,
    pub(crate) matched_by: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipboardImageInput {
    pub(crate) mime: Option<String>,
    pub(crate) bytes_base64: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipboardTextInput {
    pub(crate) text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatSendResult {
    pub(crate) message: ChatMessage,
    /// "started" | "queued"
    pub(crate) dispatch: String,
    pub(crate) queued_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatComposerState {
    pub(crate) task_id: String,
    /// "claude" | "codex"
    pub(crate) backend: String,
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) plan_mode: bool,
    /// "full" | "ask" | "readonly"
    pub(crate) permission: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatModelOption {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) backend: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnStartedEvent {
    pub(crate) task_id: String,
    pub(crate) queued_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DoneEvent {
    pub(crate) task_id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) subtype: Option<String>,
}

/// 工具调用授权请求：runner 调用 canUseTool 时通过 stdout 转过来的事实字段。
/// 前端 ToolConsentBridge 收到后弹 AskUser 浮层，再用 chat_respond_tool_consent
/// 把决策写回。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolConsentRequestEvent {
    pub(crate) task_id: String,
    pub(crate) turn_id: String,
    pub(crate) backend: String,
    pub(crate) request_id: String,
    pub(crate) tool_name: String,
    pub(crate) input: JsonValue,
    pub(crate) title: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) blocked_path: Option<String>,
    pub(crate) decision_reason: Option<String>,
    pub(crate) tool_use_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AskUserRequestEvent {
    pub(crate) task_id: String,
    pub(crate) turn_id: String,
    pub(crate) backend: String,
    pub(crate) request_id: String,
    pub(crate) spec: JsonValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInteractionRequestEvent {
    pub(crate) task_id: String,
    pub(crate) turn_id: String,
    pub(crate) backend: String,
    pub(crate) request_id: String,
    pub(crate) kind: String,
    pub(crate) payload: JsonValue,
}
