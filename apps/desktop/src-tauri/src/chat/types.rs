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

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatInterruptResult {
    pub(crate) rolled_back: bool,
    pub(crate) restored_content: String,
    pub(crate) restored_attachments: Vec<ChatAttachment>,
    pub(crate) removed_event_ids: Vec<String>,
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
    #[serde(default)]
    pub(crate) codex_settings: CodexComposerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexComposerSettings {
    #[serde(default)]
    pub(crate) profile: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(default)]
    pub(crate) runtime_workspace_roots: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) permissions: Option<CodexComposerPermissions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexComposerPermissions {
    #[serde(default)]
    pub(crate) profile: String,
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
