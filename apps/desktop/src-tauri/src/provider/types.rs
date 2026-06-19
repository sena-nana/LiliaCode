use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInteractionSettings {
    #[serde(default)]
    pub(crate) non_interrupt_mode: bool,
    #[serde(default)]
    pub(crate) debug: bool,
    #[serde(default = "default_permission_mode")]
    pub(crate) permission_mode: String,
    #[serde(default)]
    pub(crate) codex_profile: CodexProfileSettings,
    #[serde(default)]
    pub(crate) subagent_mode: SubagentModeSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubagentBackendSettings {
    #[serde(default = "default_enabled_true")]
    pub(crate) enabled: bool,
}

impl Default for SubagentBackendSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeSubagentModeSettings {
    #[serde(default = "default_enabled_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_enabled_true")]
    pub(crate) forward_subagent_text: bool,
    #[serde(default = "default_enabled_true")]
    pub(crate) agent_progress_summaries: bool,
}

impl Default for ClaudeSubagentModeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            forward_subagent_text: true,
            agent_progress_summaries: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubagentModeSettings {
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) codex: SubagentBackendSettings,
    #[serde(default)]
    pub(crate) claude: ClaudeSubagentModeSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CustomSubagentDefinition {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    pub(crate) instruction: String,
    #[serde(default)]
    pub(crate) enabled: bool,
}

fn default_codex_profile_name() -> String {
    "default".to_string()
}

fn default_enabled_true() -> bool {
    true
}

fn default_permission_mode() -> String {
    "ask".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexProfileSettings {
    #[serde(default = "default_codex_profile_name")]
    pub(crate) profile: String,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(default)]
    pub(crate) runtime_workspace_roots: Vec<String>,
    #[serde(default)]
    pub(crate) responses_api_client_metadata: Option<JsonValue>,
    #[serde(default)]
    pub(crate) additional_context: Option<String>,
    #[serde(default)]
    pub(crate) persist_extended_history: Option<bool>,
    #[serde(default)]
    pub(crate) initial_turns_page: Option<JsonValue>,
    #[serde(default)]
    pub(crate) exclude_turns: Vec<String>,
}

impl Default for CodexProfileSettings {
    fn default() -> Self {
        Self {
            profile: "default".to_string(),
            model: None,
            reasoning_effort: None,
            runtime_workspace_roots: Vec::new(),
            responses_api_client_metadata: None,
            additional_context: None,
            persist_extended_history: None,
            initial_turns_page: None,
            exclude_turns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConfig {
    pub(crate) backend: String,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
    #[serde(default)]
    pub(crate) has_api_key: bool,
    #[serde(default)]
    pub(crate) clear_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantAIConfig {
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) codex_account_spark_enabled: bool,
    #[serde(default)]
    pub(crate) has_api_key: bool,
    #[serde(default)]
    pub(crate) clear_api_key: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantAITestResult {
    pub(crate) ok: bool,
    pub(crate) error: Option<String>,
    pub(crate) models: Option<Vec<String>>,
    pub(crate) model_matched: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackendEnvStatus {
    pub(crate) backend: String,
    pub(crate) has_api_key: bool,
    pub(crate) connection_mode: String,
    pub(crate) effective_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexAppServerStatus {
    pub(crate) version: Option<String>,
    pub(crate) install_path: Option<String>,
    pub(crate) managed: bool,
    pub(crate) available: bool,
    pub(crate) supports_required_protocol: bool,
    pub(crate) failure_kind: Option<String>,
    pub(crate) issues: Vec<String>,
    pub(crate) latest_version: Option<String>,
    pub(crate) update_available: bool,
    pub(crate) release_notes: Vec<String>,
    pub(crate) update_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvStatusReport {
    pub(crate) node_available: bool,
    pub(crate) codex_cli_available: bool,
    pub(crate) codex_app_server: CodexAppServerStatus,
    pub(crate) router_modes: HashMap<String, String>,
    pub(crate) backends: HashMap<String, BackendEnvStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionMode {
    Api,
    CustomBaseUrl,
    CodexAccount,
    Unconfigured,
}

impl ConnectionMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ConnectionMode::Api => "api",
            ConnectionMode::CustomBaseUrl => "custom",
            ConnectionMode::CodexAccount => "codex-account",
            ConnectionMode::Unconfigured => "unconfigured",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackendConnectionPlan {
    pub(crate) mode: ConnectionMode,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CodexAppServerProbeStatus {
    pub(crate) public: CodexAppServerStatus,
    pub(crate) path: Option<String>,
}
