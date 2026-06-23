use std::sync::OnceLock;

use serde::Deserialize;

const PROMPT_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/prompt-contract.json");

static PROMPT_CONTRACT: OnceLock<PromptContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptContract {
    assistant: AssistantPrompts,
    suggestion: SuggestionPrompts,
    title: TitlePrompts,
    automation: AutomationPrompts,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantPrompts {
    prompt_optimize: PromptOptimizePrompts,
    auto_turn_decision: AutoTurnDecisionPrompts,
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
        crate::contract_manifest::parse_contract_json(PROMPT_CONTRACT_JSON, "prompt-contract.json")
    })
}

pub(crate) fn prompt_optimize_system_instruction() -> &'static str {
    &prompt_contract()
        .assistant
        .prompt_optimize
        .system_instruction
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
        assert_eq!(
            default_automation_agent_prompt(),
            "请根据当前上下文继续推进。"
        );
        assert_eq!(title_system_instruction(), "只输出一个中文短标题。");
        assert!(prompt_optimize_requirements()
            .iter()
            .any(|item| item.contains("明确范围")));
    }
}
