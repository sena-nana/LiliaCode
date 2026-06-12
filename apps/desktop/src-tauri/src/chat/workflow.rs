use crate::chat::types::ChatWorkflow;

pub(crate) fn workflow_kind(workflow: Option<&ChatWorkflow>) -> Option<&'static str> {
    match workflow? {
        ChatWorkflow::CodexReview { .. } => Some("codex_review"),
        ChatWorkflow::CodexFixSuggestion { .. } => Some("codex_fix_suggestion"),
        ChatWorkflow::CodexBatchApply { .. } => Some("codex_batch_apply"),
        ChatWorkflow::CodexGoal { .. } => Some("codex_goal"),
        ChatWorkflow::CodexCompact => Some("codex_compact"),
        ChatWorkflow::CodexBackgroundTerminalsClean => Some("codex_background_terminals_clean"),
        ChatWorkflow::CodexMemoryMode { .. } => Some("codex_memory_mode"),
        ChatWorkflow::CodexMemoryReset => Some("codex_memory_reset"),
        ChatWorkflow::CodexThreadFork { .. } => Some("codex_thread_fork"),
        ChatWorkflow::CodexConfigDiagnostics { .. } => Some("codex_config_diagnostics"),
        ChatWorkflow::Automation { .. } => Some("automation"),
        ChatWorkflow::SlashCommand { .. } => Some("slash_command"),
    }
}

pub(crate) fn automation_run_id(workflow: Option<&ChatWorkflow>) -> Option<String> {
    match workflow {
        Some(ChatWorkflow::Automation { automation_run_id }) => Some(automation_run_id.clone()),
        _ => None,
    }
}

pub(crate) fn parse_workflow_kind(value: &str) -> Option<&'static str> {
    match value {
        "codex_review" => Some("codex_review"),
        "codex_fix_suggestion" => Some("codex_fix_suggestion"),
        "codex_batch_apply" => Some("codex_batch_apply"),
        "codex_goal" => Some("codex_goal"),
        "codex_compact" => Some("codex_compact"),
        "codex_background_terminals_clean" => Some("codex_background_terminals_clean"),
        "codex_memory_mode" => Some("codex_memory_mode"),
        "codex_memory_reset" => Some("codex_memory_reset"),
        "codex_thread_fork" => Some("codex_thread_fork"),
        "codex_config_diagnostics" => Some("codex_config_diagnostics"),
        "automation" => Some("automation"),
        "slash_command" => Some("slash_command"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_kind_names_cover_automation() {
        let workflow = ChatWorkflow::Automation {
            automation_run_id: "run-1".to_string(),
        };

        assert_eq!(workflow_kind(Some(&workflow)), Some("automation"));
        assert_eq!(automation_run_id(Some(&workflow)).as_deref(), Some("run-1"));
        assert_eq!(parse_workflow_kind("automation"), Some("automation"));
        assert_eq!(workflow_kind(None), None);
    }
}
