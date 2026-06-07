use std::io::Write;

use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager, State};

use crate::chat::runner::spawn_agent_turn;
use crate::chat::state::{
    clear_pending_turns, clear_running_handles, default_composer, model_options_for_backend,
    new_chat_message_id, normalize_composer_for_backend, now_millis, queue_pending_turn,
    reset_cleared_guide_queue, session_key, set_guide_status_for_app, stop_running_turn, ChatStore,
};
use crate::chat::timeline_sink::persist_and_emit_message_timeline_event;
use crate::chat::types::{
    ChatAttachment, ChatComposerState, ChatInterruptResult, ChatMessage, ChatModelOption,
    ChatSendResult,
};
use crate::provider::{load_active_backend, validate_backend_ready_for_send};
use crate::store::LiliaStore;
use crate::{agent_timeline, BACKEND_CLAUDE, BACKEND_CODEX};

#[tauri::command]
pub fn chat_send_message(
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
pub fn chat_interrupt_turn(
    task_id: String,
    app: AppHandle,
    store: State<'_, ChatStore>,
) -> Result<ChatInterruptResult, String> {
    let running_turn = {
        let turns = store.running_turns.lock().unwrap();
        turns.get(&task_id).cloned()
    };
    let Some(running_turn) = running_turn else {
        return Ok(ChatInterruptResult::default());
    };

    let can_rollback = app
        .try_state::<LiliaStore>()
        .and_then(|lilia| {
            let conn = lilia.conn().ok()?;
            agent_timeline::user_only_turn_rollback_candidate(
                &conn,
                &task_id,
                &running_turn.turn_id,
            )
            .ok()
            .flatten()
            .map(|_| ())
        })
        .is_some();

    if can_rollback {
        stop_running_turn(&app, &store, &task_id, false, true)?;
        if let Some(rollback) =
            rollback_current_user_only_turn(&app, &task_id, &running_turn.turn_id)?
        {
            return Ok(ChatInterruptResult {
                rolled_back: true,
                restored_content: rollback.content,
                restored_attachments: rollback.attachments,
                removed_event_ids: rollback.removed_event_ids,
            });
        }
        return Ok(ChatInterruptResult::default());
    }

    stop_running_turn(&app, &store, &task_id, true, false)?;
    Ok(ChatInterruptResult::default())
}

fn rollback_current_user_only_turn(
    app: &AppHandle,
    task_id: &str,
    turn_id: &str,
) -> Result<Option<agent_timeline::UserOnlyTurnRollback>, String> {
    let Some(lilia) = app.try_state::<LiliaStore>() else {
        return Ok(None);
    };
    let conn = lilia.conn()?;
    agent_timeline::rollback_user_only_turn(&conn, task_id, turn_id)
}

fn write_runner_stdin(store: &ChatStore, task_id: &str, payload: JsonValue) -> Result<(), String> {
    let mut line = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    line.push('\n');

    let handle = {
        let map = store.running_stdins.lock().unwrap();
        map.get(task_id).cloned()
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

pub(crate) fn agent_interaction_response_payload(
    request_id: String,
    kind: String,
    result: JsonValue,
) -> JsonValue {
    serde_json::json!({
        "type": "interaction_response",
        "id": request_id,
        "kind": kind,
        "result": result,
    })
}

pub(crate) fn composer_permission_update_payload(
    previous: Option<&ChatComposerState>,
    next: &ChatComposerState,
) -> Option<JsonValue> {
    if !previous.is_some_and(|previous| previous.permission != next.permission) {
        return None;
    }
    match next.permission.as_str() {
        "full" | "readonly" | "ask" => Some(serde_json::json!({
            "type": "settings_update",
            "permission": next.permission,
        })),
        _ => None,
    }
}

/// 把用户对统一 Agent interaction 的响应写回 runner 的 stdin。
#[tauri::command]
pub fn chat_respond_agent_interaction(
    task_id: String,
    request_id: String,
    kind: String,
    result: JsonValue,
    store: State<'_, ChatStore>,
) -> Result<(), String> {
    let payload = agent_interaction_response_payload(request_id, kind, result);
    write_runner_stdin(&store, &task_id, payload)
}

#[tauri::command]
pub fn chat_list_models(backend: String) -> Vec<ChatModelOption> {
    model_options_for_backend(&backend)
}

#[tauri::command]
pub fn chat_get_composer_state(task_id: String, store: State<'_, ChatStore>) -> ChatComposerState {
    store
        .composers
        .lock()
        .unwrap()
        .get(&task_id)
        .cloned()
        .unwrap_or_else(|| default_composer(&task_id))
}

#[tauri::command]
pub fn chat_set_composer_state(state: ChatComposerState, store: State<'_, ChatStore>) {
    let payload = {
        let mut composers = store.composers.lock().unwrap();
        let previous = composers.get(&state.task_id);
        let payload = composer_permission_update_payload(previous, &state);
        composers.insert(state.task_id.clone(), state.clone());
        payload
    };
    let Some(payload) = payload else {
        return;
    };
    if let Err(err) = write_runner_stdin(&store, &state.task_id, payload) {
        eprintln!("[chat] runtime permission update failed: {err}");
    }
}

#[tauri::command]
pub fn chat_reset_session(task_id: String, chat_store: State<'_, ChatStore>, app: AppHandle) {
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
