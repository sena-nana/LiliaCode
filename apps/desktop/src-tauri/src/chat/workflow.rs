use crate::chat::types::ChatWorkflow;

pub(crate) fn workflow_kind(workflow: Option<&ChatWorkflow>) -> Option<&'static str> {
    match workflow? {
        ChatWorkflow::LiliaReview { .. } => Some("lilia_review"),
        ChatWorkflow::LiliaFixSuggestion { .. } => Some("lilia_fix_suggestion"),
        ChatWorkflow::LiliaBatchApply { .. } => Some("lilia_batch_apply"),
        ChatWorkflow::LiliaGoal { .. } => Some("lilia_goal"),
        ChatWorkflow::LiliaCompact => Some("lilia_compact"),
        ChatWorkflow::LiliaBackgroundTerminalsClean => Some("lilia_background_terminals_clean"),
        ChatWorkflow::LiliaMemoryMode { .. } => Some("lilia_memory_mode"),
        ChatWorkflow::LiliaMemoryReset => Some("lilia_memory_reset"),
        ChatWorkflow::LiliaSessionFork { .. } => Some("lilia_session_fork"),
        ChatWorkflow::LiliaConfigDiagnostics { .. } => Some("lilia_config_diagnostics"),
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
        "lilia_review" => Some("lilia_review"),
        "lilia_fix_suggestion" => Some("lilia_fix_suggestion"),
        "lilia_batch_apply" => Some("lilia_batch_apply"),
        "lilia_goal" => Some("lilia_goal"),
        "lilia_compact" => Some("lilia_compact"),
        "lilia_background_terminals_clean" => Some("lilia_background_terminals_clean"),
        "lilia_memory_mode" => Some("lilia_memory_mode"),
        "lilia_memory_reset" => Some("lilia_memory_reset"),
        "lilia_session_fork" => Some("lilia_session_fork"),
        "lilia_config_diagnostics" => Some("lilia_config_diagnostics"),
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
        assert_eq!(
            workflow_kind(Some(&ChatWorkflow::LiliaSessionFork { exclude_turns: None })),
            Some("lilia_session_fork")
        );
        assert_eq!(
            parse_workflow_kind("lilia_session_fork"),
            Some("lilia_session_fork")
        );
        assert_eq!(workflow_kind(None), None);
    }
}
