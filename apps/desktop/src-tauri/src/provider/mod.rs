mod assistant_ai;
mod codex_probe;
mod codex_spark;
mod codex_update;
mod commands;
mod config;
mod connection;
mod credentials;
mod subagents;
mod types;

pub use commands::*;

pub(crate) use codex_probe::validate_backend_ready_for_send;
pub(crate) use codex_spark::{
    codex_account_spark_enabled, is_codex_account_spark_request, request_codex_account_spark,
    CODEX_SPARK_BASE_URL, CODEX_SPARK_MODEL,
};
pub(crate) use config::{
    assistant_ai_secret, backend_api_key_env, backend_direct_url, build_effective_claude_settings,
    build_effective_codex_subagent_settings, load_active_backend, load_agent_interaction_settings,
    load_assistant_ai_config,
};
pub(crate) use connection::resolve_connection_for;
pub(crate) use types::{
    AssistantAIConfig, AutoTurnDecisionSettings, BackendConnectionPlan, CodexProfileSettings,
    ConnectionMode, CustomSubagentDefinition,
};

pub(crate) use subagents::CustomSubagentUpsertInput;

#[cfg(test)]
pub(crate) use codex_probe::*;
#[cfg(test)]
pub(crate) use types::*;
