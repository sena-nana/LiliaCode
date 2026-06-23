use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Runtime};

use super::codex_spark::{codex_account_spark_enabled, request_codex_account_spark};
use super::config::{assistant_ai_secret, load_assistant_ai_config};
use super::credentials::normalize_secret;
use super::types::{AssistantAIConfig, AssistantAITestResult};
use crate::chat::types::{ChatAttachment, ChatConversationReference};
use crate::prompt_contract;

const PROMPT_OPTIMIZE_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptOptimizeInput {
    pub(crate) prompt: String,
    #[serde(default)]
    pub(crate) attachments: Vec<ChatAttachment>,
    #[serde(default)]
    pub(crate) conversation_references: Vec<ChatConversationReference>,
}

pub(crate) fn test_connection(mut config: AssistantAIConfig) -> AssistantAITestResult {
    if config
        .api_key
        .as_deref()
        .and_then(normalize_secret)
        .is_none()
    {
        config.api_key = assistant_ai_secret().ok().flatten();
    }
    let base = config
        .base_url
        .as_deref()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/');
    let key = config.api_key.as_deref().unwrap_or("").trim();
    let model = config.model.as_deref().unwrap_or("").trim();
    if base.is_empty() || key.is_empty() || model.is_empty() {
        return AssistantAITestResult {
            ok: false,
            error: Some("baseUrl / apiKey / model 必须全部填写".into()),
            models: None,
            model_matched: None,
        };
    }
    let url = format!("{base}/models");
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return AssistantAITestResult {
                ok: false,
                error: Some(format!("HTTP 客户端构造失败：{e}")),
                models: None,
                model_matched: None,
            }
        }
    };
    match client.get(&url).bearer_auth(key).send() {
        Ok(resp) if resp.status().is_success() => {
            let parsed: Option<Vec<String>> = resp
                .json::<JsonValue>()
                .ok()
                .and_then(|v| v.get("data").cloned())
                .and_then(|d| d.as_array().cloned())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(String::from))
                        .collect()
                });
            let matched = parsed.as_ref().map(|list| list.iter().any(|m| m == model));
            AssistantAITestResult {
                ok: true,
                error: None,
                models: parsed,
                model_matched: matched,
            }
        }
        Ok(resp) => AssistantAITestResult {
            ok: false,
            error: Some(format!("HTTP {} from {url}", resp.status())),
            models: None,
            model_matched: None,
        },
        Err(e) => AssistantAITestResult {
            ok: false,
            error: Some(format!("请求失败：{e}")),
            models: None,
            model_matched: None,
        },
    }
}

pub(crate) fn optimize_prompt<R: Runtime>(
    app: AppHandle<R>,
    input: PromptOptimizeInput,
) -> Result<String, String> {
    let prompt = input.prompt.trim();
    if prompt.is_empty() {
        return Err("提示词为空".to_string());
    }
    let request_prompt = build_prompt_optimize_request(prompt, &input);
    let text = if codex_account_spark_enabled(&app) {
        request_codex_account_spark(
            &app,
            &request_prompt,
            prompt_contract::prompt_optimize_system_instruction(),
        )
        .map_err(|err| format!("提示词优化失败：{err}"))?
    } else {
        let model = assistant_ai_model_request(&app)?;
        request_openai_compatible(&model, &request_prompt)?
    };
    normalize_optimized_prompt(&text)
}

fn assistant_ai_model_request<R: Runtime>(app: &AppHandle<R>) -> Result<AssistantAIConfig, String> {
    let mut cfg = load_assistant_ai_config(app);
    cfg.base_url = cfg
        .base_url
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty());
    cfg.model = cfg
        .model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    cfg.api_key = assistant_ai_secret()?;
    if cfg.base_url.is_none() || cfg.model.is_none() || cfg.api_key.is_none() {
        return Err("辅助模型未配置 Base URL、API key 或模型".to_string());
    }
    Ok(cfg)
}

fn request_openai_compatible(model: &AssistantAIConfig, prompt: &str) -> Result<String, String> {
    let base_url = model
        .base_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/');
    let url = format!("{base_url}/chat/completions");
    let client = Client::builder()
        .timeout(PROMPT_OPTIMIZE_TIMEOUT)
        .build()
        .map_err(|err| format!("辅助模型 HTTP 客户端构造失败：{err}"))?;
    let resp = client
        .post(url)
        .bearer_auth(model.api_key.as_deref().unwrap_or(""))
        .json(&json!({
            "model": model.model,
            "messages": [
                { "role": "system", "content": prompt_contract::prompt_optimize_system_instruction() },
                { "role": "user", "content": prompt }
            ],
            "temperature": 0.2,
            "max_tokens": 900
        }))
        .send()
        .map_err(|err| format!("辅助模型请求失败：{err}"))?;
    if !resp.status().is_success() {
        return Err(format!("辅助模型 HTTP {}", resp.status()));
    }
    let value = resp
        .json::<JsonValue>()
        .map_err(|err| format!("辅助模型响应解析失败：{err}"))?;
    value
        .get("choices")
        .and_then(JsonValue::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| "辅助模型响应缺少 message.content".to_string())
}

fn build_prompt_optimize_request(prompt: &str, input: &PromptOptimizeInput) -> String {
    let attachments = input
        .attachments
        .iter()
        .take(8)
        .map(|item| {
            json!({
                "name": item.name,
                "path": item.path,
                "kind": item.kind,
                "size": item.size,
            })
        })
        .collect::<Vec<_>>();
    let conversation_references = input
        .conversation_references
        .iter()
        .take(8)
        .map(|item| {
            json!({
                "taskId": item.task_id,
                "title": item.title,
                "projectName": item.project_name,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "instruction": prompt_contract::prompt_optimize_request_instruction(),
        "originalPrompt": prompt,
        "attachments": attachments,
        "conversationReferences": conversation_references,
        "requirements": prompt_contract::prompt_optimize_requirements()
    })
    .to_string()
}

fn normalize_optimized_prompt(text: &str) -> Result<String, String> {
    let trimmed = text.trim().trim_matches('`').trim();
    if trimmed.is_empty() {
        return Err("辅助模型返回空提示词".to_string());
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Err("辅助模型返回了非文本结果".to_string());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_ai_requires_base_url_key_and_model() {
        let result = test_connection(AssistantAIConfig {
            base_url: Some("https://example.com/v1".to_string()),
            api_key: None,
            model: Some("model".to_string()),
            codex_account_spark_enabled: false,
            has_api_key: false,
            clear_api_key: false,
        });

        assert!(!result.ok);
        assert_eq!(result.models, None);
        assert_eq!(result.model_matched, None);
        assert!(result.error.unwrap().contains("必须全部填写"));
    }

    #[test]
    fn normalize_optimized_prompt_rejects_empty_text() {
        let err = normalize_optimized_prompt("  \n ").expect_err("empty output should fail");
        assert!(err.contains("空提示词"));
    }

    #[test]
    fn normalize_optimized_prompt_rejects_json_result() {
        let err = normalize_optimized_prompt("{\"prompt\":\"ok\"}").expect_err("json should fail");
        assert!(err.contains("非文本结果"));
    }

    #[test]
    fn build_prompt_optimize_request_keeps_scope_context() {
        let input = PromptOptimizeInput {
            prompt: "修一下输入框按钮".to_string(),
            attachments: vec![ChatAttachment {
                id: "att-1".to_string(),
                name: "ChatComposer.vue".to_string(),
                path: "apps/desktop/src/components/chat/ChatComposer.vue".to_string(),
                kind: "file".to_string(),
                size: Some(42),
                exists: true,
                mime: None,
                directory: None,
            }],
            conversation_references: vec![ChatConversationReference {
                task_id: "task-1".to_string(),
                title: "旧对话".to_string(),
                route: "/projects/lilia/tasks/task-1".to_string(),
                project_id: Some("lilia".to_string()),
                project_name: Some("Lilia".to_string()),
            }],
        };
        let prompt = build_prompt_optimize_request(&input.prompt, &input);

        assert!(prompt.contains("简单定位"));
        assert!(prompt.contains("明确范围"));
        assert!(prompt.contains("ChatComposer.vue"));
        assert!(prompt.contains("旧对话"));
    }
}
