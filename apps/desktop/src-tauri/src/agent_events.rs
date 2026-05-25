use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnContext {
    pub task_id: String,
    pub backend: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentRuntimeEvent {
    ToolUse {
        name: String,
        #[serde(default)]
        input: JsonValue,
    },
    Timeline {
        #[serde(default)]
        event: JsonValue,
    },
    Done {
        session_id: Option<String>,
        subtype: Option<String>,
    },
    Error {
        message: String,
    },
}

impl AgentRuntimeEvent {
    pub fn from_runner_json(value: &JsonValue) -> Option<Self> {
        let ty = value.get("type").and_then(|v| v.as_str())?;
        match ty {
            "tool_use" => {
                let name = value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = value.get("input").cloned().unwrap_or(JsonValue::Null);
                Some(Self::ToolUse { name, input })
            }
            "timeline" => value
                .get("event")
                .cloned()
                .map(|event| Self::Timeline { event }),
            "done" => {
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
            "error" => {
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
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(
                &json!({ "type": "tool_use", "name": "Read", "input": { "file": "a.md" } })
            ),
            Some(AgentRuntimeEvent::ToolUse {
                name: "Read".to_string(),
                input: json!({ "file": "a.md" }),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(
                &json!({ "type": "timeline", "event": { "kind": "tool" } })
            ),
            Some(AgentRuntimeEvent::Timeline {
                event: json!({ "kind": "tool" }),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(
                &json!({ "type": "done", "sessionId": "s1", "subtype": "success" })
            ),
            Some(AgentRuntimeEvent::Done {
                session_id: Some("s1".to_string()),
                subtype: Some("success".to_string()),
            })
        );
        assert_eq!(
            AgentRuntimeEvent::from_runner_json(&json!({ "type": "error", "message": "failed" })),
            Some(AgentRuntimeEvent::Error {
                message: "failed".to_string(),
            })
        );
        // 未知/历史 type 直接降级为 None，runner 端如果发了 `chunk`/`assistant_done`
        // 这类已淘汰的帧也不会让主循环 panic。
        assert!(AgentRuntimeEvent::from_runner_json(
            &json!({ "type": "chunk", "text": "hi" })
        )
        .is_none());
    }
}
