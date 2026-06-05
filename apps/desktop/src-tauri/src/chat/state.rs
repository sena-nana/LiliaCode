use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::chat::types::{ChatAttachment, ChatComposerState, ChatMessage, ChatModelOption};
use crate::store::LiliaStore;
use crate::{agent_timeline, todos, BACKEND_CLAUDE, BACKEND_CODEX, CODEX_MODEL_OPTIONS};

pub(crate) struct PendingChatTurn {
    pub(crate) content: String,
    pub(crate) composer: ChatComposerState,
    pub(crate) project_cwd: String,
    pub(crate) attachments: Vec<ChatAttachment>,
    pub(crate) message: ChatMessage,
    /// queue 时就分配好 turn_id，user message + agent turn 共享同一个 turn_id
    /// → 同一个 turn_seq；这是把"按 turn 隔离"的排序契约推到入口的关键。
    pub(crate) turn_id: String,
    pub(crate) guide_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RunningTurn {
    pub(crate) turn_id: String,
    pub(crate) backend: String,
}
// ---------- 进程内状态 ----------

#[derive(Default)]
pub(crate) struct ChatStore {
    pub(crate) composers: Mutex<HashMap<String, ChatComposerState>>,
    /// SDK session id：key = "{backend}:{task_id}"，第一次发送为空，done 后写入用于 resume。
    pub(crate) sdk_sessions: Mutex<HashMap<String, String>>,
    pub(crate) running_tasks: Mutex<HashMap<String, bool>>,
    pub(crate) pending_turns: Mutex<HashMap<String, VecDeque<PendingChatTurn>>>,
    pub(crate) running_turns: Mutex<HashMap<String, RunningTurn>>,
    pub(crate) running_children: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    pub(crate) interrupted_turns: Mutex<HashMap<String, RunningTurn>>,
    pub(crate) reset_turns: Mutex<HashMap<String, RunningTurn>>,
    /// 仍在运行的 runner 子进程 stdin。key = task_id，turn 结束时移除（Drop 即关 stdin）。
    /// 让统一 interaction response 命令能把决策写回给 runner。
    pub(crate) running_stdins: Mutex<HashMap<String, Arc<Mutex<ChildStdin>>>>,
}

pub(crate) fn session_key(backend: &str, task_id: &str) -> String {
    format!("{backend}:{task_id}")
}

pub(crate) fn load_persisted_resume_session_id(
    conn: &Connection,
    task_id: &str,
    backend: &str,
) -> Option<String> {
    agent_timeline::latest_session_id(conn, task_id, backend)
        .ok()
        .flatten()
}

pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn new_chat_message_id() -> String {
    format!("u-{}", Uuid::new_v4())
}

pub(crate) fn normalize_backend(value: &str) -> &'static str {
    match value {
        BACKEND_CODEX => BACKEND_CODEX,
        _ => BACKEND_CLAUDE,
    }
}

pub(crate) fn default_model_for_backend(backend: &str) -> &'static str {
    match normalize_backend(backend) {
        BACKEND_CODEX => CODEX_MODEL_OPTIONS[0].0,
        _ => "claude-sonnet-4-6",
    }
}

pub(crate) fn model_options_for_backend(backend: &str) -> Vec<ChatModelOption> {
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

pub(crate) fn normalize_model_for_backend(model: &str, backend: &str) -> String {
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

pub(crate) fn normalize_composer_for_backend(
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

pub(crate) fn default_composer(task_id: &str) -> ChatComposerState {
    ChatComposerState {
        task_id: task_id.to_string(),
        backend: BACKEND_CLAUDE.to_string(),
        model: default_model_for_backend(BACKEND_CLAUDE).to_string(),
        plan_mode: false,
        permission: "ask".to_string(),
        codex_settings: Default::default(),
    }
}

pub(crate) fn queue_pending_turn(
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

pub(crate) fn clear_pending_turns(store: &ChatStore, task_id: &str) -> Vec<String> {
    store
        .pending_turns
        .lock()
        .unwrap()
        .remove(task_id)
        .map(|queue| queue.into_iter().filter_map(|turn| turn.guide_id).collect())
        .unwrap_or_default()
}

pub(crate) fn set_guide_status_for_app(
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

pub(crate) fn reset_cleared_guide_queue(app: &AppHandle, guide_ids: Vec<String>) {
    for guide_id in guide_ids {
        if let Err(err) = set_guide_status_for_app(app, Some(&guide_id), "pending") {
            eprintln!("[todo-guides] reset queued guide failed: {err}");
        }
    }
}

pub(crate) struct PreparedTurnStop {
    pub(crate) child_handle: Option<Arc<Mutex<Child>>>,
    pub(crate) guide_ids: Vec<String>,
}

pub(crate) struct FinishedRunningTurn {
    pub(crate) interrupted: bool,
    pub(crate) reset: bool,
}

pub(crate) fn clear_running_handles(store: &ChatStore, task_id: &str) -> Option<RunningTurn> {
    store.running_stdins.lock().unwrap().remove(task_id);
    store.running_children.lock().unwrap().remove(task_id);
    store.running_turns.lock().unwrap().remove(task_id)
}

pub(crate) fn prepare_running_turn_stop(
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

pub(crate) fn is_turn_marked_reset(
    store: &ChatStore,
    task_id: &str,
    turn_id: &str,
    backend: &str,
) -> bool {
    store
        .reset_turns
        .lock()
        .unwrap()
        .get(task_id)
        .is_some_and(|turn| turn.turn_id == turn_id && turn.backend == backend)
}

pub(crate) fn take_turn_stop_marks(
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

pub(crate) fn finish_running_turn_handles(
    store: &ChatStore,
    task_id: &str,
    turn_id: &str,
    backend: &str,
) -> FinishedRunningTurn {
    clear_running_handles(store, task_id);
    let (interrupted, reset) = take_turn_stop_marks(store, task_id, turn_id, backend);
    FinishedRunningTurn { interrupted, reset }
}

pub(crate) fn stop_running_turn(
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

pub(crate) fn take_next_pending_turn(
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

pub(crate) fn should_emit_runner_exit_error(
    interrupted: bool,
    nonzero: bool,
    stderr_text: &str,
) -> bool {
    !interrupted && nonzero && !stderr_text.trim().is_empty()
}

pub(crate) fn persist_and_emit_interrupted_timeline_event(
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
