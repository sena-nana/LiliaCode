use std::env;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::Value as JsonValue;
use tauri::{AppHandle, Emitter, Manager};

use crate::agent_events::{AgentEventHost, AgentRuntimeEvent, AgentTurnContext};
use crate::agent_extensions::TodoMirrorExtension;
use crate::chat::state::{
    finish_running_turn_handles, is_turn_marked_reset, load_persisted_resume_session_id,
    persist_and_emit_interrupted_timeline_event, session_key, set_guide_status_for_app,
    should_emit_runner_exit_error, take_next_pending_turn, ChatStore, RunningTurn,
};
use crate::chat::timeline_sink::{
    assistant_error_text, log_agent_event_effect, normalize_timeline_text,
    persist_and_emit_error_timeline_event, persist_and_emit_message_timeline_event,
    timeline_input_from_runtime_event, TimelineThrottle,
};
use crate::chat::types::{
    AgentInteractionRequestEvent, AskUserRequestEvent, ChatAttachment, ChatComposerState,
    DoneEvent, ToolConsentRequestEvent, TurnStartedEvent,
};
use crate::provider::{build_codex_app_server_probe_status, resolve_connection_for};
use crate::store::LiliaStore;
use crate::{plugins, BACKEND_CODEX};

// ---------- 子进程定位 ----------

/// 找到 agent-runner.mjs 的实际路径。
///
/// 开发态：cargo 编出来的二进制位于 `apps/desktop/src-tauri/target/{debug|release}/`，
/// 而脚本位于 `apps/desktop/agent-runner.mjs`，相对路径回退 3 层。
/// 按候选顺序找第一个存在的文件；找不到就返回最后一个候选让上层报错更直观。
pub(crate) fn locate_agent_runner(app: &AppHandle) -> PathBuf {
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

pub(crate) fn spawn_agent_turn(
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
                    AgentRuntimeEvent::InteractionRequest {
                        id,
                        kind,
                        backend,
                        payload,
                    } => {
                        let _ = app_handle.emit(
                            "chat:agent-interaction-request",
                            AgentInteractionRequestEvent {
                                task_id: task_id_for_thread.clone(),
                                turn_id: turn_id_for_thread.clone(),
                                backend: backend
                                    .clone()
                                    .unwrap_or_else(|| backend_for_thread.clone()),
                                request_id: id.clone(),
                                kind: kind.clone(),
                                payload: payload.clone(),
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

pub(crate) fn finish_agent_turn(
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
