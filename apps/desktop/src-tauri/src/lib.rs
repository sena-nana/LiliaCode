use std::collections::{HashMap, VecDeque};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{utils::config::Color, AppHandle, Emitter, Manager, State, WindowEvent};
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
use plugins::{ClaudePlugin, ClaudeSkill, CodexMcpServer, PluginsOverview};
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
const PROVIDER_KEY_CLAUDE: &str = "provider.claude";
const PROVIDER_KEY_CODEX: &str = "provider.codex";
const CC_SWITCH_KEY: &str = "cc-switch.config";
const ROUTER_KEY_CLAUDE: &str = "router.claude";
const ROUTER_KEY_CODEX: &str = "router.codex";
const ASSISTANT_AI_KEY: &str = "assistant-ai.config";
/// 「添加项目 → 从 GitHub clone」时默认 clone 到的父目录。
const PROJECT_CLONE_PARENT_KEY: &str = "project.cloneParentDir";

const ROUTER_CC_SWITCH: &str = "cc-switch";
const ROUTER_DIRECT: &str = "direct";

const BACKEND_CLAUDE: &str = "claude";
const BACKEND_CODEX: &str = "codex";

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
    branch: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatBranchOption {
    name: String,
    current: bool,
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
struct EnvStatusReport {
    node_available: bool,
    /// codex CLI 是否能在 PATH 找到（@openai/codex-sdk 是 wrapper）。
    codex_cli_available: bool,
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

fn default_composer(task_id: &str) -> ChatComposerState {
    ChatComposerState {
        task_id: task_id.to_string(),
        backend: BACKEND_CLAUDE.to_string(),
        model: "claude-sonnet-4-6".to_string(),
        branch: "main".to_string(),
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
    });
    queue.len()
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
fn try_direct_for_backend(
    app: &AppHandle,
    backend: &'static str,
) -> BackendConnectionPlan {
    let key = match backend {
        BACKEND_CODEX => PROVIDER_KEY_CODEX,
        _ => PROVIDER_KEY_CLAUDE,
    };
    let cfg = load_provider_config(app, key).unwrap_or_default();
    let has_key = cfg.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false);
    let has_url = cfg.base_url.as_ref().map(|u| !u.is_empty()).unwrap_or(false);
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
        let _ = app_handle.emit(
            "chat:turn-started",
            TurnStartedEvent {
                task_id: task_id_for_thread.clone(),
                queued_count,
            },
        );

        let stdin_payload = serde_json::json!({
            "backend": backend_for_thread,
            "cwd": project_cwd,
            "prompt": prompt_for_thread,
            "attachments": attachments_for_thread,
            "model": composer_for_thread.model,
            "resumeSessionId": resume_session_id,
            "permission": composer_for_thread.permission,
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

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(err) => {
                let msg = format!(
                    "无法启动 node 子进程（请确保已安装 Node 18+ 并在 PATH 中）：{err}"
                );
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
                );
                return;
            }
        };

        // 把命令 JSON 写一行（带尾换行），但保留 stdin —— 后续 consent_response
        // 还要通过它写回 runner。stdin 存到 ChatStore，turn 结束时移除（Drop 关闭）。
        let stdin_handle: Option<Arc<Mutex<ChildStdin>>> = match child.stdin.take() {
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

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) if !l.trim().is_empty() => l,
                    _ => continue,
                };
                let value: JsonValue = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue, // 忽略偶发非 JSON 输出（SDK 内部 log 等）
                };
                let Some(event) = AgentRuntimeEvent::from_runner_json(&value) else {
                    continue;
                };
                log_agent_event_effect(event_host.dispatch(&event_ctx, &event));
                match &event {
                    AgentRuntimeEvent::ToolUse { .. } => {}
                    AgentRuntimeEvent::Timeline { .. } => {
                        if let Some(input) =
                            timeline_input_from_runtime_event(&event_ctx, &event)
                        {
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

        // 流结束前确保 pending 的最后一帧落地——否则 turn 末尾的尾段文本会卡在节流窗口里。
        timeline_throttle.flush_all(&app_handle);

        // 子进程的 stdout 读完即 turn 结束：把存的 stdin 移除，防止后续 consent
        // 响应往一个死管道里写。Drop ChildStdin 也会向 runner 端发 EOF。
        drop(stdin_handle);
        {
            let store = app_handle.state::<ChatStore>();
            store
                .running_stdins
                .lock()
                .unwrap()
                .remove(&task_id_for_thread);
        }

        // 等待子进程退出并收集 stderr 用于诊断（API key 缺失等）。
        let exit_status = child.wait();
        let stderr_text = child
            .stderr
            .take()
            .and_then(|mut s| {
                let mut buf = String::new();
                use std::io::Read;
                s.read_to_string(&mut buf).ok().map(|_| buf)
            })
            .unwrap_or_default();

        let nonzero = exit_status.as_ref().map(|s| !s.success()).unwrap_or(true);
        if nonzero && !stderr_text.trim().is_empty() {
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
        );
    });
}

fn finish_agent_turn(
    app_handle: AppHandle,
    task_id: String,
    backend: String,
    last_session_id: Option<String>,
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
        let mut running = store.running_tasks.lock().unwrap();
        let mut pending = store.pending_turns.lock().unwrap();
        let mut should_remove_queue = false;
        let next = if let Some(queue) = pending.get_mut(&task_id) {
            let turn = queue.pop_front();
            should_remove_queue = queue.is_empty();
            turn
        } else {
            None
        };
        if should_remove_queue {
            pending.remove(&task_id);
        }
        if next.is_none() {
            running.remove(&task_id);
        }
        next
    };
    if let Some(turn) = next {
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

fn describe_attachment_path(path: String) -> ChatAttachment {
    let raw_path = PathBuf::from(path.trim());
    let normalized_path = if raw_path.is_absolute() {
        raw_path
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(&raw_path))
            .unwrap_or(raw_path)
    };
    let metadata = std::fs::metadata(&normalized_path).ok();
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
    let size = metadata
        .as_ref()
        .and_then(|meta| if meta.is_file() { Some(meta.len()) } else { None });
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
    }
}

#[tauri::command]
fn chat_describe_attachments(paths: Vec<String>) -> Result<Vec<ChatAttachment>, String> {
    Ok(paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .map(describe_attachment_path)
        .collect())
}

#[tauri::command]
fn chat_send_message(
    app: AppHandle,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    store: State<'_, ChatStore>,
) -> Result<ChatSendResult, String> {
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
    // 同步 composer 偏好——发送时选的下拉值就是用户最新偏好。
    store
        .composers
        .lock()
        .unwrap()
        .insert(task_id.clone(), composer.clone());

    {
        let mut running = store.running_tasks.lock().unwrap();
        if running.contains_key(&task_id) {
            drop(running);
            let queued_count = queue_pending_turn(
                &store,
                &task_id,
                content,
                composer.clone(),
                project_cwd,
                attachments,
                user_msg.clone(),
                turn_id.clone(),
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
    store: State<'_, ChatStore>,
) -> Result<(), String> {
    let decision_norm = if decision == "allow" { "allow" } else { "deny" };
    let payload = serde_json::json!({
        "type": "consent_response",
        "id": request_id,
        "decision": decision_norm,
        "message": message.unwrap_or_default(),
    });
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
    stdin.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
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
    stdin.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn chat_list_models(backend: String) -> Vec<ChatModelOption> {
    match backend.as_str() {
        BACKEND_CODEX => vec![
            ChatModelOption {
                id: "gpt-5-codex".to_string(),
                label: "GPT-5 Codex".to_string(),
                backend: BACKEND_CODEX.to_string(),
            },
            ChatModelOption {
                id: "o3".to_string(),
                label: "o3".to_string(),
                backend: BACKEND_CODEX.to_string(),
            },
            ChatModelOption {
                id: "o3-mini".to_string(),
                label: "o3-mini".to_string(),
                backend: BACKEND_CODEX.to_string(),
            },
        ],
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

#[tauri::command]
fn chat_list_branches(_project_id: String) -> Vec<ChatBranchOption> {
    // 第一阶段不读真实 git。
    vec![
        ChatBranchOption {
            name: "main".to_string(),
            current: true,
        },
        ChatBranchOption {
            name: "dev".to_string(),
            current: false,
        },
    ]
}

#[tauri::command]
fn chat_get_composer_state(
    task_id: String,
    store: State<'_, ChatStore>,
) -> ChatComposerState {
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
fn chat_reset_session(
    task_id: String,
    chat_store: State<'_, ChatStore>,
    app: AppHandle,
) {
    let mut sessions = chat_store.sdk_sessions.lock().unwrap();
    sessions.remove(&session_key(BACKEND_CLAUDE, &task_id));
    sessions.remove(&session_key(BACKEND_CODEX, &task_id));
    drop(sessions);
    chat_store.running_tasks.lock().unwrap().remove(&task_id);
    chat_store.pending_turns.lock().unwrap().remove(&task_id);
    if let Some(store) = app.try_state::<LiliaStore>() {
        if let Err(err) = store
            .conn()
            .and_then(|conn| agent_timeline::clear(&conn, &task_id).map(|_| ()))
        {
            eprintln!("[agent-timeline] clear on reset failed: {err}");
        }
    }
}

fn build_backend_env_status(app: &AppHandle, backend: &str) -> BackendEnvStatus {
    let plan = resolve_connection_for(app, backend);

    // has_api_key 综合 plan / env / direct 配置三处；让 UI 区分「没配过」和「配了但当前路由没用上」。
    let key_env = match backend {
        BACKEND_CODEX => "OPENAI_API_KEY",
        _ => "ANTHROPIC_API_KEY",
    };
    let has_api_key = plan.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false)
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

#[tauri::command]
fn chat_check_env(app: AppHandle) -> EnvStatusReport {
    let node_available = cli_available("node");
    let codex_cli_available = cli_available("codex");

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
    let store = app
        .store(PROVIDER_STORE_FILE)
        .map_err(|e| format!("打开配置存储失败：{e}"))?;
    let value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
    store.set(key, value);
    store.save().map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(())
}

#[tauri::command]
fn cc_switch_get_config(app: AppHandle) -> CCSwitchConfig {
    load_cc_switch_config(&app)
}

#[tauri::command]
fn cc_switch_set_config(app: AppHandle, config: CCSwitchConfig) -> Result<(), String> {
    let store = app
        .store(PROVIDER_STORE_FILE)
        .map_err(|e| format!("打开配置存储失败：{e}"))?;
    let value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
    store.set(CC_SWITCH_KEY, value);
    store.save().map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(())
}

#[tauri::command]
fn assistant_ai_get_config(app: AppHandle) -> AssistantAIConfig {
    load_assistant_ai_config(&app)
}

#[tauri::command]
fn assistant_ai_set_config(app: AppHandle, config: AssistantAIConfig) -> Result<(), String> {
    let store = app
        .store(PROVIDER_STORE_FILE)
        .map_err(|e| format!("打开配置存储失败：{e}"))?;
    let value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
    store.set(ASSISTANT_AI_KEY, value);
    store.save().map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(())
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
            let matched = parsed
                .as_ref()
                .map(|list| list.iter().any(|m| m == model));
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
    let store = app
        .store(PROVIDER_STORE_FILE)
        .map_err(|e| format!("打开配置存储失败：{e}"))?;
    store.set(key, JsonValue::String(mode));
    store.save().map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(())
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
    let store = app
        .store(PROVIDER_STORE_FILE)
        .map_err(|e| format!("打开配置存储失败：{e}"))?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    store.set(PROJECT_CLONE_PARENT_KEY, value);
    store.save().map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(())
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
fn plugins_list_codex_mcp_servers(app: AppHandle) -> Vec<CodexMcpServer> {
    plugins::list_codex_mcp_servers(&app).0
}

/// 用系统默认编辑器打开 `~/.codex/config.toml`，文件不存在时先建一个空文件。
#[tauri::command]
fn plugins_open_codex_config(app: AppHandle) -> Result<(), String> {
    let path = plugins::codex_config_path(&app)?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建 .codex 目录失败：{e}"))?;
        }
    }
    if !path.exists() {
        std::fs::write(&path, b"")
            .map_err(|e| format!("初始化 config.toml 失败：{e}"))?;
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
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }
            if matches!(event, WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed) {
                if let Some(webview_window) = window.get_webview_window(MAIN_WINDOW_LABEL) {
                    window_state::persist_main_window_state(&window.app_handle(), &webview_window);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            chat_describe_attachments,
            chat_send_message,
            chat_respond_tool_consent,
            chat_respond_ask_user,
            chat_list_models,
            chat_list_branches,
            chat_get_composer_state,
            chat_set_composer_state,
            chat_reset_session,
            chat_check_env,
            provider_get_config,
            provider_set_config,
            cc_switch_get_config,
            cc_switch_set_config,
            assistant_ai_get_config,
            assistant_ai_set_config,
            assistant_ai_test_connection,
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
