use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::chat::types::{ChatRuntimeCommand, ChatWorkflow};

const LILIA_AGENT_PROTOCOL_MANIFEST: &str =
    include_str!("../../../../../packages/contracts/src/lilia-agent-protocol.json");

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiliaProtocolManifest {
    workflow: Vec<LiliaProtocolManifestEntry>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiliaProtocolManifestEntry {
    kind: String,
}

#[derive(Default)]
struct LiliaProtocolManifestIndex {
    workflow: BTreeSet<String>,
}

fn protocol_manifest() -> &'static LiliaProtocolManifestIndex {
    static MANIFEST: OnceLock<LiliaProtocolManifestIndex> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let manifest: LiliaProtocolManifest = serde_json::from_str(LILIA_AGENT_PROTOCOL_MANIFEST)
            .expect("lilia-agent-protocol.json must be valid");
        LiliaProtocolManifestIndex {
            workflow: manifest
                .workflow
                .into_iter()
                .map(|entry| entry.kind)
                .collect(),
        }
    })
}

pub(crate) fn workflow_kind(workflow: Option<&ChatWorkflow>) -> Option<String> {
    let kind = match workflow? {
        ChatWorkflow::LiliaReview { .. } => "lilia_review",
        ChatWorkflow::LiliaFixSuggestion { .. } => "lilia_fix_suggestion",
        ChatWorkflow::LiliaBatchApply { .. } => "lilia_batch_apply",
        ChatWorkflow::LiliaGoal { .. } => "lilia_goal",
        ChatWorkflow::LiliaCompact => "lilia_compact",
        ChatWorkflow::LiliaBackgroundTerminalsClean => "lilia_background_terminals_clean",
        ChatWorkflow::LiliaMemoryMode { .. } => "lilia_memory_mode",
        ChatWorkflow::LiliaMemoryReset => "lilia_memory_reset",
        ChatWorkflow::LiliaConfigDiagnostics { .. } => "lilia_config_diagnostics",
        ChatWorkflow::Automation { .. } => "automation",
        ChatWorkflow::SlashCommand { .. } => "slash_command",
    };
    Some(kind.to_string())
}

pub(crate) fn runtime_command_kind(command: Option<&ChatRuntimeCommand>) -> Option<String> {
    let kind = match command? {
        ChatRuntimeCommand::LiliaSessionFork { .. } => "lilia_session_fork",
        ChatRuntimeCommand::LiliaSessionManagement { .. } => "lilia_session_management",
        ChatRuntimeCommand::LiliaProviderSettings { .. } => "lilia_provider_settings",
    };
    Some(kind.to_string())
}

pub(crate) fn automation_run_id(workflow: Option<&ChatWorkflow>) -> Option<String> {
    match workflow {
        Some(ChatWorkflow::Automation { automation_run_id }) => Some(automation_run_id.clone()),
        _ => None,
    }
}

pub(crate) fn parse_workflow_kind(value: &str) -> Option<String> {
    protocol_manifest()
        .workflow
        .contains(value)
        .then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TestLiliaProtocolManifest {
        workflow: Vec<LiliaProtocolManifestEntry>,
        runtime_command: Vec<LiliaProtocolManifestEntry>,
    }

    #[test]
    fn workflow_kind_names_cover_automation() {
        let workflow = ChatWorkflow::Automation {
            automation_run_id: "run-1".to_string(),
        };

        assert_eq!(
            workflow_kind(Some(&workflow)).as_deref(),
            Some("automation")
        );
        assert_eq!(automation_run_id(Some(&workflow)).as_deref(), Some("run-1"));
        assert_eq!(
            parse_workflow_kind("automation").as_deref(),
            Some("automation")
        );
        assert_eq!(parse_workflow_kind("lilia_session_fork"), None);
        assert_eq!(parse_workflow_kind("lilia_session_management"), None);
        assert_eq!(parse_workflow_kind("lilia_provider_settings"), None);
        assert_eq!(workflow_kind(None), None);
    }

    #[test]
    fn typed_kinds_are_declared_in_protocol_manifest() {
        let workflows = [
            ChatWorkflow::LiliaReview {
                target: crate::chat::types::LiliaReviewTarget::UncommittedChanges,
                instructions: None,
                delivery: None,
            },
            ChatWorkflow::LiliaFixSuggestion {
                target: crate::chat::types::LiliaReviewTarget::UncommittedChanges,
                instructions: None,
                mode: None,
            },
            ChatWorkflow::LiliaBatchApply {
                source_turn_id: "turn-1".to_string(),
                source_kind: "fix_suggestion".to_string(),
                source_summary: "summary".to_string(),
                instructions: None,
            },
            ChatWorkflow::LiliaGoal {
                action: "start".to_string(),
                objective: None,
                status: None,
                token_budget: None,
            },
            ChatWorkflow::LiliaCompact,
            ChatWorkflow::LiliaBackgroundTerminalsClean,
            ChatWorkflow::LiliaMemoryMode {
                mode: "enabled".to_string(),
            },
            ChatWorkflow::LiliaMemoryReset,
            ChatWorkflow::LiliaConfigDiagnostics {
                include_layers: None,
            },
            ChatWorkflow::Automation {
                automation_run_id: "run-1".to_string(),
            },
            ChatWorkflow::SlashCommand {
                command_id: "native:help".to_string(),
                source: "native".to_string(),
                arguments: std::collections::BTreeMap::new(),
            },
        ];
        let runtime_commands = [
            ChatRuntimeCommand::LiliaSessionFork {
                exclude_turns: None,
            },
            ChatRuntimeCommand::LiliaSessionManagement {
                action: "rename".to_string(),
                session_id: None,
                title: None,
                tag: None,
                archived: None,
                limit: None,
                cursor: None,
                search_term: None,
                include_system_messages: None,
            },
            ChatRuntimeCommand::LiliaProviderSettings {
                action: "update".to_string(),
            },
        ];
        let manifest: TestLiliaProtocolManifest =
            serde_json::from_str(LILIA_AGENT_PROTOCOL_MANIFEST).unwrap();
        let workflow_kinds: BTreeSet<_> = manifest
            .workflow
            .into_iter()
            .map(|entry| entry.kind)
            .collect();
        let runtime_command_kinds: BTreeSet<_> = manifest
            .runtime_command
            .into_iter()
            .map(|entry| entry.kind)
            .collect();

        for workflow in &workflows {
            let kind = workflow_kind(Some(workflow)).unwrap();
            assert!(
                workflow_kinds.contains(&kind),
                "{kind} workflow must be declared in lilia-agent-protocol.json"
            );
        }
        for command in &runtime_commands {
            let kind = runtime_command_kind(Some(command)).unwrap();
            assert!(
                runtime_command_kinds.contains(&kind),
                "{kind} runtime command must be declared in lilia-agent-protocol.json"
            );
        }
        assert_eq!(workflow_kinds.len(), workflows.len());
        assert_eq!(runtime_command_kinds.len(), runtime_commands.len());
    }
}
