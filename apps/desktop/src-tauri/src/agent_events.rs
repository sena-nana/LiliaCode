use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::agent_interaction_contract;
use crate::runner_protocol_contract::{self, RunnerControlMessageTypes, RunnerRuntimeEventTypes};

fn runner_runtime_event_types() -> &'static RunnerRuntimeEventTypes {
    runner_protocol_contract::runner_runtime_event_types()
}

fn runner_control_message_types() -> &'static RunnerControlMessageTypes {
    runner_protocol_contract::runner_control_message_types()
}

pub(crate) fn runner_interaction_response_control_type() -> &'static str {
    runner_control_message_types().interaction_response.as_str()
}

pub(crate) fn runner_settings_update_control_type() -> &'static str {
    runner_control_message_types().settings_update.as_str()
}

pub(crate) fn runner_interrupt_turn_control_type() -> &'static str {
    runner_control_message_types().interrupt_turn.as_str()
}

pub(crate) fn runner_quota_usage_result_control_type() -> &'static str {
    runner_control_message_types().quota_usage_result.as_str()
}

pub(crate) fn runner_lilia_iab_result_control_type() -> &'static str {
    runner_control_message_types().lilia_iab_result.as_str()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnContext {
    pub task_id: String,
    pub backend: String,
    pub turn_id: String,
    #[serde(default)]
    pub automation_run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentRuntimeEvent {
    ToolUse {
        name: String,
        #[serde(default)]
        input: JsonValue,
    },
    TodoList {
        #[serde(default)]
        items: Vec<JsonValue>,
    },
    Timeline {
        #[serde(default)]
        event: JsonValue,
    },
    InteractionRequest {
        id: String,
        kind: String,
        #[serde(default)]
        backend: Option<String>,
        #[serde(default)]
        payload: JsonValue,
    },
    QuotaUsageRequest {
        id: String,
        #[serde(default)]
        payload: JsonValue,
    },
    ContextUsage {
        used_tokens: u64,
        #[serde(default)]
        limit_tokens: Option<u64>,
        #[serde(default)]
        used_percent: Option<f64>,
        #[serde(default)]
        source: Option<String>,
        #[serde(default)]
        unavailable_reason: Option<String>,
    },
    Done {
        session_id: Option<String>,
        subtype: Option<String>,
    },
    PromptSuggestion {
        suggestion: String,
        uuid: Option<String>,
    },
    Error {
        message: String,
    },
}

impl AgentRuntimeEvent {
    pub fn from_runner_json(value: &JsonValue) -> Option<Self> {
        let ty = value.get("type").and_then(|v| v.as_str())?;
        let types = runner_runtime_event_types();
        match ty {
            ty if ty == types.tool_use.as_str() => {
                let name = value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = value.get("input").cloned().unwrap_or(JsonValue::Null);
                Some(Self::ToolUse { name, input })
            }
            ty if ty == types.todo_list.as_str() => {
                let items = value
                    .get("items")
                    .or_else(|| value.get("todos"))
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                Some(Self::TodoList { items })
            }
            ty if ty == types.timeline.as_str() => value
                .get("event")
                .cloned()
                .map(|event| Self::Timeline { event }),
            ty if ty == types.interaction_request.as_str() => {
                let id = value.get("id").and_then(|v| v.as_str())?.to_string();
                let kind = value
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| agent_interaction_contract::ask_user_interaction_kind())
                    .to_string();
                let backend = value
                    .get("backend")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                let payload = value.get("payload").cloned().unwrap_or(JsonValue::Null);
                Some(Self::InteractionRequest {
                    id,
                    kind,
                    backend,
                    payload,
                })
            }
            ty if ty == types.quota_usage_request.as_str() => {
                let id = value.get("id").and_then(|v| v.as_str())?.to_string();
                let payload = value.get("payload").cloned().unwrap_or(JsonValue::Null);
                Some(Self::QuotaUsageRequest { id, payload })
            }
            ty if ty == types.context_usage.as_str() => {
                let used_tokens = json_u64_field(value, &["usedTokens", "used_tokens"])?;
                let limit_tokens = json_u64_field(value, &["limitTokens", "limit_tokens"]);
                let used_percent = json_f64_field(value, &["usedPercent", "used_percent"]);
                let source = value
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map(|source| source.trim().to_string())
                    .filter(|source| !source.is_empty());
                let unavailable_reason = value
                    .get("unavailableReason")
                    .or_else(|| value.get("unavailable_reason"))
                    .and_then(|v| v.as_str())
                    .map(|reason| reason.trim().to_string())
                    .filter(|reason| !reason.is_empty());
                Some(Self::ContextUsage {
                    used_tokens,
                    limit_tokens,
                    used_percent,
                    source,
                    unavailable_reason,
                })
            }
            ty if ty == types.done.as_str() => {
                let session_id = value
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .map(|sid| sid.to_string());
                let subtype = value
                    .get("subtype")
                    .and_then(|v| v.as_str())
                    .map(|subtype| subtype.to_string());
                Some(Self::Done {
                    session_id,
                    subtype,
                })
            }
            ty if ty == types.prompt_suggestion.as_str() => {
                let suggestion = value
                    .get("suggestion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if suggestion.trim().is_empty() {
                    return None;
                }
                let uuid = value
                    .get("uuid")
                    .and_then(|v| v.as_str())
                    .map(|uuid| uuid.to_string());
                Some(Self::PromptSuggestion { suggestion, uuid })
            }
            ty if ty == types.error.as_str() => {
                let message = value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知错误")
                    .to_string();
                Some(Self::Error { message })
            }
            _ => None,
        }
    }

    pub(crate) fn event_type(&self) -> &'static str {
        let types = runner_runtime_event_types();
        match self {
            Self::ToolUse { .. } => &types.tool_use,
            Self::TodoList { .. } => &types.todo_list,
            Self::Timeline { .. } => &types.timeline,
            Self::InteractionRequest { .. } => &types.interaction_request,
            Self::QuotaUsageRequest { .. } => &types.quota_usage_request,
            Self::ContextUsage { .. } => &types.context_usage,
            Self::Done { .. } => &types.done,
            Self::PromptSuggestion { .. } => &types.prompt_suggestion,
            Self::Error { .. } => &types.error,
        }
    }
}

fn json_u64_field(value: &JsonValue, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_i64().and_then(|n| u64::try_from(n).ok()))
        })
    })
}

fn json_f64_field(value: &JsonValue, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|v| v.as_f64()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextCandidate {
    pub source: String,
    pub content: String,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventError {
    pub extension_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventEffect {
    #[serde(default)]
    pub context_candidates: Vec<AgentContextCandidate>,
    #[serde(default)]
    pub errors: Vec<AgentEventError>,
}

impl AgentEventEffect {
    pub fn is_empty(&self) -> bool {
        self.context_candidates.is_empty() && self.errors.is_empty()
    }

    fn append(&mut self, mut other: AgentEventEffect) {
        self.context_candidates
            .append(&mut other.context_candidates);
        self.errors.append(&mut other.errors);
    }
}

pub trait AgentExtension: Send + Sync {
    fn id(&self) -> &'static str;

    fn enabled(&self) -> bool {
        true
    }

    fn on_event(
        &self,
        ctx: &AgentTurnContext,
        event: &AgentRuntimeEvent,
    ) -> Result<AgentEventEffect, String>;
}

#[derive(Default)]
pub struct AgentEventHost {
    extensions: Vec<Box<dyn AgentExtension>>,
}

impl AgentEventHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, extension: Box<dyn AgentExtension>) {
        self.extensions.push(extension);
    }

    pub fn dispatch(&self, ctx: &AgentTurnContext, event: &AgentRuntimeEvent) -> AgentEventEffect {
        let mut effect = AgentEventEffect::default();
        for extension in &self.extensions {
            if !extension.enabled() {
                continue;
            }
            match extension.on_event(ctx, event) {
                Ok(next) => effect.append(next),
                Err(message) => effect.errors.push(AgentEventError {
                    extension_id: extension.id().to_string(),
                    message,
                }),
            }
        }
        effect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct RecordingExtension {
        id: &'static str,
        calls: Arc<Mutex<Vec<&'static str>>>,
        enabled: bool,
        effect: AgentEventEffect,
        error: Option<String>,
    }

    impl RecordingExtension {
        fn new(id: &'static str, calls: Arc<Mutex<Vec<&'static str>>>) -> Self {
            Self {
                id,
                calls,
                enabled: true,
                effect: AgentEventEffect::default(),
                error: None,
            }
        }

        fn disabled(mut self) -> Self {
            self.enabled = false;
            self
        }

        fn with_context_candidate(mut self, content: &str) -> Self {
            self.effect.context_candidates.push(AgentContextCandidate {
                source: self.id.to_string(),
                content: content.to_string(),
                priority: 10,
            });
            self
        }

        fn with_error(mut self, message: &str) -> Self {
            self.error = Some(message.to_string());
            self
        }
    }

    impl AgentExtension for RecordingExtension {
        fn id(&self) -> &'static str {
            self.id
        }

        fn enabled(&self) -> bool {
            self.enabled
        }

        fn on_event(
            &self,
            _ctx: &AgentTurnContext,
            _event: &AgentRuntimeEvent,
        ) -> Result<AgentEventEffect, String> {
            self.calls.lock().unwrap().push(self.id);
            if let Some(message) = &self.error {
                return Err(message.clone());
            }
            Ok(self.effect.clone())
        }
    }

    fn turn_context() -> AgentTurnContext {
        AgentTurnContext {
            task_id: "task-1".to_string(),
            backend: "claude".to_string(),
            turn_id: "turn-1".to_string(),
            automation_run_id: None,
        }
    }

    #[test]
    fn dispatch_invokes_extensions_in_registration_order() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut host = AgentEventHost::new();
        host.register(Box::new(RecordingExtension::new("first", calls.clone())));
        host.register(Box::new(RecordingExtension::new("second", calls.clone())));

        let effect = host.dispatch(
            &turn_context(),
            &AgentRuntimeEvent::Timeline {
                event: json!({ "kind": "reasoning" }),
            },
        );

        assert_eq!(&*calls.lock().unwrap(), &["first", "second"]);
        assert!(effect.errors.is_empty());
    }

    #[test]
    fn disabled_or_unregistered_extensions_do_not_run() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut host = AgentEventHost::new();
        host.register(Box::new(
            RecordingExtension::new("disabled", calls.clone()).disabled(),
        ));

        let effect = host.dispatch(
            &turn_context(),
            &AgentRuntimeEvent::Done {
                session_id: None,
                subtype: None,
            },
        );

        assert!(calls.lock().unwrap().is_empty());
        assert!(effect.is_empty());

        let empty_host = AgentEventHost::new();
        let effect = empty_host.dispatch(
            &turn_context(),
            &AgentRuntimeEvent::Done {
                session_id: None,
                subtype: None,
            },
        );
        assert!(effect.is_empty());
    }

    #[test]
    fn handler_failure_returns_structured_error_and_continues() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut host = AgentEventHost::new();
        host.register(Box::new(
            RecordingExtension::new("broken", calls.clone()).with_error("boom"),
        ));
        host.register(Box::new(RecordingExtension::new("after", calls.clone())));

        let effect = host.dispatch(
            &turn_context(),
            &AgentRuntimeEvent::Error {
                message: "runner failed".to_string(),
            },
        );

        assert_eq!(&*calls.lock().unwrap(), &["broken", "after"]);
        assert_eq!(effect.errors.len(), 1);
        assert_eq!(effect.errors[0].extension_id, "broken");
        assert_eq!(effect.errors[0].message, "boom");
    }

    #[test]
    fn tool_use_extensions_can_submit_context_candidates() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut host = AgentEventHost::new();
        host.register(Box::new(
            RecordingExtension::new("memory", calls).with_context_candidate("prior context"),
        ));

        let effect = host.dispatch(
            &turn_context(),
            &AgentRuntimeEvent::ToolUse {
                name: "Read".to_string(),
                input: json!({ "file": "README.md" }),
            },
        );

        assert_eq!(effect.context_candidates.len(), 1);
        assert_eq!(effect.context_candidates[0].source, "memory");
        assert_eq!(effect.context_candidates[0].content, "prior context");
    }

    #[test]
    fn runner_json_is_normalized_to_runtime_events() {
        let types = runner_runtime_event_types();
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(
                &json!({ "type": types.tool_use, "name": "Read", "input": { "file": "a.md" } })
            ),
            Some(AgentRuntimeEvent::ToolUse {
                name: "Read".to_string(),
                input: json!({ "file": "a.md" }),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(
                &json!({ "type": types.timeline, "event": { "kind": "tool" } })
            ),
            Some(AgentRuntimeEvent::Timeline {
                event: json!({ "kind": "tool" }),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(&json!({
                "type": types.todo_list,
                "items": [
                    { "text": "Mirror provider todo", "completed": true },
                    { "content": "Keep Claude compatibility", "status": "pending" }
                ]
            })),
            Some(AgentRuntimeEvent::TodoList {
                items: vec![
                    json!({ "text": "Mirror provider todo", "completed": true }),
                    json!({ "content": "Keep Claude compatibility", "status": "pending" })
                ],
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(&json!({
                "type": types.interaction_request,
                "id": "ask-1",
                "kind": agent_interaction_contract::ask_user_interaction_kind(),
                "backend": "codex",
                "payload": {
                    "title": "Codex 想确认一下",
                    "questions": []
                }
            })),
            Some(AgentRuntimeEvent::InteractionRequest {
                id: "ask-1".to_string(),
                kind: agent_interaction_contract::ask_user_interaction_kind().to_string(),
                backend: Some("codex".to_string()),
                payload: json!({
                    "title": "Codex 想确认一下",
                    "questions": []
                }),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(&json!({
                "type": types.quota_usage_request,
                "id": "quota-1",
                "payload": { "days": 7, "scope": "tools" }
            })),
            Some(AgentRuntimeEvent::QuotaUsageRequest {
                id: "quota-1".to_string(),
                payload: json!({ "days": 7, "scope": "tools" }),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(&json!({
                "type": types.context_usage,
                "usedTokens": 4096,
                "limitTokens": 8192,
                "usedPercent": 50.0,
                "source": "runtime"
            })),
            Some(AgentRuntimeEvent::ContextUsage {
                used_tokens: 4096,
                limit_tokens: Some(8192),
                used_percent: Some(50.0),
                source: Some("runtime".to_string()),
                unavailable_reason: None,
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(
                &json!({ "type": types.done, "sessionId": "s1", "subtype": "success" })
            ),
            Some(AgentRuntimeEvent::Done {
                session_id: Some("s1".to_string()),
                subtype: Some("success".to_string()),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(&json!({
                "type": types.prompt_suggestion,
                "suggestion": "请继续检查 Claude 原生建议展示。",
                "uuid": "suggestion-1"
            })),
            Some(AgentRuntimeEvent::PromptSuggestion {
                suggestion: "请继续检查 Claude 原生建议展示。".to_string(),
                uuid: Some("suggestion-1".to_string()),
            })
        );
        assert!(AgentRuntimeEvent::from_runner_json(&json!({
            "type": types.prompt_suggestion,
            "suggestion": " "
        }))
        .is_none());
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(
                &json!({ "type": types.error, "message": "failed" })
            ),
            Some(AgentRuntimeEvent::Error {
                message: "failed".to_string(),
            })
        );
        // 未知/历史 type 直接降级为 None，runner 端如果发了 `chunk`/`assistant_done`
        // 这类已淘汰的帧也不会让主循环 panic。
        assert!(
            AgentRuntimeEvent::from_runner_json(&json!({ "type": "chunk", "text": "hi" }))
                .is_none()
        );
    }

    #[test]
    fn runner_control_message_types_are_loaded_from_protocol_contract() {
        let types = runner_control_message_types();

        assert_eq!(
            runner_interaction_response_control_type(),
            types.interaction_response.as_str()
        );
        assert_eq!(
            runner_settings_update_control_type(),
            types.settings_update.as_str()
        );
        assert_eq!(
            runner_interrupt_turn_control_type(),
            types.interrupt_turn.as_str()
        );
        assert_eq!(
            runner_quota_usage_result_control_type(),
            types.quota_usage_result.as_str()
        );
        assert_eq!(
            runner_lilia_iab_result_control_type(),
            types.lilia_iab_result.as_str()
        );
    }
}
