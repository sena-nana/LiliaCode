use lilia_contracts::{LiliaAgentWorkflow, LiliaReviewTarget};

use super::{DesktopApplicationError, DesktopTurnRequest};

pub(crate) fn compile_turn_input(
    request: &mut DesktopTurnRequest,
) -> Result<(), DesktopApplicationError> {
    let Some(workflow) = request.workflow.as_ref() else {
        return Ok(());
    };
    if matches!(workflow, LiliaAgentWorkflow::LiliaCompact) {
        return Ok(());
    }
    let Some(instruction) = model_instruction(workflow) else {
        if request.content.trim().is_empty() {
            return Err(DesktopApplicationError::InvalidInput {
                field: "workflow",
                message: "此操作需要具体任务内容或对应的设置、会话操作入口。".into(),
            });
        }
        return Ok(());
    };
    if request.content.starts_with(&instruction) {
        return Ok(());
    }
    request.content = if request.content.trim().is_empty() {
        instruction
    } else {
        format!(
            "{instruction}\n\nAdditional user input:\n{}",
            request.content
        )
    };
    Ok(())
}

fn review_scope(target: &LiliaReviewTarget) -> String {
    match target {
        LiliaReviewTarget::UncommittedChanges => "the current uncommitted changes, including staged, unstaged, and relevant untracked files".into(),
        LiliaReviewTarget::BaseBranch { branch } => format!("the current branch changes relative to the merge base with base branch {}", quoted(branch)),
        LiliaReviewTarget::Commit { sha } => format!("the changes introduced by commit {} compared with its parent", quoted(sha)),
    }
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("a string is representable as JSON")
}

fn with_instructions(mut prompt: String, instructions: Option<&str>) -> String {
    if let Some(instructions) = instructions.filter(|value| !value.trim().is_empty()) {
        prompt.push_str("\n\nUser instructions:\n");
        prompt.push_str(instructions);
    }
    prompt
}

fn model_instruction(workflow: &LiliaAgentWorkflow) -> Option<String> {
    Some(match workflow {
        LiliaAgentWorkflow::LiliaReview {
            target,
            instructions,
            delivery,
        } => {
            let presentation = if delivery.as_deref() == Some("detached") {
                "Return a self-contained review report."
            } else {
                "Report the findings in this conversation."
            };
            with_instructions(
                format!(
                    "Review {}. Inspect the actual diff and enough surrounding code to verify each issue. Do not modify files. Prioritize actionable correctness, regression, security, and missing-test findings; include severity, file and line, trigger, and impact. If no actionable issues are found, say so and describe relevant validation limits. {presentation}",
                    review_scope(target)
                ),
                instructions.as_deref(),
            )
        }
        LiliaAgentWorkflow::LiliaFixSuggestion {
            target,
            instructions,
            mode,
        } => {
            let action = if mode.as_deref() == Some("apply") {
                "Apply focused fixes for verified issues, preserve unrelated user changes, and run relevant validation. Report the changes and validation results."
            } else {
                "Propose concrete fixes, explaining each verified issue, affected file and line, intended change, and validation steps. Do not modify files."
            };
            with_instructions(
                format!(
                    "Inspect {} and identify actionable defects. {action}",
                    review_scope(target)
                ),
                instructions.as_deref(),
            )
        }
        LiliaAgentWorkflow::LiliaBatchApply {
            source_turn_id,
            source_kind,
            source_summary,
            instructions,
        } => with_instructions(
            format!(
                "Apply the actionable fixes from the prior {} response in turn {}. Recheck every finding against the current code, preserve unrelated changes, implement only applicable fixes, and run relevant validation. Report applied and skipped findings with reasons.\n\nSource findings:\n{}",
                quoted(source_kind),
                quoted(source_turn_id),
                source_summary
            ),
            instructions.as_deref(),
        ),
        LiliaAgentWorkflow::LiliaTaskWorkflow { kind, instructions } => {
            let task = match kind.as_str() {
                "generalTask" => {
                    "Complete the requested implementation using the conversation and workspace context. Inspect relevant code, make focused changes, and validate the result."
                }
                "review" => {
                    "Review the relevant code and changes for actionable defects. Report severity, file and line, evidence, and impact without modifying files."
                }
                "bugLocalization" => {
                    "Investigate the reported failure, reproduce it where possible, trace its root cause, and report the evidence and a focused repair plan."
                }
                "frontend" => {
                    "Implement the requested interface and interaction changes using the project's established components. Verify responsive layout, keyboard interaction, and visible states."
                }
                "refactor" => {
                    "Refactor the relevant code to improve structure while preserving observable behavior. Respect repository boundaries and verify behavior with focused tests."
                }
                "testAndVerification" => {
                    "Verify the requested behavior, identify concrete coverage gaps, add meaningful regression tests where needed, and report executed checks and remaining risks."
                }
                "docsAndPrompt" => {
                    "Update the relevant documentation or prompts to match the intended behavior. Check examples and references against the implementation."
                }
                "gitAndRelease" => {
                    "Prepare the requested Git or release work, inspect the exact change scope and validation results, and follow the repository's authorization requirements for publishing."
                }
                "architectureAndMemory" => {
                    "Inspect the relevant architecture and existing project memory, update the requested architecture or memory records with evidence, and keep them consistent with the code."
                }
                _ => {
                    "Carry out the requested task using the specified workflow and the conversation context. If its scope is undefined, ask for the missing objective before making changes."
                }
            };
            with_instructions(
                format!(
                    "Task workflow {}. {task} If the conversation does not provide a concrete objective, ask a focused clarification question.",
                    quoted(kind)
                ),
                instructions.as_deref(),
            )
        }
        LiliaAgentWorkflow::LiliaCompact
        | LiliaAgentWorkflow::LiliaGoal { .. }
        | LiliaAgentWorkflow::LiliaBackgroundTerminalsClean
        | LiliaAgentWorkflow::LiliaMemoryMode { .. }
        | LiliaAgentWorkflow::LiliaMemoryReset
        | LiliaAgentWorkflow::LiliaConfigDiagnostics { .. }
        | LiliaAgentWorkflow::Automation { .. }
        | LiliaAgentWorkflow::SlashCommand { .. } => return None,
    })
}

pub(crate) fn title(workflow: &LiliaAgentWorkflow) -> String {
    match workflow {
        LiliaAgentWorkflow::LiliaReview { target, .. } => review_title("审查", target),
        LiliaAgentWorkflow::LiliaFixSuggestion { target, mode, .. } => review_title(
            if mode.as_deref() == Some("apply") {
                "修复"
            } else {
                "修复建议"
            },
            target,
        ),
        LiliaAgentWorkflow::LiliaBatchApply { .. } => "应用建议".into(),
        LiliaAgentWorkflow::LiliaTaskWorkflow { kind, .. } => match kind.as_str() {
            "generalTask" => "实现任务",
            "review" => "代码审查",
            "bugLocalization" => "问题定位",
            "frontend" => "前端与交互",
            "refactor" => "重构与结构调整",
            "testAndVerification" => "测试与验证",
            "docsAndPrompt" => "文档与提示词",
            "gitAndRelease" => "Git 与发布",
            "architectureAndMemory" => "架构与记忆",
            _ => "任务工作流",
        }
        .into(),
        LiliaAgentWorkflow::LiliaCompact => "压缩上下文".into(),
        LiliaAgentWorkflow::LiliaGoal { .. } => "更新目标".into(),
        LiliaAgentWorkflow::LiliaBackgroundTerminalsClean => "清理后台终端".into(),
        LiliaAgentWorkflow::LiliaMemoryMode { .. } => "调整记忆模式".into(),
        LiliaAgentWorkflow::LiliaMemoryReset => "重置记忆".into(),
        LiliaAgentWorkflow::LiliaConfigDiagnostics { .. } => "配置诊断".into(),
        LiliaAgentWorkflow::Automation { .. } => "自动化任务".into(),
        LiliaAgentWorkflow::SlashCommand { command_id, .. } => format!("命令 {command_id}"),
    }
}

fn review_title(action: &str, target: &LiliaReviewTarget) -> String {
    match target {
        LiliaReviewTarget::UncommittedChanges => format!("{action}未提交变更"),
        LiliaReviewTarget::BaseBranch { branch } => format!("{action}分支 {branch}"),
        LiliaReviewTarget::Commit { sha } => {
            format!("{action}提交 {}", sha.chars().take(8).collect::<String>())
        }
    }
}

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
