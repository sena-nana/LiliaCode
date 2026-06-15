use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use uuid::Uuid;

use super::types::{
    now_millis, SuggestionGitHubActivityRef, SuggestionItem, SuggestionItemSource, SuggestionScope,
    MAX_SUGGESTIONS, PROMPT_LIMIT, REASON_LIMIT, SAMPLE_TEXT_LIMIT, SUMMARY_LIMIT,
};

pub(super) fn build_generation_prompt(scope: &SuggestionScope) -> String {
    let mut lines = vec![
        "你是 LiliaCode 的新对话建议助手。只能基于下方任务未完成信号、GitHub 近期活动或本地 Git 上下文提出继续处理建议。".to_string(),
        "只返回 JSON 数组，可返回 []，最多 3 项。每项字段必须是 taskIds、githubActivityIds、localGitContextIds、summary、reason、prompt。不要 markdown。".to_string(),
        "taskIds 必须引用下方任务 id；githubActivityIds 必须引用下方 GitHub 活动 id；localGitContextIds 必须引用下方本地 Git 上下文 id。每项至少引用一个有效 taskId、githubActivityId 或 localGitContextId。".to_string(),
        "summary 控制在 20 个中文字左右；reason 控制在 80 个中文字左右；prompt 是可直接填入对话框的中文提示词，控制在 300 个中文字左右。".to_string(),
        "不要提出泛化建议、体验优化、新方向、代码审查或测试补齐，除非它们被未完成信号、具体 GitHub 活动或本地 Git 状态明确指向。没有明确可继续处理的信号时返回 []。".to_string(),
        "基于 GitHub 活动的建议必须引用具体 PR、Issue 或 Push，并让 prompt 包含仓库、编号/分支或 commit 摘要等具体上下文。".to_string(),
        "基于本地 Git 的建议必须引用当前 branch，并让 prompt 包含最近提交、变更文件或未提交状态等具体上下文。".to_string(),
        format!(
            "scopeProjectId: {}",
            scope.project_id.as_deref().unwrap_or("recent-projects")
        ),
    ];
    if let Some(name) = &scope.project_name {
        lines.push(format!(
            "projectName: {}",
            truncate_chars(&compact_line(name), 80)
        ));
    }
    for task in &scope.tasks {
        lines.push(format!(
            "\n任务 {} | 标题: {} | 状态: {}",
            task.id,
            truncate_chars(&compact_line(&task.title), 80),
            task.status
        ));
        for text in &task.user_messages {
            lines.push(format!("用户: {}", truncate_chars(text, SAMPLE_TEXT_LIMIT)));
        }
        if let Some(text) = &task.assistant_message {
            lines.push(format!(
                "最近回复: {}",
                truncate_chars(text, SAMPLE_TEXT_LIMIT)
            ));
        }
        for signal in &task.unfinished_signals {
            lines.push(format!(
                "未完成信号: {}",
                truncate_chars(signal, SAMPLE_TEXT_LIMIT)
            ));
        }
    }
    if let Some(repo) = &scope.github_repo {
        lines.push(format!("\nGitHub 仓库: {}", repo.full_name));
    }
    for activity in &scope.github_activities {
        lines.push(format!(
            "GitHub 活动 {} | 类型: {} | action: {} | 标题: {}",
            activity.id,
            activity.kind,
            activity.action,
            truncate_chars(&compact_line(&activity.title), SAMPLE_TEXT_LIMIT)
        ));
        if let Some(url) = &activity.url {
            lines.push(format!("链接: {url}"));
        }
        for detail in &activity.details {
            lines.push(format!(
                "活动细节: {}",
                truncate_chars(&compact_line(detail), SAMPLE_TEXT_LIMIT)
            ));
        }
    }
    for context in &scope.local_git_contexts {
        lines.push(format!(
            "\n本地 Git 上下文 {} | branch: {} | status: {}",
            context.context.id,
            truncate_chars(&compact_line(&context.context.branch), SAMPLE_TEXT_LIMIT),
            truncate_chars(&compact_line(&context.context.status), SAMPLE_TEXT_LIMIT)
        ));
        for commit in &context.context.recent_commits {
            lines.push(format!(
                "最近提交: {}",
                truncate_chars(&compact_line(commit), SAMPLE_TEXT_LIMIT)
            ));
        }
        for file in &context.context.changed_files {
            lines.push(format!(
                "变更文件: {}",
                truncate_chars(&compact_line(file), SAMPLE_TEXT_LIMIT)
            ));
        }
    }
    lines.join("\n")
}

#[derive(Debug, Deserialize)]
pub(super) struct RawSuggestion {
    #[serde(default, rename = "taskIds")]
    pub(super) task_ids: Vec<String>,
    #[serde(default, rename = "githubActivityIds")]
    pub(super) github_activity_ids: Vec<String>,
    #[serde(default, rename = "localGitContextIds")]
    pub(super) local_git_context_ids: Vec<String>,
    pub(super) summary: Option<String>,
    pub(super) reason: Option<String>,
    pub(super) prompt: Option<String>,
}

pub(super) fn parse_model_suggestions(text: String) -> Result<Vec<RawSuggestion>, String> {
    let trimmed = text.trim();
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };
    serde_json::from_str::<Vec<RawSuggestion>>(json_text)
        .map_err(|e| format!("建议 JSON 解析失败：{e}"))
}

pub(super) fn materialize_items(
    raw: Vec<RawSuggestion>,
    scope: &SuggestionScope,
) -> Vec<SuggestionItem> {
    let generated_at = now_millis();
    let valid_task_ids: HashSet<String> = scope.tasks.iter().map(|task| task.id.clone()).collect();
    let activity_by_id = scope
        .github_activities
        .iter()
        .map(|activity| (activity.id.clone(), activity))
        .collect::<HashMap<_, _>>();
    let local_git_context_by_id = scope
        .local_git_contexts
        .iter()
        .map(|context| (context.context.id.clone(), context))
        .collect::<HashMap<_, _>>();
    raw.into_iter()
        .filter_map(|item| {
            let task_ids = item
                .task_ids
                .into_iter()
                .filter(|task_id| valid_task_ids.contains(task_id))
                .collect::<Vec<_>>();
            let github_activities = item
                .github_activity_ids
                .into_iter()
                .filter_map(|activity_id| activity_by_id.get(&activity_id).copied())
                .map(|activity| SuggestionGitHubActivityRef {
                    id: activity.id.clone(),
                    repo_full_name: activity.repo_full_name.clone(),
                    kind: activity.kind.clone(),
                    title: activity.title.clone(),
                    url: activity.url.clone(),
                })
                .collect::<Vec<_>>();
            let local_git_contexts = item
                .local_git_context_ids
                .into_iter()
                .filter_map(|context_id| local_git_context_by_id.get(&context_id).copied())
                .map(|context| context.context.clone())
                .collect::<Vec<_>>();
            if task_ids.is_empty() && github_activities.is_empty() && local_git_contexts.is_empty()
            {
                return None;
            }
            let summary = truncate_chars(&compact_line(&item.summary?), SUMMARY_LIMIT);
            let reason = truncate_chars(&compact_line(&item.reason?), REASON_LIMIT);
            let prompt = truncate_chars(item.prompt?.trim(), PROMPT_LIMIT);
            if summary.is_empty() || reason.is_empty() || prompt.is_empty() {
                return None;
            }
            Some(SuggestionItem {
                id: format!("sg-{}", Uuid::new_v4()),
                project_id: scope.project_id.clone(),
                source: if task_ids.is_empty() {
                    if github_activities.is_empty() {
                        SuggestionItemSource::LocalGit
                    } else {
                        SuggestionItemSource::Github
                    }
                } else {
                    SuggestionItemSource::Task
                },
                task_ids,
                github_activities,
                local_git_contexts,
                summary,
                reason,
                prompt,
                generated_at,
            })
        })
        .take(MAX_SUGGESTIONS)
        .collect()
}

pub(super) fn compact_line(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn truncate_chars(input: &str, max: usize) -> String {
    let mut out = String::new();
    for (index, ch) in input.chars().enumerate() {
        if index >= max {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
}
