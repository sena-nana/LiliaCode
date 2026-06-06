use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInteractionSettings {
    #[serde(default)]
    pub(crate) non_interrupt_mode: bool,
    #[serde(default)]
    pub(crate) debug: bool,
    #[serde(default)]
    pub(crate) codex_profile: CodexProfileSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexControlledPermissions {
    pub(crate) profile: String,
}

impl Default for CodexControlledPermissions {
    fn default() -> Self {
        Self {
            profile: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexProfileSettings {
    pub(crate) profile: String,
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    #[serde(default)]
    pub(crate) runtime_workspace_roots: Vec<String>,
    #[serde(default)]
    pub(crate) permissions: CodexControlledPermissions,
}

impl Default for CodexProfileSettings {
    fn default() -> Self {
        Self {
            profile: "default".to_string(),
            model: None,
            reasoning_effort: None,
            runtime_workspace_roots: Vec::new(),
            permissions: CodexControlledPermissions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConfig {
    pub(crate) backend: String,
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CCSwitchConfig {
    pub(crate) base_url: Option<String>,
}

impl Default for CCSwitchConfig {
    fn default() -> Self {
        CCSwitchConfig {
            base_url: Some(super::config::CC_SWITCH_DEFAULT_URL.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantAIConfig {
    pub(crate) base_url: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) model: Option<String>,
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
pub(crate) struct CCSwitchStatus {
    pub(crate) reachable: bool,
    pub(crate) base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexAppServerStatus {
    pub(crate) version: Option<String>,
    pub(crate) available: bool,
    pub(crate) supports_required_protocol: bool,
    pub(crate) failure_kind: Option<String>,
    pub(crate) issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnvStatusReport {
    pub(crate) node_available: bool,
    pub(crate) codex_cli_available: bool,
    pub(crate) codex_app_server: CodexAppServerStatus,
    pub(crate) cc_switch: CCSwitchStatus,
    pub(crate) router_modes: HashMap<String, String>,
    pub(crate) backends: HashMap<String, BackendEnvStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionMode {
    CcSwitch,
    CustomBaseUrl,
    Direct,
    Unconfigured,
}

impl ConnectionMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ConnectionMode::CcSwitch => "cc-switch",
            ConnectionMode::CustomBaseUrl => "custom",
            ConnectionMode::Direct => "direct",
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
