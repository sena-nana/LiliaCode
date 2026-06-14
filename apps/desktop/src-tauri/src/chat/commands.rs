use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::chat::runner::{spawn_agent_turn, write_runner_stdin_for_task};
use crate::chat::slash_commands::{
    emit_slash_command_done, execute_slash_command, persist_and_emit_slash_command_result,
};
use crate::chat::state::{
    clear_pending_turns, clear_pending_turns_for_app, clear_persisted_pending_rollback,
    clear_persisted_pending_turns_for_app, clear_running_handles,
    clear_runtime_finalization_for_app, clear_task_runtime_state_for_reset, default_composer,
    mark_pending_reset_cleanup_for_app, model_options_for_backend, new_chat_message_id,
    normalize_composer_for_backend, now_millis, persist_runtime_control_event_for_app,
    persist_runtime_state_for_app, queue_pending_turn_for_app, reset_cleared_guide_queue,
    set_guide_status_for_app, set_pending_rollback, set_pending_rollback_for_app,
    should_persist_user_message, stop_running_turn, ChatStore, RunningTurn, RuntimeControlEvent,
};
use crate::chat::timeline_sink::persist_and_emit_message_timeline_event;
use crate::chat::types::{
    ChatAttachment, ChatComposerState, ChatInterruptResult, ChatMessage, ChatModelOption,
    ChatRollbackResult, ChatRuntimeCommand, ChatRuntimeSnapshot, ChatSendResult, ChatWorkflow,
    ProviderRuntimeOptions,
};
use crate::provider::load_agent_interaction_settings;
use crate::provider::{load_active_backend, validate_backend_ready_for_send};
use crate::store::LiliaStore;
use crate::{agent_timeline, RUNTIME_CHANNEL_MUTSUKI_CORE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MutsukiCoreTurnStopKind {
    Interrupt,
    Reset,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct ResetSessionPlan {
    pub(crate) cleared_guide_ids: Vec<String>,
    pub(crate) stopped_running: bool,
    pub(crate) immediate_cleanup: bool,
}

#[tauri::command]
pub fn chat_send_message(
    app: AppHandle,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    guide_id: Option<String>,
    workflow: Option<ChatWorkflow>,
    runtime_command: Option<ChatRuntimeCommand>,
    runtime_options: Option<ProviderRuntimeOptions>,
    store: State<'_, ChatStore>,
) -> Result<ChatSendResult, String> {
    let active_backend = load_active_backend(&app);
    validate_backend_ready_for_send(&active_backend)?;
    let composer = normalize_composer_for_backend(composer, &task_id, &active_backend);
    let slash_command_id = match &workflow {
        Some(ChatWorkflow::SlashCommand { command_id, .. }) => Some(command_id.clone()),
        _ => None,
    };
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
    let persist_user_message = should_persist_user_message(&content, &workflow, &runtime_command);
    store
        .composers
        .lock()
        .unwrap()
        .insert(task_id.clone(), composer.clone());

    if let Some(command_id) = slash_command_id {
        let execution = execute_slash_command(&command_id, &project_cwd, &composer.backend)?;
        persist_and_emit_slash_command_result(
            &app,
            &task_id,
            &turn_id,
            &composer.backend,
            &execution,
        );
        emit_slash_command_done(&app, task_id.clone());
        return Ok(ChatSendResult {
            message: user_msg,
            dispatch: "started".to_string(),
            queued_count: 0,
            turn_id,
        });
    }

    {
        let mut running = store.running_tasks.lock().unwrap();
        if running.contains_key(&task_id) {
            let runtime_channel = store
                .running_turns
                .lock()
                .unwrap()
                .get(&task_id)
                .map(|turn| turn.runtime_channel.clone())
                .unwrap_or_else(|| {
                    crate::chat::state::normalize_runtime_channel(
                        &load_agent_interaction_settings(&app).agent_runtime_channel,
                    )
                    .to_string()
                });
            drop(running);
            set_guide_status_for_app(&app, guide_id.as_deref(), "queued")?;
            let queued_count = queue_pending_turn_for_app(
                &app,
                &store,
                &task_id,
                content,
                composer.clone(),
                project_cwd,
                attachments,
                workflow,
                runtime_command,
                runtime_options,
                user_msg.clone(),
                turn_id.clone(),
                runtime_channel,
                guide_id.clone(),
            );
            if persist_user_message {
                persist_and_emit_message_timeline_event(
                    &app,
                    &user_msg,
                    &composer.backend,
                    &turn_id,
                    true,
                    None,
                );
            }
            return Ok(ChatSendResult {
                message: user_msg,
                dispatch: "queued".to_string(),
                queued_count,
                turn_id,
            });
        }
        running.insert(task_id.clone(), true);
    }

    set_guide_status_for_app(&app, guide_id.as_deref(), "sent")?;
    if persist_user_message {
        persist_and_emit_message_timeline_event(
            &app,
            &user_msg,
            &composer.backend,
            &turn_id,
            false,
            None,
        );
    }

    spawn_agent_turn(
        app,
        task_id,
        content,
        composer,
        project_cwd,
        attachments,
        workflow,
        runtime_command,
        runtime_options,
        turn_id.clone(),
    );

    Ok(ChatSendResult {
        message: user_msg,
        dispatch: "started".to_string(),
        queued_count: 0,
        turn_id,
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

    let is_mutsuki_core = running_turn.runtime_channel == RUNTIME_CHANNEL_MUTSUKI_CORE;

    if can_rollback {
        if is_mutsuki_core {
            record_mutsuki_core_control_event_for_app(
                &app,
                &store,
                &task_id,
                "reset_requested",
                control_event_attributes([("mode", "interrupt_rollback".to_string())]),
                None,
            );
            let rollback = rollback_current_user_only_turn(&app, &task_id, &running_turn.turn_id)?
                .map(|rollback| ChatRollbackResult {
                    rolled_back: true,
                    restored_content: rollback.content,
                    restored_attachments: rollback.attachments,
                    removed_event_ids: rollback.removed_event_ids,
                });
            if let Some(rollback) = rollback.clone() {
                set_pending_rollback_for_app(&app, &store, &task_id, rollback);
            }
            let (mut cleared_guide_ids, result) = stage_mutsuki_core_interrupt_turn(
                &store,
                &task_id,
                &running_turn,
                MutsukiCoreTurnStopKind::Reset,
                None,
            );
            persist_runtime_state_for_app(
                &app,
                &store,
                &task_id,
                &running_turn,
                "reset_pending_finish",
                None,
                None,
            );
            cleared_guide_ids.append(&mut clear_persisted_pending_turns_for_app(&app, &task_id));
            reset_cleared_guide_queue(&app, cleared_guide_ids);
            return Ok(result);
        } else {
            stop_running_turn(&app, &store, &task_id, false, true)?;
        }
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

    if is_mutsuki_core {
        record_mutsuki_core_control_event_for_app(
            &app,
            &store,
            &task_id,
            "interrupt_requested",
            control_event_attributes([("mode", "user_interrupt".to_string())]),
            None,
        );
        let (mut cleared_guide_ids, result) = stage_mutsuki_core_interrupt_turn(
            &store,
            &task_id,
            &running_turn,
            MutsukiCoreTurnStopKind::Interrupt,
            None,
        );
        persist_runtime_state_for_app(
            &app,
            &store,
            &task_id,
            &running_turn,
            "interrupted_pending_finish",
            None,
            None,
        );
        cleared_guide_ids.append(&mut clear_persisted_pending_turns_for_app(&app, &task_id));
        reset_cleared_guide_queue(&app, cleared_guide_ids);
        return Ok(result);
    } else {
        stop_running_turn(&app, &store, &task_id, true, false)?;
    }
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

fn write_runner_stdin(
    store: &ChatStore,
    task_id: &str,
    payload: JsonValue,
) -> Result<bool, String> {
    write_runner_stdin_for_task(store, task_id, payload)
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

pub(crate) fn composer_runtime_settings_update_payload(
    previous: Option<&ChatComposerState>,
    next: &ChatComposerState,
) -> Option<JsonValue> {
    let Some(previous) = previous else {
        return None;
    };
    let mut payload = serde_json::Map::from_iter([(
        "type".to_string(),
        JsonValue::String("settings_update".to_string()),
    )]);
    if previous.permission != next.permission {
        match next.permission.as_str() {
            "full" | "readonly" | "ask" => {
                payload.insert(
                    "permission".to_string(),
                    JsonValue::String(next.permission.clone()),
                );
            }
            _ => {}
        }
    }
    if previous.model != next.model {
        let model = next.model.trim();
        if !model.is_empty() {
            payload.insert("model".to_string(), JsonValue::String(model.to_string()));
        }
    }
    if payload.len() == 1 {
        None
    } else {
        Some(JsonValue::Object(payload))
    }
}

fn composer_runtime_settings_update_attributes(payload: &JsonValue) -> BTreeMap<String, String> {
    let mut attributes = BTreeMap::new();
    if let Some(permission) = payload.get("permission").and_then(JsonValue::as_str) {
        attributes.insert("permission".to_string(), permission.to_string());
    }
    if let Some(model) = payload.get("model").and_then(JsonValue::as_str) {
        attributes.insert("model".to_string(), model.to_string());
    }
    attributes
}

pub(crate) fn control_event_attributes(
    pairs: impl IntoIterator<Item = (&'static str, String)>,
) -> BTreeMap<String, String> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

#[cfg(test)]
pub(crate) fn record_mutsuki_core_control_event(
    store: &ChatStore,
    task_id: &str,
    name: &str,
    mut attributes: BTreeMap<String, String>,
    payload: Option<JsonValue>,
) {
    let running_turn: Option<RunningTurn> = {
        let turns = store.running_turns.lock().unwrap();
        turns.get(task_id).cloned()
    };
    let Some(running_turn) = running_turn else {
        return;
    };
    if running_turn.runtime_channel != RUNTIME_CHANNEL_MUTSUKI_CORE {
        return;
    }
    attributes
        .entry("turnId".to_string())
        .or_insert(running_turn.turn_id);
    attributes
        .entry("backend".to_string())
        .or_insert(running_turn.backend);
    crate::chat::state::record_runtime_control_event(store, task_id, name, attributes, payload);
}

pub(crate) fn record_mutsuki_core_control_event_for_app<R: Runtime>(
    app: &AppHandle<R>,
    store: &ChatStore,
    task_id: &str,
    name: &str,
    mut attributes: BTreeMap<String, String>,
    payload: Option<JsonValue>,
) {
    let running_turn: Option<RunningTurn> = {
        let turns = store.running_turns.lock().unwrap();
        turns.get(task_id).cloned()
    };
    let Some(running_turn) = running_turn else {
        return;
    };
    if running_turn.runtime_channel != RUNTIME_CHANNEL_MUTSUKI_CORE {
        return;
    }
    attributes
        .entry("turnId".to_string())
        .or_insert(running_turn.turn_id);
    attributes
        .entry("backend".to_string())
        .or_insert(running_turn.backend);
    let event = RuntimeControlEvent {
        name: name.to_string(),
        attributes,
        payload,
    };
    if !persist_runtime_control_event_for_app(app, task_id, &event) {
        crate::chat::state::record_runtime_control_event(
            store,
            task_id,
            event.name,
            event.attributes,
            event.payload,
        );
    }
}

pub(crate) fn stage_mutsuki_core_turn_stop(
    store: &ChatStore,
    task_id: &str,
    running_turn: &RunningTurn,
    kind: MutsukiCoreTurnStopKind,
) -> Vec<String> {
    let cleared_guide_ids = clear_pending_turns(store, task_id);
    match kind {
        MutsukiCoreTurnStopKind::Interrupt => {
            store
                .interrupted_turns
                .lock()
                .unwrap()
                .insert(task_id.to_string(), running_turn.clone());
        }
        MutsukiCoreTurnStopKind::Reset => {
            store
                .reset_turns
                .lock()
                .unwrap()
                .insert(task_id.to_string(), running_turn.clone());
        }
    }
    cleared_guide_ids
}

pub(crate) fn stage_mutsuki_core_interrupt_turn(
    store: &ChatStore,
    task_id: &str,
    running_turn: &RunningTurn,
    kind: MutsukiCoreTurnStopKind,
    rollback: Option<ChatRollbackResult>,
) -> (Vec<String>, ChatInterruptResult) {
    let cleared_guide_ids = stage_mutsuki_core_turn_stop(store, task_id, running_turn, kind);
    if kind == MutsukiCoreTurnStopKind::Reset {
        if let Some(rollback) = rollback {
            set_pending_rollback(store, task_id, rollback);
        }
    }
    (cleared_guide_ids, ChatInterruptResult::default())
}

pub(crate) fn plan_reset_session(
    store: &ChatStore,
    task_id: &str,
    running_turn: Option<&RunningTurn>,
) -> ResetSessionPlan {
    match running_turn {
        None => ResetSessionPlan {
            cleared_guide_ids: clear_pending_turns(store, task_id),
            stopped_running: false,
            immediate_cleanup: true,
        },
        Some(running_turn) if running_turn.runtime_channel == RUNTIME_CHANNEL_MUTSUKI_CORE => {
            ResetSessionPlan {
                cleared_guide_ids: stage_mutsuki_core_turn_stop(
                    store,
                    task_id,
                    running_turn,
                    MutsukiCoreTurnStopKind::Reset,
                ),
                stopped_running: true,
                immediate_cleanup: false,
            }
        }
        Some(_) => ResetSessionPlan::default(),
    }
}

pub(crate) fn attach_stdin_delivery(
    attributes: &mut BTreeMap<String, String>,
    result: &Result<bool, String>,
) {
    match result {
        Ok(forwarded) => {
            attributes.insert("stdinForwarded".to_string(), forwarded.to_string());
        }
        Err(err) => {
            attributes.insert("stdinForwarded".to_string(), "false".to_string());
            attributes.insert("stdinError".to_string(), err.clone());
        }
    }
}

/// 把用户对统一 Agent interaction 的响应写回 runner 的 stdin。
#[tauri::command]
pub fn chat_respond_agent_interaction(
    task_id: String,
    request_id: String,
    kind: String,
    result: JsonValue,
    app: AppHandle,
    store: State<'_, ChatStore>,
) -> Result<(), String> {
    let payload = agent_interaction_response_payload(request_id.clone(), kind.clone(), result);
    let mut attributes = control_event_attributes([("requestId", request_id), ("kind", kind)]);
    let running_turn = {
        let turns = store.running_turns.lock().unwrap();
        turns.get(&task_id).cloned()
    };
    if running_turn
        .as_ref()
        .is_some_and(|turn| turn.runtime_channel == RUNTIME_CHANNEL_MUTSUKI_CORE)
    {
        attributes.insert("stdinForwarded".to_string(), "runtime".to_string());
        record_mutsuki_core_control_event_for_app(
            &app,
            &store,
            &task_id,
            "interaction_response",
            attributes,
            Some(payload),
        );
        return Ok(());
    }

    let write_result = write_runner_stdin(&store, &task_id, payload);
    attach_stdin_delivery(&mut attributes, &write_result);
    write_result.map(|_| ())
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
pub fn chat_get_runtime_snapshot(
    task_id: String,
    app: AppHandle,
    store: State<'_, ChatStore>,
) -> ChatRuntimeSnapshot {
    let conn = app
        .try_state::<LiliaStore>()
        .and_then(|store| store.conn().ok());
    crate::chat::state::chat_runtime_snapshot_with_persisted(conn.as_deref(), &store, &task_id)
}

#[tauri::command]
pub fn chat_ack_restored_rollback(task_id: String, app: AppHandle) -> Result<(), String> {
    let Some(store) = app.try_state::<LiliaStore>() else {
        return Ok(());
    };
    let conn = store.conn()?;
    clear_persisted_pending_rollback(&conn, &task_id)
}

#[tauri::command]
pub fn chat_set_composer_state(
    state: ChatComposerState,
    app: AppHandle,
    store: State<'_, ChatStore>,
) {
    let payload = {
        let mut composers = store.composers.lock().unwrap();
        let previous = composers.get(&state.task_id);
        let payload = composer_runtime_settings_update_payload(previous, &state);
        composers.insert(state.task_id.clone(), state.clone());
        payload
    };
    let Some(payload) = payload else {
        return;
    };
    let running_turn = {
        let turns = store.running_turns.lock().unwrap();
        turns.get(&state.task_id).cloned()
    };
    if running_turn
        .as_ref()
        .is_some_and(|turn| turn.runtime_channel == RUNTIME_CHANNEL_MUTSUKI_CORE)
    {
        let mut attributes = composer_runtime_settings_update_attributes(&payload);
        attributes.insert("stdinForwarded".to_string(), "runtime".to_string());
        record_mutsuki_core_control_event_for_app(
            &app,
            &store,
            &state.task_id,
            "permission_update",
            attributes,
            Some(payload),
        );
        return;
    }
    let mut attributes = composer_runtime_settings_update_attributes(&payload);
    let write_result = write_runner_stdin(&store, &state.task_id, payload);
    attach_stdin_delivery(&mut attributes, &write_result);
    if let Err(err) = write_result {
        eprintln!("[chat] runtime settings update failed: {err}");
    }
}

#[tauri::command]
pub fn chat_reset_session(task_id: String, chat_store: State<'_, ChatStore>, app: AppHandle) {
    mark_pending_reset_cleanup_for_app(&app, &chat_store, &task_id);
    let running_turn = {
        let turns = chat_store.running_turns.lock().unwrap();
        turns.get(&task_id).cloned()
    };
    let is_mutsuki_core = running_turn
        .as_ref()
        .is_some_and(|turn| turn.runtime_channel == RUNTIME_CHANNEL_MUTSUKI_CORE);
    if is_mutsuki_core {
        record_mutsuki_core_control_event_for_app(
            &app,
            &chat_store,
            &task_id,
            "reset_requested",
            control_event_attributes([("mode", "session_reset".to_string())]),
            None,
        );
    }
    let mut plan = plan_reset_session(&chat_store, &task_id, running_turn.as_ref());
    plan.cleared_guide_ids
        .append(&mut clear_persisted_pending_turns_for_app(&app, &task_id));
    if let Some(running_turn) = running_turn
        .as_ref()
        .filter(|turn| turn.runtime_channel == RUNTIME_CHANNEL_MUTSUKI_CORE)
    {
        persist_runtime_state_for_app(
            &app,
            &chat_store,
            &task_id,
            running_turn,
            "reset_pending_finish",
            None,
            None,
        );
    }
    if !is_mutsuki_core {
        plan.stopped_running = match stop_running_turn(&app, &chat_store, &task_id, false, true) {
            Ok(stopped) => stopped,
            Err(err) => {
                eprintln!("[chat] reset running turn failed: {err}");
                false
            }
        };
    }
    reset_cleared_guide_queue(&app, plan.cleared_guide_ids);
    chat_store
        .interrupted_turns
        .lock()
        .unwrap()
        .remove(&task_id);
    if !plan.stopped_running {
        chat_store.reset_turns.lock().unwrap().remove(&task_id);
        chat_store
            .pending_reset_cleanups
            .lock()
            .unwrap()
            .remove(&task_id);
        if let Err(err) = clear_runtime_finalization_for_app(&app, &task_id) {
            eprintln!("[chat] clear pending reset finalization failed: {err}");
        }
    }
    if plan.immediate_cleanup {
        chat_store.running_tasks.lock().unwrap().remove(&task_id);
        clear_running_handles(&chat_store, &task_id);
        let _ = clear_pending_turns_for_app(&app, &chat_store, &task_id);
        clear_task_runtime_state_for_reset(&app, &chat_store, &task_id);
    }
}
