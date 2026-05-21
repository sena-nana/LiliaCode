use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{utils::config::Color, AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;

const MAIN_WINDOW_LABEL: &str = "main";

// 始终使用暗色：与前端 CSS 变量 --bg = #181818 保持一致，避免 Windows 拉伸/还原时
// 露出 WebView 之外的默认白底。
const BG: Color = Color(0x18, 0x18, 0x18, 0xFF);

// CC-Switch 桌面端在 127.0.0.1 上启的本地代理端口（见 cc-switch 的
// src-tauri/src/proxy/types.rs：`listen_port: 15721`）。Claude 与 Codex 共用同一个
// 代理 URL——上游网关负责协议路由。
const CC_SWITCH_DEFAULT_URL: &str = "http://127.0.0.1:15721";

/// 真实 key 由 CC-Switch 注入；我们这边只需要任意非空字符串让 SDK 通过本地校验。
const CC_SWITCH_PLACEHOLDER_KEY: &str = "sk-cc-switch-proxy";

/// tauri_plugin_store 文件名 + key 命名约定。
const PROVIDER_STORE_FILE: &str = "provider-config.json";
const PROVIDER_KEY_CLAUDE: &str = "provider.claude";
const PROVIDER_KEY_CODEX: &str = "provider.codex";
const CC_SWITCH_KEY: &str = "cc-switch.config";
const ROUTER_KEY_CLAUDE: &str = "router.claude";
const ROUTER_KEY_CODEX: &str = "router.codex";

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
    /// 每个 task 一条独立的消息流；锁粒度做到整张表即可，量级很小。
    messages: Mutex<HashMap<String, Vec<ChatMessage>>>,
    /// composer 偏好（backend / 模型 / 分支 / 权限），按 task 隔离。
    composers: Mutex<HashMap<String, ChatComposerState>>,
    /// 各 SDK 自己的会话 id：key = "{backend}:{task_id}"，避免命名空间冲突。
    /// 第一次发送时为空；done 事件回来后写入，下次发送带上。
    sdk_sessions: Mutex<HashMap<String, String>>,
    /// 单调自增 id 生成器；避免引入 uuid 依赖。
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
    /// 子进程要 export 的 base url；None 表示用进程已有值。
    base_url: Option<String>,
    /// 子进程要 export 的 api key；None 表示用进程已有值。
    api_key: Option<String>,
}

/// 探测一个 base URL 是否可拨通。失败/解析失败/空串都视为不可达。
/// 短超时——拨不通就当它不存在，不阻塞主流程。
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
        // 没协议的话，按 host[:port] 直接用
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

/// 从 tauri_plugin_store 读 ProviderConfig；读不到或文件不存在返回 None。
fn load_provider_config(app: &AppHandle, key: &str) -> Option<ProviderConfig> {
    let store = app.store(PROVIDER_STORE_FILE).ok()?;
    let value = store.get(key)?;
    serde_json::from_value::<ProviderConfig>(value).ok()
}

/// 从 store 读 CC-Switch 配置；读不到回 Default。
fn load_cc_switch_config(app: &AppHandle) -> CCSwitchConfig {
    let read = || -> Option<CCSwitchConfig> {
        let store = app.store(PROVIDER_STORE_FILE).ok()?;
        let value = store.get(CC_SWITCH_KEY)?;
        serde_json::from_value::<CCSwitchConfig>(value).ok()
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

/// 入口：按 per-backend 路由模式分发。选了哪个就只走哪个，失败即 unconfigured
/// （让 SDK 调用直接报错而不是悄悄回退到别处）。
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
/// - 开发态：cargo 编出来的二进制位于 `apps/desktop/src-tauri/target/{debug|release}/`，
///   而脚本位于 `apps/desktop/agent-runner.mjs`，相对路径需要回退 3 层。
/// - 生产态（后续接 Tauri resources 时再加）：从 `resource_dir()` 查。
///
/// 这里按候选顺序找第一个存在的文件；找不到就返回最后一个候选让上层报错更直观。
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
    // 同步 composer 偏好——发送时选的下拉值就是用户「最新偏好」。
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
        // 父进程已有的 env 会自动继承，这里只是对相关键做覆盖；显式空字符串不能用。
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
                        // 兜底：如果某轮的文本只来自 assistant 消息没走 delta，
                        // 这里把它补到累计缓冲，避免最终 done 写入空消息。
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

        // 等待子进程退出并收集 stderr——用于诊断 API key 缺失等问题。
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

        // 记下 session id 供下一轮 resume，按 backend 隔离 key。
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
    // 第一阶段不读真实 git；后续接 git2 / 命令行时签名不变。
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
    // 「开始新对话」：清掉两个 backend 的 SDK session id 和消息历史。
    let mut sessions = store.sdk_sessions.lock().unwrap();
    sessions.remove(&session_key(BACKEND_CLAUDE, &task_id));
    sessions.remove(&session_key(BACKEND_CODEX, &task_id));
    drop(sessions);
    store.messages.lock().unwrap().remove(&task_id);
}

fn build_backend_env_status(app: &AppHandle, backend: &str) -> BackendEnvStatus {
    let plan = resolve_connection_for(app, backend);

    // 「已配置」综合三处来源：当前 plan 里有 key、env 里有 key、direct 配置里有 key。
    // RouterMode 决定 plan 实际走哪条；但 has_api_key 反映「用户在某处配过」，
    // 帮 UI 区分「没配置过」和「配置了但当前路由没用上」。
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(ChatStore::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = window.set_background_color(Some(BG));
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
            router_get_mode,
            router_set_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
