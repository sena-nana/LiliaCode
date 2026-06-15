use std::time::Duration;

use reqwest::blocking::Client;
use serde_json::{json, Value as JsonValue};
use tauri::AppHandle;

use crate::provider::{
    assistant_ai_secret, backend_api_key_env, load_active_backend, load_assistant_ai_config,
    resolve_connection_for, AssistantAIConfig, BackendConnectionPlan, ConnectionMode,
};
use crate::{BACKEND_CLAUDE, BACKEND_CODEX, CODEX_MODEL_OPTIONS};

use super::types::{ModelRequest, SuggestionSettings, SuggestionSource};

pub(super) fn resolve_model_request(
    app: &AppHandle,
    settings: &SuggestionSettings,
) -> Option<ModelRequest> {
    match settings.source {
        SuggestionSource::AssistantAi => assistant_ai_model_request(app),
        SuggestionSource::Provider => provider_model_request(app),
    }
}

fn assistant_ai_model_request(app: &AppHandle) -> Option<ModelRequest> {
    let cfg: AssistantAIConfig = load_assistant_ai_config(app);
    let base_url = cfg.base_url?.trim().trim_end_matches('/').to_string();
    let api_key = assistant_ai_secret().ok().flatten()?;
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
    if plan.mode == ConnectionMode::CodexAccount {
        return None;
    }
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

pub(super) fn request_model(model: &ModelRequest, prompt: &str) -> Result<String, String> {
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
