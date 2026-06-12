use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use mutsuki_runtime_host::{JsonlProcessPoll, JsonlProcessRegistry, JsonlProcessStdinStatus};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::agent_events::{AgentEventHost, AgentRuntimeEvent, AgentTurnContext};
use crate::agent_extensions::TodoMirrorExtension;
#[cfg(test)]
use crate::chat::state::take_next_pending_turn;
use crate::chat::state::{
    clear_runtime_state_for_app, clear_task_runtime_state_for_reset, finish_running_turn_handles,
    is_turn_marked_reset, load_persisted_resume_session_id, normalize_runtime_channel,
    persist_agent_session_id, persist_and_emit_interrupted_timeline_event,
    persist_runtime_state_for_app, session_key, set_guide_status_for_app,
    should_emit_runner_exit_error, should_persist_user_message, take_next_pending_turn_for_app,
    take_next_recoverable_pending_turn, take_pending_finalization_for_app, ChatStore,
    PersistedRuntimeState, RunningTurn,
};
use crate::chat::timeline_sink::{
    assistant_error_text, log_agent_event_effect, normalize_timeline_text,
    persist_and_emit_error_timeline_event, persist_and_emit_message_timeline_event,
    timeline_input_from_runtime_event, TimelineThrottle,
};
use crate::chat::title_update::spawn_title_update;
use crate::chat::types::{
    AgentInteractionRequestEvent, ChatAttachment, ChatComposerState, ChatRollbackResult,
    ChatWorkflow, CodexComposerSettings, DoneEvent, TurnStartedEvent,
};
use crate::provider::{
    build_codex_app_server_probe_status, load_agent_interaction_settings, resolve_connection_for,
    CodexProfileSettings,
};
use crate::store::LiliaStore;
use crate::{
    plugins, BACKEND_CLAUDE, BACKEND_CODEX, RUNTIME_CHANNEL_BUILTIN, RUNTIME_CHANNEL_MUTSUKI_CORE,
};

pub(crate) struct RunnerInvocation {
    pub(crate) task_id: String,
    pub(crate) content: String,
    pub(crate) composer: ChatComposerState,
    pub(crate) project_cwd: String,
    pub(crate) attachments: Vec<ChatAttachment>,
    pub(crate) workflow: Option<ChatWorkflow>,
    pub(crate) turn_id: String,
    pub(crate) runtime_channel: String,
    pub(crate) resume_session_id: Option<String>,
    pub(crate) queued_count: usize,
    pub(crate) script_path: PathBuf,
}

#[derive(Default)]
pub(crate) struct RunnerOutput {
    pub(crate) last_session_id: Option<String>,
    pub(crate) interrupted: bool,
    pub(crate) reset: bool,
}

pub(crate) enum RunnerSessionPoll {
    Running,
    Completed(RunnerOutput),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunnerLifecycleEvent {
    pub(crate) stage: &'static str,
    pub(crate) detail: JsonValue,
}

pub(crate) trait RunnerLifecycleObserver {
    fn record(&mut self, event: RunnerLifecycleEvent);
}

struct NoopRunnerLifecycleObserver;

impl RunnerLifecycleObserver for NoopRunnerLifecycleObserver {
    fn record(&mut self, _event: RunnerLifecycleEvent) {}
}

fn record_runner_lifecycle(
    observer: &mut dyn RunnerLifecycleObserver,
    stage: &'static str,
    detail: JsonValue,
) {
    observer.record(RunnerLifecycleEvent { stage, detail });
}

pub(crate) struct RunnerSession {
    task_id: String,
    turn_id: String,
    backend: String,
    pub(crate) process_session_id: String,
    event_ctx: AgentTurnContext,
    event_host: AgentEventHost,
    timeline_throttle: TimelineThrottle,
    last_assistant_error_text: Option<String>,
    last_session_id: Option<String>,
    reset_observed: bool,
    finished: bool,
}

impl RunnerSession {
    pub(crate) fn terminate(&self) -> Result<bool, String> {
        process_registry().terminate(&self.process_session_id)
    }
}

pub(crate) fn reattach_runner_session<R: Runtime>(
    app_handle: &AppHandle<R>,
    task_id: String,
    turn_id: String,
    backend: String,
    process_session_id: String,
) -> Result<RunnerSession, String> {
    if !process_registry().is_active(&process_session_id) {
        return Err("runner session 已结束，无法恢复接管".to_string());
    }
    let mut event_host = AgentEventHost::new();
    event_host.register(Box::new(TodoMirrorExtension::new(app_handle.clone())));
    Ok(RunnerSession {
        task_id: task_id.clone(),
        turn_id: turn_id.clone(),
        backend: backend.clone(),
        process_session_id,
        event_ctx: AgentTurnContext {
            task_id,
            backend,
            turn_id,
            automation_run_id: None,
        },
        event_host,
        timeline_throttle: TimelineThrottle::new(),
        last_assistant_error_text: None,
        last_session_id: None,
        reset_observed: false,
        finished: false,
    })
}

fn process_registry() -> &'static JsonlProcessRegistry {
    static REGISTRY: OnceLock<JsonlProcessRegistry> = OnceLock::new();
    REGISTRY.get_or_init(JsonlProcessRegistry::new)
}

pub(crate) fn running_process_session_id(store: &ChatStore, task_id: &str) -> Option<String> {
    store
        .running_process_sessions
        .lock()
        .unwrap()
        .get(task_id)
        .cloned()
}

pub(crate) fn write_runner_stdin_payload(
    process_session_id: &str,
    payload: JsonValue,
) -> Result<bool, String> {
    let Some(handle) = process_registry().stdin_handle(process_session_id) else {
        return Ok(false);
    };
    let mut line = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    line.push('\n');
    let mut stdin = handle.lock().map_err(|e| e.to_string())?;
    stdin
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())?;
    Ok(true)
}

pub(crate) fn write_runner_stdin_for_task(
    store: &ChatStore,
    task_id: &str,
    payload: JsonValue,
) -> Result<bool, String> {
    let Some(process_session_id) = running_process_session_id(store, task_id) else {
        return Ok(false);
    };
    write_runner_stdin_payload(&process_session_id, payload)
}

pub(crate) fn terminate_runner_process_session(
    store: &ChatStore,
    task_id: &str,
) -> Result<bool, String> {
    let Some(process_session_id) = running_process_session_id(store, task_id) else {
        return Ok(false);
    };
    process_registry().terminate(&process_session_id)
}

pub(crate) fn process_session_is_active(process_session_id: &str) -> bool {
    process_registry().is_active(process_session_id)
}

#[cfg(test)]
pub(crate) fn start_test_process_session(
    child: std::process::Child,
    initial_payload: &JsonValue,
) -> Result<String, String> {
    process_registry()
        .start(child, initial_payload)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
pub(crate) fn remove_test_process_session(process_session_id: &str) {
    let _ = process_registry().remove(process_session_id);
}

// ---------- 子进程定位 ----------

/// 找到 agent-runner.mjs 的实际路径。
///
/// 开发态：cargo 编出来的二进制位于 `apps/desktop/src-tauri/target/{debug|release}/`，
/// 而脚本位于 `apps/desktop/agent-runner.mjs`，相对路径回退 3 层。
/// 按候选顺序找第一个存在的文件；找不到就返回最后一个候选让上层报错更直观。
pub(crate) fn locate_agent_runner<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
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

pub(crate) fn spawn_agent_turn<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    workflow: Option<ChatWorkflow>,
    turn_id: String,
) {
    let runtime_channel = load_agent_interaction_settings(&app).agent_runtime_channel;
    let runtime_channel = normalize_runtime_channel(&runtime_channel).to_string();
    spawn_agent_turn_with_channel(
        app,
        task_id,
        content,
        composer,
        project_cwd,
        attachments,
        workflow,
        turn_id,
        runtime_channel,
    );
}

fn spawn_agent_turn_with_channel<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
    attachments: Vec<ChatAttachment>,
    workflow: Option<ChatWorkflow>,
    turn_id: String,
    runtime_channel: String,
) {
    let backend = composer.backend.clone();
    let resume_session_id = {
        let store = app.state::<ChatStore>();
        let session = store
            .sdk_sessions
            .lock()
            .unwrap()
            .get(&session_key(&runtime_channel, &backend, &task_id))
            .cloned();
        session
    }
    .or_else(|| {
        let store = app.try_state::<LiliaStore>()?;
        let conn = store.conn().ok()?;
        load_persisted_resume_session_id(&conn, &task_id, &backend, &runtime_channel)
    });

    let script_path = locate_agent_runner(&app);
    let app_handle = app.clone();
    let task_id_for_thread = task_id.clone();
    let backend_for_thread = backend.clone();
    let invocation = RunnerInvocation {
        task_id,
        content,
        composer,
        project_cwd,
        attachments,
        workflow,
        turn_id,
        runtime_channel,
        resume_session_id,
        queued_count: 0,
        script_path,
    };

    thread::spawn(move || {
        let queued_count = {
            let store = app_handle.state::<ChatStore>();
            let in_memory = store
                .pending_turns
                .lock()
                .unwrap()
                .get(&task_id_for_thread)
                .map(|q| q.len())
                .unwrap_or(0);
            let persisted = app_handle
                .try_state::<LiliaStore>()
                .and_then(|store| store.conn().ok())
                .and_then(|conn| {
                    crate::chat::state::count_pending_turns(&conn, &task_id_for_thread).ok()
                })
                .unwrap_or(0);
            in_memory + persisted
        };
        let mut invocation = invocation;
        invocation.queued_count = queued_count;

        if invocation.runtime_channel == RUNTIME_CHANNEL_MUTSUKI_CORE {
            if let Err(err) =
                crate::chat::mutsuki_core_runtime::supervise_turn(app_handle.clone(), invocation)
            {
                persist_and_emit_error_timeline_event(
                    &app_handle,
                    &task_id_for_thread,
                    &backend_for_thread,
                    None,
                    format!("MutsukiCore runtime 初始化失败：{err}"),
                );
            }
            return;
        }

        let turn_id_for_finish = invocation.turn_id.clone();
        let runtime_channel_for_finish = invocation.runtime_channel.clone();
        let automation_run_id_for_finish =
            automation_run_id_from_workflow(invocation.workflow.as_ref());
        let result = run_node_agent_runner(&app_handle, invocation);
        let mut runner_ok = true;
        let output = match result {
            Ok(output) => output,
            Err(err) => {
                runner_ok = false;
                persist_and_emit_error_timeline_event(
                    &app_handle,
                    &task_id_for_thread,
                    &backend_for_thread,
                    Some(&turn_id_for_finish),
                    err,
                );
                RunnerOutput::default()
            }
        };
        let agent_success = runner_ok && !output.interrupted && !output.reset;
        {
            let store = app_handle.state::<ChatStore>();
            crate::automation::automation_complete_agent_turn(
                &app_handle,
                &store,
                automation_run_id_for_finish,
                &turn_id_for_finish,
                agent_success,
            );
        }
        finish_agent_turn(
            app_handle,
            task_id_for_thread,
            backend_for_thread,
            runtime_channel_for_finish,
            output.last_session_id,
            agent_success,
            None,
        );
    });
}

pub(crate) fn resume_or_dispatch_persisted_pending_turn<R: Runtime>(
    app: AppHandle<R>,
    task_id: String,
) -> Result<bool, String> {
    let store = app.state::<ChatStore>();
    if store.running_tasks.lock().unwrap().contains_key(&task_id) {
        return Ok(false);
    }
    let Some(lilia_store) = app.try_state::<LiliaStore>() else {
        return Ok(false);
    };
    let conn = lilia_store.conn()?;
    let Some(turn) = take_next_recoverable_pending_turn(&conn, &store, &task_id)? else {
        return Ok(false);
    };
    store
        .running_tasks
        .lock()
        .unwrap()
        .insert(task_id.clone(), true);
    if let Err(err) = set_guide_status_for_app(&app, turn.guide_id.as_deref(), "sent") {
        eprintln!("[todo-guides] mark recovered queued guide sent failed: {err}");
    }
    if should_persist_user_message(&turn.content, &turn.workflow) {
        persist_and_emit_message_timeline_event(
            &app,
            &turn.message,
            &turn.composer.backend,
            &turn.turn_id,
            false,
            None,
        );
    }
    spawn_agent_turn_with_channel(
        app,
        task_id,
        turn.content,
        turn.composer,
        turn.project_cwd,
        turn.attachments,
        turn.workflow,
        turn.turn_id,
        turn.runtime_channel,
    );
    Ok(true)
}

pub(crate) fn run_node_agent_runner<R: Runtime>(
    app_handle: &AppHandle<R>,
    invocation: RunnerInvocation,
) -> Result<RunnerOutput, String> {
    let mut observer = NoopRunnerLifecycleObserver;
    run_node_agent_runner_with_observer(app_handle, invocation, &mut observer)
}

pub(crate) fn start_runner_session<R: Runtime>(
    app_handle: &AppHandle<R>,
    invocation: RunnerInvocation,
    observer: &mut dyn RunnerLifecycleObserver,
) -> Result<RunnerSession, String> {
    let task_id_for_thread = invocation.task_id;
    let composer_for_thread = invocation.composer;
    let prompt_for_thread = invocation.content;
    let project_cwd = invocation.project_cwd;
    let attachments_for_thread = invocation.attachments;
    let workflow_for_thread = invocation.workflow;
    let automation_run_id_for_thread =
        automation_run_id_from_workflow(workflow_for_thread.as_ref());
    let turn_id_for_thread = invocation.turn_id;
    let runtime_channel_for_thread = invocation.runtime_channel;
    let resume_session_id = invocation.resume_session_id;
    let queued_count = invocation.queued_count;
    let script_path = invocation.script_path;
    let backend_for_thread = composer_for_thread.backend.clone();
    let connection = resolve_connection_for(app_handle, &backend_for_thread);
    let extensions = plugins::runtime_extensions(app_handle, Some(&project_cwd));
    let codex_settings = if backend_for_thread == BACKEND_CODEX {
        Some(build_effective_codex_settings(
            app_handle,
            &composer_for_thread,
        ))
    } else {
        None
    };
    let mut stdin_payload = build_runner_stdin_payload(
        &backend_for_thread,
        &project_cwd,
        &prompt_for_thread,
        &attachments_for_thread,
        workflow_for_thread.as_ref(),
        &composer_for_thread,
        resume_session_id.as_deref(),
        &extensions,
    );
    if let Some(context) = build_runner_conversation_context(app_handle, &task_id_for_thread) {
        stdin_payload["conversationContext"] = context;
    }
    if let Some(settings) = codex_settings {
        stdin_payload["codexSettings"] = settings;
    }
    record_runner_lifecycle(
        observer,
        "payload_prepared",
        serde_json::json!({
            "backend": backend_for_thread,
            "cwd": project_cwd,
            "resumeSessionId": resume_session_id,
            "queuedCount": queued_count,
            "attachmentCount": attachments_for_thread.len(),
            "workflowType": workflow_kind_for_lifecycle(workflow_for_thread.as_ref()),
            "hasCodexSettings": stdin_payload.get("codexSettings").is_some(),
            "codexRuntimeWorkspaceRootCount": stdin_payload
                .get("codexSettings")
                .and_then(|settings| settings.get("runtimeWorkspaceRoots"))
                .and_then(|roots| roots.as_array())
                .map(|roots| roots.len()),
            "hasConversationContext": stdin_payload.get("conversationContext").is_some(),
        }),
    );

    let mut cmd = Command::new("node");
    cmd.arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    record_runner_lifecycle(
        observer,
        "process_configured",
        serde_json::json!({
            "scriptPath": script_path.to_string_lossy(),
            "backend": backend_for_thread,
            "hasBaseUrl": connection.base_url.is_some(),
            "hasApiKey": connection.api_key.is_some(),
        }),
    );

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            let msg = format!("无法启动 node 子进程（请确保已安装 Node 18+ 并在 PATH 中）：{err}");
            record_runner_lifecycle(
                observer,
                "process_spawn_failed",
                serde_json::json!({ "message": msg }),
            );
            return Err(msg);
        }
    };
    record_runner_lifecycle(observer, "process_spawned", serde_json::json!({}));

    let process_session_id = process_registry()
        .start(child, &stdin_payload)
        .map_err(|err| format!("无法启动 runner session：{err}"))?;
    {
        let store = app_handle.state::<ChatStore>();
        let running_turn = RunningTurn {
            turn_id: turn_id_for_thread.clone(),
            backend: backend_for_thread.clone(),
            runtime_channel: runtime_channel_for_thread.clone(),
        };
        store
            .running_process_sessions
            .lock()
            .unwrap()
            .insert(task_id_for_thread.clone(), process_session_id.clone());
        store
            .running_turns
            .lock()
            .unwrap()
            .insert(task_id_for_thread.clone(), running_turn.clone());
        let context_json = runtime_state_context_json(
            &project_cwd,
            &prompt_for_thread,
            &attachments_for_thread,
            workflow_for_thread.as_ref(),
            &composer_for_thread,
            resume_session_id.as_deref(),
        );
        persist_runtime_state_for_app(
            app_handle,
            &store,
            &task_id_for_thread,
            &running_turn,
            "running",
            Some(&process_session_id),
            Some(&context_json),
        );
    }
    record_runner_lifecycle(
        observer,
        "running_turn_registered",
        serde_json::json!({
            "taskId": task_id_for_thread,
            "turnId": turn_id_for_thread,
            "backend": backend_for_thread,
        }),
    );

    let _ = app_handle.emit(
        "chat:turn-started",
        TurnStartedEvent {
            task_id: task_id_for_thread.clone(),
            queued_count,
        },
    );
    record_runner_lifecycle(
        observer,
        "turn_started_emitted",
        serde_json::json!({ "queuedCount": queued_count }),
    );
    match process_registry().stdin_status(&process_session_id) {
        Some(JsonlProcessStdinStatus::Ready { bytes }) => {
            record_runner_lifecycle(
                observer,
                "stdin_ready",
                serde_json::json!({ "bytes": bytes }),
            );
        }
        Some(JsonlProcessStdinStatus::Unavailable) => {
            record_runner_lifecycle(observer, "stdin_unavailable", serde_json::json!({}));
        }
        Some(JsonlProcessStdinStatus::WriteFailed { bytes }) => {
            record_runner_lifecycle(
                observer,
                "stdin_write_failed",
                serde_json::json!({ "bytes": bytes }),
            );
        }
        None => {
            record_runner_lifecycle(observer, "stdin_unavailable", serde_json::json!({}));
        }
    }
    if process_registry().stdout_available(&process_session_id) {
        record_runner_lifecycle(observer, "stdout_loop_started", serde_json::json!({}));
    } else {
        record_runner_lifecycle(observer, "stdout_unavailable", serde_json::json!({}));
    }

    let mut event_host = AgentEventHost::new();
    event_host.register(Box::new(TodoMirrorExtension::new(app_handle.clone())));
    Ok(RunnerSession {
        task_id: task_id_for_thread.clone(),
        turn_id: turn_id_for_thread.clone(),
        backend: backend_for_thread.clone(),
        process_session_id,
        event_ctx: AgentTurnContext {
            task_id: task_id_for_thread,
            backend: backend_for_thread,
            turn_id: turn_id_for_thread,
            automation_run_id: automation_run_id_for_thread,
        },
        event_host,
        timeline_throttle: TimelineThrottle::new(),
        last_assistant_error_text: None,
        last_session_id: None,
        reset_observed: false,
        finished: false,
    })
}

pub(crate) fn poll_runner_session<R: Runtime>(
    app_handle: &AppHandle<R>,
    session: &mut RunnerSession,
    observer: &mut dyn RunnerLifecycleObserver,
) -> Result<RunnerSessionPoll, String> {
    if session.finished {
        return Ok(RunnerSessionPoll::Completed(RunnerOutput {
            last_session_id: session.last_session_id.clone(),
            interrupted: false,
            reset: session.reset_observed,
        }));
    }

    loop {
        if is_turn_marked_reset(
            &app_handle.state::<ChatStore>(),
            &session.task_id,
            &session.turn_id,
            &session.backend,
        ) && !session.reset_observed
        {
            session.reset_observed = true;
            observer.record(RunnerLifecycleEvent {
                stage: "reset_mark_observed",
                detail: serde_json::json!({
                    "taskId": session.task_id,
                    "turnId": session.turn_id,
                }),
            });
        }
        let Some(poll) = process_registry().poll(&session.process_session_id) else {
            return Err("runner session 不存在或已被释放".to_string());
        };
        match poll {
            JsonlProcessPoll::Pending => return Ok(RunnerSessionPoll::Running),
            JsonlProcessPoll::StdoutLine(line) => {
                let value: JsonValue = match serde_json::from_str(&line) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                let Some(event) = AgentRuntimeEvent::from_runner_json(&value) else {
                    continue;
                };
                observer.record(RunnerLifecycleEvent {
                    stage: "runner_event",
                    detail: serde_json::json!({ "kind": runner_event_kind(&event) }),
                });
                handle_runner_runtime_event(app_handle, session, observer, event);
            }
            JsonlProcessPoll::Exited(exit) => {
                if process_registry().stdout_available(&session.process_session_id) {
                    observer.record(RunnerLifecycleEvent {
                        stage: "stdout_loop_finished",
                        detail: serde_json::json!({}),
                    });
                }
                let output = finalize_runner_session(
                    app_handle,
                    session,
                    observer,
                    exit.success,
                    &exit.stderr_text,
                );
                session.finished = true;
                return Ok(RunnerSessionPoll::Completed(output));
            }
        }
    }
}

fn handle_runner_runtime_event<R: Runtime>(
    app_handle: &AppHandle<R>,
    session: &mut RunnerSession,
    observer: &mut dyn RunnerLifecycleObserver,
    event: AgentRuntimeEvent,
) {
    log_agent_event_effect(session.event_host.dispatch(&session.event_ctx, &event));
    match &event {
        AgentRuntimeEvent::ToolUse { .. } | AgentRuntimeEvent::TodoList { .. } => {}
        AgentRuntimeEvent::Timeline { .. } => {
            if let Some(input) = timeline_input_from_runtime_event(&session.event_ctx, &event) {
                if let Some(text) = assistant_error_text(&input) {
                    session.last_assistant_error_text = Some(text);
                }
                session.timeline_throttle.submit(app_handle, input);
            }
        }
        AgentRuntimeEvent::InteractionRequest {
            id,
            kind,
            backend,
            payload,
        } => {
            record_runner_lifecycle(
                observer,
                "interaction_requested",
                serde_json::json!({
                    "requestId": id,
                    "kind": kind,
                    "backend": backend.clone().unwrap_or_else(|| session.backend.clone()),
                }),
            );
            let _ = app_handle.emit(
                "chat:agent-interaction-request",
                AgentInteractionRequestEvent {
                    task_id: session.task_id.clone(),
                    turn_id: session.turn_id.clone(),
                    backend: backend.clone().unwrap_or_else(|| session.backend.clone()),
                    request_id: id.clone(),
                    kind: kind.clone(),
                    payload: payload.clone(),
                },
            );
            crate::automation::emit_interaction_signal(
                app_handle,
                session.task_id.clone(),
                session.turn_id.clone(),
                backend.clone().unwrap_or_else(|| session.backend.clone()),
                id.clone(),
                kind.clone(),
                payload.clone(),
                session.event_ctx.automation_run_id.clone(),
            );
        }
        AgentRuntimeEvent::Done { session_id, .. } => {
            if let Some(sid) = session_id {
                session.last_session_id = Some(sid.clone());
            }
            record_runner_lifecycle(
                observer,
                "runner_done",
                serde_json::json!({ "hasSessionId": session_id.is_some() }),
            );
        }
        AgentRuntimeEvent::PromptSuggestion { suggestion, uuid } => {
            if session.backend == BACKEND_CLAUDE {
                if let Err(err) = crate::conversation_suggestions::save_claude_prompt_suggestion(
                    app_handle,
                    &session.task_id,
                    suggestion,
                    uuid.as_deref(),
                ) {
                    eprintln!(
                        "[conversation-suggestions] save Claude prompt suggestion failed: {err}"
                    );
                }
            }
        }
        AgentRuntimeEvent::Error { message } => {
            record_runner_lifecycle(
                observer,
                "runner_error_event",
                serde_json::json!({ "message": message }),
            );
            session.timeline_throttle.flush_all(app_handle);
            if session
                .last_assistant_error_text
                .as_deref()
                .is_some_and(|text| normalize_timeline_text(message).contains(text))
            {
                return;
            }
            persist_and_emit_error_timeline_event(
                app_handle,
                &session.task_id,
                &session.backend,
                Some(&session.turn_id),
                message.clone(),
            );
        }
    }
}

fn finalize_runner_session<R: Runtime>(
    app_handle: &AppHandle<R>,
    session: &mut RunnerSession,
    observer: &mut dyn RunnerLifecycleObserver,
    process_success: bool,
    stderr_text: &str,
) -> RunnerOutput {
    drop(process_registry().take_stdin_handle(&session.process_session_id));
    process_registry().release_child_handle(&session.process_session_id);
    let _ = process_registry().remove(&session.process_session_id);
    record_runner_lifecycle(observer, "stdin_closed", serde_json::json!({}));
    let finished = {
        let store = app_handle.state::<ChatStore>();
        finish_running_turn_handles(&store, &session.task_id, &session.turn_id, &session.backend)
    };
    clear_runtime_state_for_app(app_handle, &session.task_id);
    record_runner_lifecycle(
        observer,
        "stop_marks_consumed",
        serde_json::json!({
            "interrupted": finished.interrupted,
            "reset": finished.reset,
        }),
    );

    if finished.reset {
        session.timeline_throttle.pending.clear();
        record_runner_lifecycle(
            observer,
            "timeline_discarded_for_reset",
            serde_json::json!({}),
        );
    } else {
        session.timeline_throttle.flush_all(app_handle);
        record_runner_lifecycle(observer, "timeline_flushed", serde_json::json!({}));
    }

    record_runner_lifecycle(
        observer,
        "process_waited",
        serde_json::json!({
            "success": process_success,
            "stderrLength": stderr_text.len(),
        }),
    );

    if finished.interrupted && !finished.reset {
        record_runner_lifecycle(
            observer,
            "interrupted_event_persisted",
            serde_json::json!({}),
        );
        persist_and_emit_interrupted_timeline_event(
            app_handle,
            &session.task_id,
            &session.backend,
            &session.turn_id,
        );
    }

    if !finished.reset
        && should_emit_runner_exit_error(finished.interrupted, !process_success, stderr_text)
    {
        record_runner_lifecycle(
            observer,
            "process_exit_error_emitted",
            serde_json::json!({ "stderrLength": stderr_text.trim().len() }),
        );
        persist_and_emit_error_timeline_event(
            app_handle,
            &session.task_id,
            &session.backend,
            Some(&session.turn_id),
            format!("agent 进程异常退出：{}", stderr_text.trim()),
        );
    }

    if !finished.interrupted && !finished.reset {
        record_runner_lifecycle(observer, "title_update_spawned", serde_json::json!({}));
        spawn_title_update(
            app_handle.clone(),
            session.task_id.clone(),
            session.backend.clone(),
            Some(session.turn_id.clone()),
        );
    }

    RunnerOutput {
        last_session_id: session.last_session_id.clone(),
        interrupted: finished.interrupted,
        reset: finished.reset,
    }
}

pub(crate) fn run_node_agent_runner_with_observer<R: Runtime>(
    app_handle: &AppHandle<R>,
    invocation: RunnerInvocation,
    observer: &mut dyn RunnerLifecycleObserver,
) -> Result<RunnerOutput, String> {
    let mut session = start_runner_session(app_handle, invocation, observer)?;
    loop {
        match poll_runner_session(app_handle, &mut session, observer)? {
            RunnerSessionPoll::Running => thread::sleep(Duration::from_millis(16)),
            RunnerSessionPoll::Completed(output) => return Ok(output),
        }
    }
}

pub(crate) fn resume_node_agent_runner_with_observer<R: Runtime>(
    app_handle: &AppHandle<R>,
    task_id: String,
    turn_id: String,
    backend: String,
    process_session_id: String,
    observer: &mut dyn RunnerLifecycleObserver,
) -> Result<RunnerOutput, String> {
    let mut session =
        reattach_runner_session(app_handle, task_id, turn_id, backend, process_session_id)?;
    loop {
        match poll_runner_session(app_handle, &mut session, observer)? {
            RunnerSessionPoll::Running => thread::sleep(Duration::from_millis(16)),
            RunnerSessionPoll::Completed(output) => return Ok(output),
        }
    }
}

pub(crate) fn resume_node_agent_runner<R: Runtime>(
    app_handle: &AppHandle<R>,
    task_id: String,
    turn_id: String,
    backend: String,
    process_session_id: String,
) -> Result<RunnerOutput, String> {
    let mut observer = NoopRunnerLifecycleObserver;
    resume_node_agent_runner_with_observer(
        app_handle,
        task_id,
        turn_id,
        backend,
        process_session_id,
        &mut observer,
    )
}

pub(crate) fn resume_persisted_node_agent_runner<R: Runtime>(
    app_handle: AppHandle<R>,
    persisted: PersistedRuntimeState,
) -> Result<(), String> {
    let process_session_id = persisted
        .process_session_id
        .clone()
        .ok_or_else(|| "persisted runner session 缺少 process_session_id".to_string())?;
    let task_id = persisted.task_id.clone();
    let turn_id = persisted.turn.turn_id.clone();
    let backend = persisted.turn.backend.clone();
    let runtime_channel = persisted.turn.runtime_channel.clone();
    let output = resume_node_agent_runner(
        &app_handle,
        task_id.clone(),
        turn_id,
        backend.clone(),
        process_session_id,
    )?;
    finish_agent_turn(
        app_handle,
        task_id,
        backend,
        runtime_channel,
        output.last_session_id,
        !output.interrupted && !output.reset,
        None,
    );
    Ok(())
}

fn workflow_kind_for_lifecycle(workflow: Option<&ChatWorkflow>) -> Option<&'static str> {
    match workflow? {
        ChatWorkflow::CodexReview { .. } => Some("codex_review"),
        ChatWorkflow::CodexFixSuggestion { .. } => Some("codex_fix_suggestion"),
        ChatWorkflow::CodexBatchApply { .. } => Some("codex_batch_apply"),
        ChatWorkflow::CodexGoal { .. } => Some("codex_goal"),
        ChatWorkflow::CodexCompact => Some("codex_compact"),
        ChatWorkflow::CodexBackgroundTerminalsClean => Some("codex_background_terminals_clean"),
        ChatWorkflow::CodexMemoryMode { .. } => Some("codex_memory_mode"),
        ChatWorkflow::CodexMemoryReset => Some("codex_memory_reset"),
        ChatWorkflow::CodexThreadFork { .. } => Some("codex_thread_fork"),
        ChatWorkflow::CodexConfigDiagnostics { .. } => Some("codex_config_diagnostics"),
        ChatWorkflow::Automation { .. } => Some("automation"),
    }
}

fn automation_run_id_from_workflow(workflow: Option<&ChatWorkflow>) -> Option<String> {
    match workflow {
        Some(ChatWorkflow::Automation { automation_run_id }) => Some(automation_run_id.clone()),
        _ => None,
    }
}

fn runner_event_kind(event: &AgentRuntimeEvent) -> &'static str {
    match event {
        AgentRuntimeEvent::ToolUse { .. } => "tool_use",
        AgentRuntimeEvent::TodoList { .. } => "todo_list",
        AgentRuntimeEvent::Timeline { .. } => "timeline",
        AgentRuntimeEvent::InteractionRequest { .. } => "interaction_request",
        AgentRuntimeEvent::Done { .. } => "done",
        AgentRuntimeEvent::PromptSuggestion { .. } => "prompt_suggestion",
        AgentRuntimeEvent::Error { .. } => "error",
    }
}

pub(crate) fn build_runner_stdin_payload<T: Serialize>(
    backend: &str,
    project_cwd: &str,
    prompt: &str,
    attachments: &[ChatAttachment],
    workflow: Option<&ChatWorkflow>,
    composer: &ChatComposerState,
    resume_session_id: Option<&str>,
    extensions: &T,
) -> JsonValue {
    serde_json::json!({
        "backend": backend,
        "cwd": project_cwd,
        "prompt": prompt,
        "attachments": attachments,
        "workflow": workflow,
        "model": composer.model,
        "resumeSessionId": resume_session_id,
        "planMode": composer.plan_mode,
        "permission": composer.permission,
        "extensions": extensions,
    })
}

fn runtime_state_context_json(
    project_cwd: &str,
    prompt: &str,
    attachments: &[ChatAttachment],
    workflow: Option<&ChatWorkflow>,
    composer: &ChatComposerState,
    resume_session_id: Option<&str>,
) -> String {
    serde_json::json!({
        "projectCwd": project_cwd,
        "promptLength": prompt.chars().count(),
        "attachmentCount": attachments.len(),
        "workflowType": workflow_kind_for_lifecycle(workflow),
        "automationRunId": automation_run_id_from_workflow(workflow),
        "resumeSessionId": resume_session_id,
        "permission": composer.permission,
        "composerRuntimeWorkspaceRoots": composer
            .codex_settings
            .runtime_workspace_roots
            .clone()
            .unwrap_or_default(),
    })
    .to_string()
}

fn build_effective_codex_settings<R: Runtime>(
    app: &AppHandle<R>,
    composer: &ChatComposerState,
) -> JsonValue {
    let global = load_agent_interaction_settings(app).codex_profile;
    let project = crate::project_shell::load_project_settings(app)
        .codex_defaults
        .unwrap_or_default();
    let local = &composer.codex_settings;
    let fallback_model = composer.model.trim();
    let default_model = crate::chat::state::default_model_for_backend(BACKEND_CODEX);
    let model = normalize_optional_string(local.model.clone())
        .or_else(|| {
            if !fallback_model.is_empty() && fallback_model != default_model {
                Some(fallback_model.to_string())
            } else {
                normalize_optional_string(project.model.clone())
                    .or_else(|| normalize_optional_string(global.model.clone()))
            }
        })
        .or_else(|| normalize_optional_string(Some(fallback_model.to_string())));
    let reasoning_effort = normalize_reasoning_effort(local.reasoning_effort.clone())
        .or_else(|| normalize_reasoning_effort(project.reasoning_effort.clone()))
        .or_else(|| normalize_reasoning_effort(global.reasoning_effort.clone()));
    let runtime_workspace_roots = effective_runtime_workspace_roots(local, &project, &global);
    let permissions_profile = normalize_permission_profile(
        local
            .permissions
            .as_ref()
            .map(|permissions| permissions.profile.clone())
            .or_else(|| {
                let project_profile =
                    normalize_permission_profile(Some(project.permissions.profile.clone()));
                if project_profile == "default" {
                    None
                } else {
                    Some(project_profile)
                }
            })
            .or_else(|| Some(global.permissions.profile.clone())),
    );
    let responses_api_client_metadata =
        normalize_json_object(local.responses_api_client_metadata.clone())
            .or_else(|| normalize_json_object(project.responses_api_client_metadata.clone()))
            .or_else(|| normalize_json_object(global.responses_api_client_metadata.clone()));
    let additional_context = normalize_optional_string(local.additional_context.clone())
        .or_else(|| normalize_optional_string(project.additional_context.clone()))
        .or_else(|| normalize_optional_string(global.additional_context.clone()));
    let persist_extended_history = local
        .persist_extended_history
        .or(project.persist_extended_history)
        .or(global.persist_extended_history);
    let initial_turns_page = normalize_json_object(local.initial_turns_page.clone())
        .or_else(|| normalize_json_object(project.initial_turns_page.clone()))
        .or_else(|| normalize_json_object(global.initial_turns_page.clone()));
    let exclude_turns = effective_string_list(
        local.exclude_turns.clone(),
        &project.exclude_turns,
        &global.exclude_turns,
    );
    let command_exec_permission_profile =
        normalize_optional_permission_profile(local.command_exec_permission_profile.clone())
            .or_else(|| {
                normalize_optional_permission_profile(
                    project.command_exec_permission_profile.clone(),
                )
            })
            .or_else(|| {
                normalize_optional_permission_profile(
                    global.command_exec_permission_profile.clone(),
                )
            });
    let profile = normalize_codex_settings_profile(local.profile.clone())
        .or_else(|| {
            let project_profile = normalize_codex_settings_profile(Some(project.profile));
            project_profile.filter(|value| value != "default")
        })
        .unwrap_or_else(|| normalize_codex_settings_profile(Some(global.profile)).unwrap());

    serde_json::json!({
        "profile": profile,
        "model": model,
        "reasoningEffort": reasoning_effort,
        "runtimeWorkspaceRoots": runtime_workspace_roots,
        "permissions": {
            "profile": permissions_profile,
        },
        "responsesApiClientMetadata": responses_api_client_metadata,
        "additionalContext": additional_context,
        "persistExtendedHistory": persist_extended_history,
        "initialTurnsPage": initial_turns_page,
        "excludeTurns": exclude_turns,
        "commandExecPermissionProfile": command_exec_permission_profile,
    })
}

fn effective_runtime_workspace_roots(
    local: &CodexComposerSettings,
    project: &CodexProfileSettings,
    global: &CodexProfileSettings,
) -> Vec<String> {
    match local.runtime_workspace_roots.clone() {
        Some(roots) => normalize_runtime_workspace_roots(roots),
        None if !project.runtime_workspace_roots.is_empty() => {
            normalize_runtime_workspace_roots(project.runtime_workspace_roots.clone())
        }
        None => normalize_runtime_workspace_roots(global.runtime_workspace_roots.clone()),
    }
}

fn effective_string_list(
    local: Option<Vec<String>>,
    project: &[String],
    global: &[String],
) -> Vec<String> {
    match local {
        Some(values) => normalize_string_list(values),
        None if !project.is_empty() => normalize_string_list(project.to_vec()),
        None => normalize_string_list(global.to_vec()),
    }
}

fn normalize_json_object(value: Option<serde_json::Value>) -> Option<serde_json::Value> {
    match value {
        Some(serde_json::Value::Object(map)) if !map.is_empty() => {
            Some(serde_json::Value::Object(map))
        }
        _ => None,
    }
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

fn normalize_reasoning_effort(value: Option<String>) -> Option<String> {
    let value = normalize_optional_string(value)?;
    match value.as_str() {
        "low" | "medium" | "high" | "xhigh" => Some(value),
        _ => None,
    }
}

fn normalize_runtime_workspace_roots(roots: Vec<String>) -> Vec<String> {
    normalize_string_list(roots)
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || normalized.iter().any(|seen| seen == trimmed) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn normalize_permission_profile(value: Option<String>) -> String {
    match normalize_optional_string(value).as_deref() {
        Some("readOnly") => "readOnly".to_string(),
        Some("workspaceWrite") => "workspaceWrite".to_string(),
        Some("dangerFullAccess") => "dangerFullAccess".to_string(),
        _ => "default".to_string(),
    }
}

fn normalize_optional_permission_profile(value: Option<String>) -> Option<String> {
    match normalize_optional_string(value).as_deref() {
        Some("default") => Some("default".to_string()),
        Some("readOnly") => Some("readOnly".to_string()),
        Some("workspaceWrite") => Some("workspaceWrite".to_string()),
        Some("dangerFullAccess") => Some("dangerFullAccess".to_string()),
        _ => None,
    }
}

fn normalize_codex_settings_profile(value: Option<String>) -> Option<String> {
    match normalize_optional_string(value).as_deref() {
        Some("fast") => Some("fast".to_string()),
        Some("balanced") => Some("balanced".to_string()),
        Some("deep") => Some("deep".to_string()),
        Some("default") => Some("default".to_string()),
        _ => None,
    }
}

pub(crate) fn finish_agent_turn<R: Runtime>(
    app_handle: AppHandle<R>,
    task_id: String,
    backend: String,
    runtime_channel: String,
    last_session_id: Option<String>,
    advance_queue: bool,
    rollback: Option<ChatRollbackResult>,
) {
    // 记下 session id 供下一轮 resume。
    if normalize_runtime_channel(&runtime_channel) == RUNTIME_CHANNEL_BUILTIN {
        if let Some(sid) = last_session_id.clone() {
            let store = app_handle.state::<ChatStore>();
            store.sdk_sessions.lock().unwrap().insert(
                session_key(&runtime_channel, &backend, &task_id),
                sid.clone(),
            );
            if let Some(store) = app_handle.try_state::<LiliaStore>() {
                match store.conn().and_then(|conn| {
                    persist_agent_session_id(&conn, &task_id, &backend, &runtime_channel, &sid)
                }) {
                    Ok(()) => {}
                    Err(err) => eprintln!("[agent-session] persist checkpoint failed: {err}"),
                }
            }
        }
    }

    let (pending_rollback, pending_reset_cleanup) = {
        let store = app_handle.state::<ChatStore>();
        take_pending_finalization_for_app(&app_handle, &store, &task_id)
    };
    let completion = build_turn_completion(
        task_id.clone(),
        last_session_id.clone(),
        rollback,
        pending_rollback,
        pending_reset_cleanup,
    );

    let _ = app_handle.emit("chat:done", completion.done_event);

    if completion.reset_cleanup_requested {
        let store = app_handle.state::<ChatStore>();
        clear_task_runtime_state_for_reset(&app_handle, &store, &task_id);
    }

    let next_dispatch = {
        let store = app_handle.state::<ChatStore>();
        plan_next_turn_dispatch(&app_handle, &store, &task_id, advance_queue)
    };
    if let Some(turn) = next_dispatch.next_turn {
        if let Err(err) = set_guide_status_for_app(&app_handle, turn.guide_id.as_deref(), "sent") {
            eprintln!("[todo-guides] mark queued guide sent failed: {err}");
        }
        if should_persist_user_message(&turn.content, &turn.workflow) {
            persist_and_emit_message_timeline_event(
                &app_handle,
                &turn.message,
                &turn.composer.backend,
                &turn.turn_id,
                false,
                None,
            );
        }
        spawn_agent_turn_with_channel(
            app_handle,
            task_id,
            turn.content,
            turn.composer,
            turn.project_cwd,
            turn.attachments,
            turn.workflow,
            turn.turn_id,
            runtime_channel,
        );
    }
}

#[derive(Debug)]
struct TurnCompletion {
    done_event: DoneEvent,
    reset_cleanup_requested: bool,
}

struct NextTurnDispatch {
    next_turn: Option<crate::chat::state::PendingChatTurn>,
}

fn build_turn_completion(
    task_id: String,
    session_id: Option<String>,
    explicit_rollback: Option<ChatRollbackResult>,
    pending_rollback: Option<ChatRollbackResult>,
    pending_reset_cleanup: bool,
) -> TurnCompletion {
    TurnCompletion {
        done_event: DoneEvent {
            task_id,
            session_id,
            subtype: None,
            rollback: explicit_rollback.or(pending_rollback),
        },
        reset_cleanup_requested: pending_reset_cleanup,
    }
}

fn plan_next_turn_dispatch<R: Runtime>(
    app: &AppHandle<R>,
    store: &ChatStore,
    task_id: &str,
    advance_queue: bool,
) -> NextTurnDispatch {
    NextTurnDispatch {
        next_turn: take_next_pending_turn_for_app(app, store, task_id, advance_queue),
    }
}

const CONVERSATION_CONTEXT_TASK_LIMIT: i64 = 24;
const CONVERSATION_CONTEXT_MESSAGE_LIMIT: i64 = 24;
const CONVERSATION_CONTEXT_TEXT_LIMIT: usize = 2_000;

fn build_runner_conversation_context<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
) -> Option<JsonValue> {
    let store = app.try_state::<LiliaStore>()?;
    let conn = store.conn().ok()?;
    let current = load_context_task_row(&conn, task_id).ok().flatten()?;
    let parent_task_id = current.parent_id.clone()?;
    let project_id = current.project_id.clone();
    let mut tasks = Vec::new();
    let mut seen = std::collections::HashSet::new();

    seen.insert(task_id.to_string());
    if let Ok(task) = context_task_json(&conn, current, true) {
        tasks.push(task);
    }

    if seen.insert(parent_task_id.clone()) {
        if let Ok(Some(task)) = load_context_task(&conn, &parent_task_id, true) {
            tasks.push(task);
        }
    }

    if let Ok(mut related) = load_related_context_tasks(
        &conn,
        project_id.as_deref(),
        CONVERSATION_CONTEXT_TASK_LIMIT,
    ) {
        for task in related.drain(..) {
            let Some(id) = task.get("taskId").and_then(|value| value.as_str()) else {
                continue;
            };
            if seen.insert(id.to_string()) {
                tasks.push(task);
            }
        }
    }

    Some(serde_json::json!({
        "currentTaskId": task_id,
        "parentTaskId": parent_task_id,
        "tasks": tasks,
    }))
}

struct ContextTaskRow {
    id: String,
    project_id: Option<String>,
    title: String,
    status: String,
    created_at: i64,
    parent_id: Option<String>,
}

fn load_context_task_row(
    conn: &rusqlite::Connection,
    task_id: &str,
) -> Result<Option<ContextTaskRow>, String> {
    conn.query_row(
        r#"SELECT id, project_id, title, status, created_at, parent_id
           FROM tasks
           WHERE id = ?1 AND archived = 0"#,
        params![task_id],
        |row| {
            Ok(ContextTaskRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                parent_id: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("conversation context: 查询任务失败：{e}"))
}

fn load_context_task(
    conn: &rusqlite::Connection,
    task_id: &str,
    include_messages: bool,
) -> Result<Option<JsonValue>, String> {
    let Some(row) = load_context_task_row(conn, task_id)? else {
        return Ok(None);
    };
    Ok(Some(context_task_json(conn, row, include_messages)?))
}

fn load_related_context_tasks(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
    limit: i64,
) -> Result<Vec<JsonValue>, String> {
    let rows = if let Some(project_id) = project_id {
        let mut stmt = conn
            .prepare(
                r#"SELECT id, project_id, title, status, created_at, parent_id
                   FROM tasks
                   WHERE project_id = ?1 AND archived = 0
                   ORDER BY pinned DESC, sort_order ASC
                   LIMIT ?2"#,
            )
            .map_err(|e| format!("conversation context: prepare 失败：{e}"))?;
        let mapped = stmt
            .query_map(params![project_id, limit], context_task_row_from_sql)
            .map_err(|e| format!("conversation context: query 失败：{e}"))?;
        collect_context_task_rows(mapped)?
    } else {
        let mut stmt = conn
            .prepare(
                r#"SELECT id, project_id, title, status, created_at, parent_id
                   FROM tasks
                   WHERE project_id IS NULL AND archived = 0
                   ORDER BY pinned DESC, sort_order ASC
                   LIMIT ?1"#,
            )
            .map_err(|e| format!("conversation context: prepare 失败：{e}"))?;
        let mapped = stmt
            .query_map(params![limit], context_task_row_from_sql)
            .map_err(|e| format!("conversation context: query 失败：{e}"))?;
        collect_context_task_rows(mapped)?
    };

    rows.into_iter()
        .map(|row| context_task_json(conn, row, false))
        .collect()
}

fn context_task_row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContextTaskRow> {
    Ok(ContextTaskRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        created_at: row.get(4)?,
        parent_id: row.get(5)?,
    })
}

fn collect_context_task_rows<I>(rows: I) -> Result<Vec<ContextTaskRow>, String>
where
    I: IntoIterator<Item = rusqlite::Result<ContextTaskRow>>,
{
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("conversation context: row 失败：{e}"))?);
    }
    Ok(out)
}

fn context_task_json(
    conn: &rusqlite::Connection,
    task: ContextTaskRow,
    include_messages: bool,
) -> Result<JsonValue, String> {
    let messages = if include_messages {
        load_context_messages(conn, &task.id)?
    } else {
        Vec::new()
    };
    let truncated = include_messages && messages.len() as i64 >= CONVERSATION_CONTEXT_MESSAGE_LIMIT;
    Ok(serde_json::json!({
        "taskId": task.id,
        "projectId": task.project_id,
        "title": task.title,
        "status": task.status,
        "createdAt": task.created_at,
        "parentId": task.parent_id,
        "messages": messages,
        "truncated": truncated,
    }))
}

fn load_context_messages(
    conn: &rusqlite::Connection,
    task_id: &str,
) -> Result<Vec<JsonValue>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT summary, payload, created_at
               FROM agent_timeline_events
               WHERE task_id = ?1 AND kind = 'message'
               ORDER BY turn_seq ASC, intra_turn_order ASC, created_at ASC
               LIMIT ?2"#,
        )
        .map_err(|e| format!("conversation context: prepare messages 失败：{e}"))?;
    let rows = stmt
        .query_map(
            params![task_id, CONVERSATION_CONTEXT_MESSAGE_LIMIT],
            |row| {
                let summary: Option<String> = row.get(0)?;
                let payload_text: String = row.get(1)?;
                let created_at: i64 = row.get(2)?;
                Ok((summary, payload_text, created_at))
            },
        )
        .map_err(|e| format!("conversation context: query messages 失败：{e}"))?;

    let mut out = Vec::new();
    for row in rows {
        let (summary, payload_text, created_at) =
            row.map_err(|e| format!("conversation context: message row 失败：{e}"))?;
        let payload = serde_json::from_str::<JsonValue>(&payload_text).unwrap_or(JsonValue::Null);
        let role = payload
            .get("role")
            .and_then(|value| value.as_str())
            .unwrap_or("assistant");
        let content = payload
            .get("content")
            .and_then(|value| value.as_str())
            .or(summary.as_deref())
            .unwrap_or("");
        if content.trim().is_empty() {
            continue;
        }
        out.push(serde_json::json!({
            "role": role,
            "content": clip_context_text(content),
            "createdAt": created_at,
        }));
    }
    Ok(out)
}

fn clip_context_text(text: &str) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= CONVERSATION_CONTEXT_TEXT_LIMIT {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RUNTIME_CHANNEL_BUILTIN;

    #[derive(Default)]
    struct CollectingLifecycleObserver {
        events: Vec<RunnerLifecycleEvent>,
    }

    impl RunnerLifecycleObserver for CollectingLifecycleObserver {
        fn record(&mut self, event: RunnerLifecycleEvent) {
            self.events.push(event);
        }
    }

    #[test]
    fn runner_lifecycle_observer_records_stable_stage_and_detail() {
        let mut observer = CollectingLifecycleObserver::default();

        record_runner_lifecycle(
            &mut observer,
            "process_spawned",
            serde_json::json!({ "pid": 42 }),
        );

        assert_eq!(observer.events.len(), 1);
        assert_eq!(observer.events[0].stage, "process_spawned");
        assert_eq!(observer.events[0].detail["pid"], serde_json::json!(42));
    }

    #[test]
    fn runner_lifecycle_classifies_workflow_and_runtime_events() {
        let workflow = ChatWorkflow::CodexMemoryReset;
        assert_eq!(
            workflow_kind_for_lifecycle(Some(&workflow)),
            Some("codex_memory_reset")
        );
        assert_eq!(workflow_kind_for_lifecycle(None), None);

        assert_eq!(
            runner_event_kind(&AgentRuntimeEvent::InteractionRequest {
                id: "ask-1".to_string(),
                kind: "tool_consent".to_string(),
                backend: Some(BACKEND_CODEX.to_string()),
                payload: JsonValue::Null,
            }),
            "interaction_request"
        );
        assert_eq!(
            runner_event_kind(&AgentRuntimeEvent::Done {
                session_id: Some("thread-1".to_string()),
                subtype: None,
            }),
            "done"
        );
    }

    #[test]
    fn turn_completion_prefers_explicit_rollback_over_pending() {
        let explicit = ChatRollbackResult {
            rolled_back: true,
            restored_content: "explicit".to_string(),
            restored_attachments: Vec::new(),
            removed_event_ids: vec!["evt-explicit".to_string()],
        };
        let pending = ChatRollbackResult {
            rolled_back: true,
            restored_content: "pending".to_string(),
            restored_attachments: Vec::new(),
            removed_event_ids: vec!["evt-pending".to_string()],
        };

        let completion = build_turn_completion(
            "task-1".to_string(),
            Some("session-1".to_string()),
            Some(explicit),
            Some(pending),
            true,
        );

        assert_eq!(completion.done_event.task_id, "task-1");
        assert_eq!(
            completion.done_event.session_id.as_deref(),
            Some("session-1")
        );
        let rollback = completion.done_event.rollback.expect("rollback");
        assert_eq!(rollback.restored_content, "explicit");
        assert_eq!(rollback.removed_event_ids, vec!["evt-explicit".to_string()]);
        assert!(completion.reset_cleanup_requested);
    }

    #[test]
    fn turn_completion_falls_back_to_pending_rollback() {
        let pending = ChatRollbackResult {
            rolled_back: true,
            restored_content: "pending".to_string(),
            restored_attachments: Vec::new(),
            removed_event_ids: vec!["evt-pending".to_string()],
        };

        let completion =
            build_turn_completion("task-2".to_string(), None, None, Some(pending), false);

        assert_eq!(completion.done_event.task_id, "task-2");
        assert!(completion.done_event.session_id.is_none());
        let rollback = completion.done_event.rollback.expect("rollback");
        assert_eq!(rollback.restored_content, "pending");
        assert_eq!(rollback.removed_event_ids, vec!["evt-pending".to_string()]);
        assert!(!completion.reset_cleanup_requested);
    }

    #[test]
    fn next_turn_dispatch_advances_queue_when_allowed() {
        let store = ChatStore::default();
        store
            .running_tasks
            .lock()
            .unwrap()
            .insert("task-1".to_string(), true);
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(crate::chat::state::PendingChatTurn {
                content: "next".to_string(),
                composer: crate::chat::state::default_composer("task-1"),
                project_cwd: "C:\\repo".to_string(),
                attachments: Vec::new(),
                workflow: None,
                message: crate::chat::types::ChatMessage {
                    id: "u-next".to_string(),
                    task_id: "task-1".to_string(),
                    role: "user".to_string(),
                    content: "next".to_string(),
                    attachments: Vec::new(),
                    created_at: 1,
                },
                turn_id: "turn-next".to_string(),
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
                guide_id: Some("guide-next".to_string()),
            });

        let dispatch = NextTurnDispatch {
            next_turn: take_next_pending_turn(&store, "task-1", true),
        };

        let turn = dispatch.next_turn.expect("next turn");
        assert_eq!(turn.turn_id, "turn-next");
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
        assert!(store.running_tasks.lock().unwrap().get("task-1").is_some());
    }

    #[test]
    fn next_turn_dispatch_keeps_queue_when_not_advancing() {
        let store = ChatStore::default();
        store
            .running_tasks
            .lock()
            .unwrap()
            .insert("task-1".to_string(), true);
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(crate::chat::state::PendingChatTurn {
                content: "queued".to_string(),
                composer: crate::chat::state::default_composer("task-1"),
                project_cwd: "C:\\repo".to_string(),
                attachments: Vec::new(),
                workflow: None,
                message: crate::chat::types::ChatMessage {
                    id: "u-queued".to_string(),
                    task_id: "task-1".to_string(),
                    role: "user".to_string(),
                    content: "queued".to_string(),
                    attachments: Vec::new(),
                    created_at: 1,
                },
                turn_id: "turn-queued".to_string(),
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
                guide_id: Some("guide-queued".to_string()),
            });

        let dispatch = NextTurnDispatch {
            next_turn: take_next_pending_turn(&store, "task-1", false),
        };

        assert!(dispatch.next_turn.is_none());
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
}
