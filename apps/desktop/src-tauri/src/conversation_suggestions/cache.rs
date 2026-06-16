use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::settings_store::{load_store_value, save_store_value};

use super::types::{
    now_millis, ModelRequest, SuggestionItem, SuggestionScope, SuggestionSettings,
    SuggestionSource, CACHE_KEY, CACHE_TTL_MS,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SuggestionCacheEntry {
    pub(super) cache_key: String,
    pub(super) generated_at: i64,
    pub(super) items: Vec<SuggestionItem>,
}

type SuggestionCache = HashMap<String, SuggestionCacheEntry>;

pub(super) fn normalize_settings(settings: Option<SuggestionSettings>) -> SuggestionSettings {
    let settings = settings.unwrap_or_default();
    SuggestionSettings {
        enabled: settings.enabled,
        source: settings.source,
    }
}

pub(super) fn cache_scope_key(project_id: Option<&str>, source: &SuggestionSource) -> String {
    format!(
        "{}:{}",
        match source {
            SuggestionSource::Provider => "provider",
            SuggestionSource::AssistantAi => "assistant-ai",
        },
        project_id.unwrap_or("__recent__")
    )
}

pub(super) fn load_cache_hit<R: Runtime>(
    app: &AppHandle<R>,
    scope: &str,
    cache_key: &str,
) -> Option<SuggestionCacheEntry> {
    let cache: SuggestionCache = load_store_value(app, CACHE_KEY).unwrap_or_default();
    let hit = cache.get(scope)?;
    cache_entry_is_valid(hit, cache_key, now_millis()).then(|| hit.clone())
}

pub(super) fn cache_entry_is_valid(
    entry: &SuggestionCacheEntry,
    cache_key: &str,
    now: i64,
) -> bool {
    entry.cache_key == cache_key && now.saturating_sub(entry.generated_at) <= CACHE_TTL_MS
}

pub(super) fn save_cache(
    app: &AppHandle,
    scope: String,
    cache_key: String,
    items: Vec<SuggestionItem>,
) {
    let mut cache: SuggestionCache = load_store_value(app, CACHE_KEY).unwrap_or_default();
    cache.insert(
        scope,
        SuggestionCacheEntry {
            cache_key,
            generated_at: now_millis(),
            items,
        },
    );
    if let Err(err) = save_store_value(app, CACHE_KEY, &cache) {
        eprintln!("[conversation-suggestions] save cache failed: {err}");
    }
}

pub(super) fn build_cache_key(scope: &SuggestionScope, model: &ModelRequest) -> String {
    let signal_fingerprint = scope
        .tasks
        .iter()
        .map(|task| {
            format!(
                "{}@{}:{}",
                task.id,
                task.latest_updated_at,
                task.unfinished_signals.join(" / ")
            )
        })
        .collect::<Vec<_>>()
        .join("||");
    let github_fingerprint = scope
        .github_activities
        .iter()
        .map(|activity| activity.fingerprint.as_str())
        .collect::<Vec<_>>()
        .join("||");
    let local_git_fingerprint = scope
        .local_git_contexts
        .iter()
        .map(|context| context.fingerprint.as_str())
        .collect::<Vec<_>>()
        .join("||");
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        scope.project_id.as_deref().unwrap_or("__recent__"),
        source_label(&model.source),
        model.backend.as_deref().unwrap_or("assistant-ai"),
        model.model,
        scope.latest_updated_at,
        signal_fingerprint,
        scope
            .github_repo
            .as_ref()
            .map(|repo| repo.full_name.as_str())
            .unwrap_or("__no_github_repo__"),
        github_fingerprint,
        local_git_fingerprint
    )
}

fn source_label(source: &SuggestionSource) -> &'static str {
    match source {
        SuggestionSource::Provider => "provider",
        SuggestionSource::AssistantAi => "assistant-ai",
    }
}
