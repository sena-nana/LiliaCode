use std::sync::OnceLock;

use serde::Deserialize;

use super::types::{
    AgentInteractionSettings, AutoTurnDecisionSettings, ClaudeSubagentModeSettings,
    CodexProfileSettings, SubagentBackendSettings, SubagentModeSettings,
};

const AGENT_INTERACTION_DEFAULTS_JSON: &str =
    include_str!("../../../../../packages/contracts/src/agent-interaction-defaults.json");

static AGENT_INTERACTION_DEFAULTS: OnceLock<AgentInteractionSettings> = OnceLock::new();
static RAW_AGENT_INTERACTION_DEFAULTS: OnceLock<RawAgentInteractionSettings> = OnceLock::new();
static DEFAULT_PERMISSION_MODE: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAgentInteractionSettings {
    non_interrupt_mode: bool,
    debug: bool,
    permission_mode: String,
    main_agent_prompt_mode: String,
    codex_profile: CodexProfileSettings,
    subagent_mode: SubagentModeSettings,
    auto_turn_decision: AutoTurnDecisionSettings,
}

fn raw_agent_interaction_defaults() -> &'static RawAgentInteractionSettings {
    RAW_AGENT_INTERACTION_DEFAULTS.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            AGENT_INTERACTION_DEFAULTS_JSON,
            "agent-interaction-defaults.json",
        )
    })
}

pub(super) fn agent_interaction_settings() -> AgentInteractionSettings {
    AGENT_INTERACTION_DEFAULTS
        .get_or_init(|| {
            let raw = raw_agent_interaction_defaults();
            AgentInteractionSettings {
                non_interrupt_mode: raw.non_interrupt_mode,
                debug: raw.debug,
                permission_mode: raw.permission_mode.clone(),
                main_agent_prompt_mode: raw.main_agent_prompt_mode.clone(),
                codex_profile: raw.codex_profile.clone(),
                subagent_mode: raw.subagent_mode.clone(),
                auto_turn_decision: raw.auto_turn_decision.clone(),
            }
        })
        .clone()
}

pub(super) fn subagent_backend_settings() -> SubagentBackendSettings {
    raw_agent_interaction_defaults().subagent_mode.codex.clone()
}

pub(super) fn claude_subagent_mode_settings() -> ClaudeSubagentModeSettings {
    raw_agent_interaction_defaults()
        .subagent_mode
        .claude
        .clone()
}

pub(super) fn subagent_mode_settings() -> SubagentModeSettings {
    raw_agent_interaction_defaults().subagent_mode.clone()
}

pub(super) fn codex_subagent_enabled() -> bool {
    raw_agent_interaction_defaults().subagent_mode.codex.enabled
}

pub(super) fn claude_subagent_enabled() -> bool {
    raw_agent_interaction_defaults()
        .subagent_mode
        .claude
        .enabled
}

pub(super) fn claude_forward_subagent_text() -> bool {
    raw_agent_interaction_defaults()
        .subagent_mode
        .claude
        .forward_subagent_text
}

pub(super) fn claude_agent_progress_summaries() -> bool {
    raw_agent_interaction_defaults()
        .subagent_mode
        .claude
        .agent_progress_summaries
}

pub(super) fn auto_turn_decision_settings() -> AutoTurnDecisionSettings {
    raw_agent_interaction_defaults().auto_turn_decision.clone()
}

pub(super) fn auto_turn_decision_enabled() -> bool {
    raw_agent_interaction_defaults().auto_turn_decision.enabled
}

pub(super) fn auto_turn_decision_allow_model_tier() -> bool {
    raw_agent_interaction_defaults()
        .auto_turn_decision
        .allow_model_tier
}

pub(super) fn auto_turn_decision_allow_reasoning_effort() -> bool {
    raw_agent_interaction_defaults()
        .auto_turn_decision
        .allow_reasoning_effort
}

pub(super) fn auto_turn_decision_allow_plan_mode() -> bool {
    raw_agent_interaction_defaults()
        .auto_turn_decision
        .allow_plan_mode
}

pub(super) fn auto_turn_decision_allow_goal_mode() -> bool {
    raw_agent_interaction_defaults()
        .auto_turn_decision
        .allow_goal_mode
}

pub(super) fn auto_turn_decision_allow_session_fork() -> bool {
    raw_agent_interaction_defaults()
        .auto_turn_decision
        .allow_session_fork
}

pub(super) fn permission_mode() -> String {
    DEFAULT_PERMISSION_MODE
        .get_or_init(|| raw_agent_interaction_defaults().permission_mode.clone())
        .clone()
}

pub(super) fn main_agent_prompt_mode() -> String {
    raw_agent_interaction_defaults()
        .main_agent_prompt_mode
        .clone()
}

pub(super) fn codex_profile_settings() -> CodexProfileSettings {
    raw_agent_interaction_defaults().codex_profile.clone()
}

pub(super) fn codex_profile_name() -> String {
    raw_agent_interaction_defaults()
        .codex_profile
        .profile
        .clone()
}
