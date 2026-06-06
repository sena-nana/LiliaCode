use std::collections::{HashMap, HashSet};
use std::time::Duration;

use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::provider::{
    backend_api_key_env, load_active_backend, load_assistant_ai_config, resolve_connection_for,
    AssistantAIConfig, BackendConnectionPlan,
};
use crate::settings_store::{load_store_value, save_store_value};
use crate::store::LiliaStore;
use crate::{BACKEND_CLAUDE, BACKEND_CODEX, CODEX_MODEL_OPTIONS};

const SETTINGS_KEY: &str = "conversation-suggestions.settings";
const CACHE_KEY: &str = "conversation-suggestions.cache";
const CACHE_TTL_MS: i64 = 24 * 60 * 60 * 1000;
const MAX_TASKS_PER_SCOPE: usize = 3;
const TASK_CANDIDATE_LIMIT: usize = 12;
const MAX_SUGGESTIONS: usize = 3;
const SAMPLE_TEXT_LIMIT: usize = 280;
const SUMMARY_LIMIT: usize = 40;
const REASON_LIMIT: usize = 120;
const PROMPT_LIMIT: usize = 600;
const UNFINISHED_SIGNAL_LIMIT: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SuggestionSource {
    Provider,
    AssistantAi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionSettings {
    pub(crate) enabled: bool,
    pub(crate) source: SuggestionSource,
}

impl Default for SuggestionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            source: SuggestionSource::AssistantAi,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionItem {
    pub(crate) id: String,
    pub(crate) project_id: Option<String>,
    pub(crate) task_ids: Vec<String>,
    pub(crate) summary: String,
    pub(crate) reason: String,
    pub(crate) prompt: String,
    pub(crate) generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuggestionCacheEntry {
    cache_key: String,
    generated_at: i64,
    items: Vec<SuggestionItem>,
}

type SuggestionCache = HashMap<String, SuggestionCacheEntry>;

#[derive(Debug, Clone)]
struct TaskSample {
    id: String,
    title: String,
    status: String,
    project_id: Option<String>,
    user_messages: Vec<String>,
    assistant_message: Option<String>,
    unfinished_signals: Vec<String>,
    latest_updated_at: i64,
}

#[derive(Debug, Clone)]
struct SuggestionScope {
    project_id: Option<String>,
    tasks: Vec<TaskSample>,
    latest_updated_at: i64,
}

#[derive(Debug, Clone)]
struct ModelRequest {
    source: SuggestionSource,
    backend: Option<String>,
    model: String,
    base_url: String,
    api_key: String,
}

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

    let conn = store.conn()?;
    let Some(scope) = build_scope(&conn, project_id.as_deref())? else {
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

fn normalize_settings(settings: Option<SuggestionSettings>) -> SuggestionSettings {
    let settings = settings.unwrap_or_default();
    SuggestionSettings {
        enabled: settings.enabled,
        source: settings.source,
    }
}

fn now_millis() -> i64 {
    crate::util::now_millis() as i64
}

fn cache_scope_key(project_id: Option<&str>, source: &SuggestionSource) -> String {
    format!(
        "{}:{}",
        match source {
            SuggestionSource::Provider => "provider",
            SuggestionSource::AssistantAi => "assistant-ai",
        },
        project_id.unwrap_or("__recent__")
    )
}

fn load_cache_hit(app: &AppHandle, scope: &str, cache_key: &str) -> Option<SuggestionCacheEntry> {
    let cache: SuggestionCache = load_store_value(app, CACHE_KEY).unwrap_or_default();
    let hit = cache.get(scope)?;
    cache_entry_is_valid(hit, cache_key, now_millis()).then(|| hit.clone())
}

fn cache_entry_is_valid(entry: &SuggestionCacheEntry, cache_key: &str, now: i64) -> bool {
    entry.cache_key == cache_key && now.saturating_sub(entry.generated_at) <= CACHE_TTL_MS
}

fn save_cache(app: &AppHandle, scope: String, cache_key: String, items: Vec<SuggestionItem>) {
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

fn build_cache_key(scope: &SuggestionScope, model: &ModelRequest) -> String {
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
    format!(
        "{}|{}|{}|{}|{}|{}",
        scope.project_id.as_deref().unwrap_or("__recent__"),
        source_label(&model.source),
        model.backend.as_deref().unwrap_or("assistant-ai"),
        model.model,
        scope.latest_updated_at,
        signal_fingerprint
    )
}

fn source_label(source: &SuggestionSource) -> &'static str {
    match source {
        SuggestionSource::Provider => "provider",
        SuggestionSource::AssistantAi => "assistant-ai",
    }
}

fn build_scope(
    conn: &Connection,
    requested_project_id: Option<&str>,
) -> Result<Option<SuggestionScope>, String> {
    let tasks = if let Some(project_id) = requested_project_id {
        load_task_samples(conn, Some(project_id), TASK_CANDIDATE_LIMIT)?
    } else {
        load_recent_task_samples(conn, TASK_CANDIDATE_LIMIT)?
    };
    let tasks = tasks
        .into_iter()
        .filter(|task| !task.unfinished_signals.is_empty())
        .take(MAX_TASKS_PER_SCOPE)
        .collect::<Vec<_>>();
    if tasks.is_empty() {
        return Ok(None);
    }
    let latest_updated_at = tasks
        .iter()
        .map(|task| task.latest_updated_at)
        .max()
        .unwrap_or(0);
    let project_id = requested_project_id
        .map(str::to_string)
        .or_else(|| tasks.iter().find_map(|task| task.project_id.clone()));
    Ok(Some(SuggestionScope {
        project_id,
        tasks,
        latest_updated_at,
    }))
}

fn load_task_samples(
    conn: &Connection,
    project_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TaskSample>, String> {
    let mut out = Vec::new();
    if let Some(project_id) = project_id {
        let mut stmt = conn
            .prepare(
                r#"SELECT t.id, t.title, t.status, t.project_id, MAX(e.updated_at) AS latest
                   FROM tasks t
                   INNER JOIN agent_timeline_events e ON e.task_id = t.id
                   WHERE t.project_id = ?1 AND t.archived = 0
                   GROUP BY t.id
                   ORDER BY latest DESC
                   LIMIT ?2"#,
            )
            .map_err(|e| format!("conversation_suggestions: prepare tasks 失败：{e}"))?;
        let rows = stmt
            .query_map(params![project_id, limit as i64], read_task_sample_row)
            .map_err(|e| format!("conversation_suggestions: query tasks 失败：{e}"))?;
        for row in rows {
            let (id, title, status, project_id, latest) =
                row.map_err(|e| format!("conversation_suggestions: task row 失败：{e}"))?;
            out.push(load_sample_details(
                conn, id, title, status, project_id, latest,
            )?);
        }
    } else {
        let mut stmt = conn
            .prepare(
                r#"SELECT t.id, t.title, t.status, t.project_id, MAX(e.updated_at) AS latest
                   FROM tasks t
                   INNER JOIN agent_timeline_events e ON e.task_id = t.id
                   WHERE t.project_id IS NULL AND t.archived = 0
                   GROUP BY t.id
                   ORDER BY latest DESC
                   LIMIT ?1"#,
            )
            .map_err(|e| format!("conversation_suggestions: prepare tasks 失败：{e}"))?;
        let rows = stmt
            .query_map(params![limit as i64], read_task_sample_row)
            .map_err(|e| format!("conversation_suggestions: query tasks 失败：{e}"))?;
        for row in rows {
            let (id, title, status, project_id, latest) =
                row.map_err(|e| format!("conversation_suggestions: task row 失败：{e}"))?;
            out.push(load_sample_details(
                conn, id, title, status, project_id, latest,
            )?);
        }
    }
    Ok(out)
}

fn read_task_sample_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(String, String, String, Option<String>, i64)> {
    Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?,
        row.get::<_, i64>(4)?,
    ))
}

fn load_recent_task_samples(conn: &Connection, limit: usize) -> Result<Vec<TaskSample>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT t.id, t.title, t.status, t.project_id, MAX(e.updated_at) AS latest
               FROM tasks t
               INNER JOIN agent_timeline_events e ON e.task_id = t.id
               WHERE t.archived = 0
               GROUP BY t.id
               ORDER BY latest DESC
               LIMIT ?1"#,
        )
        .map_err(|e| format!("conversation_suggestions: prepare recent tasks 失败：{e}"))?;
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| format!("conversation_suggestions: query recent tasks 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, title, status, project_id, latest) =
            row.map_err(|e| format!("conversation_suggestions: recent task row 失败：{e}"))?;
        out.push(load_sample_details(
            conn, id, title, status, project_id, latest,
        )?);
    }
    Ok(out)
}

fn load_sample_details(
    conn: &Connection,
    id: String,
    title: String,
    status: String,
    project_id: Option<String>,
    latest_updated_at: i64,
) -> Result<TaskSample, String> {
    Ok(TaskSample {
        user_messages: load_event_texts(conn, &id, "message", Some("user"), None, 2)?,
        assistant_message: load_event_texts(
            conn,
            &id,
            "message",
            Some("assistant"),
            Some("success"),
            1,
        )?
        .into_iter()
        .next(),
        unfinished_signals: load_unfinished_signal_texts(conn, &id, UNFINISHED_SIGNAL_LIMIT)?,
        id,
        title,
        status,
        project_id,
        latest_updated_at,
    })
}

fn load_event_texts(
    conn: &Connection,
    task_id: &str,
    kind: &str,
    role: Option<&str>,
    status: Option<&str>,
    limit: usize,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT summary, payload, status
               FROM agent_timeline_events
               WHERE task_id = ?1 AND kind = ?2
               ORDER BY updated_at DESC
               LIMIT 20"#,
        )
        .map_err(|e| format!("conversation_suggestions: prepare events 失败：{e}"))?;
    let rows = stmt
        .query_map(params![task_id, kind], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("conversation_suggestions: query events 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (summary, payload_text, event_status) =
            row.map_err(|e| format!("conversation_suggestions: event row 失败：{e}"))?;
        if status.is_some() && Some(event_status.as_str()) != status {
            continue;
        }
        let payload = serde_json::from_str::<JsonValue>(&payload_text).unwrap_or(JsonValue::Null);
        if role.is_some() && payload.get("role").and_then(|v| v.as_str()) != role {
            continue;
        }
        let text = payload
            .get("content")
            .and_then(|v| v.as_str())
            .or(summary.as_deref())
            .map(compact_line)
            .unwrap_or_default();
        if !text.is_empty() {
            out.push(truncate_chars(&text, SAMPLE_TEXT_LIMIT));
        }
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

fn load_unfinished_signal_texts(
    conn: &Connection,
    task_id: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for text in load_unfinished_task_todos(conn, task_id, limit)? {
        push_unfinished_signal(&mut out, &mut seen, "todo", &text, limit);
        if out.len() >= limit {
            return Ok(out);
        }
    }
    load_unfinished_timeline_signals(conn, task_id, limit, &mut out, &mut seen)?;
    Ok(out)
}

fn load_unfinished_task_todos(
    conn: &Connection,
    task_id: &str,
    limit: usize,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT text
               FROM task_todos
               WHERE task_id = ?1 AND done = 0
               ORDER BY "order" ASC, updated_at DESC
               LIMIT ?2"#,
        )
        .map_err(|e| format!("conversation_suggestions: prepare todos 失败：{e}"))?;
    let rows = stmt
        .query_map(params![task_id, limit as i64], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| format!("conversation_suggestions: query todos 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let text = row.map_err(|e| format!("conversation_suggestions: todo row 失败：{e}"))?;
        out.push(text);
    }
    Ok(out)
}

fn load_unfinished_timeline_signals(
    conn: &Connection,
    task_id: &str,
    limit: usize,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT kind, title, summary, payload
               FROM agent_timeline_events
               WHERE task_id = ?1
                 AND kind IN ('todo_list', 'error')
               ORDER BY updated_at DESC
               LIMIT 20"#,
        )
        .map_err(|e| format!("conversation_suggestions: prepare signals 失败：{e}"))?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| format!("conversation_suggestions: query signals 失败：{e}"))?;
    for row in rows {
        let (kind, title, summary, payload_text) =
            row.map_err(|e| format!("conversation_suggestions: signal row 失败：{e}"))?;
        let payload = serde_json::from_str::<JsonValue>(&payload_text).unwrap_or(JsonValue::Null);
        if kind == "todo_list" {
            for text in unfinished_todo_payload_items(&payload) {
                push_unfinished_signal(out, seen, "todo", &text, limit);
                if out.len() >= limit {
                    return Ok(());
                }
            }
        } else if kind == "error" {
            push_unfinished_signal(
                out,
                seen,
                "error",
                summary.as_deref().unwrap_or(title.as_str()),
                limit,
            );
        }
        if out.len() >= limit {
            break;
        }
    }
    Ok(())
}

fn push_unfinished_signal(
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    kind: &str,
    text: &str,
    limit: usize,
) {
    if out.len() >= limit {
        return;
    }
    let text = compact_line(text);
    if text.is_empty() || !seen.insert(format!("{kind}:{text}")) {
        return;
    }
    out.push(truncate_chars(
        &format!("{kind}: {text}"),
        SAMPLE_TEXT_LIMIT,
    ));
}

fn unfinished_todo_payload_items(payload: &JsonValue) -> Vec<String> {
    let Some(items) = payload
        .get("items")
        .or_else(|| payload.get("todos"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(text) = item.as_str().map(str::trim).filter(|text| !text.is_empty()) {
                return Some(text.to_string());
            }
            if todo_item_is_done(item) {
                return None;
            }
            item.get("content")
                .or_else(|| item.get("text"))
                .or_else(|| item.get("title"))
                .or_else(|| item.get("description"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        })
        .collect()
}

fn todo_item_is_done(item: &JsonValue) -> bool {
    item.get("completed")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || item
            .get("done")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        || item
            .get("status")
            .and_then(|value| value.as_str())
            .map(|status| status.eq_ignore_ascii_case("completed"))
            .unwrap_or(false)
}

fn resolve_model_request(app: &AppHandle, settings: &SuggestionSettings) -> Option<ModelRequest> {
    match settings.source {
        SuggestionSource::AssistantAi => assistant_ai_model_request(app),
        SuggestionSource::Provider => provider_model_request(app),
    }
}

fn assistant_ai_model_request(app: &AppHandle) -> Option<ModelRequest> {
    let cfg: AssistantAIConfig = load_assistant_ai_config(app);
    let base_url = cfg.base_url?.trim().trim_end_matches('/').to_string();
    let api_key = cfg.api_key?.trim().to_string();
    let model = cfg.model?.trim().to_string();
    if base_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return None;
    }
    Some(ModelRequest {
        source: SuggestionSource::AssistantAi,
        backend: None,
        model,
        base_url,
        api_key,
    })
}

fn provider_model_request(app: &AppHandle) -> Option<ModelRequest> {
    let backend = load_active_backend(app);
    let plan = resolve_connection_for(app, &backend);
    let base_url = effective_base_url(&backend, &plan)?;
    let api_key = provider_api_key(&backend, plan.api_key.as_deref())?;
    Some(ModelRequest {
        source: SuggestionSource::Provider,
        model: if backend == BACKEND_CODEX {
            CODEX_MODEL_OPTIONS[0].0.to_string()
        } else {
            "claude-sonnet-4-6".to_string()
        },
        backend: Some(backend),
        base_url,
        api_key,
    })
}

fn provider_api_key(backend: &str, plan_api_key: Option<&str>) -> Option<String> {
    plan_api_key
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var(backend_api_key_env(backend))
                .ok()
                .map(|key| key.trim().to_string())
                .filter(|key| !key.is_empty())
        })
}

fn effective_base_url(backend: &str, plan: &BackendConnectionPlan) -> Option<String> {
    let base = plan.base_url.clone().or_else(|| {
        if backend == BACKEND_CODEX {
            Some("https://api.openai.com/v1".to_string())
        } else {
            Some("https://api.anthropic.com".to_string())
        }
    })?;
    let trimmed = base.trim().trim_end_matches('/').to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn build_generation_prompt(scope: &SuggestionScope) -> String {
    let mut lines = vec![
        "你是 LiliaCode 的新对话建议助手。只能基于下方任务里的未完成信号提出继续处理建议。".to_string(),
        "只返回 JSON 数组，可返回 []，最多 3 项。每项字段必须是 taskIds、summary、reason、prompt。不要 markdown。".to_string(),
        "taskIds 必须引用下方任务 id；summary 控制在 20 个中文字左右；reason 控制在 80 个中文字左右；prompt 是可直接填入对话框的中文提示词，控制在 300 个中文字左右。".to_string(),
        "不要提出泛化建议、体验优化、新方向、代码审查或测试补齐，除非它们被未完成信号明确指向。没有明确可继续处理的未完成信号时返回 []。".to_string(),
        format!(
            "scopeProjectId: {}",
            scope.project_id.as_deref().unwrap_or("recent-projects")
        ),
    ];
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
    lines.join("\n")
}

fn request_model(model: &ModelRequest, prompt: &str) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| format!("HTTP 客户端构造失败：{e}"))?;
    if model.backend.as_deref() == Some(BACKEND_CLAUDE) {
        request_anthropic(&client, model, prompt)
    } else {
        request_openai_compatible(&client, model, prompt)
    }
}

fn request_openai_compatible(
    client: &Client,
    model: &ModelRequest,
    prompt: &str,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", model.base_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .bearer_auth(&model.api_key)
        .json(&json!({
            "model": model.model,
            "messages": [
                { "role": "system", "content": "只输出严格 JSON。" },
                { "role": "user", "content": prompt }
            ],
            "temperature": 0.2,
            "max_tokens": 700
        }))
        .send()
        .map_err(|e| format!("OpenAI 兼容请求失败：{e}"))?;
    if !resp.status().is_success() {
        return Err(format!("OpenAI 兼容 HTTP {}", resp.status()));
    }
    let value = resp
        .json::<JsonValue>()
        .map_err(|e| format!("OpenAI 响应解析失败：{e}"))?;
    value
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "OpenAI 响应缺少 message.content".to_string())
}

fn request_anthropic(
    client: &Client,
    model: &ModelRequest,
    prompt: &str,
) -> Result<String, String> {
    let base = model.base_url.trim_end_matches('/');
    let url = if base.ends_with("/v1") {
        format!("{base}/messages")
    } else {
        format!("{base}/v1/messages")
    };
    let resp = client
        .post(&url)
        .header("x-api-key", &model.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model.model,
            "max_tokens": 700,
            "temperature": 0.2,
            "system": "只输出严格 JSON。",
            "messages": [{ "role": "user", "content": prompt }]
        }))
        .send()
        .map_err(|e| format!("Anthropic 请求失败：{e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Anthropic HTTP {}", resp.status()));
    }
    let value = resp
        .json::<JsonValue>()
        .map_err(|e| format!("Anthropic 响应解析失败：{e}"))?;
    value
        .get("content")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find_map(|item| item.get("text").and_then(|v| v.as_str()))
        })
        .map(str::to_string)
        .ok_or_else(|| "Anthropic 响应缺少 text".to_string())
}

#[derive(Debug, Deserialize)]
struct RawSuggestion {
    #[serde(default, rename = "taskIds")]
    task_ids: Vec<String>,
    summary: Option<String>,
    reason: Option<String>,
    prompt: Option<String>,
}

fn parse_model_suggestions(text: String) -> Result<Vec<RawSuggestion>, String> {
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

fn materialize_items(raw: Vec<RawSuggestion>, scope: &SuggestionScope) -> Vec<SuggestionItem> {
    let generated_at = now_millis();
    let valid_task_ids: HashSet<String> = scope.tasks.iter().map(|task| task.id.clone()).collect();
    raw.into_iter()
        .filter_map(|item| {
            let task_ids = item
                .task_ids
                .into_iter()
                .filter(|task_id| valid_task_ids.contains(task_id))
                .collect::<Vec<_>>();
            if task_ids.is_empty() {
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
                task_ids,
                summary,
                reason,
                prompt,
                generated_at,
            })
        })
        .take(MAX_SUGGESTIONS)
        .collect()
}

fn compact_line(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(input: &str, max: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE tasks (
              id TEXT PRIMARY KEY,
              project_id TEXT,
              session_id TEXT NOT NULL,
              title TEXT NOT NULL,
              status TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              parent_id TEXT,
              archived INTEGER NOT NULL DEFAULT 0,
              sort_order INTEGER NOT NULL DEFAULT 0,
              pinned INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE agent_timeline_events (
              id TEXT PRIMARY KEY,
              task_id TEXT NOT NULL,
              turn_id TEXT,
              backend TEXT NOT NULL,
              kind TEXT NOT NULL,
              status TEXT NOT NULL,
              title TEXT NOT NULL,
              summary TEXT,
              payload TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              turn_seq INTEGER NOT NULL,
              intra_turn_order INTEGER NOT NULL
            );
            CREATE TABLE task_todos (
              id           TEXT PRIMARY KEY,
              task_id      TEXT NOT NULL,
              text         TEXT NOT NULL,
              done         INTEGER NOT NULL DEFAULT 0,
              "order"      INTEGER NOT NULL,
              source       TEXT NOT NULL CHECK (source IN ('lilia','agent')),
              priority     TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('high','normal','low')),
              guide_status TEXT CHECK (guide_status IS NULL OR guide_status IN ('pending','queued','sent')),
              attachments_json TEXT NOT NULL DEFAULT '[]',
              created_at   INTEGER NOT NULL,
              updated_at   INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();
    }

    fn insert_task(conn: &Connection, id: &str, project_id: &str, archived: bool) {
        conn.execute(
            "INSERT INTO tasks (id, project_id, session_id, title, status, created_at, archived) VALUES (?1, ?2, ?1, ?3, 'running', 1, ?4)",
            params![id, project_id, format!("任务 {id}"), if archived { 1 } else { 0 }],
        )
        .unwrap();
    }

    fn insert_todo(conn: &Connection, task_id: &str, text: &str, done: bool, order: i64) {
        conn.execute(
            r#"INSERT INTO task_todos
               (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, 'agent', 'normal', NULL, '[]', ?6, ?6)"#,
            params![
                format!("{task_id}-todo-{order}"),
                task_id,
                text,
                if done { 1 } else { 0 },
                order,
                order + 100
            ],
        )
        .unwrap();
    }

    fn insert_todo_list_event(conn: &Connection, task_id: &str, updated_at: i64, items: JsonValue) {
        conn.execute(
            r#"INSERT INTO agent_timeline_events
               (id, task_id, turn_id, backend, kind, status, title, summary, payload, created_at, updated_at, turn_seq, intra_turn_order)
               VALUES (?1, ?2, 'turn', 'claude', 'todo_list', 'success', 'Todo', NULL, ?3, ?4, ?4, 0, 1)"#,
            params![
                format!("{task_id}-todo-list-{updated_at}"),
                task_id,
                json!({ "items": items }).to_string(),
                updated_at
            ],
        )
        .unwrap();
    }

    fn insert_event(conn: &Connection, task_id: &str, updated_at: i64, content: &str) {
        conn.execute(
            r#"INSERT INTO agent_timeline_events
               (id, task_id, turn_id, backend, kind, status, title, summary, payload, created_at, updated_at, turn_seq, intra_turn_order)
               VALUES (?1, ?2, 'turn', 'claude', 'message', 'success', '用户输入', ?3, ?4, ?5, ?5, 0, 0)"#,
            params![
                format!("{task_id}-{updated_at}"),
                task_id,
                content,
                json!({ "role": "user", "content": content }).to_string(),
                updated_at
            ],
        )
        .unwrap();
    }

    #[test]
    fn task_sampling_uses_recent_unarchived_project_tasks() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn);
        insert_task(&conn, "old", "p1", false);
        insert_task(&conn, "new", "p1", false);
        insert_task(&conn, "archived", "p1", true);
        insert_event(&conn, "old", 10, "旧对话");
        insert_event(&conn, "new", 30, "新对话");
        insert_event(&conn, "archived", 50, "归档对话");

        let samples = load_task_samples(&conn, Some("p1"), 3).unwrap();

        assert_eq!(
            samples.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["new", "old"]
        );
    }

    #[test]
    fn prompt_builder_truncates_long_history() {
        let scope = SuggestionScope {
            project_id: Some("p1".to_string()),
            latest_updated_at: 1,
            tasks: vec![TaskSample {
                id: "t1".to_string(),
                title: "x".repeat(200),
                status: "running".to_string(),
                project_id: Some("p1".to_string()),
                user_messages: vec!["a".repeat(1000)],
                assistant_message: None,
                unfinished_signals: vec!["todo: ".to_string() + &"b".repeat(1000)],
                latest_updated_at: 1,
            }],
        };

        let prompt = build_generation_prompt(&scope);

        assert!(prompt.chars().count() < 1000);
        assert!(prompt.contains('…'));
    }

    #[test]
    fn scope_ignores_recent_tasks_without_unfinished_signals() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn);
        insert_task(&conn, "done", "p1", false);
        insert_event(&conn, "done", 20, "最近对话");
        insert_todo(&conn, "done", "已完成事项", true, 0);

        let scope = build_scope(&conn, Some("p1")).unwrap();

        assert!(scope.is_none());
    }

    #[test]
    fn scope_uses_unfinished_task_todos_in_prompt() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn);
        insert_task(&conn, "todo-task", "p1", false);
        insert_event(&conn, "todo-task", 20, "继续做权限检查");
        insert_todo(&conn, "todo-task", "补齐权限失败回退", false, 0);

        let scope = build_scope(&conn, Some("p1")).unwrap().unwrap();
        let prompt = build_generation_prompt(&scope);

        assert_eq!(scope.tasks.len(), 1);
        assert!(prompt.contains("未完成信号: todo: 补齐权限失败回退"));
        assert!(prompt.contains("可返回 []"));
        assert!(prompt.contains("不要提出泛化建议"));
    }

    #[test]
    fn todo_list_payload_samples_only_unfinished_items() {
        let conn = Connection::open_in_memory().unwrap();
        create_schema(&conn);
        insert_task(&conn, "timeline-todo", "p1", false);
        insert_event(&conn, "timeline-todo", 20, "处理同步");
        insert_todo_list_event(
            &conn,
            "timeline-todo",
            30,
            json!([
                { "content": "已经完成", "status": "completed" },
                { "content": "继续同步 pending 状态", "status": "pending" },
                { "text": "布尔完成项", "done": true }
            ]),
        );

        let scope = build_scope(&conn, Some("p1")).unwrap().unwrap();
        let signals = &scope.tasks[0].unfinished_signals;

        assert_eq!(signals, &vec!["todo: 继续同步 pending 状态".to_string()]);
    }

    #[test]
    fn materialize_supports_zero_to_three_items_and_requires_task_anchor() {
        let scope = SuggestionScope {
            project_id: Some("p1".to_string()),
            latest_updated_at: 1,
            tasks: vec![TaskSample {
                id: "t1".to_string(),
                title: "任务 t1".to_string(),
                status: "running".to_string(),
                project_id: Some("p1".to_string()),
                user_messages: Vec::new(),
                assistant_message: None,
                unfinished_signals: vec!["todo: 补齐测试".to_string()],
                latest_updated_at: 1,
            }],
        };
        assert!(materialize_items(Vec::new(), &scope).is_empty());

        let raw = vec![
            RawSuggestion {
                task_ids: Vec::new(),
                summary: Some("泛化建议".to_string()),
                reason: Some("没有任务锚点".to_string()),
                prompt: Some("请随便优化一下。".to_string()),
            },
            RawSuggestion {
                task_ids: vec!["missing".to_string()],
                summary: Some("错误锚点".to_string()),
                reason: Some("引用不存在任务".to_string()),
                prompt: Some("请处理不存在任务。".to_string()),
            },
            RawSuggestion {
                task_ids: vec!["t1".to_string()],
                summary: Some("继续一".to_string()),
                reason: Some("锚定未完成信号一".to_string()),
                prompt: Some("请继续处理一。".to_string()),
            },
            RawSuggestion {
                task_ids: vec!["t1".to_string()],
                summary: Some("继续二".to_string()),
                reason: Some("锚定未完成信号二".to_string()),
                prompt: Some("请继续处理二。".to_string()),
            },
            RawSuggestion {
                task_ids: vec!["t1".to_string()],
                summary: Some("继续三".to_string()),
                reason: Some("锚定未完成信号三".to_string()),
                prompt: Some("请继续处理三。".to_string()),
            },
            RawSuggestion {
                task_ids: vec!["t1".to_string()],
                summary: Some("继续四".to_string()),
                reason: Some("超过数量上限".to_string()),
                prompt: Some("请继续处理四。".to_string()),
            },
        ];

        let items = materialize_items(raw, &scope);

        assert_eq!(items.len(), 3);
        assert!(items
            .iter()
            .all(|item| item.task_ids == vec!["t1".to_string()]));
        assert_eq!(items[0].summary, "继续一");
    }

    #[test]
    fn cache_key_changes_when_unfinished_signals_change() {
        let model = ModelRequest {
            source: SuggestionSource::AssistantAi,
            backend: None,
            model: "mini".to_string(),
            base_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
        };
        let mut scope = SuggestionScope {
            project_id: Some("p1".to_string()),
            latest_updated_at: 20,
            tasks: vec![TaskSample {
                id: "t1".to_string(),
                title: "任务 t1".to_string(),
                status: "running".to_string(),
                project_id: Some("p1".to_string()),
                user_messages: Vec::new(),
                assistant_message: None,
                unfinished_signals: vec!["todo: 第一项".to_string()],
                latest_updated_at: 20,
            }],
        };
        let first = build_cache_key(&scope, &model);
        scope.tasks[0].unfinished_signals = vec!["todo: 第二项".to_string()];
        let second = build_cache_key(&scope, &model);

        assert_ne!(first, second);
    }

    #[test]
    fn invalid_model_json_returns_error() {
        assert!(parse_model_suggestions("not json".to_string()).is_err());
    }

    #[test]
    fn cache_hit_requires_same_key_and_fresh_entry() {
        let item = SuggestionItem {
            id: "sg-1".to_string(),
            project_id: Some("p1".to_string()),
            task_ids: vec!["t1".to_string()],
            summary: "继续测试".to_string(),
            reason: "缓存命中需要稳定判断".to_string(),
            prompt: "请验证建议缓存命中。".to_string(),
            generated_at: 100,
        };
        let entry = SuggestionCacheEntry {
            cache_key: "p1|assistant-ai|assistant-ai|mini|20".to_string(),
            generated_at: 100,
            items: vec![item],
        };

        assert!(cache_entry_is_valid(
            &entry,
            "p1|assistant-ai|assistant-ai|mini|20",
            100 + CACHE_TTL_MS,
        ));
        assert!(!cache_entry_is_valid(
            &entry,
            "p1|assistant-ai|assistant-ai|mini|30",
            100 + CACHE_TTL_MS,
        ));
        assert!(!cache_entry_is_valid(
            &entry,
            "p1|assistant-ai|assistant-ai|mini|20",
            101 + CACHE_TTL_MS,
        ));
    }
}
