use std::collections::HashMap;

use serde_json::Value as JsonValue;
use tauri::AppHandle;

use crate::settings_store::save_store_value;
use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

use super::assistant_ai;
use super::codex_probe::{build_codex_app_server_probe_status_cached, cli_available};
use super::codex_update::{
    check_codex_app_server_update_status, install_or_update_codex_app_server,
};
use super::config::{
    known_provider_key_for_backend, load_active_backend, load_agent_interaction_settings,
    load_router_mode, normalize_agent_interaction_settings, public_assistant_ai_config,
    public_provider_config, router_key_for_backend, save_assistant_ai_config_metadata,
    save_provider_config_metadata, AGENT_INTERACTION_KEY, PROVIDER_ACTIVE_BACKEND_KEY, ROUTER_API,
    ROUTER_CODEX_ACCOUNT,
};
use super::connection::build_backend_env_status;
use super::credentials::{apply_secret_update, assistant_ai_account, provider_account};
use super::subagents::{
    delete_custom_subagent, load_custom_subagents, upsert_custom_subagent,
    CustomSubagentUpsertInput,
};
use super::types::{
    AgentInteractionSettings, AssistantAIConfig, AssistantAITestResult, CodexAppServerStatus,
    CustomSubagentDefinition, EnvStatusReport, ProviderConfig,
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
        router_modes,
        backends,
    }
}

#[tauri::command]
pub fn provider_codex_app_server_check_update() -> CodexAppServerStatus {
    check_codex_app_server_update_status()
}

#[tauri::command]
pub fn provider_codex_app_server_install_update() -> Result<CodexAppServerStatus, String> {
    install_or_update_codex_app_server()
}

#[tauri::command]
pub fn provider_get_config(app: AppHandle, backend: String) -> ProviderConfig {
    public_provider_config(&app, &backend)
}

#[tauri::command]
pub fn provider_set_config(app: AppHandle, config: ProviderConfig) -> Result<(), String> {
    let key = known_provider_key_for_backend(&config.backend)?;
    let account = provider_account(&config.backend)?;
    apply_secret_update(&account, config.api_key.as_deref(), config.clear_api_key)?;
    save_provider_config_metadata(&app, key, config.backend, config.base_url)
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
pub fn assistant_ai_get_config(app: AppHandle) -> AssistantAIConfig {
    public_assistant_ai_config(&app)
}

#[tauri::command]
pub fn assistant_ai_set_config(app: AppHandle, config: AssistantAIConfig) -> Result<(), String> {
    apply_secret_update(
        assistant_ai_account(),
        config.api_key.as_deref(),
        config.clear_api_key,
    )?;
    save_assistant_ai_config_metadata(&app, config.base_url, config.model)
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
pub fn agent_interaction_list_subagents() -> Result<Vec<CustomSubagentDefinition>, String> {
    load_custom_subagents()
}

#[tauri::command]
pub fn agent_interaction_upsert_subagent(
    input: CustomSubagentUpsertInput,
) -> Result<CustomSubagentDefinition, String> {
    upsert_custom_subagent(input)
}

#[tauri::command]
pub fn agent_interaction_delete_subagent(id: String) -> Result<(), String> {
    delete_custom_subagent(&id)
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
    if !matches!(mode.as_str(), ROUTER_API | ROUTER_CODEX_ACCOUNT) {
        return Err(format!("未知路由模式: {mode}"));
    }
    if mode == ROUTER_CODEX_ACCOUNT && backend != BACKEND_CODEX {
        return Err("Claude 不支持 Codex 官方账号模式".to_string());
    }
    let key = router_key_for_backend(&backend)?;
    save_store_value(&app, key, &JsonValue::String(mode))
}
