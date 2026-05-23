use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{utils::config::Color, AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_store::StoreExt;

mod plugins;
mod projects_tasks;
mod store;
mod todos;
mod util;
use plugins::{
    ClaudePlugin, ClaudeSkill, CodexMcpServer, PluginsOverview,
};
use store::LiliaStore;
use todos::AgentTodoItem;

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
    created_at: u64,
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

// 推给前端的流事件 payload。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChunkEvent {
    task_id: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolEvent {
    task_id: String,
    name: String,
    input: JsonValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    task_id: String,
    session_id: Option<String>,
    subtype: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEvent {
    task_id: String,
    message: String,
}

// ---------- 进程内状态 ----------

#[derive(Default)]
struct ChatStore {
    messages: Mutex<HashMap<String, Vec<ChatMessage>>>,
    composers: Mutex<HashMap<String, ChatComposerState>>,
    /// SDK session id：key = "{backend}:{task_id}"，第一次发送为空，done 后写入用于 resume。
    sdk_sessions: Mutex<HashMap<String, String>>,
    next_id: AtomicU64,
}

impl ChatStore {
    fn new_id(&self, prefix: &str) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("{prefix}-{n}")
    }
}

fn session_key(backend: &str, task_id: &str) -> String {
    format!("{backend}:{task_id}")
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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

// ---------- Commands ----------

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[tauri::command]
fn chat_list_messages(task_id: String, store: State<'_, ChatStore>) -> Vec<ChatMessage> {
    store
        .messages
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .unwrap_or_default()
}

#[tauri::command]
fn chat_send_message(
    app: AppHandle,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    store: State<'_, ChatStore>,
) -> Result<ChatMessage, String> {
    // 1) 写入 user 消息并立即返回，给前端一个乐观渲染的锚点。
    let user_msg = ChatMessage {
        id: store.new_id("u"),
        task_id: task_id.clone(),
        role: "user".to_string(),
        content: content.clone(),
        created_at: now_millis(),
    };
    {
        let mut all = store.messages.lock().unwrap();
        all.entry(task_id.clone())
            .or_default()
            .push(user_msg.clone());
    }
    // 同步 composer 偏好——发送时选的下拉值就是用户最新偏好。
    store
        .composers
        .lock()
        .unwrap()
        .insert(task_id.clone(), composer.clone());

    // 按 backend 找上一轮的 SDK session id 用于 resume。
    let backend = composer.backend.clone();
    let resume_session_id = store
        .sdk_sessions
        .lock()
        .unwrap()
        .get(&session_key(&backend, &task_id))
        .cloned();

    // 2) 起 Node 子进程跑对应 backend 的 Agent SDK，把它的事件流转成 Tauri 事件。
    let script_path = locate_agent_runner(&app);
    let connection = resolve_connection_for(&app, &backend);
    let app_handle = app.clone();
    let task_id_for_thread = task_id.clone();
    let composer_for_thread = composer.clone();
    let prompt_for_thread = content.clone();
    let backend_for_thread = backend.clone();

    thread::spawn(move || {
        let stdin_payload = serde_json::json!({
            "backend": backend_for_thread,
            "cwd": project_cwd,
            "prompt": prompt_for_thread,
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
                let _ = app_handle.emit(
                    "chat:error",
                    ErrorEvent {
                        task_id: task_id_for_thread.clone(),
                        message: msg,
                    },
                );
                return;
            }
        };

        // 把命令 JSON 一次性写完然后关 stdin，让 Node 进入 EOF 分支。
        if let Some(mut stdin) = child.stdin.take() {
            let bytes = serde_json::to_vec(&stdin_payload).unwrap_or_default();
            let _ = stdin.write_all(&bytes);
        }

        // 累计 assistant 完整文本，done 时落盘到消息历史。
        let mut assistant_buf = String::new();
        let mut last_session_id: Option<String> = None;

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
                let ty = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match ty {
                    "chunk" => {
                        if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                            assistant_buf.push_str(text);
                            let _ = app_handle.emit(
                                "chat:chunk",
                                ChunkEvent {
                                    task_id: task_id_for_thread.clone(),
                                    text: text.to_string(),
                                },
                            );
                        }
                    }
                    "tool_use" => {
                        let name = value
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let input = value.get("input").cloned().unwrap_or(JsonValue::Null);

                        // 自动通道：SDK 的 TodoWrite 工具事件 → 同步到 task_todos 表。
                        // 失败仅 log，不影响后续 tool 事件 emit（fall back 到普通 tool 通知）。
                        if name == "TodoWrite" {
                            if let Some(arr) = input.get("todos").and_then(|v| v.as_array()) {
                                let parsed: Vec<AgentTodoItem> = arr
                                    .iter()
                                    .filter_map(|v| {
                                        serde_json::from_value::<AgentTodoItem>(v.clone()).ok()
                                    })
                                    .collect();
                                let store = app_handle.state::<LiliaStore>();
                                match store.conn().and_then(|conn| {
                                    todos::apply_agent_event_impl(
                                        &conn,
                                        &task_id_for_thread,
                                        &parsed,
                                    )
                                }) {
                                    Ok(_) => {
                                        let _ = app_handle.emit(
                                            "todo-changed",
                                            serde_json::json!({
                                                "taskId": task_id_for_thread,
                                            }),
                                        );
                                    }
                                    Err(err) => {
                                        eprintln!(
                                            "[todo] apply_agent_event failed: {err}"
                                        );
                                    }
                                }
                            }
                        }

                        let _ = app_handle.emit(
                            "chat:tool",
                            ToolEvent {
                                task_id: task_id_for_thread.clone(),
                                name,
                                input,
                            },
                        );
                    }
                    "assistant_done" => {
                        // 兜底：如果某轮文本只来自 assistant 消息没走 delta，把它补到累计缓冲。
                        if assistant_buf.is_empty() {
                            if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                                assistant_buf.push_str(text);
                                let _ = app_handle.emit(
                                    "chat:chunk",
                                    ChunkEvent {
                                        task_id: task_id_for_thread.clone(),
                                        text: text.to_string(),
                                    },
                                );
                            }
                        }
                        if let Some(sid) = value.get("sessionId").and_then(|v| v.as_str()) {
                            last_session_id = Some(sid.to_string());
                        }
                    }
                    "done" => {
                        if let Some(sid) = value.get("sessionId").and_then(|v| v.as_str()) {
                            last_session_id = Some(sid.to_string());
                        }
                    }
                    "error" => {
                        let msg = value
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("未知错误")
                            .to_string();
                        let _ = app_handle.emit(
                            "chat:error",
                            ErrorEvent {
                                task_id: task_id_for_thread.clone(),
                                message: msg,
                            },
                        );
                    }
                    _ => {}
                }
            }
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
            let _ = app_handle.emit(
                "chat:error",
                ErrorEvent {
                    task_id: task_id_for_thread.clone(),
                    message: format!("agent 进程异常退出：{}", stderr_text.trim()),
                },
            );
        }

        // 落盘 assistant 消息到历史。
        if !assistant_buf.is_empty() {
            let store = app_handle.state::<ChatStore>();
            let reply = ChatMessage {
                id: store.new_id("a"),
                task_id: task_id_for_thread.clone(),
                role: "assistant".to_string(),
                content: assistant_buf,
                created_at: now_millis(),
            };
            store
                .messages
                .lock()
                .unwrap()
                .entry(task_id_for_thread.clone())
                .or_default()
                .push(reply);
        }

        // 记下 session id 供下一轮 resume。
        if let Some(sid) = last_session_id.clone() {
            let store = app_handle.state::<ChatStore>();
            store.sdk_sessions.lock().unwrap().insert(
                session_key(&backend_for_thread, &task_id_for_thread),
                sid,
            );
        }

        let _ = app_handle.emit(
            "chat:done",
            DoneEvent {
                task_id: task_id_for_thread,
                session_id: last_session_id,
                subtype: None,
            },
        );
    });

    Ok(user_msg)
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
fn chat_reset_session(task_id: String, store: State<'_, ChatStore>) {
    let mut sessions = store.sdk_sessions.lock().unwrap();
    sessions.remove(&session_key(BACKEND_CLAUDE, &task_id));
    sessions.remove(&session_key(BACKEND_CODEX, &task_id));
    drop(sessions);
    store.messages.lock().unwrap().remove(&task_id);
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
        .invoke_handler(tauri::generate_handler![
            ping,
            chat_list_messages,
            chat_send_message,
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
            projects_tasks::project_reorder,
            projects_tasks::task_reorder,
            projects_tasks::task_reparent,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
