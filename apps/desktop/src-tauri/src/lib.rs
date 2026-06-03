use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose, Engine as _};
use ignore::WalkBuilder;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{
    utils::config::Color, AppHandle, Emitter, Manager, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

pub mod agent_events;
mod agent_extensions;
pub mod agent_timeline;
mod plugins;
mod projects_tasks;
mod store;
mod todos;
mod util;
mod window_state;
use agent_events::{AgentEventEffect, AgentEventHost, AgentRuntimeEvent, AgentTurnContext};
use agent_extensions::TodoMirrorExtension;
use agent_timeline::AgentTimelineEventInput;
use plugins::{
    ClaudeMcpServer, ClaudeMcpServerInput, ClaudePlugin, ClaudeSkill, CodexMcpServer,
    PluginsOverview,
};
use store::LiliaStore;

const MAIN_WINDOW_LABEL: &str = "main";

// 始终使用暗色：与前端 CSS 变量 --bg = #181818 保持一致，避免 Windows 拉伸/还原时
// 露出 WebView 之外的默认白底。
const BG: Color = Color(0x18, 0x18, 0x18, 0xFF);

// CC-Switch 桌面端的本地代理端口（cc-switch src-tauri/src/proxy/types.rs: listen_port: 15721）。
const CC_SWITCH_DEFAULT_URL: &str = "http://127.0.0.1:15721";

/// 真实 key 由 CC-Switch 注入；这里只需要任意非空字符串让 SDK 通过本地校验。
const CC_SWITCH_PLACEHOLDER_KEY: &str = "sk-cc-switch-proxy";

const PROVIDER_STORE_FILE: &str = "provider-config.json";
const PROVIDER_ACTIVE_BACKEND_KEY: &str = "provider.activeBackend";
const PROVIDER_KEY_CLAUDE: &str = "provider.claude";
const PROVIDER_KEY_CODEX: &str = "provider.codex";
const CC_SWITCH_KEY: &str = "cc-switch.config";
const ROUTER_KEY_CLAUDE: &str = "router.claude";
const ROUTER_KEY_CODEX: &str = "router.codex";
const ASSISTANT_AI_KEY: &str = "assistant-ai.config";
const AGENT_INTERACTION_KEY: &str = "agent-interaction.config";
/// 「添加项目 → 从 GitHub clone」时默认 clone 到的父目录。
const PROJECT_CLONE_PARENT_KEY: &str = "project.cloneParentDir";

const ROUTER_CC_SWITCH: &str = "cc-switch";
const ROUTER_DIRECT: &str = "direct";

const BACKEND_CLAUDE: &str = "claude";
const BACKEND_CODEX: &str = "codex";
const CODEX_MODEL_OPTIONS: [(&str, &str); 3] = [
    ("gpt-5.5", "GPT-5.5"),
    ("gpt-5.4", "GPT-5.4"),
    ("gpt-5.4-mini", "GPT-5.4 Mini"),
];
const MIN_CODEX_APP_SERVER_VERSION: (u32, u32, u32) = (0, 128, 0);
static CLIPBOARD_IMAGE_DISPLAY_SEQ: AtomicU64 = AtomicU64::new(0);
static CLIPBOARD_TEXT_DISPLAY_SEQ: AtomicU64 = AtomicU64::new(0);
static POPUP_WINDOW_SEQ: AtomicU64 = AtomicU64::new(0);

const POPUP_WINDOW_SETTINGS_KEY: &str = "popup-window.config";
const POPUP_LAST_PROJECT_KEY: &str = "popup-window.lastProjectId";
const POPUP_WINDOW_PREFIX: &str = "popup-";
const POPUP_EXISTING_TASK_PREFIX: &str = "popup-task-";
const POPUP_WIDTH: f64 = 430.0;
const POPUP_HEIGHT: f64 = 760.0;
const POPUP_MIN_WIDTH: f64 = 360.0;
const POPUP_MIN_HEIGHT: f64 = 520.0;

// ---------- 契约（与 packages/contracts 同形） ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    id: String,
    task_id: String,
    role: String, // "user" | "assistant" | "system"
    content: String,
    attachments: Vec<ChatAttachment>,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatAttachment {
    id: String,
    name: String,
    path: String,
    kind: String,
    size: Option<u64>,
    #[serde(default = "default_attachment_exists")]
    exists: bool,
    #[serde(default)]
    mime: Option<String>,
    #[serde(default)]
    directory: Option<ChatAttachmentDirectoryMeta>,
}

fn default_attachment_exists() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatAttachmentDirectoryMeta {
    file_count: u64,
    directory_count: u64,
    total_size: u64,
    truncated: bool,
    unreadable_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatContextSearchResult {
    attachment: ChatAttachment,
    relative_path: String,
    matched_by: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipboardImageInput {
    mime: Option<String>,
    bytes_base64: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipboardTextInput {
    text: String,
}

#[derive(Debug, Clone)]
struct PendingChatTurn {
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    message: ChatMessage,
    /// queue 时就分配好 turn_id，user message + agent turn 共享同一个 turn_id
    /// → 同一个 turn_seq；这是把"按 turn 隔离"的排序契约推到入口的关键。
    turn_id: String,
    guide_id: Option<String>,
}

#[derive(Debug, Clone)]
struct RunningTurn {
    turn_id: String,
    backend: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatSendResult {
    message: ChatMessage,
    /// "started" | "queued"
    dispatch: String,
    queued_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatComposerState {
    task_id: String,
    /// "claude" | "codex"
    backend: String,
    model: String,
    #[serde(default)]
    plan_mode: bool,
    /// "full" | "ask" | "readonly"
    permission: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatModelOption {
    id: String,
    label: String,
    backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentInteractionSettings {
    #[serde(default)]
    non_interrupt_mode: bool,
    #[serde(default)]
    debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ProviderConfig {
    backend: String,
    base_url: Option<String>,
    api_key: Option<String>,
}

/// 项目相关偏好。当前只有 git clone 的默认父目录。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ProjectSettings {
    clone_parent_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PopupWindowSettings {
    shortcut: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MainNavigateEvent {
    route: String,
}

/// CC-Switch 代理层配置。Claude 与 Codex 共用同一个 baseUrl。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CCSwitchConfig {
    base_url: Option<String>,
}

impl Default for CCSwitchConfig {
    fn default() -> Self {
        CCSwitchConfig {
            base_url: Some(CC_SWITCH_DEFAULT_URL.to_string()),
        }
    }
}

/// 辅助模型（Assistant AI）配置。独立于 ProviderConfig，**不参与 Agent 主循环**，
/// 仅供周边系统（Memory 助手、Tool Call 后处理、摘要等）消费。
/// 三件套齐全才算启用；任一缺失消费方应 short-circuit 跳过。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AssistantAIConfig {
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssistantAITestResult {
    ok: bool,
    error: Option<String>,
    /// 端点不支持 /models 时为 None，UI 据此降级提示。
    models: Option<Vec<String>>,
    /// 配置里的 model 是否出现在 models 列表里。models 为 None 时也为 None。
    model_matched: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendEnvStatus {
    backend: String,
    has_api_key: bool,
    /// "cc-switch" | "custom" | "direct" | "unconfigured"
    connection_mode: String,
    effective_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CCSwitchStatus {
    reachable: bool,
    base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerStatus {
    version: Option<String>,
    available: bool,
    supports_required_protocol: bool,
    issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvStatusReport {
    node_available: bool,
    /// codex CLI 是否能在 PATH 找到。
    codex_cli_available: bool,
    /// Codex app-server / dynamic tool / AskUser 所需能力检查。
    codex_app_server: CodexAppServerStatus,
    cc_switch: CCSwitchStatus,
    /// 每个 backend 当前生效的路由模式（"cc-switch" | "direct"）。
    router_modes: HashMap<String, String>,
    backends: HashMap<String, BackendEnvStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnStartedEvent {
    task_id: String,
    queued_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    task_id: String,
    session_id: Option<String>,
    subtype: Option<String>,
}

/// 工具调用授权请求：runner 调用 canUseTool 时通过 stdout 转过来的事实字段。
/// 前端 ToolConsentBridge 收到后弹 AskUser 浮层，再用 chat_respond_tool_consent
/// 把决策写回。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolConsentRequestEvent {
    task_id: String,
    turn_id: String,
    backend: String,
    request_id: String,
    tool_name: String,
    input: JsonValue,
    title: Option<String>,
    display_name: Option<String>,
    description: Option<String>,
    blocked_path: Option<String>,
    decision_reason: Option<String>,
    tool_use_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AskUserRequestEvent {
    task_id: String,
    turn_id: String,
    backend: String,
    request_id: String,
    spec: JsonValue,
}

fn timeline_input_from_runtime_event(
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
    let id = source_id.map(|sid| format!("{}:{}:{sid}", ctx.task_id, ctx.turn_id));

    Some(AgentTimelineEventInput {
        id,
        task_id: ctx.task_id.clone(),
        turn_id: Some(ctx.turn_id.clone()),
        backend: ctx.backend.clone(),
        kind,
        status,
        title,
        summary,
        payload,
        created_at: None,
        updated_at: None,
    })
}

fn assistant_error_text(input: &AgentTimelineEventInput) -> Option<String> {
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

fn normalize_timeline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 直接落库 + emit 一条 timeline 输入，不做节流。
/// 任何调用方（throttle、用户消息、错误事件）都共用同一条物理路径，
/// 保证「emit 的 payload = DB 写入的快照」始终成立。
fn persist_and_emit_input(app_handle: &AppHandle, input: AgentTimelineEventInput) {
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
struct TimelineThrottle {
    interval: Duration,
    last_emit_at: HashMap<String, Instant>,
    pending: HashMap<String, AgentTimelineEventInput>,
}

/// 约 60Hz。token 流速度通常 30-50/s，16ms 窗口能放过绝大多数 chunk，
/// 同时给 WebView2 留出聚合空间避免高频 IPC 抖动。
const TIMELINE_EMIT_INTERVAL: Duration = Duration::from_millis(16);

/// 这些 status 视为「此 id 的最终状态」，必须立即 emit + 落库，不进 pending。
fn is_terminal_timeline_status(status: &str) -> bool {
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
    fn new() -> Self {
        Self {
            interval: TIMELINE_EMIT_INTERVAL,
            last_emit_at: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    fn submit(&mut self, app_handle: &AppHandle, input: AgentTimelineEventInput) {
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

    fn flush_all(&mut self, app_handle: &AppHandle) {
        let drained: Vec<_> = self.pending.drain().collect();
        for (_id, input) in drained {
            persist_and_emit_input(app_handle, input);
        }
    }
}

fn persist_and_emit_message_timeline_event(
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

fn persist_and_emit_error_timeline_event(
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

fn log_agent_event_effect(effect: AgentEventEffect) {
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

#[cfg(test)]
mod agent_event_sink_tests {
    use super::*;
    use serde_json::json;

    fn turn_context() -> AgentTurnContext {
        AgentTurnContext {
            task_id: "task-1".to_string(),
            backend: BACKEND_CLAUDE.to_string(),
            turn_id: "turn-1".to_string(),
        }
    }

    #[test]
    fn timeline_runtime_event_maps_to_timeline_input() {
        let input = timeline_input_from_runtime_event(
            &turn_context(),
            &AgentRuntimeEvent::Timeline {
                event: json!({
                    "kind": "command",
                    "status": "success",
                    "title": "Run tests",
                    "summary": "17 passed",
                    "payload": { "command": "cargo test" },
                    "sourceId": "cargo-test"
                }),
            },
        )
        .unwrap();

        assert_eq!(input.id, Some("task-1:turn-1:cargo-test".to_string()));
        assert_eq!(input.task_id, "task-1");
        assert_eq!(input.turn_id, Some("turn-1".to_string()));
        assert_eq!(input.backend, BACKEND_CLAUDE);
        assert_eq!(input.kind, "command");
        assert_eq!(input.status, "success");
        assert_eq!(input.title, "Run tests");
        assert_eq!(input.summary, Some("17 passed".to_string()));
        assert_eq!(input.payload, json!({ "command": "cargo test" }));
        assert_eq!(input.created_at, None);
        assert_eq!(input.updated_at, None);
    }

    #[test]
    fn timeline_runtime_event_without_payload_still_maps() {
        // runner 不再产出 display；事件缺 payload 时也要兜底为 null 而非被丢弃。
        let input = timeline_input_from_runtime_event(
            &turn_context(),
            &AgentRuntimeEvent::Timeline {
                event: json!({
                    "kind": "reasoning",
                    "status": "running",
                    "title": "思考中"
                }),
            },
        )
        .unwrap();

        assert_eq!(input.kind, "reasoning");
        assert_eq!(input.status, "running");
        assert_eq!(input.title, "思考中");
        assert_eq!(input.payload, JsonValue::Null);
    }

    #[test]
    fn runner_error_duplicate_detection_uses_assistant_error_message() {
        let input = timeline_input_from_runtime_event(
            &turn_context(),
            &AgentRuntimeEvent::Timeline {
                event: json!({
                    "kind": "message",
                    "status": "error",
                    "title": "Assistant",
                    "payload": {
                        "role": "assistant",
                        "content": "API Error: 503 所有供应商已熔断，无可用渠道."
                    }
                }),
            },
        )
        .unwrap();
        let assistant_text = assistant_error_text(&input);

        assert_eq!(
            assistant_text.as_deref(),
            Some("API Error: 503 所有供应商已熔断，无可用渠道.")
        );
        let assistant_text = assistant_text.unwrap();
        assert!(normalize_timeline_text(
            "Claude Code returned an error result: API Error: 503 所有供应商已熔断，无可用渠道.",
        )
        .contains(&assistant_text));
        assert!(!normalize_timeline_text("无法启动 node 子进程").contains(&assistant_text));
    }

    #[test]
    fn non_object_timeline_event_is_ignored() {
        let input = timeline_input_from_runtime_event(
            &turn_context(),
            &AgentRuntimeEvent::Timeline {
                event: json!("not an object"),
            },
        );

        assert!(input.is_none());
    }

    #[test]
    fn non_timeline_runtime_event_is_not_a_timeline_input() {
        let input = timeline_input_from_runtime_event(
            &turn_context(),
            &AgentRuntimeEvent::ToolUse {
                name: "Read".to_string(),
                input: json!({ "file": "README.md" }),
            },
        );

        assert!(input.is_none());
    }

    #[test]
    fn chat_message_ids_do_not_reset_to_counter_values() {
        let first = new_chat_message_id();
        let second = new_chat_message_id();

        assert!(first.starts_with("u-"));
        assert!(second.starts_with("u-"));
        assert_ne!(first, second);
        assert_ne!(first, "u-0");
    }

    #[test]
    fn active_backend_normalizes_unknown_values_to_claude() {
        assert_eq!(normalize_backend(BACKEND_CLAUDE), BACKEND_CLAUDE);
        assert_eq!(normalize_backend(BACKEND_CODEX), BACKEND_CODEX);
        assert_eq!(normalize_backend(""), BACKEND_CLAUDE);
        assert_eq!(normalize_backend("unknown"), BACKEND_CLAUDE);
    }

    #[test]
    fn codex_cli_version_parser_reads_codex_cli_output() {
        assert_eq!(
            parse_codex_cli_version("codex-cli 0.128.0"),
            Some((0, 128, 0))
        );
        assert_eq!(
            parse_codex_cli_version("codex-cli 0.130.0-alpha.5"),
            Some((0, 130, 0))
        );
        assert_eq!(
            parse_codex_cli_version("codex-cli 0.136.0-alpha.2"),
            Some((0, 136, 0))
        );
    }

    #[test]
    fn codex_cli_probe_skips_failed_windowsapps_candidate() {
        let candidates = vec![
            "C:\\Program Files\\WindowsApps\\OpenAI.Codex\\codex.exe".to_string(),
            "C:\\Users\\me\\AppData\\Local\\OpenAI\\Codex\\bin\\codex.exe".to_string(),
        ];
        let status = build_codex_app_server_probe_status_with(&candidates, |program, args| {
            if program.contains("WindowsApps") {
                return Err("Access is denied.".to_string());
            }
            match args {
                ["--version"] => Ok("codex-cli 0.130.0-alpha.5".to_string()),
                ["app-server", "--help"] => {
                    Ok("Usage: codex app-server [OPTIONS] [COMMAND]".to_string())
                }
                _ => Err("unexpected args".to_string()),
            }
        });

        assert_eq!(
            status.path.as_deref(),
            Some("C:\\Users\\me\\AppData\\Local\\OpenAI\\Codex\\bin\\codex.exe")
        );
        assert!(status.public.supports_required_protocol);
        assert_eq!(status.public.version.as_deref(), Some("codex-cli 0.130.0-alpha.5"));
    }

    #[test]
    fn codex_send_block_reason_mentions_cli_and_responses_support() {
        let reason = codex_send_block_reason(&CodexAppServerStatus {
            version: Some("codex-cli 0.125.0".to_string()),
            available: true,
            supports_required_protocol: false,
            issues: vec!["当前 codex CLI 版本过低，需要 0.128.0 或更新版本。".to_string()],
        })
        .unwrap();

        assert!(reason.contains("当前 codex CLI 版本过低"));
        assert!(reason.contains("0.128.0"));
        assert!(reason.contains("OpenAI Responses API"));

        assert!(codex_send_block_reason(&CodexAppServerStatus {
            version: Some("codex-cli 0.128.0".to_string()),
            available: true,
            supports_required_protocol: true,
            issues: Vec::new(),
        })
        .is_none());
    }

    #[test]
    fn composer_uses_active_backend_and_matching_default_model() {
        let composer = ChatComposerState {
            task_id: "stale-task".to_string(),
            backend: BACKEND_CLAUDE.to_string(),
            model: "claude-sonnet-4-6".to_string(),
            plan_mode: true,
            permission: "readonly".to_string(),
        };

        let normalized = normalize_composer_for_backend(composer, "task-1", BACKEND_CODEX);

        assert_eq!(normalized.task_id, "task-1");
        assert_eq!(normalized.backend, BACKEND_CODEX);
        assert_eq!(normalized.model, "gpt-5.5");
        assert!(normalized.plan_mode);
        assert_eq!(normalized.permission, "readonly");
    }

    #[test]
    fn composer_keeps_model_when_it_belongs_to_active_backend() {
        let composer = ChatComposerState {
            task_id: "task-1".to_string(),
            backend: BACKEND_CLAUDE.to_string(),
            model: "gpt-5.4-mini".to_string(),
            plan_mode: false,
            permission: "ask".to_string(),
        };

        let normalized = normalize_composer_for_backend(composer, "task-1", BACKEND_CODEX);

        assert_eq!(normalized.backend, BACKEND_CODEX);
        assert_eq!(normalized.model, "gpt-5.4-mini");
    }

    fn pending_turn(id: &str) -> PendingChatTurn {
        PendingChatTurn {
            content: format!("content {id}"),
            composer: default_composer("task-1"),
            project_cwd: "D:\\PROJECT\\workspace\\Lilia".to_string(),
            attachments: Vec::new(),
            message: ChatMessage {
                id: format!("u-{id}"),
                task_id: "task-1".to_string(),
                role: "user".to_string(),
                content: format!("content {id}"),
                attachments: Vec::new(),
                created_at: 100,
            },
            turn_id: format!("turn-{id}"),
            guide_id: None,
        }
    }

    #[test]
    fn clearing_pending_turns_removes_executable_queue_and_returns_guide_ids() {
        let store = ChatStore::default();
        {
            let mut pending = store.pending_turns.lock().unwrap();
            pending
                .entry("task-1".to_string())
                .or_default()
                .push_back(PendingChatTurn {
                    guide_id: Some("guide-1".to_string()),
                    ..pending_turn("queued")
                });
        }

        assert_eq!(
            clear_pending_turns(&store, "task-1"),
            vec!["guide-1".to_string()]
        );
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn prepare_running_turn_stop_for_interrupt_marks_interrupted_and_clears_queue() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-running".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-queued".to_string()),
                ..pending_turn("queued")
            });

        let prepared = prepare_running_turn_stop(&store, "task-1", true, false).unwrap();

        assert_eq!(prepared.guide_ids, vec!["guide-queued".to_string()]);
        assert!(prepared.child_handle.is_none());
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
        assert_eq!(
            store
                .interrupted_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|turn| turn.turn_id.as_str()),
            Some("turn-running")
        );
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn prepare_running_turn_stop_for_reset_marks_reset_without_interrupted_event() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-reset".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );

        let prepared = prepare_running_turn_stop(&store, "task-1", false, true).unwrap();

        assert!(prepared.guide_ids.is_empty());
        assert!(store.interrupted_turns.lock().unwrap().get("task-1").is_none());
        assert_eq!(
            store
                .reset_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|turn| turn.turn_id.as_str()),
            Some("turn-reset")
        );
    }

    #[test]
    fn reset_marker_is_read_without_consuming_until_turn_finish() {
        let store = ChatStore::default();
        store.reset_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-reset".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );

        assert!(is_turn_marked_reset(
            &store,
            "task-1",
            "turn-reset",
            BACKEND_CLAUDE
        ));
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_some());

        let (interrupted, reset) =
            take_turn_stop_marks(&store, "task-1", "turn-reset", BACKEND_CLAUDE);
        assert!(!interrupted);
        assert!(reset);
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn finish_running_turn_handles_clears_handles_and_consumes_matching_marks() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-stop".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );
        store.interrupted_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-stop".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );

        let finished = finish_running_turn_handles(&store, "task-1", "turn-stop", BACKEND_CLAUDE);

        assert!(finished.interrupted);
        assert!(!finished.reset);
        assert!(store.running_turns.lock().unwrap().get("task-1").is_none());
        assert!(store.interrupted_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn reset_finish_does_not_advance_pending_queue() {
        let store = ChatStore::default();
        store
            .running_tasks
            .lock()
            .unwrap()
            .insert("task-1".to_string(), true);
        store.reset_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-reset".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(pending_turn("queued"));

        let finished = finish_running_turn_handles(&store, "task-1", "turn-reset", BACKEND_CLAUDE);
        assert!(finished.reset);
        assert!(take_next_pending_turn(&store, "task-1", !finished.reset).is_none());

        assert_eq!(
            store
                .pending_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|queue| queue.len()),
            Some(1)
        );
        assert!(store.running_tasks.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn prepare_running_turn_stop_without_running_turn_leaves_queue_intact() {
        let store = ChatStore::default();
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-still-queued".to_string()),
                ..pending_turn("queued")
            });

        assert!(prepare_running_turn_stop(&store, "task-1", true, true).is_none());
        assert_eq!(
            store
                .pending_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|queue| queue.len()),
            Some(1)
        );
        assert!(store.interrupted_turns.lock().unwrap().get("task-1").is_none());
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn interrupted_exit_does_not_emit_runner_error() {
        assert!(!should_emit_runner_exit_error(
            true,
            true,
            "agent 进程被终止",
        ));
        assert!(should_emit_runner_exit_error(
            false,
            true,
            "agent 进程异常退出",
        ));
        assert!(!should_emit_runner_exit_error(false, true, "   "));
    }

    fn create_resume_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE tasks (
              id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL
            );
            CREATE TABLE agent_timeline_events (
              id                TEXT PRIMARY KEY,
              task_id           TEXT NOT NULL,
              turn_id           TEXT,
              backend           TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              kind              TEXT NOT NULL,
              status            TEXT NOT NULL,
              title             TEXT NOT NULL,
              summary           TEXT,
              payload           TEXT NOT NULL,
              created_at        INTEGER NOT NULL,
              updated_at        INTEGER NOT NULL,
              turn_seq          INTEGER NOT NULL,
              intra_turn_order  INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();
    }

    #[test]
    fn persisted_resume_session_id_ignores_unscoped_task_session_id() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            "INSERT INTO tasks (id, session_id) VALUES ('task-1', 'claude-session')",
            [],
        )
        .unwrap();
        agent_timeline::insert(
            &conn,
            AgentTimelineEventInput {
                id: Some("codex-turn".to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                backend: BACKEND_CODEX.to_string(),
                kind: "turn".to_string(),
                status: "success".to_string(),
                title: "Codex done".to_string(),
                summary: None,
                payload: json!({ "sessionId": "codex-thread" }),
                created_at: Some(200),
                updated_at: Some(200),
            },
        )
        .unwrap();

        assert_eq!(
            load_persisted_resume_session_id(&conn, "task-1", BACKEND_CODEX),
            Some("codex-thread".to_string())
        );
        assert_eq!(
            load_persisted_resume_session_id(&conn, "task-1", BACKEND_CLAUDE),
            None
        );
    }
}

// ---------- 进程内状态 ----------

#[derive(Default)]
struct ChatStore {
    composers: Mutex<HashMap<String, ChatComposerState>>,
    /// SDK session id：key = "{backend}:{task_id}"，第一次发送为空，done 后写入用于 resume。
    sdk_sessions: Mutex<HashMap<String, String>>,
    running_tasks: Mutex<HashMap<String, bool>>,
    pending_turns: Mutex<HashMap<String, VecDeque<PendingChatTurn>>>,
    running_turns: Mutex<HashMap<String, RunningTurn>>,
    running_children: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    interrupted_turns: Mutex<HashMap<String, RunningTurn>>,
    reset_turns: Mutex<HashMap<String, RunningTurn>>,
    /// 仍在运行的 runner 子进程 stdin。key = task_id，turn 结束时移除（Drop 即关 stdin）。
    /// 让 chat_respond_tool_consent 命令能把决策写回给 runner。
    running_stdins: Mutex<HashMap<String, Arc<Mutex<ChildStdin>>>>,
}

fn session_key(backend: &str, task_id: &str) -> String {
    format!("{backend}:{task_id}")
}

fn load_persisted_resume_session_id(
    conn: &Connection,
    task_id: &str,
    backend: &str,
) -> Option<String> {
    agent_timeline::latest_session_id(conn, task_id, backend)
        .ok()
        .flatten()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn new_chat_message_id() -> String {
    format!("u-{}", Uuid::new_v4())
}

fn normalize_backend(value: &str) -> &'static str {
    match value {
        BACKEND_CODEX => BACKEND_CODEX,
        _ => BACKEND_CLAUDE,
    }
}

fn default_model_for_backend(backend: &str) -> &'static str {
    match normalize_backend(backend) {
        BACKEND_CODEX => CODEX_MODEL_OPTIONS[0].0,
        _ => "claude-sonnet-4-6",
    }
}

fn model_options_for_backend(backend: &str) -> Vec<ChatModelOption> {
    match normalize_backend(backend) {
        BACKEND_CODEX => CODEX_MODEL_OPTIONS
            .iter()
            .map(|(id, label)| ChatModelOption {
                id: (*id).to_string(),
                label: (*label).to_string(),
                backend: BACKEND_CODEX.to_string(),
            })
            .collect(),
        _ => vec![
            ChatModelOption {
                id: "claude-opus-4-7".to_string(),
                label: "Opus 4.7".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
            ChatModelOption {
                id: "claude-sonnet-4-6".to_string(),
                label: "Sonnet 4.6".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
            ChatModelOption {
                id: "claude-haiku-4-5".to_string(),
                label: "Haiku 4.5".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        ],
    }
}

fn normalize_model_for_backend(model: &str, backend: &str) -> String {
    let backend = normalize_backend(backend);
    if model_options_for_backend(backend)
        .iter()
        .any(|option| option.id == model)
    {
        model.to_string()
    } else {
        default_model_for_backend(backend).to_string()
    }
}

fn normalize_composer_for_backend(
    mut composer: ChatComposerState,
    task_id: &str,
    backend: &str,
) -> ChatComposerState {
    let backend = normalize_backend(backend);
    composer.task_id = task_id.to_string();
    composer.backend = backend.to_string();
    composer.model = normalize_model_for_backend(&composer.model, backend);
    composer
}

fn default_composer(task_id: &str) -> ChatComposerState {
    ChatComposerState {
        task_id: task_id.to_string(),
        backend: BACKEND_CLAUDE.to_string(),
        model: default_model_for_backend(BACKEND_CLAUDE).to_string(),
        plan_mode: false,
        permission: "ask".to_string(),
    }
}

fn queue_pending_turn(
    store: &ChatStore,
    task_id: &str,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    message: ChatMessage,
    turn_id: String,
    guide_id: Option<String>,
) -> usize {
    let mut pending = store.pending_turns.lock().unwrap();
    let queue = pending.entry(task_id.to_string()).or_default();
    queue.push_back(PendingChatTurn {
        content,
        composer,
        project_cwd,
        attachments,
        message,
        turn_id,
        guide_id,
    });
    queue.len()
}

fn clear_pending_turns(store: &ChatStore, task_id: &str) -> Vec<String> {
    store
        .pending_turns
        .lock()
        .unwrap()
        .remove(task_id)
        .map(|queue| queue.into_iter().filter_map(|turn| turn.guide_id).collect())
        .unwrap_or_default()
}

fn set_guide_status_for_app(
    app: &AppHandle,
    guide_id: Option<&str>,
    status: &str,
) -> Result<(), String> {
    let Some(guide_id) = guide_id else {
        return Ok(());
    };
    let Some(store) = app.try_state::<LiliaStore>() else {
        return Ok(());
    };
    todos::set_lilia_guide_status(app, &store, guide_id, status)
}

fn reset_cleared_guide_queue(app: &AppHandle, guide_ids: Vec<String>) {
    for guide_id in guide_ids {
        if let Err(err) = set_guide_status_for_app(app, Some(&guide_id), "pending") {
            eprintln!("[todo-guides] reset queued guide failed: {err}");
        }
    }
}

struct PreparedTurnStop {
    child_handle: Option<Arc<Mutex<Child>>>,
    guide_ids: Vec<String>,
}

struct FinishedRunningTurn {
    interrupted: bool,
    reset: bool,
}

fn clear_running_handles(store: &ChatStore, task_id: &str) -> Option<RunningTurn> {
    store.running_stdins.lock().unwrap().remove(task_id);
    store.running_children.lock().unwrap().remove(task_id);
    store.running_turns.lock().unwrap().remove(task_id)
}

fn prepare_running_turn_stop(
    store: &ChatStore,
    task_id: &str,
    mark_interrupted: bool,
    mark_reset: bool,
) -> Option<PreparedTurnStop> {
    let running_turn = {
        let turns = store.running_turns.lock().unwrap();
        turns.get(task_id).cloned()
    }?;

    let guide_ids = clear_pending_turns(store, task_id);
    if mark_interrupted {
        store
            .interrupted_turns
            .lock()
            .unwrap()
            .insert(task_id.to_string(), running_turn.clone());
    }
    if mark_reset {
        store
            .reset_turns
            .lock()
            .unwrap()
            .insert(task_id.to_string(), running_turn);
    }

    let child_handle = {
        let children = store.running_children.lock().unwrap();
        children.get(task_id).cloned()
    };
    Some(PreparedTurnStop {
        child_handle,
        guide_ids,
    })
}

fn is_turn_marked_reset(store: &ChatStore, task_id: &str, turn_id: &str, backend: &str) -> bool {
    store
        .reset_turns
        .lock()
        .unwrap()
        .get(task_id)
        .is_some_and(|turn| turn.turn_id == turn_id && turn.backend == backend)
}

fn take_turn_stop_marks(
    store: &ChatStore,
    task_id: &str,
    turn_id: &str,
    backend: &str,
) -> (bool, bool) {
    let interrupted = store
        .interrupted_turns
        .lock()
        .unwrap()
        .remove(task_id)
        .is_some_and(|turn| turn.turn_id == turn_id && turn.backend == backend);
    let reset = store
        .reset_turns
        .lock()
        .unwrap()
        .remove(task_id)
        .is_some_and(|turn| turn.turn_id == turn_id && turn.backend == backend);
    (interrupted, reset)
}

fn finish_running_turn_handles(
    store: &ChatStore,
    task_id: &str,
    turn_id: &str,
    backend: &str,
) -> FinishedRunningTurn {
    clear_running_handles(store, task_id);
    let (interrupted, reset) = take_turn_stop_marks(store, task_id, turn_id, backend);
    FinishedRunningTurn { interrupted, reset }
}

fn stop_running_turn(
    app: &AppHandle,
    store: &ChatStore,
    task_id: &str,
    mark_interrupted: bool,
    mark_reset: bool,
) -> Result<bool, String> {
    let Some(prepared) = prepare_running_turn_stop(store, task_id, mark_interrupted, mark_reset)
    else {
        return Ok(false);
    };
    reset_cleared_guide_queue(app, prepared.guide_ids);
    if let Some(child_handle) = prepared.child_handle {
        let mut child = child_handle.lock().map_err(|err| err.to_string())?;
        terminate_agent_child(&mut child)?;
    }
    clear_running_handles(store, task_id);
    Ok(true)
}

fn take_next_pending_turn(
    store: &ChatStore,
    task_id: &str,
    advance_queue: bool,
) -> Option<PendingChatTurn> {
    let mut running = store.running_tasks.lock().unwrap();
    if !advance_queue {
        running.remove(task_id);
        return None;
    }

    let mut pending = store.pending_turns.lock().unwrap();
    let mut should_remove_queue = false;
    let next = if let Some(queue) = pending.get_mut(task_id) {
        let turn = queue.pop_front();
        should_remove_queue = queue.is_empty();
        turn
    } else {
        None
    };
    if should_remove_queue {
        pending.remove(task_id);
    }
    if next.is_none() {
        running.remove(task_id);
    }
    next
}

fn should_emit_runner_exit_error(interrupted: bool, nonzero: bool, stderr_text: &str) -> bool {
    !interrupted && nonzero && !stderr_text.trim().is_empty()
}

fn persist_and_emit_interrupted_timeline_event(
    app_handle: &AppHandle,
    task_id: &str,
    backend: &str,
    turn_id: &str,
) {
    let now = now_millis() as i64;
    let message = "用户打断了当前 Agent 运行";
    persist_and_emit_input(
        app_handle,
        AgentTimelineEventInput {
            id: Some(format!("{turn_id}:interrupted")),
            task_id: task_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            backend: backend.to_string(),
            kind: "error".to_string(),
            status: "error".to_string(),
            title: "Agent 已打断".to_string(),
            summary: Some(message.to_string()),
            payload: serde_json::json!({
                "backend": backend,
                "interrupted": true,
                "message": message,
            }),
            created_at: Some(now),
            updated_at: Some(now),
        },
    );
}

fn kill_child(child: &mut Child) -> Result<(), String> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(err) if matches!(err.kind(), ErrorKind::InvalidInput | ErrorKind::NotFound) => Ok(()),
        Err(err) => Err(format!("终止 agent 进程失败：{err}")),
    }
}

#[cfg(windows)]
fn terminate_agent_child(child: &mut Child) -> Result<(), String> {
    let pid = child.id().to_string();
    let taskkill = Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match taskkill {
        Ok(status) if status.success() => Ok(()),
        _ => kill_child(child),
    }
}

#[cfg(not(windows))]
fn terminate_agent_child(child: &mut Child) -> Result<(), String> {
    let pid = child.id().to_string();
    let _ = Command::new("pkill")
        .args(["-TERM", "-P", pid.as_str()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    kill_child(child)
}

// ---------- 连接解析 ----------

/// 与前端 ConnectionMode 字符串对齐的四档枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionMode {
    CcSwitch,
    CustomBaseUrl,
    Direct,
    Unconfigured,
}

impl ConnectionMode {
    fn as_str(self) -> &'static str {
        match self {
            ConnectionMode::CcSwitch => "cc-switch",
            ConnectionMode::CustomBaseUrl => "custom",
            ConnectionMode::Direct => "direct",
            ConnectionMode::Unconfigured => "unconfigured",
        }
    }
}

#[derive(Debug, Clone)]
struct BackendConnectionPlan {
    mode: ConnectionMode,
    base_url: Option<String>,
    api_key: Option<String>,
}

/// 探测一个 base URL 是否可拨通。短超时——拨不通就当它不存在，不阻塞主流程。
fn url_reachable(url: Option<&str>) -> bool {
    let Some(url) = url else { return false };
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Some(host_port) = parse_host_port(trimmed) else {
        return false;
    };
    let Ok(addrs) = host_port.to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok())
}

/// 从 `http(s)://host:port[/...]` 抽出 `host:port`；缺端口时按协议给默认。
fn parse_host_port(url: &str) -> Option<String> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else {
        ("http", url)
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    if authority.contains(':') {
        Some(authority.to_string())
    } else {
        let default_port = if scheme == "https" { 443 } else { 80 };
        Some(format!("{authority}:{default_port}"))
    }
}

/// 从 tauri_plugin_store 读 ProviderConfig。
fn load_provider_config(app: &AppHandle, key: &str) -> Option<ProviderConfig> {
    let store = app.store(PROVIDER_STORE_FILE).ok()?;
    let value = store.get(key)?;
    serde_json::from_value::<ProviderConfig>(value).ok()
}

fn load_active_backend(app: &AppHandle) -> String {
    let read = || -> Option<String> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let value = store.get(PROVIDER_ACTIVE_BACKEND_KEY)?;
        value.as_str().map(|s| normalize_backend(s).to_string())
    };
    read().unwrap_or_else(|| BACKEND_CLAUDE.to_string())
}

/// 读 CC-Switch 配置；读不到回 Default。
fn load_cc_switch_config(app: &AppHandle) -> CCSwitchConfig {
    let read = || -> Option<CCSwitchConfig> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let value = store.get(CC_SWITCH_KEY)?;
        serde_json::from_value::<CCSwitchConfig>(value).ok()
    };
    read().unwrap_or_default()
}

/// 读辅助模型配置；读不到回 Default（三件套全 None）。
fn load_assistant_ai_config(app: &AppHandle) -> AssistantAIConfig {
    let read = || -> Option<AssistantAIConfig> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let value = store.get(ASSISTANT_AI_KEY)?;
        serde_json::from_value::<AssistantAIConfig>(value).ok()
    };
    read().unwrap_or_default()
}

fn load_agent_interaction_settings(app: &AppHandle) -> AgentInteractionSettings {
    let read = || -> Option<AgentInteractionSettings> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let value = store.get(AGENT_INTERACTION_KEY)?;
        serde_json::from_value::<AgentInteractionSettings>(value).ok()
    };
    read().unwrap_or_default()
}

/// 读某个 backend 的路由模式；未设置返回默认 "cc-switch"。
fn load_router_mode(app: &AppHandle, backend: &str) -> String {
    let key = match backend {
        BACKEND_CODEX => ROUTER_KEY_CODEX,
        _ => ROUTER_KEY_CLAUDE,
    };
    let read = || -> Option<String> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let value = store.get(key)?;
        value.as_str().map(|s| s.to_string())
    };
    read()
        .filter(|m| matches!(m.as_str(), ROUTER_CC_SWITCH | ROUTER_DIRECT))
        .unwrap_or_else(|| ROUTER_CC_SWITCH.to_string())
}

/// CC-Switch 路由：检查共用代理 URL 是否非空 + 可达。
fn try_cc_switch_for_backend(app: &AppHandle) -> Option<BackendConnectionPlan> {
    let cfg = load_cc_switch_config(app);
    let url = cfg.base_url.filter(|s| !s.is_empty())?;
    if !url_reachable(Some(&url)) {
        return None;
    }
    Some(BackendConnectionPlan {
        mode: ConnectionMode::CcSwitch,
        base_url: Some(url),
        api_key: Some(CC_SWITCH_PLACEHOLDER_KEY.to_string()),
    })
}

/// direct 路由：用 store 里的 ProviderConfig；apiKey/baseUrl 都空则 unconfigured。
fn try_direct_for_backend(app: &AppHandle, backend: &'static str) -> BackendConnectionPlan {
    let key = match backend {
        BACKEND_CODEX => PROVIDER_KEY_CODEX,
        _ => PROVIDER_KEY_CLAUDE,
    };
    let cfg = load_provider_config(app, key).unwrap_or_default();
    let has_key = cfg.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false);
    let has_url = cfg
        .base_url
        .as_ref()
        .map(|u| !u.is_empty())
        .unwrap_or(false);
    if !has_key && !has_url {
        return BackendConnectionPlan {
            mode: ConnectionMode::Unconfigured,
            base_url: None,
            api_key: None,
        };
    }
    let mode = if has_url {
        ConnectionMode::CustomBaseUrl
    } else {
        ConnectionMode::Direct
    };
    BackendConnectionPlan {
        mode,
        base_url: cfg.base_url.filter(|s| !s.is_empty()),
        api_key: cfg.api_key.filter(|s| !s.is_empty()),
    }
}

/// 入口：按 per-backend 路由模式分发。选了哪个就只走哪个，失败即 unconfigured。
fn resolve_connection_for(app: &AppHandle, backend_str: &str) -> BackendConnectionPlan {
    let backend: &'static str = if backend_str == BACKEND_CODEX {
        BACKEND_CODEX
    } else {
        BACKEND_CLAUDE
    };
    let mode = load_router_mode(app, backend);
    match mode.as_str() {
        ROUTER_DIRECT => try_direct_for_backend(app, backend),
        _ => try_cc_switch_for_backend(app).unwrap_or(BackendConnectionPlan {
            mode: ConnectionMode::Unconfigured,
            base_url: None,
            api_key: None,
        }),
    }
}

// ---------- 子进程定位 ----------

/// 找到 agent-runner.mjs 的实际路径。
///
/// 开发态：cargo 编出来的二进制位于 `apps/desktop/src-tauri/target/{debug|release}/`，
/// 而脚本位于 `apps/desktop/agent-runner.mjs`，相对路径回退 3 层。
/// 按候选顺序找第一个存在的文件；找不到就返回最后一个候选让上层报错更直观。
fn locate_agent_runner(app: &AppHandle) -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1) 与 binary 同目录 → 适合未来 sidecar/资源拷贝场景
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("agent-runner.mjs"));
            // 2) 开发态：target/debug → 回退 3 层到 apps/desktop
            candidates.push(dir.join("../../../agent-runner.mjs"));
        }
    }

    // 3) Tauri resource_dir 兜底
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("agent-runner.mjs"));
    }

    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }
    candidates
        .into_iter()
        .last()
        .unwrap_or_else(|| PathBuf::from("agent-runner.mjs"))
}

fn spawn_agent_turn(
    app: AppHandle,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    turn_id: String,
) {
    let backend = composer.backend.clone();
    let resume_session_id = {
        let store = app.state::<ChatStore>();
        let session = store
            .sdk_sessions
            .lock()
            .unwrap()
            .get(&session_key(&backend, &task_id))
            .cloned();
        session
    }
    .or_else(|| {
        let store = app.try_state::<LiliaStore>()?;
        let conn = store.conn().ok()?;
        load_persisted_resume_session_id(&conn, &task_id, &backend)
    });

    let script_path = locate_agent_runner(&app);
    let connection = resolve_connection_for(&app, &backend);
    let app_handle = app.clone();
    let task_id_for_thread = task_id.clone();
    let composer_for_thread = composer.clone();
    let prompt_for_thread = content.clone();
    let attachments_for_thread = attachments.clone();
    let backend_for_thread = backend.clone();
    let turn_id_for_thread = turn_id;

    thread::spawn(move || {
        let queued_count = {
            let store = app_handle.state::<ChatStore>();
            let count = store
                .pending_turns
                .lock()
                .unwrap()
                .get(&task_id_for_thread)
                .map(|q| q.len())
                .unwrap_or(0);
            count
        };

        let extensions = plugins::runtime_extensions(&app_handle, Some(&project_cwd));
        let stdin_payload = serde_json::json!({
            "backend": backend_for_thread,
            "cwd": project_cwd,
            "prompt": prompt_for_thread,
            "attachments": attachments_for_thread,
            "model": composer_for_thread.model,
            "resumeSessionId": resume_session_id,
            "planMode": composer_for_thread.plan_mode,
            "permission": composer_for_thread.permission,
            "extensions": extensions,
        });

        let mut cmd = Command::new("node");
        cmd.arg(&script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // 按 backend 选择要注入的 env 键名：claude→ANTHROPIC_*，codex→OPENAI_*。
        let (base_key, key_key) = match backend_for_thread.as_str() {
            BACKEND_CODEX => ("OPENAI_BASE_URL", "OPENAI_API_KEY"),
            _ => ("ANTHROPIC_BASE_URL", "ANTHROPIC_API_KEY"),
        };
        if let Some(url) = &connection.base_url {
            cmd.env(base_key, url);
        }
        if let Some(key) = &connection.api_key {
            cmd.env(key_key, key);
        }
        if backend_for_thread == BACKEND_CODEX {
            let codex_app_server = build_codex_app_server_probe_status();
            if let Some(path) = codex_app_server.path {
                cmd.env("LILIA_CODEX_CLI_PATH", path);
            }
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(err) => {
                let msg =
                    format!("无法启动 node 子进程（请确保已安装 Node 18+ 并在 PATH 中）：{err}");
                persist_and_emit_error_timeline_event(
                    &app_handle,
                    &task_id_for_thread,
                    &backend_for_thread,
                    Some(&turn_id_for_thread),
                    msg,
                );
                finish_agent_turn(
                    app_handle,
                    task_id_for_thread,
                    backend_for_thread,
                    None,
                    true,
                );
                return;
            }
        };

        let child_stdout = child.stdout.take();
        let child_stderr = child.stderr.take();
        let child_handle = Arc::new(Mutex::new(child));
        {
            let store = app_handle.state::<ChatStore>();
            store
                .running_children
                .lock()
                .unwrap()
                .insert(task_id_for_thread.clone(), child_handle.clone());
            store.running_turns.lock().unwrap().insert(
                task_id_for_thread.clone(),
                RunningTurn {
                    turn_id: turn_id_for_thread.clone(),
                    backend: backend_for_thread.clone(),
                },
            );
        }

        let _ = app_handle.emit(
            "chat:turn-started",
            TurnStartedEvent {
                task_id: task_id_for_thread.clone(),
                queued_count,
            },
        );

        // 把命令 JSON 写一行（带尾换行），但保留 stdin —— 后续 consent_response
        // 还要通过它写回 runner。stdin 存到 ChatStore，turn 结束时移除（Drop 关闭）。
        let stdin_handle: Option<Arc<Mutex<ChildStdin>>> = match child_handle
            .lock()
            .ok()
            .and_then(|mut child| child.stdin.take())
        {
            Some(mut stdin) => {
                let mut bytes = serde_json::to_vec(&stdin_payload).unwrap_or_default();
                bytes.push(b'\n');
                if stdin.write_all(&bytes).is_err() {
                    // 子进程已经死了；让 stdout 循环正常退出兜底
                    None
                } else {
                    let shared = Arc::new(Mutex::new(stdin));
                    let store = app_handle.state::<ChatStore>();
                    store
                        .running_stdins
                        .lock()
                        .unwrap()
                        .insert(task_id_for_thread.clone(), shared.clone());
                    Some(shared)
                }
            }
            None => None,
        };

        let mut last_session_id: Option<String> = None;
        let event_ctx = AgentTurnContext {
            task_id: task_id_for_thread.clone(),
            backend: backend_for_thread.clone(),
            turn_id: turn_id_for_thread.clone(),
        };
        let mut event_host = AgentEventHost::new();
        event_host.register(Box::new(TodoMirrorExtension::new(app_handle.clone())));
        let mut timeline_throttle = TimelineThrottle::new();
        let mut last_assistant_error_text: Option<String> = None;

        if let Some(stdout) = child_stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let store = app_handle.state::<ChatStore>();
                let line = match line {
                    Ok(l) if !l.trim().is_empty() => l,
                    _ => continue,
                };
                if is_turn_marked_reset(
                    &store,
                    &task_id_for_thread,
                    &turn_id_for_thread,
                    &backend_for_thread,
                ) {
                    break;
                }
                let value: JsonValue = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue, // 忽略偶发非 JSON 输出（SDK 内部 log 等）
                };
                let Some(event) = AgentRuntimeEvent::from_runner_json(&value) else {
                    continue;
                };
                log_agent_event_effect(event_host.dispatch(&event_ctx, &event));
                match &event {
                    AgentRuntimeEvent::ToolUse { .. } | AgentRuntimeEvent::TodoList { .. } => {}
                    AgentRuntimeEvent::Timeline { .. } => {
                        if let Some(input) = timeline_input_from_runtime_event(&event_ctx, &event) {
                            if let Some(text) = assistant_error_text(&input) {
                                last_assistant_error_text = Some(text);
                            }
                            timeline_throttle.submit(&app_handle, input);
                        }
                    }
                    AgentRuntimeEvent::ConsentRequest {
                        id,
                        tool_name,
                        input,
                        title,
                        display_name,
                        description,
                        blocked_path,
                        decision_reason,
                        tool_use_id,
                    } => {
                        let _ = app_handle.emit(
                            "chat:tool-consent-request",
                            ToolConsentRequestEvent {
                                task_id: task_id_for_thread.clone(),
                                turn_id: turn_id_for_thread.clone(),
                                backend: backend_for_thread.clone(),
                                request_id: id.clone(),
                                tool_name: tool_name.clone(),
                                input: input.clone(),
                                title: title.clone(),
                                display_name: display_name.clone(),
                                description: description.clone(),
                                blocked_path: blocked_path.clone(),
                                decision_reason: decision_reason.clone(),
                                tool_use_id: tool_use_id.clone(),
                            },
                        );
                    }
                    AgentRuntimeEvent::AskUserRequest { id, spec } => {
                        let _ = app_handle.emit(
                            "chat:ask-user-request",
                            AskUserRequestEvent {
                                task_id: task_id_for_thread.clone(),
                                turn_id: turn_id_for_thread.clone(),
                                backend: backend_for_thread.clone(),
                                request_id: id.clone(),
                                spec: spec.clone(),
                            },
                        );
                    }
                    AgentRuntimeEvent::Done { session_id, .. } => {
                        if let Some(sid) = session_id {
                            last_session_id = Some(sid.clone());
                        }
                    }
                    AgentRuntimeEvent::Error { message } => {
                        timeline_throttle.flush_all(&app_handle);
                        if last_assistant_error_text
                            .as_deref()
                            .is_some_and(|text| normalize_timeline_text(message).contains(text))
                        {
                            continue;
                        }
                        persist_and_emit_error_timeline_event(
                            &app_handle,
                            &task_id_for_thread,
                            &backend_for_thread,
                            Some(&turn_id_for_thread),
                            message.clone(),
                        );
                    }
                }
            }
        }

        // 子进程的 stdout 读完即 turn 结束：把存的 stdin 移除，防止后续 consent
        // 响应往一个死管道里写。Drop ChildStdin 也会向 runner 端发 EOF。
        drop(stdin_handle);
        let finished = {
            let store = app_handle.state::<ChatStore>();
            finish_running_turn_handles(
                &store,
                &task_id_for_thread,
                &turn_id_for_thread,
                &backend_for_thread,
            )
        };

        // 流结束前确保 pending 的最后一帧落地；reset 后则丢弃旧 turn 缓冲，避免回写已清空的 timeline。
        if finished.reset {
            timeline_throttle.pending.clear();
        } else {
            timeline_throttle.flush_all(&app_handle);
        }

        // 等待子进程退出并收集 stderr 用于诊断（API key 缺失等）。
        let exit_status = child_handle
            .lock()
            .map_err(|err| std::io::Error::new(ErrorKind::Other, err.to_string()))
            .and_then(|mut child| child.wait());
        let stderr_text = child_stderr
            .and_then(|mut s| {
                let mut buf = String::new();
                use std::io::Read;
                s.read_to_string(&mut buf).ok().map(|_| buf)
            })
            .unwrap_or_default();

        if finished.interrupted && !finished.reset {
            persist_and_emit_interrupted_timeline_event(
                &app_handle,
                &task_id_for_thread,
                &backend_for_thread,
                &turn_id_for_thread,
            );
        }

        let nonzero = exit_status.as_ref().map(|s| !s.success()).unwrap_or(true);
        if !finished.reset
            && should_emit_runner_exit_error(finished.interrupted, nonzero, &stderr_text)
        {
            persist_and_emit_error_timeline_event(
                &app_handle,
                &task_id_for_thread,
                &backend_for_thread,
                Some(&turn_id_for_thread),
                format!("agent 进程异常退出：{}", stderr_text.trim()),
            );
        }

        finish_agent_turn(
            app_handle,
            task_id_for_thread,
            backend_for_thread,
            last_session_id,
            !finished.interrupted && !finished.reset,
        );
    });
}

fn finish_agent_turn(
    app_handle: AppHandle,
    task_id: String,
    backend: String,
    last_session_id: Option<String>,
    advance_queue: bool,
) {
    // 记下 session id 供下一轮 resume。
    if let Some(sid) = last_session_id.clone() {
        let store = app_handle.state::<ChatStore>();
        store
            .sdk_sessions
            .lock()
            .unwrap()
            .insert(session_key(&backend, &task_id), sid.clone());
    }

    let _ = app_handle.emit(
        "chat:done",
        DoneEvent {
            task_id: task_id.clone(),
            session_id: last_session_id,
            subtype: None,
        },
    );

    let next = {
        let store = app_handle.state::<ChatStore>();
        take_next_pending_turn(&store, &task_id, advance_queue)
    };
    if let Some(turn) = next {
        if let Err(err) = set_guide_status_for_app(&app_handle, turn.guide_id.as_deref(), "sent") {
            eprintln!("[todo-guides] mark queued guide sent failed: {err}");
        }
        persist_and_emit_message_timeline_event(
            &app_handle,
            &turn.message,
            &turn.composer.backend,
            &turn.turn_id,
            false,
        );
        spawn_agent_turn(
            app_handle,
            task_id,
            turn.content,
            turn.composer,
            turn.project_cwd,
            turn.attachments,
            turn.turn_id,
        );
    }
}

// ---------- Commands ----------

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

const DIRECTORY_SCAN_LIMIT: usize = 1000;
const CONTEXT_SEARCH_SCAN_LIMIT: usize = 6000;
const DEFAULT_CONTEXT_SEARCH_LIMIT: usize = 12;
const MAX_CONTEXT_SEARCH_LIMIT: usize = 50;

fn image_mime_for_path(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())?;
    let mime = match ext.as_str() {
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => return None,
    };
    Some(mime.to_string())
}

fn clipboard_image_extension(mime: Option<&str>, name: Option<&str>) -> &'static str {
    let normalized = mime.unwrap_or("").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        _ => name
            .and_then(|value| Path::new(value).extension())
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .and_then(|ext| match ext.as_str() {
                "avif" => Some("avif"),
                "bmp" => Some("bmp"),
                "gif" => Some("gif"),
                "jpg" | "jpeg" => Some("jpg"),
                "png" => Some("png"),
                "svg" => Some("svg"),
                "webp" => Some("webp"),
                _ => None,
            })
            .unwrap_or("png"),
    }
}

fn normalize_clipboard_image_mime(mime: Option<&str>, ext: &str) -> String {
    let normalized = mime.unwrap_or("").trim().to_ascii_lowercase();
    if normalized.starts_with("image/") {
        return normalized;
    }
    match ext {
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "image/png",
    }
    .to_string()
}

fn clipboard_images_cache_dir(home: &Path) -> PathBuf {
    home.join("cache").join("clipboard-images")
}

fn clipboard_image_path(home: &Path, mime: Option<&str>, name: Option<&str>, now: u64) -> PathBuf {
    let ext = clipboard_image_extension(mime, name);
    clipboard_images_cache_dir(home).join(format!(
        "clipboard-{now}-{}.{}",
        Uuid::new_v4(),
        ext
    ))
}

fn clipboard_image_display_name(ext: &str, seq: u64) -> String {
    format!("图片 {seq}.{ext}")
}

fn clipboard_texts_cache_dir(home: &Path) -> PathBuf {
    home.join("cache").join("clipboard-texts")
}

fn clipboard_text_path(home: &Path, now: u64) -> PathBuf {
    clipboard_texts_cache_dir(home).join(format!("clipboard-{now}-{}.txt", Uuid::new_v4()))
}

fn clipboard_text_display_name(seq: u64) -> String {
    format!("粘贴文本 {seq}.txt")
}

fn save_clipboard_image_to_cache(
    home: &Path,
    input: ClipboardImageInput,
    now: u64,
    display_seq: u64,
) -> Result<ChatAttachment, String> {
    let path = clipboard_image_path(home, input.mime.as_deref(), input.name.as_deref(), now);
    let parent = path
        .parent()
        .ok_or_else(|| "无法解析剪贴板图片缓存目录".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("创建剪贴板图片缓存目录失败：{e}"))?;
    let bytes = general_purpose::STANDARD
        .decode(input.bytes_base64.trim())
        .map_err(|e| format!("解析剪贴板图片失败：{e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("保存剪贴板图片失败：{e}"))?;
    let mut attachment = describe_attachment_path(path.to_string_lossy().to_string());
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    attachment.name = clipboard_image_display_name(&ext, display_seq);
    attachment.mime = Some(normalize_clipboard_image_mime(input.mime.as_deref(), &ext));
    Ok(attachment)
}

fn save_clipboard_text_to_cache(
    home: &Path,
    input: ClipboardTextInput,
    now: u64,
    display_seq: u64,
) -> Result<ChatAttachment, String> {
    let path = clipboard_text_path(home, now);
    let parent = path
        .parent()
        .ok_or_else(|| "无法解析剪贴板文本缓存目录".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("创建剪贴板文本缓存目录失败：{e}"))?;
    fs::write(&path, input.text.as_bytes()).map_err(|e| format!("保存剪贴板文本失败：{e}"))?;
    let mut attachment = describe_attachment_path(path.to_string_lossy().to_string());
    attachment.name = clipboard_text_display_name(display_seq);
    attachment.mime = None;
    Ok(attachment)
}

fn scan_directory_meta(path: &Path) -> ChatAttachmentDirectoryMeta {
    let mut meta = ChatAttachmentDirectoryMeta {
        file_count: 0,
        directory_count: 0,
        total_size: 0,
        truncated: false,
        unreadable_count: 0,
    };
    let mut scanned = 0usize;
    let mut queue = VecDeque::from([path.to_path_buf()]);

    'scan: while let Some(dir) = queue.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => {
                meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                continue;
            }
        };
        for entry in entries {
            if scanned >= DIRECTORY_SCAN_LIMIT {
                meta.truncated = true;
                break 'scan;
            }
            scanned += 1;
            let Ok(entry) = entry else {
                meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                continue;
            };
            if file_type.is_dir() {
                meta.directory_count = meta.directory_count.saturating_add(1);
                queue.push_back(entry.path());
            } else if file_type.is_file() {
                meta.file_count = meta.file_count.saturating_add(1);
                if let Ok(file_meta) = entry.metadata() {
                    meta.total_size = meta.total_size.saturating_add(file_meta.len());
                } else {
                    meta.unreadable_count = meta.unreadable_count.saturating_add(1);
                }
            }
        }
    }

    meta
}

fn describe_attachment_path(path: String) -> ChatAttachment {
    let raw_path = PathBuf::from(path.trim());
    let normalized_path = if raw_path.is_absolute() {
        raw_path
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(&raw_path))
            .unwrap_or(raw_path)
    };
    let metadata = fs::metadata(&normalized_path).ok();
    let kind = metadata
        .as_ref()
        .map(|meta| {
            if meta.is_file() {
                "file"
            } else if meta.is_dir() {
                "directory"
            } else {
                "unknown"
            }
        })
        .unwrap_or("unknown")
        .to_string();
    let exists = metadata.is_some();
    let size = metadata.as_ref().and_then(|meta| {
        if meta.is_file() {
            Some(meta.len())
        } else {
            None
        }
    });
    let mime = if kind == "file" {
        image_mime_for_path(&normalized_path)
    } else {
        None
    };
    let directory = metadata.as_ref().and_then(|meta| {
        if meta.is_dir() {
            Some(scan_directory_meta(&normalized_path))
        } else {
            None
        }
    });
    let path_text = normalized_path.to_string_lossy().to_string();
    let name = normalized_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path_text.clone());

    ChatAttachment {
        id: format!("att-{}", Uuid::new_v4()),
        name,
        path: path_text,
        kind,
        size,
        exists,
        mime,
        directory,
    }
}

fn should_skip_context_search_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(
        name.as_str(),
        ".git" | "node_modules" | "dist" | "target" | ".cache" | "build"
    ) {
        return true;
    }
    if name == "cache" {
        return path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|parent| parent.to_str())
            .map(|parent| parent.eq_ignore_ascii_case(".yarn"))
            .unwrap_or(false);
    }
    false
}

fn relative_path_text(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sorted_child_paths(dir: &Path) -> Result<Vec<PathBuf>, ()> {
    let mut paths = Vec::new();
    let entries = fs::read_dir(dir).map_err(|_| ())?;
    for entry in entries {
        if let Ok(entry) = entry {
            paths.push(entry.path());
        }
    }
    paths.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase())
            .unwrap_or_default()
    });
    Ok(paths)
}

fn context_query_is_path_like(query: &str) -> bool {
    query.contains('/') || query.contains('\\')
}

fn context_query_allows_hidden(query: &str) -> bool {
    query.contains('.')
}

fn is_hidden_context_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn normalize_context_path_query(query: &str) -> String {
    let mut normalized = query.trim().replace('\\', "/");
    while let Some(rest) = normalized.strip_prefix("./") {
        normalized = rest.to_string();
    }
    normalized
}

fn context_relative_path_buf(path_text: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    if path_text.trim().is_empty() {
        return Some(path);
    }
    for component in Path::new(path_text).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(path)
}

fn context_browse_dir_from_query(query: &str) -> Option<(PathBuf, String)> {
    let normalized = normalize_context_path_query(query);
    if !context_query_is_path_like(query) {
        return None;
    }
    if normalized.is_empty() {
        return Some((PathBuf::new(), normalized));
    }
    if normalized.ends_with('/') {
        let dir_text = normalized.trim_end_matches('/');
        return context_relative_path_buf(dir_text).map(|dir| (dir, normalized));
    }
    let slash = normalized.rfind('/')?;
    let dir_text = &normalized[..slash];
    context_relative_path_buf(dir_text).map(|dir| (dir, normalized))
}

fn context_search_match(root: &Path, path: &Path, query: &str) -> Option<(String, String)> {
    let relative_path = relative_path_text(root, path);
    if query.is_empty() {
        return Some((relative_path, "name".to_string()));
    }
    let query = query.to_ascii_lowercase().replace('\\', "/");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.contains(&query) {
        return Some((relative_path, "name".to_string()));
    }
    if relative_path.to_ascii_lowercase().contains(&query) {
        return Some((relative_path, "path".to_string()));
    }
    None
}

fn push_context_search_result(
    root: &Path,
    path: &Path,
    query: &str,
    results: &mut Vec<ChatContextSearchResult>,
) {
    if let Some((relative_path, matched_by)) = context_search_match(root, path, query) {
        results.push(ChatContextSearchResult {
            attachment: describe_attachment_path(path.to_string_lossy().to_string()),
            relative_path,
            matched_by,
        });
    }
}

fn search_context_browse_dir(
    root: &Path,
    query: &str,
    limit: usize,
) -> Vec<ChatContextSearchResult> {
    let Some((relative_dir, normalized_query)) = context_browse_dir_from_query(query) else {
        return Vec::new();
    };
    let dir = root.join(relative_dir);
    if !dir.is_dir() {
        return Vec::new();
    }
    let allow_hidden = context_query_allows_hidden(query);
    let mut results = Vec::new();
    let mut scanned = 0usize;
    let Ok(children) = sorted_child_paths(&dir) else {
        return results;
    };
    for path in children {
        if scanned >= CONTEXT_SEARCH_SCAN_LIMIT || results.len() >= limit {
            break;
        }
        scanned += 1;
        if !allow_hidden && is_hidden_context_path(&path) {
            continue;
        }
        push_context_search_result(root, &path, &normalized_query, &mut results);
    }
    results
}

fn search_context_project(root: &Path, query: &str, limit: usize) -> Vec<ChatContextSearchResult> {
    let allow_hidden = context_query_allows_hidden(query);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!allow_hidden)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .ignore(true)
        .parents(true);

    let filter_root = root.to_path_buf();
    let mut results = Vec::new();
    let mut scanned = 0usize;
    builder.filter_entry(move |entry| {
        entry.path() == filter_root || !should_skip_context_search_dir(entry.path())
    });
    let walker = builder.build();

    for entry in walker {
        if scanned >= CONTEXT_SEARCH_SCAN_LIMIT || results.len() >= limit {
            break;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path == root {
            continue;
        }
        scanned += 1;
        push_context_search_result(root, path, query, &mut results);
    }
    results
}

#[tauri::command]
fn chat_describe_attachments(paths: Vec<String>) -> Result<Vec<ChatAttachment>, String> {
    Ok(paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .map(describe_attachment_path)
        .collect())
}

#[cfg(windows)]
fn read_windows_clipboard_file_paths() -> Result<Vec<String>, String> {
    use clipboard_win::{formats::FileList, Clipboard, Getter};

    let _clipboard = Clipboard::new_attempts(10)
        .map_err(|e| format!("打开剪贴板失败：{e}"))?;
    let mut paths = Vec::<String>::new();
    match FileList.read_clipboard(&mut paths) {
        Ok(_) => Ok(paths),
        Err(_) => Ok(Vec::new()),
    }
}

#[cfg(not(windows))]
fn read_windows_clipboard_file_paths() -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

#[tauri::command]
fn chat_read_clipboard_file_paths() -> Result<Vec<String>, String> {
    read_windows_clipboard_file_paths().map(|paths| {
        paths
            .into_iter()
            .filter(|path| !path.trim().is_empty())
            .collect()
    })
}

#[tauri::command]
fn chat_save_clipboard_image(input: ClipboardImageInput) -> Result<ChatAttachment, String> {
    let home = store::resolve_lilia_home();
    let display_seq = CLIPBOARD_IMAGE_DISPLAY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    save_clipboard_image_to_cache(&home, input, now_millis(), display_seq)
}

#[tauri::command]
fn chat_save_clipboard_text(input: ClipboardTextInput) -> Result<ChatAttachment, String> {
    let home = store::resolve_lilia_home();
    let display_seq = CLIPBOARD_TEXT_DISPLAY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    save_clipboard_text_to_cache(&home, input, now_millis(), display_seq)
}

#[tauri::command]
fn chat_search_context_attachments(
    project_cwd: String,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ChatContextSearchResult>, String> {
    let root = PathBuf::from(project_cwd.trim());
    if root.as_os_str().is_empty() || !root.is_dir() {
        return Ok(Vec::new());
    }
    let limit = limit
        .unwrap_or(DEFAULT_CONTEXT_SEARCH_LIMIT)
        .clamp(1, MAX_CONTEXT_SEARCH_LIMIT);
    let query = query.trim();
    let results = if context_query_is_path_like(query) {
        search_context_browse_dir(&root, query, limit)
    } else {
        search_context_project(&root, query, limit)
    };

    Ok(results)
}

#[cfg(test)]
mod context_search_tests {
    use super::*;

    fn temp_context_root(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("lilia-context-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn relative_paths(results: &[ChatContextSearchResult]) -> Vec<String> {
        results
            .iter()
            .map(|result| result.relative_path.clone())
            .collect()
    }

    #[test]
    fn clipboard_image_extension_uses_mime_then_safe_name_extension() {
        assert_eq!(
            clipboard_image_extension(Some("image/jpeg"), Some("ignored.png")),
            "jpg"
        );
        assert_eq!(
            clipboard_image_extension(None, Some("screen.webp")),
            "webp"
        );
        assert_eq!(
            clipboard_image_extension(Some("text/plain"), Some("note.txt")),
            "png"
        );
    }

    #[test]
    fn clipboard_image_display_name_uses_short_sequence_name() {
        assert_eq!(clipboard_image_display_name("png", 1), "图片 1.png");
        assert_eq!(clipboard_image_display_name("webp", 12), "图片 12.webp");
    }

    #[test]
    fn clipboard_text_display_name_uses_short_sequence_name() {
        assert_eq!(clipboard_text_display_name(1), "粘贴文本 1.txt");
        assert_eq!(clipboard_text_display_name(12), "粘贴文本 12.txt");
    }

    #[test]
    fn clipboard_image_is_saved_under_cache_and_described_as_image_file() {
        let home = temp_context_root("clipboard-image");
        let input = ClipboardImageInput {
            mime: Some("image/png".to_string()),
            bytes_base64: general_purpose::STANDARD.encode([1_u8, 2, 3, 4]),
            name: None,
        };

        let attachment = save_clipboard_image_to_cache(&home, input, 12345, 1).unwrap();

        assert_eq!(attachment.name, "图片 1.png");
        assert_eq!(attachment.kind, "file");
        assert_eq!(attachment.exists, true);
        assert_eq!(attachment.mime.as_deref(), Some("image/png"));
        let path = PathBuf::from(&attachment.path);
        assert!(path.exists());
        assert_eq!(
            path.parent().unwrap(),
            clipboard_images_cache_dir(&home).as_path()
        );
        assert_eq!(fs::read(&path).unwrap(), vec![1_u8, 2, 3, 4]);

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn clipboard_text_is_saved_under_cache_and_described_as_file() {
        let home = temp_context_root("clipboard-text");
        let input = ClipboardTextInput {
            text: "很长的粘贴文本\nwith ascii".to_string(),
        };

        let attachment = save_clipboard_text_to_cache(&home, input, 12345, 1).unwrap();

        assert_eq!(attachment.name, "粘贴文本 1.txt");
        assert_eq!(attachment.kind, "file");
        assert_eq!(attachment.exists, true);
        assert_eq!(attachment.mime, None);
        assert_eq!(attachment.size, Some("很长的粘贴文本\nwith ascii".len() as u64));
        let path = PathBuf::from(&attachment.path);
        assert!(path.exists());
        assert_eq!(
            path.parent().unwrap(),
            clipboard_texts_cache_dir(&home).as_path()
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "很长的粘贴文本\nwith ascii"
        );

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn context_search_hides_dot_files_until_query_contains_dot() {
        let root = temp_context_root("hidden");
        write_file(&root.join(".env"), "token=1");
        write_file(&root.join("src").join("env.ts"), "export {}");

        let hidden = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "env".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(!hidden.contains(&".env".to_string()));
        assert!(hidden.contains(&"src/env.ts".to_string()));

        let explicit = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                ".".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(explicit.contains(&".env".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn context_search_hides_gitignored_entries_until_path_query() {
        let root = temp_context_root("ignored");
        write_file(&root.join(".gitignore"), "dist/\n");
        write_file(&root.join("dist").join("app.js"), "console.log(1)");
        write_file(&root.join("src").join("dist-note.md"), "note");

        let hidden = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "dist".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(!hidden.contains(&"dist".to_string()));
        assert!(!hidden.contains(&"dist/app.js".to_string()));
        assert!(hidden.contains(&"src/dist-note.md".to_string()));

        let explicit = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "dist/".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(explicit.contains(&"dist/app.js".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn context_path_query_lists_direct_children_only() {
        let root = temp_context_root("browse");
        write_file(&root.join("big-dir").join("inside.md"), "inside");
        write_file(&root.join("big-dir").join("nested").join("deep.md"), "deep");

        let paths = relative_paths(
            &chat_search_context_attachments(
                root.to_string_lossy().to_string(),
                "big-dir/".to_string(),
                Some(20),
            )
            .unwrap(),
        );
        assert!(paths.contains(&"big-dir/inside.md".to_string()));
        assert!(paths.contains(&"big-dir/nested".to_string()));
        assert!(!paths.contains(&"big-dir/nested/deep.md".to_string()));

        let _ = fs::remove_dir_all(root);
    }
}

#[tauri::command]
fn chat_send_message(
    app: AppHandle,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    guide_id: Option<String>,
    store: State<'_, ChatStore>,
) -> Result<ChatSendResult, String> {
    let active_backend = load_active_backend(&app);
    validate_backend_ready_for_send(&active_backend)?;
    let composer = normalize_composer_for_backend(composer, &task_id, &active_backend);
    // 1) 写入 user 消息并立即返回，给前端一个乐观渲染的锚点。
    let user_msg = ChatMessage {
        id: new_chat_message_id(),
        task_id: task_id.clone(),
        role: "user".to_string(),
        content: content.clone(),
        attachments: attachments.clone(),
        created_at: now_millis(),
    };
    // turn_id 在 user 消息入库前就分配，并与 agent turn 共享 —— 让两者落到同一个
    // turn_seq，user 消息天然占据 turn 内 intra_turn_order=0 的位置。
    let turn_id = format!("turn-{}", now_millis());
    store
        .composers
        .lock()
        .unwrap()
        .insert(task_id.clone(), composer.clone());

    {
        let mut running = store.running_tasks.lock().unwrap();
        if running.contains_key(&task_id) {
            drop(running);
            set_guide_status_for_app(&app, guide_id.as_deref(), "queued")?;
            let queued_count = queue_pending_turn(
                &store,
                &task_id,
                content,
                composer.clone(),
                project_cwd,
                attachments,
                user_msg.clone(),
                turn_id.clone(),
                guide_id.clone(),
            );
            persist_and_emit_message_timeline_event(
                &app,
                &user_msg,
                &composer.backend,
                &turn_id,
                true,
            );
            return Ok(ChatSendResult {
                message: user_msg,
                dispatch: "queued".to_string(),
                queued_count,
            });
        }
        running.insert(task_id.clone(), true);
    }

    set_guide_status_for_app(&app, guide_id.as_deref(), "sent")?;
    persist_and_emit_message_timeline_event(&app, &user_msg, &composer.backend, &turn_id, false);

    spawn_agent_turn(
        app,
        task_id,
        content,
        composer,
        project_cwd,
        attachments,
        turn_id,
    );

    Ok(ChatSendResult {
        message: user_msg,
        dispatch: "started".to_string(),
        queued_count: 0,
    })
}

#[tauri::command]
fn chat_interrupt_turn(
    task_id: String,
    app: AppHandle,
    store: State<'_, ChatStore>,
) -> Result<(), String> {
    stop_running_turn(&app, &store, &task_id, true, false).map(|_| ())
}

/// 把用户对一次工具调用的决策（allow / deny）写回 runner 的 stdin。
/// 通过 ChatStore.running_stdins 找到该 task 当前的 runner 子进程；若进程已退出
/// （比如用户拖太久 turn 已 timeout / cancel），静默返回——SDK 端的 promise 也
/// 会随子进程死亡被丢弃，没有进一步副作用。
#[tauri::command]
fn chat_respond_tool_consent(
    task_id: String,
    request_id: String,
    decision: String,
    message: Option<String>,
    updated_input: Option<JsonValue>,
    store: State<'_, ChatStore>,
) -> Result<(), String> {
    let decision_norm = if decision == "allow" { "allow" } else { "deny" };
    let mut payload = serde_json::json!({
        "type": "consent_response",
        "id": request_id,
        "decision": decision_norm,
        "message": message.unwrap_or_default(),
    });
    if let Some(value) = updated_input {
        if !value.is_null() {
            payload["updatedInput"] = value;
        }
    }
    let mut line = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    line.push('\n');

    let handle = {
        let map = store.running_stdins.lock().unwrap();
        map.get(&task_id).cloned()
    };
    let Some(handle) = handle else {
        return Ok(()); // runner 已退出；忽略
    };
    let mut stdin = handle.lock().map_err(|e| e.to_string())?;
    stdin
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// 把用户对 Claude AskUserQuestion 的回答写回 runner 的 stdin。
#[tauri::command]
fn chat_respond_ask_user(
    task_id: String,
    request_id: String,
    result: JsonValue,
    store: State<'_, ChatStore>,
) -> Result<(), String> {
    let payload = serde_json::json!({
        "type": "ask_user_response",
        "id": request_id,
        "result": result,
    });
    let mut line = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    line.push('\n');

    let handle = {
        let map = store.running_stdins.lock().unwrap();
        map.get(&task_id).cloned()
    };
    let Some(handle) = handle else {
        return Ok(());
    };
    let mut stdin = handle.lock().map_err(|e| e.to_string())?;
    stdin
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn chat_list_models(backend: String) -> Vec<ChatModelOption> {
    model_options_for_backend(&backend)
}

#[tauri::command]
fn chat_get_composer_state(task_id: String, store: State<'_, ChatStore>) -> ChatComposerState {
    store
        .composers
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .unwrap_or_else(|| default_composer(&task_id))
}

#[tauri::command]
fn chat_set_composer_state(state: ChatComposerState, store: State<'_, ChatStore>) {
    store
        .composers
        .lock()
        .unwrap()
        .insert(state.task_id.clone(), state);
}

#[tauri::command]
fn chat_reset_session(task_id: String, chat_store: State<'_, ChatStore>, app: AppHandle) {
    let mut sessions = chat_store.sdk_sessions.lock().unwrap();
    sessions.remove(&session_key(BACKEND_CLAUDE, &task_id));
    sessions.remove(&session_key(BACKEND_CODEX, &task_id));
    drop(sessions);
    chat_store.running_tasks.lock().unwrap().remove(&task_id);
    let stopped_running = match stop_running_turn(&app, &chat_store, &task_id, false, true) {
        Ok(stopped) => stopped,
        Err(err) => {
            eprintln!("[chat] reset running turn failed: {err}");
            false
        }
    };
    reset_cleared_guide_queue(&app, clear_pending_turns(&chat_store, &task_id));
    chat_store
        .interrupted_turns
        .lock()
        .unwrap()
        .remove(&task_id);
    if !stopped_running {
        chat_store.reset_turns.lock().unwrap().remove(&task_id);
    }
    clear_running_handles(&chat_store, &task_id);
    if let Some(store) = app.try_state::<LiliaStore>() {
        if let Err(err) = store
            .conn()
            .and_then(|conn| agent_timeline::clear(&conn, &task_id).map(|_| ()))
        {
            eprintln!("[agent-timeline] clear on reset failed: {err}");
        }
    }
}

fn save_provider_store_value<T: Serialize>(
    app: &AppHandle,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let store = app
        .store(PROVIDER_STORE_FILE)
        .map_err(|e| format!("打开配置存储失败：{e}"))?;
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    store.set(key, value);
    store.save().map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(())
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_popup_window_settings(settings: PopupWindowSettings) -> PopupWindowSettings {
    PopupWindowSettings {
        shortcut: normalize_optional_string(settings.shortcut),
    }
}

fn load_popup_window_settings(app: &AppHandle) -> PopupWindowSettings {
    let read = || -> Option<PopupWindowSettings> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let value = store.get(POPUP_WINDOW_SETTINGS_KEY)?;
        serde_json::from_value::<PopupWindowSettings>(value).ok()
    };
    normalize_popup_window_settings(read().unwrap_or_default())
}

fn save_popup_window_settings(
    app: &AppHandle,
    settings: &PopupWindowSettings,
) -> Result<(), String> {
    save_provider_store_value(app, POPUP_WINDOW_SETTINGS_KEY, settings)
}

fn load_popup_last_project_id(app: &AppHandle) -> Option<String> {
    let store = app.store(PROVIDER_STORE_FILE).ok()?;
    let value = store.get(POPUP_LAST_PROJECT_KEY)?;
    normalize_optional_string(serde_json::from_value::<String>(value).ok())
}

fn save_popup_last_project_id(app: &AppHandle, project_id: &str) -> Result<(), String> {
    save_provider_store_value(app, POPUP_LAST_PROJECT_KEY, &project_id)
}

fn project_exists(app: &AppHandle, project_id: &str) -> bool {
    let Some(store) = app.try_state::<LiliaStore>() else {
        return false;
    };
    let Ok(conn) = store.conn() else {
        return false;
    };
    conn.query_row(
        "SELECT 1 FROM projects WHERE id = ?1 LIMIT 1",
        rusqlite::params![project_id],
        |_| Ok(()),
    )
    .is_ok()
}

fn sanitize_window_label_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "unknown".to_string()
    } else {
        out
    }
}

fn unique_popup_label(kind: &str) -> String {
    let seq = POPUP_WINDOW_SEQ.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    format!("{POPUP_WINDOW_PREFIX}{kind}-{now}-{seq}")
}

fn popup_task_label(task_id: &str) -> String {
    format!(
        "{POPUP_EXISTING_TASK_PREFIX}{}",
        sanitize_window_label_segment(task_id)
    )
}

fn normalize_popup_route(route: &str) -> String {
    let route = route.trim();
    if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    }
}

fn popup_route_bootstrap_script(route: &str) -> String {
    let route = serde_json::to_string(&normalize_popup_route(route))
        .unwrap_or_else(|_| "\"/popup/chats/new\"".to_string());
    format!("window.location.hash = \"#\" + {route};")
}

fn focus_window(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn build_popup_window(app: &AppHandle, label: String, route: String) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(&label) {
        existing
            .emit(
                "lilia:popup:navigate",
                MainNavigateEvent {
                    route: normalize_popup_route(&route),
                },
            )
            .map_err(|e| format!("通知弹出窗口导航失败：{e}"))?;
        focus_window(&existing);
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App("index.html".into()),
    )
        .initialization_script(popup_route_bootstrap_script(&route))
        .title("LiliaCode")
        .inner_size(POPUP_WIDTH, POPUP_HEIGHT)
        .min_inner_size(POPUP_MIN_WIDTH, POPUP_MIN_HEIGHT)
        .center()
        .decorations(false)
        .resizable(true)
        .background_color(BG)
        .build()
        .map_err(|e| format!("创建弹出窗口失败：{e}"))?;
    focus_window(&window);
    Ok(())
}

fn open_new_chat_window(
    app: &AppHandle,
    project_id: Option<String>,
    allow_orphan_fallback: bool,
) -> Result<(), String> {
    let route = if let Some(project_id) = normalize_optional_string(project_id) {
        if project_exists(app, &project_id) {
            save_popup_last_project_id(app, &project_id)?;
            format!("popup/projects/{project_id}/new")
        } else if allow_orphan_fallback {
            "popup/chats/new".to_string()
        } else {
            return Err("项目不存在，无法在弹出窗口中创建对话".to_string());
        }
    } else {
        "popup/chats/new".to_string()
    };
    build_popup_window(app, unique_popup_label("new"), route)
}

fn open_popup_for_shortcut(app: &AppHandle) -> Result<(), String> {
    let project_id = load_popup_last_project_id(app).filter(|id| project_exists(app, id));
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(err) = open_new_chat_window(&app, project_id, true) {
            eprintln!("{err}");
        }
    });
    Ok(())
}

fn register_popup_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("注册弹出窗口快捷键失败：{e}"))
}

fn unregister_popup_shortcut(app: &AppHandle, shortcut: &str) {
    if let Err(err) = app.global_shortcut().unregister(shortcut) {
        eprintln!("[popup-window] unregister shortcut failed: {err}");
    }
}

fn register_initial_popup_shortcut(app: &AppHandle) {
    let settings = load_popup_window_settings(app);
    if let Some(shortcut) = settings.shortcut.as_deref() {
        if let Err(err) = register_popup_shortcut(app, shortcut) {
            eprintln!("[popup-window] register shortcut failed: {err}");
        }
    }
}

#[tauri::command]
fn popup_get_window_settings(app: AppHandle) -> PopupWindowSettings {
    load_popup_window_settings(&app)
}

#[tauri::command]
fn popup_set_window_settings(
    app: AppHandle,
    settings: PopupWindowSettings,
) -> Result<(), String> {
    let previous = load_popup_window_settings(&app);
    let next = normalize_popup_window_settings(settings);
    if previous.shortcut == next.shortcut {
        return save_popup_window_settings(&app, &next);
    }

    if let Some(shortcut) = previous.shortcut.as_deref() {
        unregister_popup_shortcut(&app, shortcut);
    }

    if let Some(shortcut) = next.shortcut.as_deref() {
        if let Err(err) = register_popup_shortcut(&app, shortcut) {
            if let Some(previous_shortcut) = previous.shortcut.as_deref() {
                let _ = register_popup_shortcut(&app, previous_shortcut);
            }
            return Err(err);
        }
    }

    save_popup_window_settings(&app, &next)
}

#[tauri::command]
fn popup_remember_last_project(app: AppHandle, project_id: String) -> Result<(), String> {
    let project_id = project_id.trim();
    if project_id.is_empty() || !project_exists(&app, project_id) {
        return Ok(());
    }
    save_popup_last_project_id(&app, project_id)
}

#[tauri::command]
async fn popup_open_new_chat(app: AppHandle, project_id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_new_chat_window(&app, project_id, false))
        .await
        .map_err(|err| format!("弹出窗口任务执行失败：{err}"))?
}

#[tauri::command]
async fn popup_open_task(
    app: AppHandle,
    project_id: Option<String>,
    task_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let task_id = task_id.trim();
        if task_id.is_empty() {
            return Err("缺少对话 ID".to_string());
        }
        let route = if let Some(project_id) = normalize_optional_string(project_id) {
            save_popup_last_project_id(&app, &project_id)?;
            format!("popup/projects/{project_id}/tasks/{task_id}")
        } else {
            format!("popup/chats/{task_id}")
        };
        build_popup_window(&app, popup_task_label(task_id), route)
    })
    .await
    .map_err(|err| format!("弹出窗口任务执行失败：{err}"))?
}

#[tauri::command]
fn popup_focus_main(app: AppHandle, route: String) -> Result<(), String> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Err("主窗口不存在".to_string());
    };
    window
        .emit("lilia:main:navigate", MainNavigateEvent { route })
        .map_err(|e| format!("通知主窗口导航失败：{e}"))?;
    focus_window(&window);
    Ok(())
}

fn build_backend_env_status(app: &AppHandle, backend: &str) -> BackendEnvStatus {
    let plan = resolve_connection_for(app, backend);

    // has_api_key 综合 plan / env / direct 配置三处；让 UI 区分「没配过」和「配了但当前路由没用上」。
    let key_env = match backend {
        BACKEND_CODEX => "OPENAI_API_KEY",
        _ => "ANTHROPIC_API_KEY",
    };
    let has_api_key = plan
        .api_key
        .as_ref()
        .map(|k| !k.is_empty())
        .unwrap_or(false)
        || env::var(key_env).map(|v| !v.is_empty()).unwrap_or(false)
        || load_provider_config(
            app,
            if backend == BACKEND_CODEX {
                PROVIDER_KEY_CODEX
            } else {
                PROVIDER_KEY_CLAUDE
            },
        )
        .and_then(|c| c.api_key.filter(|s| !s.is_empty()))
        .is_some();

    let effective_url = match plan.mode {
        ConnectionMode::CcSwitch => plan.base_url.clone(),
        ConnectionMode::CustomBaseUrl => plan.base_url.clone(),
        ConnectionMode::Direct => match backend {
            BACKEND_CODEX => Some("https://api.openai.com/v1".to_string()),
            _ => Some("https://api.anthropic.com".to_string()),
        },
        ConnectionMode::Unconfigured => None,
    };

    BackendEnvStatus {
        backend: backend.to_string(),
        has_api_key,
        connection_mode: plan.mode.as_str().to_string(),
        effective_url,
    }
}

fn build_cc_switch_status(app: &AppHandle) -> CCSwitchStatus {
    let cfg = load_cc_switch_config(app);
    let url = cfg.base_url.filter(|s| !s.is_empty());
    CCSwitchStatus {
        reachable: url_reachable(url.as_deref()),
        base_url: url,
    }
}

fn cli_available(name: &str) -> bool {
    let candidates: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    for ext in candidates {
        let candidate = format!("{name}{ext}");
        let ok = Command::new(&candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexCliProbe {
    program: String,
    version_output: String,
    app_server_help: String,
}

#[derive(Debug, Clone)]
struct CodexAppServerProbeStatus {
    public: CodexAppServerStatus,
    path: Option<String>,
}

fn codex_candidate_filenames() -> &'static [&'static str] {
    if cfg!(windows) {
        &["codex.cmd", "codex.exe", "codex.bat", "codex"]
    } else {
        &["codex"]
    }
}

fn windows_native_codex_paths() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let Some(local_app_data) = env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    let bin = PathBuf::from(local_app_data).join("OpenAI").join("Codex").join("bin");
    let mut paths = Vec::new();
    let root_codex = bin.join("codex.exe");
    if root_codex.exists() {
        paths.push(root_codex.to_string_lossy().to_string());
    }
    if let Ok(entries) = fs::read_dir(&bin) {
        for entry in entries.flatten() {
            let path = entry.path().join("codex.exe");
            if path.exists() {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    paths
}

fn normalize_candidate_key(path: &str) -> String {
    if cfg!(windows) {
        path.to_ascii_lowercase()
    } else {
        path.to_string()
    }
}

fn push_unique_candidate(candidates: &mut Vec<String>, seen: &mut HashSet<String>, path: String) {
    if path.trim().is_empty() {
        return;
    }
    let key = normalize_candidate_key(path.trim());
    if seen.insert(key) {
        candidates.push(path.trim().to_string());
    }
}

fn where_codex_candidates() -> Vec<String> {
    let output = Command::new(if cfg!(windows) { "where.exe" } else { "which" })
        .arg("codex")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn path_codex_candidates() -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            for filename in codex_candidate_filenames() {
                let candidate = dir.join(filename);
                if candidate.exists() {
                    paths.push(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    paths
}

fn codex_cli_candidate_paths() -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let path_candidates = path_codex_candidates();
    let where_candidates = where_codex_candidates();
    for path in &path_candidates {
        let is_windows_apps = cfg!(windows) && path.to_ascii_lowercase().contains("\\windowsapps\\");
        if !is_windows_apps {
            push_unique_candidate(&mut candidates, &mut seen, path.clone());
        }
    }
    for path in windows_native_codex_paths() {
        push_unique_candidate(&mut candidates, &mut seen, path);
    }
    for path in &where_candidates {
        let is_windows_apps = cfg!(windows) && path.to_ascii_lowercase().contains("\\windowsapps\\");
        if !is_windows_apps {
            push_unique_candidate(&mut candidates, &mut seen, path.clone());
        }
    }
    for candidate in codex_candidate_filenames() {
        push_unique_candidate(&mut candidates, &mut seen, (*candidate).to_string());
    }
    for path in path_candidates.into_iter().chain(where_candidates) {
        let is_windows_apps = cfg!(windows) && path.to_ascii_lowercase().contains("\\windowsapps\\");
        if is_windows_apps {
            push_unique_candidate(&mut candidates, &mut seen, path);
        }
    }
    candidates
}

fn command_output_result(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !err.is_empty() { err } else { out };
        return Err(if detail.is_empty() {
            format!("command exited with {}", output.status)
        } else {
            detail
        });
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !text.is_empty() {
        return Ok(text);
    }
    let text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if text.is_empty() {
        Err("command produced no output".to_string())
    } else {
        Ok(text)
    }
}

fn resolve_codex_cli_probe_with<F>(candidates: &[String], mut command_output: F) -> Option<CodexCliProbe>
where
    F: FnMut(&str, &[&str]) -> Result<String, String>,
{
    for candidate in candidates {
        let version = match command_output(candidate, &["--version"]) {
            Ok(output) => output,
            Err(_) => continue,
        };
        let parsed_version = parse_codex_cli_version(&version);
        let help = match command_output(candidate, &["app-server", "--help"]) {
            Ok(output) => output,
            Err(_) => continue,
        };
        let app_server_available = help.contains("codex app-server") || help.contains("Usage:");
        let version_supported = parsed_version
            .map(|version| codex_version_at_least(version, MIN_CODEX_APP_SERVER_VERSION))
            .unwrap_or(false);
        if app_server_available && version_supported {
            return Some(CodexCliProbe {
                program: candidate.clone(),
                version_output: version,
                app_server_help: help,
            });
        }
    }
    None
}

fn parse_codex_cli_version(output: &str) -> Option<(u32, u32, u32)> {
    let version = output
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    let patch_text = parts.next().unwrap_or("0");
    let patch_digits: String = patch_text
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    let patch = patch_digits.parse::<u32>().ok()?;
    Some((major, minor, patch))
}

fn codex_version_at_least(version: (u32, u32, u32), minimum: (u32, u32, u32)) -> bool {
    version >= minimum
}

fn build_codex_app_server_probe_status_with<F>(
    candidates: &[String],
    mut command_output: F,
) -> CodexAppServerProbeStatus
where
    F: FnMut(&str, &[&str]) -> Result<String, String>,
{
    let codex_cli = resolve_codex_cli_probe_with(candidates, &mut command_output);
    let mut issues = Vec::new();
    let Some(codex_cli) = codex_cli else {
        if candidates.is_empty() {
            issues.push("未找到 codex CLI。请先安装 Codex CLI 后重新检测。".to_string());
        } else {
            issues.push("Codex app-server 环境不满足。".to_string());
        }
        return CodexAppServerProbeStatus {
            public: CodexAppServerStatus {
                version: None,
                available: false,
                supports_required_protocol: false,
                issues,
            },
            path: None,
        };
    };

    let parsed_version = parse_codex_cli_version(&codex_cli.version_output);
    if parsed_version.is_none() {
        issues.push("无法读取 codex CLI 版本。".to_string());
    }

    let available = codex_cli.app_server_help.contains("codex app-server")
        || codex_cli.app_server_help.contains("Usage:");
    if !available {
        issues.push("当前 codex CLI 不支持 app-server 子命令。".to_string());
    }

    let version_supported = parsed_version
        .map(|version| codex_version_at_least(version, MIN_CODEX_APP_SERVER_VERSION))
        .unwrap_or(false);
    if !version_supported {
        issues.push("当前 codex CLI 版本过低，需要 0.128.0 或更新版本以支持 Lilia 的流式事件、工具审批和 AskUser。".to_string());
    }

    let path = Some(codex_cli.program);
    CodexAppServerProbeStatus {
        public: CodexAppServerStatus {
            version: Some(codex_cli.version_output),
            available,
            supports_required_protocol: available && version_supported,
            issues,
        },
        path,
    }
}

fn build_codex_app_server_probe_status() -> CodexAppServerProbeStatus {
    build_codex_app_server_probe_status_with(&codex_cli_candidate_paths(), command_output_result)
}

fn codex_send_block_reason(status: &CodexAppServerStatus) -> Option<String> {
    if status.supports_required_protocol {
        return None;
    }

    let detail = if status.issues.is_empty() {
        "Codex app-server 环境不满足。".to_string()
    } else {
        status.issues.join(" ")
    };
    Some(format!(
        "{detail} 请升级 Codex CLI 到 0.128.0 或更新版本；并确认 CC-Switch 当前选中的上游 provider 支持 OpenAI Responses API 与 Codex 模型白名单。"
    ))
}

fn validate_backend_ready_for_send(active_backend: &str) -> Result<(), String> {
    if active_backend != BACKEND_CODEX {
        return Ok(());
    }
    let status = build_codex_app_server_probe_status();
    if let Some(reason) = codex_send_block_reason(&status.public) {
        return Err(reason);
    }
    Ok(())
}

#[tauri::command]
fn chat_check_env(app: AppHandle) -> EnvStatusReport {
    let node_available = cli_available("node");
    let codex_app_server = build_codex_app_server_probe_status();
    let codex_cli_available = codex_app_server.path.is_some();

    let mut backends = HashMap::new();
    backends.insert(
        BACKEND_CLAUDE.to_string(),
        build_backend_env_status(&app, BACKEND_CLAUDE),
    );
    backends.insert(
        BACKEND_CODEX.to_string(),
        build_backend_env_status(&app, BACKEND_CODEX),
    );

    let mut router_modes = HashMap::new();
    router_modes.insert(
        BACKEND_CLAUDE.to_string(),
        load_router_mode(&app, BACKEND_CLAUDE),
    );
    router_modes.insert(
        BACKEND_CODEX.to_string(),
        load_router_mode(&app, BACKEND_CODEX),
    );

    EnvStatusReport {
        node_available,
        codex_cli_available,
        codex_app_server: codex_app_server.public,
        cc_switch: build_cc_switch_status(&app),
        router_modes,
        backends,
    }
}

#[tauri::command]
fn provider_get_config(app: AppHandle, backend: String) -> ProviderConfig {
    let key = match backend.as_str() {
        BACKEND_CODEX => PROVIDER_KEY_CODEX,
        _ => PROVIDER_KEY_CLAUDE,
    };
    load_provider_config(&app, key).unwrap_or_else(|| ProviderConfig {
        backend: backend.clone(),
        base_url: None,
        api_key: None,
    })
}

#[tauri::command]
fn provider_set_config(app: AppHandle, config: ProviderConfig) -> Result<(), String> {
    let key = match config.backend.as_str() {
        BACKEND_CODEX => PROVIDER_KEY_CODEX,
        BACKEND_CLAUDE => PROVIDER_KEY_CLAUDE,
        other => return Err(format!("未知 backend: {other}")),
    };
    save_provider_store_value(&app, key, &config)
}

#[tauri::command]
fn provider_get_active_backend(app: AppHandle) -> String {
    load_active_backend(&app)
}

#[tauri::command]
fn provider_set_active_backend(app: AppHandle, backend: String) -> Result<(), String> {
    match backend.as_str() {
        BACKEND_CLAUDE | BACKEND_CODEX => {
            save_provider_store_value(&app, PROVIDER_ACTIVE_BACKEND_KEY, &backend)
        }
        other => Err(format!("未知 backend: {other}")),
    }
}

#[tauri::command]
fn cc_switch_get_config(app: AppHandle) -> CCSwitchConfig {
    load_cc_switch_config(&app)
}

#[tauri::command]
fn cc_switch_set_config(app: AppHandle, config: CCSwitchConfig) -> Result<(), String> {
    save_provider_store_value(&app, CC_SWITCH_KEY, &config)
}

#[tauri::command]
fn assistant_ai_get_config(app: AppHandle) -> AssistantAIConfig {
    load_assistant_ai_config(&app)
}

#[tauri::command]
fn assistant_ai_set_config(app: AppHandle, config: AssistantAIConfig) -> Result<(), String> {
    save_provider_store_value(&app, ASSISTANT_AI_KEY, &config)
}

#[tauri::command]
fn agent_interaction_get_settings(app: AppHandle) -> AgentInteractionSettings {
    load_agent_interaction_settings(&app)
}

#[tauri::command]
fn agent_interaction_set_settings(
    app: AppHandle,
    settings: AgentInteractionSettings,
) -> Result<(), String> {
    save_provider_store_value(&app, AGENT_INTERACTION_KEY, &settings)
}

/// 连通性 ping：GET {baseUrl}/models，3 秒超时。
/// 不消耗 token，能同时验证 baseUrl 可达、apiKey 被接受、配置的 model 是否在列表里。
#[tauri::command]
fn assistant_ai_test_connection(config: AssistantAIConfig) -> AssistantAITestResult {
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

#[tauri::command]
fn router_get_mode(app: AppHandle, backend: String) -> String {
    load_router_mode(&app, &backend)
}

#[tauri::command]
fn router_set_mode(app: AppHandle, backend: String, mode: String) -> Result<(), String> {
    if !matches!(mode.as_str(), ROUTER_CC_SWITCH | ROUTER_DIRECT) {
        return Err(format!("未知路由模式: {mode}"));
    }
    let key = match backend.as_str() {
        BACKEND_CODEX => ROUTER_KEY_CODEX,
        BACKEND_CLAUDE => ROUTER_KEY_CLAUDE,
        other => return Err(format!("未知 backend: {other}")),
    };
    save_provider_store_value(&app, key, &JsonValue::String(mode))
}

// ---------- Project / Git ----------

fn load_project_settings(app: &AppHandle) -> ProjectSettings {
    let read = || -> Option<ProjectSettings> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let raw = store.get(PROJECT_CLONE_PARENT_KEY)?;
        // 兼容历史可能存的纯字符串。
        if let Some(s) = raw.as_str() {
            return Some(ProjectSettings {
                clone_parent_dir: Some(s.to_string()),
            });
        }
        serde_json::from_value::<ProjectSettings>(raw).ok()
    };
    read().unwrap_or_default()
}

#[tauri::command]
fn project_get_settings(app: AppHandle) -> ProjectSettings {
    load_project_settings(&app)
}

#[tauri::command]
fn project_set_settings(app: AppHandle, settings: ProjectSettings) -> Result<(), String> {
    save_provider_store_value(&app, PROJECT_CLONE_PARENT_KEY, &settings)
}

/// 从 git URL 推断仓库目录名。`https://github.com/foo/bar.git` → `bar`。
fn derive_repo_dir_name(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let stripped = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let last = stripped
        .rsplit(|c| c == '/' || c == ':')
        .next()
        .unwrap_or("");
    let cleaned = last.trim().trim_end_matches('/');
    if cleaned.is_empty() {
        "repo".to_string()
    } else {
        cleaned.to_string()
    }
}

/// 在已有的同级目录里挑一个不冲突的名字：`bar`、`bar-2`、`bar-3`…
fn unique_target_path(parent: &Path, base_name: &str) -> PathBuf {
    let candidate = parent.join(base_name);
    if !candidate.exists() {
        return candidate;
    }
    for i in 2..1024 {
        let p = parent.join(format!("{base_name}-{i}"));
        if !p.exists() {
            return p;
        }
    }
    parent.join(base_name)
}

/// 用系统默认文件管理器打开 `path` 指向的目录/文件。
/// Windows: 资源管理器；macOS: Finder；Linux: xdg-open。
#[tauri::command]
fn system_open_path(app: AppHandle, path: String) -> Result<(), String> {
    let p = path.trim();
    if p.is_empty() {
        return Err("路径为空".to_string());
    }
    if !Path::new(p).exists() {
        return Err(format!("路径不存在：{p}"));
    }
    app.opener()
        .open_path(p.to_string(), None::<&str>)
        .map_err(|e| format!("打开路径失败：{e}"))
}

/// 尝试用 VSCode 打开 `path`。
/// PATH 里依次找 `code` / `code.cmd` / `code.exe`；都找不到时返回友好错误。
#[tauri::command]
fn system_open_in_vscode(path: String) -> Result<(), String> {
    let p = path.trim();
    if p.is_empty() {
        return Err("路径为空".to_string());
    }
    if !Path::new(p).exists() {
        return Err(format!("路径不存在：{p}"));
    }
    let candidates: &[&str] = if cfg!(windows) {
        &["code.cmd", "code.exe", "code"]
    } else {
        &["code"]
    };
    let mut last_err: Option<String> = None;
    for cmd_name in candidates {
        match Command::new(cmd_name)
            .arg(p)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e.to_string());
                continue;
            }
        }
    }
    Err(format!(
        "未能启动 VSCode（请确认 `code` 命令在 PATH 中；可在 VSCode 内执行 Shell Command: Install 'code' command in PATH）：{}",
        last_err.unwrap_or_else(|| "unknown".to_string())
    ))
}

/// 同步调用 `git clone <url> <target>`；成功后返回 target 绝对路径。
#[tauri::command]
fn git_clone_repo(url: String, parent_dir: String) -> Result<String, String> {
    let url_trim = url.trim();
    if url_trim.is_empty() {
        return Err("仓库 URL 不能为空".to_string());
    }
    let parent_path = Path::new(parent_dir.trim());
    if !parent_path.is_dir() {
        return Err(format!("目标父目录不存在：{}", parent_path.display()));
    }
    let base = derive_repo_dir_name(url_trim);
    let target = unique_target_path(parent_path, &base);

    let output = Command::new("git")
        .arg("clone")
        .arg("--progress")
        .arg(url_trim)
        .arg(&target)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("无法启动 git（请确认 git 在 PATH 中）：{e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!(
            "git clone 失败：{}",
            if stderr.trim().is_empty() {
                format!("exit {}", output.status.code().unwrap_or(-1))
            } else {
                stderr.trim().to_string()
            }
        ));
    }

    Ok(target.to_string_lossy().to_string())
}

// ---------- Plugins / Skills ----------
// 委托给 `plugins.rs`，这里只做参数转译 + Tauri 错误包装。

#[tauri::command]
fn plugins_overview(app: AppHandle, project_cwd: Option<String>) -> PluginsOverview {
    plugins::overview(&app, project_cwd.as_deref())
}

#[tauri::command]
fn plugins_list_claude_skills(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
) -> Vec<ClaudeSkill> {
    plugins::list_claude_skills(&app, &scope, project_cwd.as_deref()).0
}

#[tauri::command]
fn plugins_create_claude_skill(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
    description: String,
) -> Result<ClaudeSkill, String> {
    plugins::create_claude_skill(&app, &scope, project_cwd.as_deref(), &name, &description)
}

#[tauri::command]
fn plugins_delete_claude_skill(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
) -> Result<(), String> {
    plugins::delete_claude_skill(&app, &scope, project_cwd.as_deref(), &name)
}

#[tauri::command]
fn plugins_set_claude_skill_enabled(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    plugins::set_claude_skill_enabled(&app, &scope, project_cwd.as_deref(), &name, enabled)
}

#[tauri::command]
fn plugins_list_claude_plugins(app: AppHandle, scope: String) -> Vec<ClaudePlugin> {
    plugins::list_claude_plugins(&app, &scope).0
}

#[tauri::command]
fn plugins_set_claude_plugin_enabled(
    app: AppHandle,
    scope: String,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    plugins::set_claude_plugin_enabled(&app, &scope, &name, enabled)
}

#[tauri::command]
fn plugins_create_claude_mcp_server(
    input: ClaudeMcpServerInput,
) -> Result<ClaudeMcpServer, String> {
    plugins::create_claude_mcp_server(input)
}

#[tauri::command]
fn plugins_update_claude_mcp_server(
    name: String,
    input: ClaudeMcpServerInput,
) -> Result<ClaudeMcpServer, String> {
    plugins::update_claude_mcp_server(&name, input)
}

#[tauri::command]
fn plugins_delete_claude_mcp_server(name: String) -> Result<(), String> {
    plugins::delete_claude_mcp_server(&name)
}

#[tauri::command]
fn plugins_set_claude_mcp_server_enabled(name: String, enabled: bool) -> Result<(), String> {
    plugins::set_claude_mcp_server_enabled(&name, enabled)
}

/// 用系统默认编辑器打开 Lilia 自管 Claude MCP 配置。
#[tauri::command]
fn plugins_open_claude_mcp_config(app: AppHandle) -> Result<(), String> {
    let path = plugins::claude_mcp_config_path();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建 Claude MCP 配置目录失败：{e}"))?;
        }
    }
    if !path.exists() {
        std::fs::write(&path, b"{\n  \"servers\": []\n}\n")
            .map_err(|e| format!("初始化 Claude MCP 配置失败：{e}"))?;
    }
    let path_str = path.to_string_lossy().to_string();
    app.opener()
        .open_path(path_str, None::<&str>)
        .map_err(|e| format!("打开 Claude MCP 配置失败：{e}"))?;
    Ok(())
}

#[tauri::command]
fn plugins_list_codex_mcp_servers(app: AppHandle) -> Vec<CodexMcpServer> {
    plugins::list_codex_mcp_servers(&app).0
}

/// 用系统默认编辑器打开 `~/.codex/config.toml`，文件不存在时先建一个空文件。
#[tauri::command]
fn plugins_open_codex_config(app: AppHandle) -> Result<(), String> {
    let path = plugins::codex_config_path(&app)?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建 .codex 目录失败：{e}"))?;
        }
    }
    if !path.exists() {
        std::fs::write(&path, b"").map_err(|e| format!("初始化 config.toml 失败：{e}"))?;
    }
    let path_str = path.to_string_lossy().to_string();
    app.opener()
        .open_path(path_str, None::<&str>)
        .map_err(|e| format!("打开 config.toml 失败：{e}"))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Err(err) = open_popup_for_shortcut(app) {
                            eprintln!("[popup-window] shortcut failed: {err}");
                        }
                    }
                })
                .build(),
        )
        .manage(ChatStore::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = window.set_background_color(Some(BG));
                if let Some(state) = window_state::load_main_window_state(app.handle()) {
                    window_state::restore_main_window_state(&window, state);
                }
                let _ = window.show();
            }
            // 初始化 lilia-store（SQLite）。失败时打印到 stderr 并继续运行——
            // 让 Tauri 窗口先出来，后续 Todo / Memory / Roadmap 命令会因取不到
            // State 而报错，但聊天主流程仍可用。
            let home = store::resolve_lilia_home();
            match LiliaStore::new(&home) {
                Ok(s) => {
                    app.manage(s);
                }
                Err(err) => {
                    eprintln!("[lilia-store] init failed at {}: {err}", home.display());
                }
            }
            register_initial_popup_shortcut(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }
            if matches!(
                event,
                WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
            ) {
                if let Some(webview_window) = window.get_webview_window(MAIN_WINDOW_LABEL) {
                    window_state::persist_main_window_state(&window.app_handle(), &webview_window);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            chat_describe_attachments,
            chat_read_clipboard_file_paths,
            chat_save_clipboard_image,
            chat_save_clipboard_text,
            chat_search_context_attachments,
            chat_send_message,
            chat_interrupt_turn,
            chat_respond_tool_consent,
            chat_respond_ask_user,
            chat_list_models,
            chat_get_composer_state,
            chat_set_composer_state,
            chat_reset_session,
            chat_check_env,
            provider_get_config,
            provider_set_config,
            provider_get_active_backend,
            provider_set_active_backend,
            cc_switch_get_config,
            cc_switch_set_config,
            assistant_ai_get_config,
            assistant_ai_set_config,
            assistant_ai_test_connection,
            agent_interaction_get_settings,
            agent_interaction_set_settings,
            popup_get_window_settings,
            popup_set_window_settings,
            popup_remember_last_project,
            popup_open_new_chat,
            popup_open_task,
            popup_focus_main,
            router_get_mode,
            router_set_mode,
            project_get_settings,
            project_set_settings,
            git_clone_repo,
            system_open_path,
            system_open_in_vscode,
            plugins_overview,
            plugins_list_claude_skills,
            plugins_create_claude_skill,
            plugins_delete_claude_skill,
            plugins_set_claude_skill_enabled,
            plugins_list_claude_plugins,
            plugins_set_claude_plugin_enabled,
            plugins_create_claude_mcp_server,
            plugins_update_claude_mcp_server,
            plugins_delete_claude_mcp_server,
            plugins_set_claude_mcp_server_enabled,
            plugins_open_claude_mcp_config,
            plugins_list_codex_mcp_servers,
            plugins_open_codex_config,
            todos::todo_list,
            todos::todo_create,
            todos::todo_update,
            todos::todo_delete,
            todos::todo_apply_agent_event,
            projects_tasks::project_list,
            projects_tasks::project_get,
            projects_tasks::project_create,
            projects_tasks::project_rename,
            projects_tasks::project_remove,
            projects_tasks::project_toggle_pin,
            projects_tasks::task_list,
            projects_tasks::task_get,
            projects_tasks::task_create,
            projects_tasks::task_update,
            projects_tasks::task_delete,
            projects_tasks::task_promote,
            projects_tasks::task_archive_project,
            projects_tasks::task_archive,
            projects_tasks::task_toggle_pin,
            projects_tasks::project_reorder,
            projects_tasks::task_reorder,
            projects_tasks::task_reparent,
            agent_timeline::agent_timeline_list,
            agent_timeline::agent_timeline_clear_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod popup_window_tests {
    use super::*;

    #[test]
    fn popup_route_bootstrap_script_sets_hash_route() {
        let script = popup_route_bootstrap_script("popup/chats/new");

        assert!(script.contains("\"/popup/chats/new\""));
        assert!(script.contains("window.location.hash"));
    }
}
