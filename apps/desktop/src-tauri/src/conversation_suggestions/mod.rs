mod cache;
mod claude_native;
mod generation;
mod github_context;
mod local_git;
mod model;
mod scope;
mod types;

pub(crate) use claude_native::save_claude_prompt_suggestion;
pub(crate) use types::{SuggestionItem, SuggestionSettings};

use tauri::{AppHandle, State};

use crate::settings_store::{load_store_value, save_store_value};
use crate::store::LiliaStore;

use cache::{build_cache_key, cache_scope_key, load_cache_hit, normalize_settings, save_cache};
use claude_native::{load_claude_native_suggestions, should_use_claude_native_suggestions};
use generation::{build_generation_prompt, materialize_items, parse_model_suggestions};
use model::{request_model, resolve_model_request};
use scope::build_scope;
use types::SETTINGS_KEY;

#[tauri::command]
pub fn conversation_suggestions_get_settings(app: AppHandle) -> SuggestionSettings {
    normalize_settings(load_store_value(&app, SETTINGS_KEY))
}

#[tauri::command]
pub fn conversation_suggestions_set_settings(
    app: AppHandle,
    settings: SuggestionSettings,
) -> Result<(), String> {
    save_store_value(&app, SETTINGS_KEY, &normalize_settings(Some(settings)))
}

#[tauri::command]
pub fn conversation_suggestions_get(
    app: AppHandle,
    store: State<'_, LiliaStore>,
    project_id: Option<String>,
    force_refresh: Option<bool>,
) -> Result<Vec<SuggestionItem>, String> {
    let settings = conversation_suggestions_get_settings(app.clone());
    if !settings.enabled {
        return Ok(Vec::new());
    }

    if should_use_claude_native_suggestions(&settings, force_refresh) {
        if let Some(items) = load_claude_native_suggestions(&app, project_id.as_deref()) {
            return Ok(items);
        }
    }

    let conn = store.conn()?;
    let Some(scope) = build_scope(&app, &conn, project_id.as_deref())? else {
        return Ok(Vec::new());
    };
    let Some(model) = resolve_model_request(&app, &settings) else {
        return Ok(Vec::new());
    };
    let cache_key = build_cache_key(&scope, &model);
    let cache_scope = cache_scope_key(project_id.as_deref(), &settings.source);
    if force_refresh != Some(true) {
        if let Some(hit) = load_cache_hit(&app, &cache_scope, &cache_key) {
            return Ok(hit.items);
        }
    }

    let prompt = build_generation_prompt(&scope);
    match request_model(&model, &prompt).and_then(parse_model_suggestions) {
        Ok(items) => {
            let generated = materialize_items(items, &scope);
            save_cache(&app, cache_scope, cache_key, generated.clone());
            Ok(generated)
        }
        Err(err) => {
            eprintln!("[conversation-suggestions] generate failed: {err}");
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests;
