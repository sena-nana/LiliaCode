use std::collections::HashSet;

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::chat::state::ChatStore;
use crate::codex_history::{codex_thread_search_blocking, query_codex_thread_runtime_states};

use super::generation::{compact_line, truncate_chars};
use super::types::{
    CodexThreadSample, ProjectContext, SuggestionCodexThreadRef, CODEX_THREAD_FETCH_LIMIT,
    CODEX_THREAD_LIMIT, SAMPLE_TEXT_LIMIT,
};

pub(super) fn load_recent_codex_threads(
    app: &AppHandle,
    conn: &Connection,
    project: &ProjectContext,
    requested_project_id: Option<&str>,
) -> Result<Vec<CodexThreadSample>, String> {
    let active_thread_ids = active_codex_thread_ids(app, conn, requested_project_id);
    let result = codex_thread_search_blocking(
        app.clone(),
        crate::codex_history::CodexThreadSearchInput {
            search_term: None,
            cursor: None,
            limit: Some(CODEX_THREAD_FETCH_LIMIT),
            archived: Some(false),
        },
    )?;
    let project_cwd = project.cwd.as_deref().map(normalize_path);
    let has_thread_cwd = project_cwd.is_some()
        && result.threads.iter().any(|thread| {
            thread
                .project_cwd
                .as_deref()
                .is_some_and(|cwd| !cwd.trim().is_empty())
        });
    let samples = result
        .threads
        .into_iter()
        .filter(|thread| !thread.archived)
        .filter(|thread| !active_thread_ids.contains(thread.id.trim()))
        .filter(|thread| {
            let Some(project_cwd) = project_cwd.as_deref() else {
                return true;
            };
            match thread.project_cwd.as_deref().map(normalize_path) {
                Some(thread_cwd) => thread_cwd == project_cwd,
                None => !has_thread_cwd,
            }
        })
        .filter_map(|thread| {
            let id = thread.id.trim().to_string();
            if id.is_empty() {
                return None;
            }
            let title = truncate_chars(&compact_line(&thread.title), SAMPLE_TEXT_LIMIT);
            let preview = thread
                .preview
                .as_deref()
                .map(compact_line)
                .filter(|preview| !preview.is_empty())
                .map(|preview| truncate_chars(&preview, SAMPLE_TEXT_LIMIT));
            let fingerprint = format!(
                "{}@{}:{}:{}",
                id,
                thread.updated_at.unwrap_or(0),
                title,
                preview.as_deref().unwrap_or("")
            );
            Some(CodexThreadSample {
                thread: SuggestionCodexThreadRef {
                    id,
                    title,
                    updated_at: thread.updated_at,
                    preview,
                },
                fingerprint,
            })
        })
        .take(CODEX_THREAD_LIMIT)
        .collect();
    Ok(samples)
}

fn active_codex_thread_ids(
    app: &AppHandle,
    conn: &Connection,
    requested_project_id: Option<&str>,
) -> HashSet<String> {
    let Some(chat_store) = app.try_state::<ChatStore>() else {
        return HashSet::new();
    };
    query_codex_thread_runtime_states(conn, &chat_store)
        .unwrap_or_default()
        .into_iter()
        .filter(|state| state.pending)
        .filter(|state| match requested_project_id {
            Some(project_id) => state.project_id.as_deref() == Some(project_id),
            None => true,
        })
        .map(|state| state.thread_id.trim().to_string())
        .filter(|thread_id| !thread_id.is_empty())
        .collect()
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}
