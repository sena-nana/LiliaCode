use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use tauri::{AppHandle, Manager, Runtime};

use crate::settings_store::{load_store_value, save_store_value};
use crate::store::LiliaStore;

use super::generation::{compact_line, truncate_chars};
use super::types::{
    now_millis, SuggestionItem, SuggestionItemSource, SuggestionSettings, SuggestionSource,
    CACHE_TTL_MS, CLAUDE_NATIVE_CACHE_KEY, PROMPT_LIMIT, SUMMARY_LIMIT,
};

pub(super) type ClaudeNativeSuggestionCache = HashMap<String, SuggestionItem>;

pub(crate) fn save_claude_prompt_suggestion<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    suggestion: &str,
    uuid: Option<&str>,
) -> Result<(), String> {
    let prompt = truncate_chars(suggestion.trim(), PROMPT_LIMIT);
    if prompt.is_empty() {
        return Ok(());
    }
    let Some(store) = app.try_state::<LiliaStore>() else {
        return Ok(());
    };
    let conn = store.conn()?;
    let project_id = conn
        .query_row(
            "SELECT project_id FROM tasks WHERE id = ?1 AND archived = 0",
            params![task_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| format!("conversation_suggestions: query Claude suggestion task 失败：{e}"))?
        .flatten();
    let Some(project_id) = project_id else {
        return Ok(());
    };

    let item = SuggestionItem {
        id: format!(
            "claude-{}",
            uuid.filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| task_id)
        ),
        project_id: Some(project_id.clone()),
        task_ids: vec![task_id.to_string()],
        source: SuggestionItemSource::Claude,
        github_activities: Vec::new(),
        local_git_contexts: Vec::new(),
        summary: summarize_claude_prompt_suggestion(&prompt),
        reason: "Claude 根据上一轮对话预测的下一条提示。".to_string(),
        prompt,
        generated_at: now_millis(),
    };

    let mut cache: ClaudeNativeSuggestionCache =
        load_store_value(app, CLAUDE_NATIVE_CACHE_KEY).unwrap_or_default();
    cache.insert(project_id, item);
    save_store_value(app, CLAUDE_NATIVE_CACHE_KEY, &cache)
}
pub(super) fn load_claude_native_suggestions<R: Runtime>(
    app: &AppHandle<R>,
    project_id: Option<&str>,
) -> Option<Vec<SuggestionItem>> {
    let project_id = project_id?;
    let cache: ClaudeNativeSuggestionCache =
        load_store_value(app, CLAUDE_NATIVE_CACHE_KEY).unwrap_or_default();
    filter_claude_native_suggestions(&cache, project_id, now_millis())
}

pub(super) fn should_use_claude_native_suggestions(
    settings: &SuggestionSettings,
    force_refresh: Option<bool>,
) -> bool {
    matches!(settings.source, SuggestionSource::AssistantAi) && !force_refresh.unwrap_or(false)
}

pub(super) fn filter_claude_native_suggestions(
    cache: &ClaudeNativeSuggestionCache,
    project_id: &str,
    now: i64,
) -> Option<Vec<SuggestionItem>> {
    let hit = cache.get(project_id)?;
    if now.saturating_sub(hit.generated_at) > CACHE_TTL_MS
        || hit.project_id.as_deref() != Some(project_id)
    {
        return None;
    }
    Some(vec![hit.clone()])
}

fn summarize_claude_prompt_suggestion(prompt: &str) -> String {
    let compact = compact_line(prompt);
    let compact = compact
        .trim_start_matches("请")
        .trim_start_matches("帮我")
        .trim();
    truncate_chars(
        if compact.is_empty() { prompt } else { compact },
        SUMMARY_LIMIT,
    )
}
