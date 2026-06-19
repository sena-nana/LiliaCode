use std::env;
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value as JsonValue};
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

use crate::agent_timeline::{self, AgentTimelineEventInput};
use crate::chat::state::default_model_for_backend;
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::codex_history;
use crate::projects_tasks::events::emit_tasks_changed;
use crate::provider::{
    assistant_ai_secret, backend_api_key_env, backend_direct_url, codex_account_spark_enabled,
    is_codex_account_spark_request, load_active_backend, load_assistant_ai_config,
    request_codex_account_spark, resolve_connection_for, AssistantAIConfig, BackendConnectionPlan,
    ConnectionMode, CODEX_SPARK_BASE_URL, CODEX_SPARK_MODEL,
};
use crate::store::LiliaStore;
use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

const TITLE_KIND: &str = "title_update";
const TITLE_LABEL: &str = "标题已更新";
const TITLE_MAX_CHARS: usize = 18;
const TITLE_MIN_CHARS: usize = 2;
const SAMPLE_TEXT_LIMIT: usize = 260;
const TITLE_SPARK_INSTRUCTION: &str = "只输出一个中文短标题。";

#[derive(Debug, Clone)]
struct TaskTitleState {
    id: String,
    project_id: Option<String>,
    title: String,
    title_source: String,
}

#[derive(Debug, Clone)]
struct ModelRequest {
    backend: String,
    model: String,
    base_url: String,
    api_key: String,
}

pub(crate) fn spawn_title_update<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    backend: String,
    turn_id: Option<String>,
) {
    thread::spawn(move || {
        if let Err(err) = run_title_update(&app, &task_id, &backend, turn_id.as_deref()) {
            eprintln!("[title-update] skipped: {err}");
        }
    });
}

fn run_title_update<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    backend: &str,
    turn_id: Option<&str>,
) -> Result<(), String> {
    let store = app
        .try_state::<LiliaStore>()
        .ok_or_else(|| "store unavailable".to_string())?;
    let conn = store.conn()?;
    let task =
        load_task_title_state(&conn, task_id)?.ok_or_else(|| "task not found".to_string())?;
    let prompt = build_title_prompt(&conn, task_id, &task.title)?;
    if prompt.is_none() {
        return Ok(());
    }
    let prompt = prompt.unwrap();
    let mut last_error = None;
    let mut proposed = None;
    for model in resolve_model_requests(app, backend) {
        match request_title(app, &model, &prompt).and_then(normalize_title) {
            Ok(title) => {
                proposed = Some(title);
                break;
            }
            Err(err) => {
                eprintln!("[title-update] model failed: {err}");
                last_error = Some(err);
            }
        }
    }
    let proposed =
        proposed.ok_or_else(|| last_error.unwrap_or_else(|| "model unavailable".to_string()))?;
    if proposed == compact_line(&task.title) {
        return Ok(());
    }

    if task.title_source == "manual" {
        persist_title_event(
            app,
            &task,
            backend,
            turn_id,
            "requires_action",
            &proposed,
            true,
        )?;
    } else {
        conn.execute(
            "UPDATE tasks SET title = ?1, title_source = 'auto' WHERE id = ?2 AND archived = 0",
            params![proposed.as_str(), task.id.as_str()],
        )
        .map_err(|e| format!("update auto title failed: {e}"))?;
        emit_tasks_changed(app, task.project_id.clone());
        spawn_codex_thread_title_sync(
            app.clone(),
            task.id.clone(),
            backend.to_string(),
            proposed.clone(),
        );
        persist_title_event(app, &task, backend, turn_id, "success", &proposed, false)?;
    }
    Ok(())
}

#[tauri::command]
pub fn chat_respond_title_update(
    app: AppHandle,
    task_id: String,
    request_id: String,
    decision: String,
) -> Result<(), String> {
    let store = app.state::<LiliaStore>();
    let conn = store.conn()?;
    let event_id = title_event_id(&task_id, &request_id);
    let event = agent_timeline::list(&conn, &task_id)?
        .into_iter()
        .find(|event| event.id == event_id)
        .ok_or_else(|| "标题更新请求已失效".to_string())?;
    if event.kind != TITLE_KIND || event.status != "requires_action" {
        return Ok(());
    }
    let payload = read_payload_record(&event.payload);
    let proposed = payload
        .get("proposedTitle")
        .and_then(|value| value.as_str())
        .and_then(|title| normalize_title(title.to_string()).ok())
        .ok_or_else(|| "标题更新请求缺少候选标题".to_string())?;
    let task = load_task_title_state(&conn, &task_id)?.ok_or_else(|| "任务不存在".to_string())?;
    let accepted = decision == "accept";
    if accepted {
        conn.execute(
            "UPDATE tasks SET title = ?1, title_source = 'manual' WHERE id = ?2 AND archived = 0",
            params![proposed.as_str(), task_id.as_str()],
        )
        .map_err(|e| format!("accept title update failed: {e}"))?;
        emit_tasks_changed(&app, task.project_id.clone());
        spawn_codex_thread_title_sync(
            app.clone(),
            task_id.clone(),
            event.backend.clone(),
            proposed.clone(),
        );
    }

    let status = if accepted { "success" } else { "skipped" };
    let mut next_payload = payload;
    next_payload.insert("accepted".to_string(), JsonValue::Bool(accepted));
    next_payload.insert("decision".to_string(), JsonValue::String(decision));
    persist_and_emit_input(
        &app,
        AgentTimelineEventInput {
            id: Some(event_id),
            task_id,
            turn_id: event.turn_id,
            backend: event.backend,
            kind: TITLE_KIND.to_string(),
            status: status.to_string(),
            title: TITLE_LABEL.to_string(),
            summary: Some(proposed),
            payload: JsonValue::Object(next_payload.into_iter().collect()),
            created_at: Some(event.created_at),
            updated_at: None,
        },
    );
    Ok(())
}

fn spawn_codex_thread_title_sync<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    backend: String,
    title: String,
) {
    if backend != BACKEND_CODEX {
        return;
    }
    thread::spawn(move || {
        if let Err(err) = codex_history::sync_thread_title_blocking(&app, &task_id, &title) {
            eprintln!("[title-update] Codex thread title sync skipped: {err}");
        }
    });
}

fn load_task_title_state(
    conn: &Connection,
    task_id: &str,
) -> Result<Option<TaskTitleState>, String> {
    conn.query_row(
        "SELECT id, project_id, title, title_source FROM tasks WHERE id = ?1 AND archived = 0",
        params![task_id],
        |row| {
            Ok(TaskTitleState {
                id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
                title_source: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("load task title failed: {e}"))
}

fn build_title_prompt(
    conn: &Connection,
    task_id: &str,
    current_title: &str,
) -> Result<Option<String>, String> {
    let samples = load_timeline_samples(conn, task_id)?;
    if samples.is_empty() {
        return Ok(None);
    }
    let mut lines = vec![
        "你是 LiliaCode 的对话标题助手。基于下方最近对话内容生成一个新的中文短标题。".to_string(),
        "只输出标题本身，不要引号、解释、Markdown 或标点包装。".to_string(),
        "标题应概括当前真实任务方向或根因，6 到 18 个中文字，避免“帮我”“请你”等泛词。".to_string(),
        format!(
            "当前标题: {}",
            truncate_chars(&compact_line(current_title), 80)
        ),
    ];
    lines.extend(samples);
    Ok(Some(lines.join("\n")))
}

fn load_timeline_samples(conn: &Connection, task_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT kind, title, summary, payload
               FROM agent_timeline_events
               WHERE task_id = ?1
                 AND kind IN ('message','todo_list','error')
               ORDER BY turn_seq DESC, intra_turn_order DESC
               LIMIT 16"#,
        )
        .map_err(|e| format!("prepare title samples failed: {e}"))?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| format!("query title samples failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (kind, title, summary, payload_text) =
            row.map_err(|e| format!("read title sample failed: {e}"))?;
        let payload = serde_json::from_str::<JsonValue>(&payload_text).unwrap_or(JsonValue::Null);
        let label = if kind == "message" {
            match payload.get("role").and_then(|value| value.as_str()) {
                Some("assistant") => "助手",
                Some("user") => "用户",
                Some("system") => "系统",
                _ => "消息",
            }
        } else if kind == "todo_list" {
            "待办"
        } else {
            "错误"
        };
        let text = payload
            .get("content")
            .and_then(|value| value.as_str())
            .or(summary.as_deref())
            .unwrap_or(title.as_str());
        let text = truncate_chars(&compact_line(text), SAMPLE_TEXT_LIMIT);
        if !text.is_empty() {
            out.push(format!("{label}: {text}"));
        }
    }
    out.reverse();
    Ok(out)
}

fn resolve_model_requests<R: Runtime>(app: &AppHandle<R>, backend: &str) -> Vec<ModelRequest> {
    let mut requests = Vec::new();
    if let Some(request) = codex_spark_model_request(app) {
        requests.push(request);
    }
    if let Some(request) = assistant_ai_model_request(app) {
        requests.push(request);
    }
    if let Some(request) = provider_model_request(app, backend) {
        requests.push(request);
    }
    requests
}

fn codex_spark_model_request<R: Runtime>(app: &AppHandle<R>) -> Option<ModelRequest> {
    if !codex_account_spark_enabled(app) {
        return None;
    }
    Some(ModelRequest {
        backend: BACKEND_CODEX.to_string(),
        model: CODEX_SPARK_MODEL.to_string(),
        base_url: CODEX_SPARK_BASE_URL.to_string(),
        api_key: String::new(),
    })
}

fn is_codex_spark_request(model: &ModelRequest) -> bool {
    is_codex_account_spark_request(Some(&model.backend), &model.model, &model.base_url)
}

fn assistant_ai_model_request<R: Runtime>(app: &AppHandle<R>) -> Option<ModelRequest> {
    let cfg: AssistantAIConfig = load_assistant_ai_config(app);
    let base_url = cfg.base_url?.trim().trim_end_matches('/').to_string();
    let api_key = assistant_ai_secret().ok().flatten()?;
    let model = cfg.model?.trim().to_string();
    if base_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return None;
    }
    Some(ModelRequest {
        backend: BACKEND_CODEX.to_string(),
        model,
        base_url,
        api_key,
    })
}

fn provider_model_request<R: Runtime>(app: &AppHandle<R>, backend: &str) -> Option<ModelRequest> {
    let backend = if backend == BACKEND_CODEX || backend == BACKEND_CLAUDE {
        backend.to_string()
    } else {
        load_active_backend(app)
    };
    let plan: BackendConnectionPlan = resolve_connection_for(app, &backend);
    if plan.mode == ConnectionMode::CodexAccount {
        return None;
    }
    let api_key = plan
        .api_key
        .or_else(|| env::var(backend_api_key_env(&backend)).ok())?
        .trim()
        .to_string();
    if api_key.is_empty() {
        return None;
    }
    let base_url = plan
        .base_url
        .unwrap_or_else(|| backend_direct_url(&backend).to_string())
        .trim()
        .trim_end_matches('/')
        .to_string();
    if base_url.is_empty() {
        return None;
    }
    Some(ModelRequest {
        model: default_model_for_backend(&backend).to_string(),
        backend,
        base_url,
        api_key,
    })
}

fn request_title<R: Runtime>(
    app: &AppHandle<R>,
    model: &ModelRequest,
    prompt: &str,
) -> Result<String, String> {
    if is_codex_spark_request(model) {
        return request_codex_account_spark(app, prompt, TITLE_SPARK_INSTRUCTION)
            .map_err(|err| format!("title Codex Spark request failed: {err}"));
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client failed: {e}"))?;
    if model.backend == BACKEND_CLAUDE {
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
                { "role": "system", "content": "只输出一个中文短标题。" },
                { "role": "user", "content": prompt }
            ],
            "temperature": 0.2,
            "max_tokens": 80
        }))
        .send()
        .map_err(|e| format!("title request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("title request HTTP {}", resp.status()));
    }
    let value = resp
        .json::<JsonValue>()
        .map_err(|e| format!("title response parse failed: {e}"))?;
    value
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| "title response missing content".to_string())
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
            "max_tokens": 80,
            "temperature": 0.2,
            "system": "只输出一个中文短标题。",
            "messages": [{ "role": "user", "content": prompt }]
        }))
        .send()
        .map_err(|e| format!("title request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("title request HTTP {}", resp.status()));
    }
    let value = resp
        .json::<JsonValue>()
        .map_err(|e| format!("title response parse failed: {e}"))?;
    value
        .get("content")
        .and_then(|value| value.as_array())
        .and_then(|items| {
            items
                .iter()
                .find_map(|item| item.get("text").and_then(|value| value.as_str()))
        })
        .map(str::to_string)
        .ok_or_else(|| "title response missing text".to_string())
}

fn persist_title_event<R: Runtime>(
    app: &AppHandle<R>,
    task: &TaskTitleState,
    backend: &str,
    turn_id: Option<&str>,
    status: &str,
    proposed: &str,
    requires_action: bool,
) -> Result<(), String> {
    let request_id = requires_action.then(|| Uuid::new_v4().to_string());
    let id = request_id
        .as_ref()
        .map(|request_id| title_event_id(&task.id, request_id));
    persist_and_emit_input(
        app,
        AgentTimelineEventInput {
            id,
            task_id: task.id.clone(),
            turn_id: turn_id.map(str::to_string),
            backend: backend.to_string(),
            kind: TITLE_KIND.to_string(),
            status: status.to_string(),
            title: TITLE_LABEL.to_string(),
            summary: Some(proposed.to_string()),
            payload: json!({
                "proposedTitle": proposed,
                "previousTitle": task.title,
                "source": if requires_action { "manual-blocked" } else { "auto" },
                "requestId": request_id,
                "accepted": if requires_action { JsonValue::Null } else { JsonValue::Bool(true) },
            }),
            created_at: None,
            updated_at: None,
        },
    );
    Ok(())
}

fn title_event_id(task_id: &str, request_id: &str) -> String {
    format!("title-update:{task_id}:{request_id}")
}

fn read_payload_record(value: &JsonValue) -> serde_json::Map<String, JsonValue> {
    value
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new)
}

fn normalize_title(raw: String) -> Result<String, String> {
    let mut title = raw
        .trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('“')
        .trim_matches('”')
        .trim_matches('《')
        .trim_matches('》')
        .trim()
        .to_string();
    if let Some(stripped) = title
        .strip_prefix("标题：")
        .or_else(|| title.strip_prefix("标题:"))
    {
        title = stripped.trim().to_string();
    }
    title = title
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('“')
        .trim_matches('”')
        .trim_matches('《')
        .trim_matches('》')
        .trim()
        .to_string();
    title = compact_line(&title)
        .trim_end_matches(['。', '.', '，', ',', '；', ';', '：', ':'])
        .to_string();
    title = truncate_chars(&title, TITLE_MAX_CHARS);
    let len = title.chars().count();
    if len < TITLE_MIN_CHARS {
        return Err("title too short".to_string());
    }
    Ok(title)
}

fn compact_line(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(input: &str, max: usize) -> String {
    let mut out = String::new();
    for (index, ch) in input.chars().enumerate() {
        if index >= max {
            return out;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_title_strips_wrappers_and_limits_length() {
        assert_eq!(
            normalize_title("标题：`对话标题事件化实现进度需要继续确认更多内容`".to_string())
                .unwrap(),
            "对话标题事件化实现进度需要继续确认更"
        );
        assert!(normalize_title(" ".to_string()).is_err());
    }
}
