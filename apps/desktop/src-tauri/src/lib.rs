use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{utils::config::Color, AppHandle, Emitter, Manager, State};

const MAIN_WINDOW_LABEL: &str = "main";

// 始终使用暗色：与前端 CSS 变量 --bg = #181818 保持一致，避免 Windows 拉伸/还原时
// 露出 WebView 之外的默认白底。
const BG: Color = Color(0x18, 0x18, 0x18, 0xFF);

// CC-Switch 桌面端在 127.0.0.1 上启的本地代理端口（见 cc-switch 的
// src-tauri/src/proxy/types.rs：`listen_port: 15721`）。
// 它接收 Anthropic API 格式的请求，再按当前 active provider 转发；Lilia 把
// ANTHROPIC_BASE_URL 指过去就能复用 CC-Switch 的 provider 切换 / failover / 健康检查。
const CC_SWITCH_PROXY_HOST: &str = "127.0.0.1";
const CC_SWITCH_PROXY_PORT: u16 = 15721;
const CC_SWITCH_PROXY_URL: &str = "http://127.0.0.1:15721";

/// 真实 key 由 CC-Switch 注入；我们这边只需要任意非空字符串让 SDK 通过本地校验。
const CC_SWITCH_PLACEHOLDER_KEY: &str = "sk-cc-switch-proxy";

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
    model: String,
    branch: String,
    permission: String, // "full" | "ask" | "readonly"
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatModelOption {
    id: String,
    label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatBranchOption {
    name: String,
    current: bool,
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
    /// composer 偏好（模型 / 分支 / 权限），按 task 隔离。
    composers: Mutex<HashMap<String, ChatComposerState>>,
    /// Claude Agent SDK 的 session_id，让多轮对话能 resume 到同一个 session。
    /// 第一次发送时为 None；done 事件回来后写入，下次发送带上。
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

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn default_composer(task_id: &str) -> ChatComposerState {
    ChatComposerState {
        task_id: task_id.to_string(),
        model: "claude-sonnet-4-6".to_string(),
        branch: "main".to_string(),
        permission: "ask".to_string(),
    }
}

/// 当前对 Claude 的连接方式。优先级：
///   1. 用户显式设置了 `ANTHROPIC_BASE_URL` → 用 custom（不动 key 来源）
///   2. CC-Switch 本地代理可达（127.0.0.1:15721） → 走 cc-switch
///   3. 设了 `ANTHROPIC_API_KEY` → 直连官方 API
///   4. 全都没有 → unconfigured（发送会失败）
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
struct ConnectionPlan {
    mode: ConnectionMode,
    /// 子进程要 export 的 ANTHROPIC_BASE_URL；None 表示用进程已有值。
    base_url: Option<String>,
    /// 子进程要 export 的 ANTHROPIC_API_KEY；None 表示用进程已有值。
    api_key: Option<String>,
}

/// 探测 CC-Switch 是否在监听。短超时——拨不通就当它不存在，不阻塞主流程。
fn cc_switch_reachable() -> bool {
    let addr = match format!("{CC_SWITCH_PROXY_HOST}:{CC_SWITCH_PROXY_PORT}").parse::<SocketAddr>()
    {
        Ok(a) => a,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

fn resolve_connection() -> ConnectionPlan {
    let env_base = env::var("ANTHROPIC_BASE_URL").ok().filter(|s| !s.is_empty());
    let env_key = env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty());

    // 1) 显式 base_url 永远优先——用户自己设了说明他们清楚要去哪。
    //    指向 CC-Switch 同样会被归类为 cc-switch（host:port 匹配），方便 UI 显示。
    if let Some(base) = env_base.as_ref() {
        let mode = if base.contains(&format!("{CC_SWITCH_PROXY_HOST}:{CC_SWITCH_PROXY_PORT}"))
            || base.contains("localhost:15721")
        {
            ConnectionMode::CcSwitch
        } else {
            ConnectionMode::CustomBaseUrl
        };
        return ConnectionPlan {
            mode,
            base_url: None, // 进程已有就不重复注入
            api_key: if env_key.is_some() {
                None
            } else {
                Some(CC_SWITCH_PLACEHOLDER_KEY.to_string())
            },
        };
    }

    // 2) 没设 base_url，但 CC-Switch 在跑 → 默认替用户连过去。
    if cc_switch_reachable() {
        return ConnectionPlan {
            mode: ConnectionMode::CcSwitch,
            base_url: Some(CC_SWITCH_PROXY_URL.to_string()),
            api_key: Some(env_key.unwrap_or_else(|| CC_SWITCH_PLACEHOLDER_KEY.to_string())),
        };
    }

    // 3) 退回直连。
    if env_key.is_some() {
        return ConnectionPlan {
            mode: ConnectionMode::Direct,
            base_url: None,
            api_key: None,
        };
    }

    ConnectionPlan {
        mode: ConnectionMode::Unconfigured,
        base_url: None,
        api_key: None,
    }
}

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

    // 上一轮拿到的 SDK session id，若有则让 SDK resume 到同一会话。
    let resume_session_id = store.sdk_sessions.lock().unwrap().get(&task_id).cloned();

    // 2) 起 Node 子进程跑 Claude Agent SDK，把它的事件流转成 Tauri 事件。
    let script_path = locate_agent_runner(&app);
    let connection = resolve_connection();
    let app_handle = app.clone();
    let task_id_for_thread = task_id.clone();
    let composer_for_thread = composer.clone();
    let prompt_for_thread = content.clone();

    thread::spawn(move || {
        let stdin_payload = serde_json::json!({
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
        // 把连接计划落到子进程 env 上。父进程已有的 env 会自动继承，这里只是
        // 对相关键做覆盖；显式空字符串不能用（SDK 会当成「设了」），故用 None。
        if let Some(url) = &connection.base_url {
            cmd.env("ANTHROPIC_BASE_URL", url);
        }
        if let Some(key) = &connection.api_key {
            cmd.env("ANTHROPIC_API_KEY", key);
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

        // 等待子进程退出并收集 stderr——用于诊断 ANTHROPIC_API_KEY 缺失等问题。
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
            store
                .sdk_sessions
                .lock()
                .unwrap()
                .insert(task_id_for_thread.clone(), sid);
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
fn chat_list_models() -> Vec<ChatModelOption> {
    vec![
        ChatModelOption {
            id: "claude-opus-4-7".to_string(),
            label: "Opus 4.7".to_string(),
        },
        ChatModelOption {
            id: "claude-sonnet-4-6".to_string(),
            label: "Sonnet 4.6".to_string(),
        },
        ChatModelOption {
            id: "claude-haiku-4-5".to_string(),
            label: "Haiku 4.5".to_string(),
        },
    ]
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
    // 「开始新对话」：清掉 SDK session id 和消息历史。前端要主动 list 一遍。
    store.sdk_sessions.lock().unwrap().remove(&task_id);
    store.messages.lock().unwrap().remove(&task_id);
}

#[tauri::command]
fn chat_check_env() -> serde_json::Value {
    // 给前端一个「环境健康检查」入口：node 是否能跑、当前用什么模式连 Claude。
    // 不做严格的版本探测，缺什么就如实告诉用户。
    let node_ok = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let plan = resolve_connection();
    let has_key = env::var("ANTHROPIC_API_KEY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let custom_base = env::var("ANTHROPIC_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty());

    // 给前端一个用户可读的「目标地址」：CC-Switch 就给代理 URL，custom 就回显 env，
    // direct 给官方 endpoint，unconfigured 给 null。
    let effective_url = match plan.mode {
        ConnectionMode::CcSwitch => Some(CC_SWITCH_PROXY_URL.to_string()),
        ConnectionMode::CustomBaseUrl => custom_base.clone(),
        ConnectionMode::Direct => Some("https://api.anthropic.com".to_string()),
        ConnectionMode::Unconfigured => None,
    };

    serde_json::json!({
        "hasApiKey": has_key,
        "nodeAvailable": node_ok,
        "ccSwitchReachable": cc_switch_reachable(),
        "connectionMode": plan.mode.as_str(),
        "effectiveUrl": effective_url,
        "envBaseUrl": custom_base,
    })
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
