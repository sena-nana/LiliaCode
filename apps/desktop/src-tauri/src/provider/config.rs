use serde_json::Value as JsonValue;
use tauri::{AppHandle, Runtime};

use crate::chat::state::normalize_backend;
use crate::settings_store::load_store_value;
use crate::{BACKEND_CLAUDE, BACKEND_CODEX};
use crate::{RUNTIME_CHANNEL_BUILTIN, RUNTIME_CHANNEL_MUTSUKI_CORE};

use super::types::{
    AgentInteractionSettings, AssistantAIConfig, CCSwitchConfig, CodexControlledPermissions,
    CodexProfileSettings, ProviderConfig,
};

pub(crate) const CC_SWITCH_DEFAULT_URL: &str = "http://127.0.0.1:15721";
pub(crate) const CC_SWITCH_PLACEHOLDER_KEY: &str = "sk-cc-switch-proxy";
pub(crate) const PROVIDER_ACTIVE_BACKEND_KEY: &str = "provider.activeBackend";
pub(crate) const PROVIDER_KEY_CLAUDE: &str = "provider.claude";
pub(crate) const PROVIDER_KEY_CODEX: &str = "provider.codex";
pub(crate) const CC_SWITCH_KEY: &str = "cc-switch.config";
pub(crate) const ROUTER_KEY_CLAUDE: &str = "router.claude";
pub(crate) const ROUTER_KEY_CODEX: &str = "router.codex";
pub(crate) const ASSISTANT_AI_KEY: &str = "assistant-ai.config";
pub(crate) const AGENT_INTERACTION_KEY: &str = "agent-interaction.config";
pub(crate) const ROUTER_CC_SWITCH: &str = "cc-switch";
pub(crate) const ROUTER_DIRECT: &str = "direct";

pub(crate) fn provider_key_for_backend(backend: &str) -> &'static str {
    match backend {
        BACKEND_CODEX => PROVIDER_KEY_CODEX,
        _ => PROVIDER_KEY_CLAUDE,
    }
}

pub(crate) fn known_provider_key_for_backend(backend: &str) -> Result<&'static str, String> {
    match backend {
        BACKEND_CODEX => Ok(PROVIDER_KEY_CODEX),
        BACKEND_CLAUDE => Ok(PROVIDER_KEY_CLAUDE),
        other => Err(format!("未知 backend: {other}")),
    }
}

pub(crate) fn router_key_for_backend(backend: &str) -> Result<&'static str, String> {
    match backend {
        BACKEND_CODEX => Ok(ROUTER_KEY_CODEX),
        BACKEND_CLAUDE => Ok(ROUTER_KEY_CLAUDE),
        other => Err(format!("未知 backend: {other}")),
    }
}

pub(crate) fn backend_api_key_env(backend: &str) -> &'static str {
    match backend {
        BACKEND_CODEX => "OPENAI_API_KEY",
        _ => "ANTHROPIC_API_KEY",
    }
}

pub(crate) fn backend_direct_url(backend: &str) -> &'static str {
    match backend {
        BACKEND_CODEX => "https://api.openai.com/v1",
        _ => "https://api.anthropic.com",
    }
}

pub(crate) fn load_provider_config<R: Runtime>(
    app: &AppHandle<R>,
    key: &str,
) -> Option<ProviderConfig> {
    load_store_value(app, key)
}

pub(crate) fn load_active_backend<R: Runtime>(app: &AppHandle<R>) -> String {
    load_store_value::<String, _>(app, PROVIDER_ACTIVE_BACKEND_KEY)
        .map(|s| normalize_backend(&s).to_string())
        .unwrap_or_else(|| BACKEND_CLAUDE.to_string())
}

pub(crate) fn load_cc_switch_config<R: Runtime>(app: &AppHandle<R>) -> CCSwitchConfig {
    load_store_value(app, CC_SWITCH_KEY).unwrap_or_default()
}

pub(crate) fn load_assistant_ai_config<R: Runtime>(app: &AppHandle<R>) -> AssistantAIConfig {
    load_store_value(app, ASSISTANT_AI_KEY).unwrap_or_default()
}

pub(crate) fn load_agent_interaction_settings<R: Runtime>(
    app: &AppHandle<R>,
) -> AgentInteractionSettings {
    normalize_agent_interaction_settings(load_store_value(app, AGENT_INTERACTION_KEY))
}

pub(crate) fn normalize_agent_interaction_settings(
    settings: Option<AgentInteractionSettings>,
) -> AgentInteractionSettings {
    let settings = settings.unwrap_or_default();
    AgentInteractionSettings {
        non_interrupt_mode: settings.non_interrupt_mode,
        debug: settings.debug,
        agent_runtime_channel: normalize_agent_runtime_channel(settings.agent_runtime_channel),
        codex_profile: normalize_codex_profile_settings(settings.codex_profile),
    }
}

pub(crate) fn normalize_agent_runtime_channel(channel: String) -> String {
    match channel.as_str() {
        RUNTIME_CHANNEL_MUTSUKI_CORE => RUNTIME_CHANNEL_MUTSUKI_CORE.to_string(),
        _ => RUNTIME_CHANNEL_BUILTIN.to_string(),
    }
}

pub(crate) fn normalize_codex_profile_settings(
    settings: CodexProfileSettings,
) -> CodexProfileSettings {
    CodexProfileSettings {
        profile: match settings.profile.as_str() {
            "fast" | "balanced" | "deep" => settings.profile,
            _ => "default".to_string(),
        },
        model: normalize_optional_string(settings.model),
        reasoning_effort: normalize_reasoning_effort(settings.reasoning_effort),
        runtime_workspace_roots: normalize_runtime_workspace_roots(
            settings.runtime_workspace_roots,
        ),
        permissions: CodexControlledPermissions {
            profile: match settings.permissions.profile.as_str() {
                "readOnly" | "workspaceWrite" | "dangerFullAccess" => settings.permissions.profile,
                _ => "default".to_string(),
            },
        },
        responses_api_client_metadata: normalize_json_object(
            settings.responses_api_client_metadata,
        ),
        additional_context: normalize_optional_string(settings.additional_context),
        persist_extended_history: settings.persist_extended_history,
        initial_turns_page: normalize_json_object(settings.initial_turns_page),
        exclude_turns: normalize_string_list(settings.exclude_turns),
        command_exec_permission_profile: normalize_optional_permission_profile(
            settings.command_exec_permission_profile,
        ),
    }
}

pub(crate) fn normalize_json_object(value: Option<JsonValue>) -> Option<JsonValue> {
    match value {
        Some(JsonValue::Object(map)) if !map.is_empty() => Some(JsonValue::Object(map)),
        _ => None,
    }
}

pub(crate) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn normalize_reasoning_effort(value: Option<String>) -> Option<String> {
    let value = normalize_optional_string(value)?;
    match value.as_str() {
        "low" | "medium" | "high" | "xhigh" => Some(value),
        _ => None,
    }
}

pub(crate) fn normalize_runtime_workspace_roots(roots: Vec<String>) -> Vec<String> {
    normalize_string_list(roots)
}

pub(crate) fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || normalized.iter().any(|seen| seen == trimmed) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

pub(crate) fn normalize_optional_permission_profile(value: Option<String>) -> Option<String> {
    let value = normalize_optional_string(value)?;
    match value.as_str() {
        "default" | "readOnly" | "workspaceWrite" | "dangerFullAccess" => Some(value),
        _ => None,
    }
}

pub(crate) fn load_router_mode<R: Runtime>(app: &AppHandle<R>, backend: &str) -> String {
    let key = router_key_for_backend(normalize_backend(backend)).unwrap_or(ROUTER_KEY_CLAUDE);
    load_store_value::<String, _>(app, key)
        .filter(|m| matches!(m.as_str(), ROUTER_CC_SWITCH | ROUTER_DIRECT))
        .unwrap_or_else(|| ROUTER_CC_SWITCH.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_profile_settings_are_normalized_to_controlled_values() {
        let normalized = normalize_codex_profile_settings(CodexProfileSettings {
            profile: "unknown".to_string(),
            model: Some("  gpt-5.5  ".to_string()),
            reasoning_effort: Some("ultra".to_string()),
            runtime_workspace_roots: vec![
                " C:/repo ".to_string(),
                "".to_string(),
                "C:/repo".to_string(),
                "D:/shared".to_string(),
            ],
            permissions: CodexControlledPermissions {
                profile: "{\"fileSystem\":true}".to_string(),
            },
            responses_api_client_metadata: Some(serde_json::json!({ "surface": "lilia" })),
            additional_context: Some("  extra context  ".to_string()),
            persist_extended_history: Some(true),
            initial_turns_page: Some(serde_json::json!([])),
            exclude_turns: vec![" turn-1 ".to_string(), "turn-1".to_string(), "".to_string()],
            command_exec_permission_profile: Some("workspaceWrite".to_string()),
        });

        assert_eq!(normalized.profile, "default");
        assert_eq!(normalized.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(normalized.reasoning_effort, None);
        assert_eq!(
            normalized.runtime_workspace_roots,
            vec!["C:/repo", "D:/shared"]
        );
        assert_eq!(normalized.permissions.profile, "default");
        assert_eq!(
            normalized.responses_api_client_metadata,
            Some(serde_json::json!({ "surface": "lilia" }))
        );
        assert_eq!(
            normalized.additional_context.as_deref(),
            Some("extra context")
        );
        assert_eq!(normalized.persist_extended_history, Some(true));
        assert_eq!(normalized.initial_turns_page, None);
        assert_eq!(normalized.exclude_turns, vec!["turn-1"]);
        assert_eq!(
            normalized.command_exec_permission_profile.as_deref(),
            Some("workspaceWrite")
        );
    }
}
