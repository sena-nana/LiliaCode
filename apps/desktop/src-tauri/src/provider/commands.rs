use std::collections::HashMap;

use serde_json::Value as JsonValue;
use tauri::AppHandle;

use crate::settings_store::save_store_value;
use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

use super::assistant_ai;
use super::codex_probe::{build_codex_app_server_probe_status_cached, cli_available};
use super::config::{
    known_provider_key_for_backend, load_active_backend, load_agent_interaction_settings,
    load_assistant_ai_config, load_cc_switch_config, load_provider_config, load_router_mode,
    normalize_agent_interaction_settings, provider_key_for_backend, router_key_for_backend,
    AGENT_INTERACTION_KEY, ASSISTANT_AI_KEY, CC_SWITCH_KEY, PROVIDER_ACTIVE_BACKEND_KEY,
    ROUTER_CC_SWITCH, ROUTER_DIRECT,
};
use super::connection::{build_backend_env_status, build_cc_switch_status};
use super::types::{
    AgentInteractionSettings, AssistantAIConfig, AssistantAITestResult, CCSwitchConfig,
    EnvStatusReport, ProviderConfig,
};

#[tauri::command]
pub fn chat_check_env(app: AppHandle, force_refresh: Option<bool>) -> EnvStatusReport {
    let node_available = cli_available("node");
    let codex_app_server =
        build_codex_app_server_probe_status_cached(force_refresh.unwrap_or(false));
    let codex_cli_available = codex_app_server.path.is_some();

    let mut backends = HashMap::new();
    backends.insert(
        BACKEND_CLAUDE.to_string(),
        build_backend_env_status(&app, BACKEND_CLAUDE),
    );
    backends.insert(
        BACKEND_CODEX.to_string(),
        build_backend_env_status(&app, BACKEND_CODEX),
    );

    let mut router_modes = HashMap::new();
    router_modes.insert(
        BACKEND_CLAUDE.to_string(),
        load_router_mode(&app, BACKEND_CLAUDE),
    );
    router_modes.insert(
        BACKEND_CODEX.to_string(),
        load_router_mode(&app, BACKEND_CODEX),
    );

    EnvStatusReport {
        node_available,
        codex_cli_available,
        codex_app_server: codex_app_server.public,
        cc_switch: build_cc_switch_status(&app),
        router_modes,
        backends,
    }
}

#[tauri::command]
pub fn provider_get_config(app: AppHandle, backend: String) -> ProviderConfig {
    load_provider_config(&app, provider_key_for_backend(&backend)).unwrap_or_else(|| {
        ProviderConfig {
            backend: backend.clone(),
            base_url: None,
            api_key: None,
        }
    })
}

#[tauri::command]
pub fn provider_set_config(app: AppHandle, config: ProviderConfig) -> Result<(), String> {
    let key = known_provider_key_for_backend(&config.backend)?;
    save_store_value(&app, key, &config)
}

#[tauri::command]
pub fn provider_get_active_backend(app: AppHandle) -> String {
    load_active_backend(&app)
}

#[tauri::command]
pub fn provider_set_active_backend(app: AppHandle, backend: String) -> Result<(), String> {
    match backend.as_str() {
        BACKEND_CLAUDE | BACKEND_CODEX => {
            save_store_value(&app, PROVIDER_ACTIVE_BACKEND_KEY, &backend)
        }
        other => Err(format!("未知 backend: {other}")),
    }
}

#[tauri::command]
pub fn cc_switch_get_config(app: AppHandle) -> CCSwitchConfig {
    load_cc_switch_config(&app)
}

#[tauri::command]
pub fn cc_switch_set_config(app: AppHandle, config: CCSwitchConfig) -> Result<(), String> {
    save_store_value(&app, CC_SWITCH_KEY, &config)
}

#[tauri::command]
pub fn assistant_ai_get_config(app: AppHandle) -> AssistantAIConfig {
    load_assistant_ai_config(&app)
}

#[tauri::command]
pub fn assistant_ai_set_config(app: AppHandle, config: AssistantAIConfig) -> Result<(), String> {
    save_store_value(&app, ASSISTANT_AI_KEY, &config)
}

#[tauri::command]
pub fn agent_interaction_get_settings(app: AppHandle) -> AgentInteractionSettings {
    load_agent_interaction_settings(&app)
}

#[tauri::command]
pub fn agent_interaction_set_settings(
    app: AppHandle,
    settings: AgentInteractionSettings,
) -> Result<(), String> {
    let settings = normalize_agent_interaction_settings(Some(settings));
    save_store_value(&app, AGENT_INTERACTION_KEY, &settings)
}

#[tauri::command]
pub fn assistant_ai_test_connection(config: AssistantAIConfig) -> AssistantAITestResult {
    assistant_ai::test_connection(config)
}

#[tauri::command]
pub fn router_get_mode(app: AppHandle, backend: String) -> String {
    load_router_mode(&app, &backend)
}

#[tauri::command]
pub fn router_set_mode(app: AppHandle, backend: String, mode: String) -> Result<(), String> {
    if !matches!(mode.as_str(), ROUTER_CC_SWITCH | ROUTER_DIRECT) {
        return Err(format!("未知路由模式: {mode}"));
    }
    let key = router_key_for_backend(&backend)?;
    save_store_value(&app, key, &JsonValue::String(mode))
}
