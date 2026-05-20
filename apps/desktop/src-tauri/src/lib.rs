use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{utils::config::Color, AppHandle, Emitter, Manager, State};

const MAIN_WINDOW_LABEL: &str = "main";

// 始终使用暗色：与前端 CSS 变量 --bg = #181818 保持一致，避免 Windows 拉伸/还原时
// 露出 WebView 之外的默认白底。
const BG: Color = Color(0x18, 0x18, 0x18, 0xFF);

// ---------- 契约（与 packages/contracts 同形） ----------
//
// 第一阶段：消息历史与 composer 偏好都只放在进程内存里，进程退出就丢。
// 这是有意为之——真正的 Claude Code JSONL 解析是下一个迭代要做的事，
// 现在先把命令表面和事件通道立起来，前端不需要再依赖 stub 即可跑通端到端。

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

// ---------- 进程内状态 ----------

#[derive(Default)]
struct ChatStore {
    /// 每个 task 一条独立的消息流；锁粒度做到整张表即可，量级很小。
    messages: Mutex<HashMap<String, Vec<ChatMessage>>>,
    /// composer 偏好（模型 / 分支 / 权限），按 task 隔离。
    composers: Mutex<HashMap<String, ChatComposerState>>,
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
    store: State<'_, ChatStore>,
) -> ChatMessage {
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
    // 顺手同步 composer 偏好——发送时点击下拉调的最新值应该被记住。
    store
        .composers
        .lock()
        .unwrap()
        .insert(task_id.clone(), composer);

    // 2) 异步「假 assistant」：起一个线程 sleep 350ms 后 emit 回复——
    //    现在没有真后端，要的就是「输入回车 → 短暂空气 → 回复出现」的节奏感。
    //    真接上 Claude Code 后，把这个 thread::spawn 换成 stream 监听器即可，
    //    事件名 `chat:message` 保持不变，前端不动。
    let app_handle = app.clone();
    let task_id_clone = task_id.clone();
    let echo_content = format!("（占位回复）你说的是：{content}");
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(350));
        let store = app_handle.state::<ChatStore>();
        let reply = ChatMessage {
            id: store.new_id("a"),
            task_id: task_id_clone.clone(),
            role: "assistant".to_string(),
            content: echo_content,
            created_at: now_millis(),
        };
        store
            .messages
            .lock()
            .unwrap()
            .entry(task_id_clone)
            .or_default()
            .push(reply.clone());
        let _ = app_handle.emit("chat:message", reply);
    });

    user_msg
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
