use crate::chat::types::{ChatRuntimeCommand, ChatWorkflow};

#[cfg(test)]
const LILIA_AGENT_PROTOCOL_MANIFEST: &str =
    include_str!("../../../../../packages/contracts/src/lilia-agent-protocol.json");

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
        ChatRuntimeCommand::SessionFork { .. } => "session_fork",
        ChatRuntimeCommand::SessionManagement { .. } => "session_management",
        ChatRuntimeCommand::RuntimeSettings { .. } => "runtime_settings",
        ChatRuntimeCommand::RemoteEnvironment { .. } => "remote_environment",
        ChatRuntimeCommand::SandboxDiagnostics { .. } => "sandbox_diagnostics",
        ChatRuntimeCommand::ProcessSession { .. } => "process_session",
    };
    Some(kind.to_string())
}

pub(crate) fn automation_run_id(workflow: Option<&ChatWorkflow>) -> Option<String> {
    match workflow {
        Some(ChatWorkflow::Automation { automation_run_id }) => Some(automation_run_id.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TestLiliaProtocolManifest {
        workflow: Vec<LiliaProtocolManifestEntry>,
        runtime_command: Vec<LiliaProtocolManifestEntry>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct LiliaProtocolManifestEntry {
        kind: String,
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
            ChatRuntimeCommand::SessionFork {
                exclude_turns: None,
                source_turn_id: None,
                mode: None,
            },
            ChatRuntimeCommand::SessionManagement {
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
            ChatRuntimeCommand::RuntimeSettings {
                action: "update".to_string(),
            },
            ChatRuntimeCommand::RemoteEnvironment {
                action: "diagnose".to_string(),
                environment_id: None,
                environment: None,
            },
            ChatRuntimeCommand::SandboxDiagnostics {
                include_details: Some(true),
            },
            ChatRuntimeCommand::ProcessSession {
                action: "spawn".to_string(),
                process_id: None,
                command: Some("npm test".to_string()),
                cwd: None,
                stdin: None,
                rows: None,
                cols: None,
                env: None,
                tty: Some(true),
                permission_profile: None,
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
