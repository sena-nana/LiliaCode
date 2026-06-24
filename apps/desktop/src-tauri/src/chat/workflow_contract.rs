use std::sync::OnceLock;

use serde::Deserialize;

const LILIA_WORKFLOW_CONTRACT_JSON: &str =
    include_str!("../../../../../packages/contracts/src/lilia-workflow-contract.json");
const RUNTIME_COMMAND_CONTRACT_JSON: &str =
    include_str!("../../../../../packages/contracts/src/runtime-command-contract.json");
const SESSION_MANAGEMENT_CONTRACT_JSON: &str =
    include_str!("../../../../../packages/contracts/src/session-management-contract.json");

static LILIA_WORKFLOW_CONTRACT: OnceLock<LiliaWorkflowContract> = OnceLock::new();
static RUNTIME_COMMAND_CONTRACT: OnceLock<RuntimeCommandContract> = OnceLock::new();
static SESSION_MANAGEMENT_CONTRACT: OnceLock<SessionManagementContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiliaWorkflowContract {
    task_workflow: WorkflowTypeEntry,
    review: WorkflowTypeEntry,
    fix_suggestion: WorkflowTypeEntry,
    batch_apply: WorkflowTypeEntry,
    goal: WorkflowTypeEntry,
    memory_mode: WorkflowTypeEntry,
    memory_reset: WorkflowTypeEntry,
    compact: WorkflowTypeEntry,
    background_terminals_clean: WorkflowTypeEntry,
    config_diagnostics: WorkflowTypeEntry,
    automation: WorkflowTypeEntry,
    slash_command: WorkflowTypeEntry,
}

#[derive(Debug, Deserialize)]
struct WorkflowTypeEntry {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCommandContract {
    runtime_settings: RuntimeCommandTypeEntry,
    remote_environment: RuntimeCommandTypeEntry,
    sandbox_diagnostics: RuntimeCommandTypeEntry,
    session_fork: RuntimeCommandTypeEntry,
    process_session: RuntimeCommandTypeEntry,
}

#[derive(Debug, Deserialize)]
struct RuntimeCommandTypeEntry {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionManagementContract {
    runtime_command_type: String,
}

fn lilia_workflow_contract() -> &'static LiliaWorkflowContract {
    LILIA_WORKFLOW_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            LILIA_WORKFLOW_CONTRACT_JSON,
            "lilia-workflow-contract.json",
        )
    })
}

fn runtime_command_contract() -> &'static RuntimeCommandContract {
    RUNTIME_COMMAND_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            RUNTIME_COMMAND_CONTRACT_JSON,
            "runtime-command-contract.json",
        )
    })
}

fn session_management_contract() -> &'static SessionManagementContract {
    SESSION_MANAGEMENT_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            SESSION_MANAGEMENT_CONTRACT_JSON,
            "session-management-contract.json",
        )
    })
}

pub(super) fn review_workflow_type() -> &'static str {
    &lilia_workflow_contract().review.kind
}

pub(super) fn task_workflow_type() -> &'static str {
    &lilia_workflow_contract().task_workflow.kind
}

pub(super) fn fix_suggestion_workflow_type() -> &'static str {
    &lilia_workflow_contract().fix_suggestion.kind
}

pub(super) fn batch_apply_workflow_type() -> &'static str {
    &lilia_workflow_contract().batch_apply.kind
}

pub(super) fn goal_workflow_type() -> &'static str {
    &lilia_workflow_contract().goal.kind
}

pub(super) fn memory_mode_workflow_type() -> &'static str {
    &lilia_workflow_contract().memory_mode.kind
}

pub(super) fn memory_reset_workflow_type() -> &'static str {
    &lilia_workflow_contract().memory_reset.kind
}

pub(super) fn compact_workflow_type() -> &'static str {
    &lilia_workflow_contract().compact.kind
}

pub(super) fn background_terminals_clean_workflow_type() -> &'static str {
    &lilia_workflow_contract().background_terminals_clean.kind
}

pub(super) fn config_diagnostics_workflow_type() -> &'static str {
    &lilia_workflow_contract().config_diagnostics.kind
}

pub(super) fn automation_workflow_type() -> &'static str {
    &lilia_workflow_contract().automation.kind
}

pub(super) fn slash_command_workflow_type() -> &'static str {
    &lilia_workflow_contract().slash_command.kind
}

pub(super) fn session_fork_runtime_command_type() -> &'static str {
    &runtime_command_contract().session_fork.kind
}

pub(super) fn session_management_runtime_command_type() -> &'static str {
    &session_management_contract().runtime_command_type
}

pub(super) fn runtime_settings_command_type() -> &'static str {
    &runtime_command_contract().runtime_settings.kind
}

pub(super) fn remote_environment_command_type() -> &'static str {
    &runtime_command_contract().remote_environment.kind
}

pub(super) fn sandbox_diagnostics_command_type() -> &'static str {
    &runtime_command_contract().sandbox_diagnostics.kind
}

pub(super) fn process_session_command_type() -> &'static str {
    &runtime_command_contract().process_session.kind
}
