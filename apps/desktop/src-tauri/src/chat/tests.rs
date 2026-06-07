#[cfg(test)]
mod agent_event_sink_tests {
    use rusqlite::Connection;
    use serde_json::{json, Value as JsonValue};

    use crate::agent_events::{AgentRuntimeEvent, AgentTurnContext};
    use crate::agent_timeline;
    use crate::agent_timeline::AgentTimelineEventInput;
    use crate::chat::commands::{
        agent_interaction_response_payload, composer_permission_update_payload,
    };
    use crate::chat::state::*;
    use crate::chat::timeline_sink::*;
    use crate::chat::types::*;
    use crate::provider::*;
    use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

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
    fn timeline_runtime_event_can_override_turn_and_timestamps_for_history() {
        let input = timeline_input_from_runtime_event(
            &turn_context(),
            &AgentRuntimeEvent::Timeline {
                event: json!({
                    "kind": "message",
                    "status": "success",
                    "title": "Assistant",
                    "summary": "历史回复",
                    "payload": {
                        "role": "assistant",
                        "content": "历史回复",
                        "history": true
                    },
                    "sourceId": "codex-history:thread-1:codex-turn-1:item-1",
                    "turnIdOverride": "codex-turn-1",
                    "createdAt": 10_000,
                    "updatedAt": 12_000
                }),
            },
        )
        .unwrap();

        assert_eq!(
            input.id,
            Some("task-1:codex-turn-1:codex-history:thread-1:codex-turn-1:item-1".to_string())
        );
        assert_eq!(input.turn_id, Some("codex-turn-1".to_string()));
        assert_eq!(input.created_at, Some(10_000));
        assert_eq!(input.updated_at, Some(12_000));
    }

    #[test]
    fn runner_error_duplicate_detection_uses_assistant_error_message() {
        let input = timeline_input_from_runtime_event(
            &turn_context(),
            &AgentRuntimeEvent::Timeline {
                event: json!({
                    "kind": "message",
                    "status": "error",
                    "title": "Assistant",
                    "payload": {
                        "role": "assistant",
                        "content": "API Error: 503 所有供应商已熔断，无可用渠道."
                    }
                }),
            },
        )
        .unwrap();
        let assistant_text = assistant_error_text(&input);

        assert_eq!(
            assistant_text.as_deref(),
            Some("API Error: 503 所有供应商已熔断，无可用渠道.")
        );
        let assistant_text = assistant_text.unwrap();
        assert!(normalize_timeline_text(
            "Claude Code returned an error result: API Error: 503 所有供应商已熔断，无可用渠道.",
        )
        .contains(&assistant_text));
        assert!(!normalize_timeline_text("无法启动 node 子进程").contains(&assistant_text));
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
    fn agent_interaction_response_payload_uses_runner_id_field() {
        assert_eq!(
            agent_interaction_response_payload(
                "ask-1".to_string(),
                "plan_approval".to_string(),
                json!({
                    "cancelled": false,
                    "answers": {
                        "approve-plan": {
                            "questionId": "approve-plan",
                            "value": "yes"
                        }
                    }
                }),
            ),
            json!({
                "type": "interaction_response",
                "id": "ask-1",
                "kind": "plan_approval",
                "result": {
                    "cancelled": false,
                    "answers": {
                        "approve-plan": {
                            "questionId": "approve-plan",
                            "value": "yes"
                        }
                    }
                }
            })
        );
    }

    #[test]
    fn composer_permission_update_payload_requires_existing_changed_permission() {
        let mut previous = default_composer("task-1");
        previous.permission = "ask".to_string();
        let mut next = previous.clone();

        assert_eq!(composer_permission_update_payload(None, &next), None);
        assert_eq!(composer_permission_update_payload(Some(&previous), &next), None);

        next.permission = "readonly".to_string();
        assert_eq!(
            composer_permission_update_payload(Some(&previous), &next),
            Some(json!({
                "type": "settings_update",
                "permission": "readonly",
            }))
        );

        next.permission = "invalid".to_string();
        assert_eq!(composer_permission_update_payload(Some(&previous), &next), None);
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

    #[test]
    fn active_backend_normalizes_unknown_values_to_claude() {
        assert_eq!(normalize_backend(BACKEND_CLAUDE), BACKEND_CLAUDE);
        assert_eq!(normalize_backend(BACKEND_CODEX), BACKEND_CODEX);
        assert_eq!(normalize_backend(""), BACKEND_CLAUDE);
        assert_eq!(normalize_backend("unknown"), BACKEND_CLAUDE);
    }

    #[test]
    fn codex_cli_version_parser_reads_codex_cli_output() {
        assert_eq!(
            parse_codex_cli_version("codex-cli 0.128.0"),
            Some((0, 128, 0))
        );
        assert_eq!(
            parse_codex_cli_version("codex-cli 0.130.0-alpha.5"),
            Some((0, 130, 0))
        );
        assert_eq!(
            parse_codex_cli_version("codex-cli 0.136.0-alpha.2"),
            Some((0, 136, 0))
        );
    }

    #[test]
    fn codex_cli_probe_skips_failed_windowsapps_candidate() {
        let candidates = vec![
            "C:\\Program Files\\WindowsApps\\OpenAI.Codex\\codex.exe".to_string(),
            "C:\\Users\\me\\AppData\\Local\\OpenAI\\Codex\\bin\\codex.exe".to_string(),
        ];
        let status = build_codex_app_server_probe_status_with(&candidates, |program, args| {
            if program.contains("WindowsApps") {
                return Err("Access is denied.".to_string());
            }
            match args {
                ["--version"] => Ok("codex-cli 0.130.0-alpha.5".to_string()),
                ["app-server", "--help"] => {
                    Ok("Usage: codex app-server [OPTIONS] [COMMAND]".to_string())
                }
                _ => Err("unexpected args".to_string()),
            }
        });

        assert_eq!(
            status.path.as_deref(),
            Some("C:\\Users\\me\\AppData\\Local\\OpenAI\\Codex\\bin\\codex.exe")
        );
        assert!(status.public.supports_required_protocol);
        assert_eq!(
            status.public.version.as_deref(),
            Some("codex-cli 0.130.0-alpha.5")
        );
    }

    #[test]
    fn codex_send_block_reason_mentions_cli_and_responses_support() {
        let reason = codex_send_block_reason(&CodexAppServerStatus {
            version: Some("codex-cli 0.125.0".to_string()),
            available: true,
            supports_required_protocol: false,
            failure_kind: Some("experimentalApiUnsupported".to_string()),
            issues: vec!["当前 codex CLI 版本过低，需要 0.128.0 或更新版本。".to_string()],
        })
        .unwrap();

        assert!(reason.contains("当前 codex CLI 版本过低"));
        assert!(reason.contains("0.128.0"));
        assert!(!reason.contains("OpenAI Responses API"));

        let reason = codex_send_block_reason(&CodexAppServerStatus {
            version: Some("codex-cli 0.128.0".to_string()),
            available: true,
            supports_required_protocol: false,
            failure_kind: Some("providerIncompatible".to_string()),
            issues: vec!["当前上游 provider 不兼容 Codex。".to_string()],
        })
        .unwrap();
        assert!(reason.contains("OpenAI Responses API"));

        assert!(codex_send_block_reason(&CodexAppServerStatus {
            version: Some("codex-cli 0.128.0".to_string()),
            available: true,
            supports_required_protocol: true,
            failure_kind: None,
            issues: Vec::new(),
        })
        .is_none());
    }

    #[test]
    fn composer_uses_active_backend_and_matching_default_model() {
        let composer = ChatComposerState {
            task_id: "stale-task".to_string(),
            backend: BACKEND_CLAUDE.to_string(),
            model: "claude-sonnet-4-6".to_string(),
            plan_mode: true,
            permission: "readonly".to_string(),
            codex_settings: Default::default(),
        };

        let normalized = normalize_composer_for_backend(composer, "task-1", BACKEND_CODEX);

        assert_eq!(normalized.task_id, "task-1");
        assert_eq!(normalized.backend, BACKEND_CODEX);
        assert_eq!(normalized.model, "gpt-5.5");
        assert!(normalized.plan_mode);
        assert_eq!(normalized.permission, "readonly");
    }

    #[test]
    fn composer_keeps_model_when_it_belongs_to_active_backend() {
        let composer = ChatComposerState {
            task_id: "task-1".to_string(),
            backend: BACKEND_CLAUDE.to_string(),
            model: "gpt-5.4-mini".to_string(),
            plan_mode: false,
            permission: "ask".to_string(),
            codex_settings: Default::default(),
        };

        let normalized = normalize_composer_for_backend(composer, "task-1", BACKEND_CODEX);

        assert_eq!(normalized.backend, BACKEND_CODEX);
        assert_eq!(normalized.model, "gpt-5.4-mini");
    }

    fn pending_turn(id: &str) -> PendingChatTurn {
        PendingChatTurn {
            content: format!("content {id}"),
            composer: default_composer("task-1"),
            project_cwd: "D:\\PROJECT\\workspace\\Lilia".to_string(),
            attachments: Vec::new(),
            message: ChatMessage {
                id: format!("u-{id}"),
                task_id: "task-1".to_string(),
                role: "user".to_string(),
                content: format!("content {id}"),
                attachments: Vec::new(),
                created_at: 100,
            },
            turn_id: format!("turn-{id}"),
            guide_id: None,
        }
    }

    #[test]
    fn clearing_pending_turns_removes_executable_queue_and_returns_guide_ids() {
        let store = ChatStore::default();
        {
            let mut pending = store.pending_turns.lock().unwrap();
            pending
                .entry("task-1".to_string())
                .or_default()
                .push_back(PendingChatTurn {
                    guide_id: Some("guide-1".to_string()),
                    ..pending_turn("queued")
                });
        }

        assert_eq!(
            clear_pending_turns(&store, "task-1"),
            vec!["guide-1".to_string()]
        );
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn prepare_running_turn_stop_for_interrupt_marks_interrupted_and_clears_queue() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-running".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-queued".to_string()),
                ..pending_turn("queued")
            });

        let prepared = prepare_running_turn_stop(&store, "task-1", true, false).unwrap();

        assert_eq!(prepared.guide_ids, vec!["guide-queued".to_string()]);
        assert!(prepared.child_handle.is_none());
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
        assert_eq!(
            store
                .interrupted_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|turn| turn.turn_id.as_str()),
            Some("turn-running")
        );
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn prepare_running_turn_stop_for_reset_marks_reset_without_interrupted_event() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-reset".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );

        let prepared = prepare_running_turn_stop(&store, "task-1", false, true).unwrap();

        assert!(prepared.guide_ids.is_empty());
        assert!(store
            .interrupted_turns
            .lock()
            .unwrap()
            .get("task-1")
            .is_none());
        assert_eq!(
            store
                .reset_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|turn| turn.turn_id.as_str()),
            Some("turn-reset")
        );
    }

    #[test]
    fn reset_marker_is_read_without_consuming_until_turn_finish() {
        let store = ChatStore::default();
        store.reset_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-reset".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );

        assert!(is_turn_marked_reset(
            &store,
            "task-1",
            "turn-reset",
            BACKEND_CLAUDE
        ));
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_some());

        let (interrupted, reset) =
            take_turn_stop_marks(&store, "task-1", "turn-reset", BACKEND_CLAUDE);
        assert!(!interrupted);
        assert!(reset);
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn finish_running_turn_handles_clears_handles_and_consumes_matching_marks() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-stop".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );
        store.interrupted_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-stop".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );

        let finished = finish_running_turn_handles(&store, "task-1", "turn-stop", BACKEND_CLAUDE);

        assert!(finished.interrupted);
        assert!(!finished.reset);
        assert!(store.running_turns.lock().unwrap().get("task-1").is_none());
        assert!(store
            .interrupted_turns
            .lock()
            .unwrap()
            .get("task-1")
            .is_none());
    }

    #[test]
    fn reset_finish_does_not_advance_pending_queue() {
        let store = ChatStore::default();
        store
            .running_tasks
            .lock()
            .unwrap()
            .insert("task-1".to_string(), true);
        store.reset_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-reset".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
            },
        );
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(pending_turn("queued"));

        let finished = finish_running_turn_handles(&store, "task-1", "turn-reset", BACKEND_CLAUDE);
        assert!(finished.reset);
        assert!(take_next_pending_turn(&store, "task-1", !finished.reset).is_none());

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

    #[test]
    fn prepare_running_turn_stop_without_running_turn_leaves_queue_intact() {
        let store = ChatStore::default();
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-still-queued".to_string()),
                ..pending_turn("queued")
            });

        assert!(prepare_running_turn_stop(&store, "task-1", true, true).is_none());
        assert_eq!(
            store
                .pending_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|queue| queue.len()),
            Some(1)
        );
        assert!(store
            .interrupted_turns
            .lock()
            .unwrap()
            .get("task-1")
            .is_none());
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn interrupted_exit_does_not_emit_runner_error() {
        assert!(!should_emit_runner_exit_error(
            true,
            true,
            "agent 进程被终止",
        ));
        assert!(should_emit_runner_exit_error(
            false,
            true,
            "agent 进程异常退出",
        ));
        assert!(!should_emit_runner_exit_error(false, true, "   "));
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
