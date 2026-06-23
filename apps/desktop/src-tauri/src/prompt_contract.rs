use std::sync::OnceLock;

use serde::Deserialize;

const PROMPT_TEXT_JSON: &str = include_str!("../../../../packages/contracts/src/prompt-text.json");

static PROMPT_CONTRACT: OnceLock<PromptContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptContract {
    main_agent: MainAgentPrompts,
    codex: CodexPrompts,
    assistant: AssistantPrompts,
    suggestion: SuggestionPrompts,
    title: TitlePrompts,
    automation: AutomationPrompts,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainAgentPrompts {
    pub(crate) base_prompt: String,
    pub(crate) tools_prompt: String,
    pub(crate) skills: std::collections::BTreeMap<String, MainAgentSkillPrompt>,
    pub(crate) modes: MainAgentModePrompts,
    pub(crate) skill_order: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainAgentSkillPrompt {
    pub(crate) title: String,
    pub(crate) prompt: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainAgentModePrompts {
    pub(crate) conservative: String,
    pub(crate) aggressive: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexPrompts {
    subagents: CodexSubagentPrompts,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexSubagentPrompts {
    pub(crate) delegation_header: String,
    pub(crate) baseline_intro: String,
    pub(crate) baseline_prompt: String,
    pub(crate) baseline_label: String,
    pub(crate) available_label: String,
    pub(crate) agent_specific_instruction_label: String,
    pub(crate) agent_role_label: String,
    pub(crate) delegation_rules_label: String,
    pub(crate) delegation_rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantPrompts {
    prompt_router: PromptRouterPrompts,
    prompt_optimize: PromptOptimizePrompts,
    auto_turn_decision: AutoTurnDecisionPrompts,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptRouterPrompts {
    system_instruction: String,
    request_instruction: String,
    requirements: Vec<String>,
    scenarios: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptOptimizePrompts {
    system_instruction: String,
    request_instruction: String,
    requirements: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutoTurnDecisionPrompts {
    system_instruction: String,
    request_instruction: String,
    tier_policy: AutoTurnDecisionTierPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AutoTurnDecisionTierPolicy {
    pub(crate) light: String,
    pub(crate) normal: String,
    pub(crate) deep: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuggestionPrompts {
    system_instruction: String,
    generation_rules: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TitlePrompts {
    system_instruction: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutomationPrompts {
    default_agent_prompt: String,
    default_human_prompt: String,
}

fn prompt_contract() -> &'static PromptContract {
    PROMPT_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(PROMPT_TEXT_JSON, "prompt-text.json")
    })
}

pub(crate) fn prompt_optimize_system_instruction() -> &'static str {
    &prompt_contract()
        .assistant
        .prompt_optimize
        .system_instruction
}

pub(crate) fn main_agent_prompts() -> &'static MainAgentPrompts {
    &prompt_contract().main_agent
}

pub(crate) fn main_agent_prompt_mode(mode: &str) -> &'static str {
    match mode {
        "aggressive" => &prompt_contract().main_agent.modes.aggressive,
        _ => &prompt_contract().main_agent.modes.conservative,
    }
}

pub(crate) fn build_main_agent_prompt(mode: &str) -> String {
    let prompts = main_agent_prompts();
    let mut parts = vec![
        prompts.base_prompt.trim().to_string(),
        main_agent_prompt_mode(mode).trim().to_string(),
        prompts.tools_prompt.trim().to_string(),
    ];
    let skills = prompts
        .skill_order
        .iter()
        .filter_map(|key| prompts.skills.get(key))
        .map(|skill| format!("## {}\n{}", skill.title.trim(), skill.prompt.trim()))
        .collect::<Vec<_>>()
        .join("\n\n");
    if !skills.trim().is_empty() {
        parts.push(skills);
    }
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn prompt_optimize_request_instruction() -> &'static str {
    &prompt_contract()
        .assistant
        .prompt_optimize
        .request_instruction
}

pub(crate) fn prompt_optimize_requirements() -> &'static [String] {
    &prompt_contract().assistant.prompt_optimize.requirements
}

pub(crate) fn prompt_router_system_instruction() -> &'static str {
    &prompt_contract().assistant.prompt_router.system_instruction
}

pub(crate) fn prompt_router_request_instruction() -> &'static str {
    &prompt_contract()
        .assistant
        .prompt_router
        .request_instruction
}

pub(crate) fn prompt_router_requirements() -> &'static [String] {
    &prompt_contract().assistant.prompt_router.requirements
}

pub(crate) fn prompt_router_scenarios() -> &'static [String] {
    &prompt_contract().assistant.prompt_router.scenarios
}

pub(crate) fn auto_turn_decision_system_instruction() -> &'static str {
    &prompt_contract()
        .assistant
        .auto_turn_decision
        .system_instruction
}

pub(crate) fn auto_turn_decision_request_instruction() -> &'static str {
    &prompt_contract()
        .assistant
        .auto_turn_decision
        .request_instruction
}

pub(crate) fn auto_turn_decision_tier_policy() -> &'static AutoTurnDecisionTierPolicy {
    &prompt_contract().assistant.auto_turn_decision.tier_policy
}

pub(crate) fn codex_subagent_prompts() -> &'static CodexSubagentPrompts {
    &prompt_contract().codex.subagents
}

pub(crate) fn suggestion_system_instruction() -> &'static str {
    &prompt_contract().suggestion.system_instruction
}

pub(crate) fn suggestion_generation_rules() -> &'static [String] {
    &prompt_contract().suggestion.generation_rules
}

pub(crate) fn title_system_instruction() -> &'static str {
    &prompt_contract().title.system_instruction
}

pub(crate) fn default_automation_agent_prompt() -> &'static str {
    &prompt_contract().automation.default_agent_prompt
}

pub(crate) fn default_automation_human_prompt() -> &'static str {
    &prompt_contract().automation.default_human_prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contract_loads_shared_defaults() {
        assert!(!default_automation_agent_prompt().trim().is_empty());
        assert!(!default_automation_human_prompt().trim().is_empty());
        let main_agent = main_agent_prompts();
        assert!(!main_agent.base_prompt.trim().is_empty());
        assert!(!main_agent.tools_prompt.trim().is_empty());
        assert!(!main_agent.skills.is_empty());
        assert!(!main_agent.skill_order.is_empty());
        assert!(!main_agent_prompt_mode("conservative").trim().is_empty());
        assert!(!main_agent_prompt_mode("aggressive").trim().is_empty());
        assert_eq!(
            main_agent.skill_order.len(),
            main_agent.skills.len(),
            "mainAgent.skillOrder must list every configured skill"
        );
        let built_main_agent_prompt = build_main_agent_prompt("aggressive");
        assert!(built_main_agent_prompt.len() > main_agent.base_prompt.len());
        for key in main_agent.skills.keys() {
            assert!(main_agent.skill_order.contains(key));
        }
        for key in &main_agent.skill_order {
            let skill = main_agent
                .skills
                .get(key)
                .unwrap_or_else(|| panic!("mainAgent.skillOrder references missing skill: {key}"));
            assert!(
                !skill.title.trim().is_empty(),
                "mainAgent skill {key} must have a title"
            );
            assert!(
                !skill.prompt.trim().is_empty(),
                "mainAgent skill {key} must have a prompt"
            );
            assert!(
                built_main_agent_prompt.contains(&format!("## {}", skill.title.trim())),
                "mainAgent prompt must include skill title: {}",
                skill.title
            );
        }
        assert!(!title_system_instruction().trim().is_empty());
        assert!(!prompt_router_system_instruction().trim().is_empty());
        assert!(!prompt_router_request_instruction().trim().is_empty());
        assert!(!prompt_router_requirements().is_empty());
        assert!(!prompt_router_scenarios().is_empty());
        assert!(!prompt_optimize_system_instruction().trim().is_empty());
        assert!(!prompt_optimize_request_instruction().trim().is_empty());
        assert!(!prompt_optimize_requirements().is_empty());
        assert!(!auto_turn_decision_system_instruction().trim().is_empty());
        assert!(!auto_turn_decision_request_instruction().trim().is_empty());
        assert!(!suggestion_system_instruction().trim().is_empty());
        assert!(!suggestion_generation_rules().is_empty());

        let subagent_prompts = codex_subagent_prompts();
        assert!(!subagent_prompts.baseline_prompt.trim().is_empty());
        assert!(!subagent_prompts
            .agent_specific_instruction_label
            .trim()
            .is_empty());
        assert!(!subagent_prompts.delegation_rules.is_empty());
    }
}
