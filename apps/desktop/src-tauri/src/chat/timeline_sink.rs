use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Manager};

use crate::agent_events::{AgentEventEffect, AgentRuntimeEvent, AgentTurnContext};
use crate::agent_timeline;
use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::types::ChatMessage;
use crate::store::LiliaStore;
use crate::util::now_millis;

pub(crate) fn timeline_input_from_runtime_event(
    ctx: &AgentTurnContext,
    event: &AgentRuntimeEvent,
) -> Option<AgentTimelineEventInput> {
    let AgentRuntimeEvent::Timeline { event } = event else {
        return None;
    };
    let Some(obj) = event.as_object() else {
        return None;
    };

    let kind = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("tool")
        .to_string();
    let status = obj
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("info")
        .to_string();
    let title = obj
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(kind.as_str())
        .to_string();
    let summary = obj
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let payload = obj.get("payload").cloned().unwrap_or(JsonValue::Null);
    let source_id = obj.get("sourceId").and_then(|v| v.as_str());
    let turn_id = obj
        .get("turnIdOverride")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| ctx.turn_id.clone());
    let id = source_id.map(|sid| format!("{}:{}:{sid}", ctx.task_id, turn_id));
    let created_at = obj.get("createdAt").and_then(|v| v.as_i64());
    let updated_at = obj.get("updatedAt").and_then(|v| v.as_i64());

    Some(AgentTimelineEventInput {
        id,
        task_id: ctx.task_id.clone(),
        turn_id: Some(turn_id),
        backend: ctx.backend.clone(),
        kind,
        status,
        title,
        summary,
        payload,
        created_at,
        updated_at,
    })
}

pub(crate) fn assistant_error_text(input: &AgentTimelineEventInput) -> Option<String> {
    if input.kind != "message" || !matches!(input.status.as_str(), "error" | "failed") {
        return None;
    }
    let obj = input.payload.as_object()?;
    if obj.get("role").and_then(|v| v.as_str()) != Some("assistant") {
        return None;
    }
    let text = normalize_timeline_text(obj.get("content")?.as_str()?);
    (!text.is_empty()).then_some(text)
}

pub(crate) fn normalize_timeline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 直接落库 + emit 一条 timeline 输入，不做节流。
/// 任何调用方（throttle、用户消息、错误事件）都共用同一条物理路径，
/// 保证「emit 的 payload = DB 写入的快照」始终成立。
pub(crate) fn persist_and_emit_input(app_handle: &AppHandle, input: AgentTimelineEventInput) {
    let store = app_handle.state::<LiliaStore>();
    match store
        .conn()
        .and_then(|conn| agent_timeline::insert(&conn, input))
    {
        Ok(saved) => {
            let _ = app_handle.emit("agent:timeline", saved);
        }
        Err(err) => {
            eprintln!("[agent-timeline] persist failed: {err}");
        }
    }
}

/// 同一 turn 内 timeline 事件的合并节流器。
///
/// 设计目标：UI = Rust 时间线的镜像。前端不维护任何独立的"流式中间态"，
/// 而是直接把每条 `agent:timeline` 事件作为最新快照应用到本地副本上。
/// 这里把同 id 的 event 合并成「最新一帧」按 `interval` 发出，让镜像通道
/// 既能跟上 token 速度，又不被高频小事件淹没 WebView2 IPC。
///
/// 上游（runner pacer）按 33ms emit 流式快照，>16ms 窗口 → throttle 几乎不
/// 缓存任何 pending。剩下的 burst 来源（todo mirror、Codex item 等）靠
/// `submit` 入口的机会式 flush 兜底；`flush_all` 在 turn 末尾收尾最后一帧。
pub(crate) struct TimelineThrottle {
    pub(crate) interval: Duration,
    pub(crate) last_emit_at: HashMap<String, Instant>,
    pub(crate) pending: HashMap<String, AgentTimelineEventInput>,
}

/// 约 60Hz。token 流速度通常 30-50/s，16ms 窗口能放过绝大多数 chunk，
/// 同时给 WebView2 留出聚合空间避免高频 IPC 抖动。
const TIMELINE_EMIT_INTERVAL: Duration = Duration::from_millis(16);

/// 这些 status 视为「此 id 的最终状态」，必须立即 emit + 落库，不进 pending。
pub(crate) fn is_terminal_timeline_status(status: &str) -> bool {
    matches!(
        status,
        "success"
            | "completed"
            | "done"
            | "failed"
            | "error"
            | "cancelled"
            | "skipped"
            | "requires_action"
    )
}

impl TimelineThrottle {
    pub(crate) fn new() -> Self {
        Self {
            interval: TIMELINE_EMIT_INTERVAL,
            last_emit_at: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    pub(crate) fn submit(&mut self, app_handle: &AppHandle, input: AgentTimelineEventInput) {
        let now = Instant::now();
        // 机会式 flush：burst 结束后没有同 id 后续 submit 时，思考末尾帧靠这里
        // 跟着下一条 timeline 事件（典型是 tool_use）被带出去。
        self.flush_due_pending(app_handle, now);

        let Some(id) = input.id.clone() else {
            // 没有稳定 id 的事件每条都是新行，节流无意义。
            persist_and_emit_input(app_handle, input);
            return;
        };

        let terminal = is_terminal_timeline_status(&input.status);
        let due = self
            .last_emit_at
            .get(&id)
            .map_or(true, |t| now.duration_since(*t) >= self.interval);

        if terminal || due {
            persist_and_emit_input(app_handle, input);
            self.last_emit_at.insert(id.clone(), now);
            self.pending.remove(&id);
        } else {
            self.pending.insert(id, input);
        }
    }

    /// 把所有 `last_emit_at + interval <= now` 的 pending 项立即 emit。
    fn flush_due_pending(&mut self, app_handle: &AppHandle, now: Instant) {
        let interval = self.interval;
        let due_ids: Vec<String> = self
            .pending
            .keys()
            .filter(|id| {
                self.last_emit_at
                    .get(*id)
                    .map_or(true, |t| now.duration_since(*t) >= interval)
            })
            .cloned()
            .collect();
        for id in due_ids {
            if let Some(input) = self.pending.remove(&id) {
                persist_and_emit_input(app_handle, input);
                self.last_emit_at.insert(id, now);
            }
        }
    }

    pub(crate) fn flush_all(&mut self, app_handle: &AppHandle) {
        let drained: Vec<_> = self.pending.drain().collect();
        for (_id, input) in drained {
            persist_and_emit_input(app_handle, input);
        }
    }
}

pub(crate) fn persist_and_emit_message_timeline_event(
    app_handle: &AppHandle,
    message: &ChatMessage,
    backend: &str,
    turn_id: &str,
    queued: bool,
) {
    let input = AgentTimelineEventInput {
        id: Some(message.id.clone()),
        task_id: message.task_id.clone(),
        // user message 与它触发的 agent turn 共享 turn_id，所以两者会被分到同一个
        // turn_seq，user 消息天然落在 turn 内部第一位（intra_turn_order=0）。
        turn_id: Some(turn_id.to_string()),
        backend: backend.to_string(),
        kind: "message".to_string(),
        status: if queued { "pending" } else { "success" }.to_string(),
        title: "用户输入".to_string(),
        summary: Some(message.content.clone()),
        payload: serde_json::json!({
            "role": message.role,
            "content": message.content,
            "attachments": message.attachments,
            "queued": queued,
        }),
        created_at: Some(message.created_at as i64),
        updated_at: Some(now_millis() as i64),
    };

    persist_and_emit_input(app_handle, input);
}

pub(crate) fn persist_and_emit_error_timeline_event(
    app_handle: &AppHandle,
    task_id: &str,
    backend: &str,
    turn_id: Option<&str>,
    message: String,
) {
    let now = now_millis();
    let input = AgentTimelineEventInput {
        id: None,
        task_id: task_id.to_string(),
        turn_id: turn_id.map(|id| id.to_string()),
        backend: backend.to_string(),
        kind: "error".to_string(),
        status: "error".to_string(),
        title: "错误".to_string(),
        summary: Some(message.clone()),
        payload: serde_json::json!({ "message": message }),
        created_at: Some(now as i64),
        updated_at: Some(now as i64),
    };

    persist_and_emit_input(app_handle, input);
}

pub(crate) fn log_agent_event_effect(effect: AgentEventEffect) {
    for err in effect.errors {
        eprintln!(
            "[agent-event] extension {} failed: {}",
            err.extension_id, err.message
        );
    }
    if !effect.context_candidates.is_empty() {
        eprintln!(
            "[agent-event] collected {} context candidate(s)",
            effect.context_candidates.len()
        );
    }
}
