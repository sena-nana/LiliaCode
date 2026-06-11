#[cfg(test)]
mod agent_event_sink_tests {
    use std::collections::BTreeMap;

    use rusqlite::Connection;
    use serde_json::{json, Value as JsonValue};

    use crate::agent_events::{AgentRuntimeEvent, AgentTurnContext};
    use crate::agent_timeline;
    use crate::agent_timeline::AgentTimelineEventInput;
    use crate::chat::commands::{
        agent_interaction_response_payload, attach_stdin_delivery,
        composer_permission_update_payload, control_event_attributes, plan_reset_session,
        record_nanobot_control_event, stage_nanobot_interrupt_turn, stage_nanobot_turn_stop,
        NanobotTurnStopKind, ResetSessionPlan,
    };
    use crate::chat::runner::build_runner_stdin_payload;
    use crate::chat::state::*;
    use crate::chat::timeline_sink::*;
    use crate::chat::types::*;
    use crate::provider::*;
    use crate::{BACKEND_CLAUDE, BACKEND_CODEX, RUNTIME_CHANNEL_BUILTIN, RUNTIME_CHANNEL_NANOBOT};

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
        assert_eq!(
            composer_permission_update_payload(Some(&previous), &next),
            None
        );

        next.permission = "readonly".to_string();
        assert_eq!(
            composer_permission_update_payload(Some(&previous), &next),
            Some(json!({
                "type": "settings_update",
                "permission": "readonly",
            }))
        );

        next.permission = "invalid".to_string();
        assert_eq!(
            composer_permission_update_payload(Some(&previous), &next),
            None
        );
    }

    #[test]
    fn runtime_control_event_queue_is_drained() {
        let store = ChatStore::default();
        record_runtime_control_event(
            &store,
            "task-1",
            "interaction_response",
            control_event_attributes([("requestId", "ask-1".to_string())]),
            None,
        );

        let drained = take_runtime_control_events(&store, "task-1");

        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].name, "interaction_response");
        assert_eq!(
            drained[0].attributes.get("requestId").map(String::as_str),
            Some("ask-1")
        );
        assert!(take_runtime_control_events(&store, "task-1").is_empty());
    }

    #[test]
    fn runtime_control_events_roundtrip_through_persistent_queue() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        let event = RuntimeControlEvent {
            name: "interaction_response".to_string(),
            attributes: control_event_attributes([
                ("requestId", "ask-1".to_string()),
                ("kind", "ask_user".to_string()),
            ]),
            payload: Some(json!({
                "type": "interaction_response",
                "id": "ask-1",
                "kind": "ask_user",
                "result": { "cancelled": false, "answers": {} },
            })),
        };

        persist_runtime_control_event(&conn, "task-1", &event).unwrap();
        let restored = list_runtime_control_events(&conn, "task-1").unwrap();

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].name, "interaction_response");
        assert_eq!(
            restored[0].attributes.get("requestId").map(String::as_str),
            Some("ask-1")
        );
        assert_eq!(restored[0].payload, event.payload);

        clear_runtime_control_events(&conn, "task-1").unwrap();
        assert!(list_runtime_control_events(&conn, "task-1")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn runtime_control_event_ids_clear_only_acked_rows() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        let first = RuntimeControlEvent {
            name: "interaction_response".to_string(),
            attributes: control_event_attributes([("requestId", "ask-1".to_string())]),
            payload: Some(json!({ "type": "interaction_response", "id": "ask-1" })),
        };
        let second = RuntimeControlEvent {
            name: "permission_update".to_string(),
            attributes: control_event_attributes([("permission", "readonly".to_string())]),
            payload: Some(json!({ "type": "settings_update", "permission": "readonly" })),
        };

        persist_runtime_control_event(&conn, "task-1", &first).unwrap();
        persist_runtime_control_event(&conn, "task-1", &second).unwrap();
        let deliveries = list_persisted_runtime_control_events(&conn, "task-1").unwrap();

        assert_eq!(deliveries.len(), 2);
        clear_runtime_control_event_ids(&conn, &[deliveries[0].persisted_id.unwrap()]).unwrap();
        let remaining = list_persisted_runtime_control_events(&conn, "task-1").unwrap();

        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].event.name, "permission_update");
    }

    #[test]
    fn clearing_runtime_state_also_clears_persisted_control_events() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        let running_turn = RunningTurn {
            turn_id: "turn-1".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };
        persist_runtime_state(
            &conn,
            &store,
            "task-1",
            &running_turn,
            "running",
            None,
            None,
        )
        .unwrap();
        persist_runtime_control_event(
            &conn,
            "task-1",
            &RuntimeControlEvent {
                name: "reset_requested".to_string(),
                attributes: control_event_attributes([("mode", "session_reset".to_string())]),
                payload: None,
            },
        )
        .unwrap();

        clear_runtime_state(&conn, "task-1").unwrap();

        assert!(load_any_runtime_state(&conn, "task-1").unwrap().is_none());
        assert!(list_runtime_control_events(&conn, "task-1")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn pending_turns_roundtrip_through_persistent_queue() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        let turn = PendingChatTurn {
            guide_id: Some("guide-1".to_string()),
            ..pending_turn("queued")
        };

        persist_pending_turn(&conn, "task-1", &turn).unwrap();

        assert_eq!(count_pending_turns(&conn, "task-1").unwrap(), 1);
        let restored = take_next_persisted_pending_turn(&conn, "task-1")
            .unwrap()
            .expect("persisted pending turn");
        assert_eq!(restored.content, "content queued");
        assert_eq!(restored.turn_id, turn.turn_id);
        assert_eq!(restored.runtime_channel, RUNTIME_CHANNEL_BUILTIN);
        assert_eq!(restored.guide_id.as_deref(), Some("guide-1"));
        assert_eq!(count_pending_turns(&conn, "task-1").unwrap(), 0);
    }

    #[test]
    fn clear_persisted_pending_turns_returns_guide_ids() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        persist_pending_turn(
            &conn,
            "task-1",
            &PendingChatTurn {
                guide_id: Some("guide-1".to_string()),
                ..pending_turn("queued-1")
            },
        )
        .unwrap();
        persist_pending_turn(
            &conn,
            "task-1",
            &PendingChatTurn {
                guide_id: None,
                ..pending_turn("queued-2")
            },
        )
        .unwrap();

        let guide_ids = clear_persisted_pending_turns(&conn, "task-1").unwrap();

        assert_eq!(guide_ids, vec!["guide-1".to_string()]);
        assert_eq!(count_pending_turns(&conn, "task-1").unwrap(), 0);
    }

    #[test]
    fn stdin_delivery_attributes_capture_forwarded_and_error_states() {
        let mut forwarded = BTreeMap::new();
        attach_stdin_delivery(&mut forwarded, &Ok(true));
        assert_eq!(
            forwarded.get("stdinForwarded").map(String::as_str),
            Some("true")
        );
        assert!(forwarded.get("stdinError").is_none());

        let mut missing = BTreeMap::new();
        attach_stdin_delivery(&mut missing, &Ok(false));
        assert_eq!(
            missing.get("stdinForwarded").map(String::as_str),
            Some("false")
        );
        assert!(missing.get("stdinError").is_none());

        let mut failed = BTreeMap::new();
        attach_stdin_delivery(&mut failed, &Err("broken pipe".to_string()));
        assert_eq!(
            failed.get("stdinForwarded").map(String::as_str),
            Some("false")
        );
        assert_eq!(
            failed.get("stdinError").map(String::as_str),
            Some("broken pipe")
        );
    }

    #[test]
    fn record_nanobot_control_event_only_records_nanobot_running_turn() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "builtin-task".to_string(),
            RunningTurn {
                turn_id: "turn-builtin".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
            },
        );
        store.running_turns.lock().unwrap().insert(
            "nanobot-task".to_string(),
            RunningTurn {
                turn_id: "turn-nanobot".to_string(),
                backend: BACKEND_CODEX.to_string(),
                runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
            },
        );

        record_nanobot_control_event(
            &store,
            "builtin-task",
            "permission_update",
            control_event_attributes([("permission", "full".to_string())]),
            None,
        );
        record_nanobot_control_event(
            &store,
            "nanobot-task",
            "permission_update",
            control_event_attributes([("permission", "full".to_string())]),
            Some(json!({ "type": "settings_update", "permission": "full" })),
        );

        assert!(take_runtime_control_events(&store, "builtin-task").is_empty());
        let drained = take_runtime_control_events(&store, "nanobot-task");
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].name, "permission_update");
        assert_eq!(
            drained[0].attributes.get("turnId").map(String::as_str),
            Some("turn-nanobot")
        );
        assert_eq!(
            drained[0].attributes.get("backend").map(String::as_str),
            Some(BACKEND_CODEX)
        );
        assert_eq!(
            drained[0].payload,
            Some(json!({ "type": "settings_update", "permission": "full" }))
        );
    }

    #[test]
    fn empty_codex_workflows_do_not_persist_user_message() {
        let workflows = vec![
            ChatWorkflow::CodexReview {
                target: CodexReviewTarget::UncommittedChanges,
                instructions: None,
                delivery: Some("inline".to_string()),
            },
            ChatWorkflow::CodexFixSuggestion {
                target: CodexReviewTarget::UncommittedChanges,
                instructions: None,
                mode: Some("suggest".to_string()),
            },
            ChatWorkflow::CodexBatchApply {
                source_turn_id: "turn-source".to_string(),
                source_kind: "fix_suggestion".to_string(),
                source_summary: "建议修复权限边界".to_string(),
                instructions: None,
            },
            ChatWorkflow::CodexGoal {
                action: "set".to_string(),
                objective: Some("完成 Thread Goal 接入".to_string()),
                status: Some("active".to_string()),
                token_budget: None,
            },
            ChatWorkflow::CodexCompact,
            ChatWorkflow::CodexBackgroundTerminalsClean,
            ChatWorkflow::CodexMemoryMode {
                mode: "enabled".to_string(),
            },
            ChatWorkflow::CodexMemoryReset,
            ChatWorkflow::CodexThreadFork {
                exclude_turns: Some(true),
            },
            ChatWorkflow::CodexConfigDiagnostics {
                include_layers: Some(true),
            },
        ];

        for workflow in workflows {
            let workflow = Some(workflow);
            assert!(!should_persist_user_message("", &workflow));
            assert!(!should_persist_user_message("  ", &workflow));
            assert!(should_persist_user_message("补充说明", &workflow));
        }
        assert!(should_persist_user_message("", &None));
    }

    #[test]
    fn chat_workflow_serializes_struct_variant_fields_as_camel_case() {
        let goal = serde_json::from_value::<ChatWorkflow>(json!({
            "type": "codex_goal",
            "action": "set",
            "objective": "完成接口接入",
            "status": "active",
            "tokenBudget": 100,
        }))
        .unwrap();
        let ChatWorkflow::CodexGoal { token_budget, .. } = &goal else {
            panic!("unexpected workflow: {goal:?}");
        };
        assert_eq!(*token_budget, Some(100));
        let goal_json = serde_json::to_value(&goal).unwrap();
        assert_eq!(goal_json["tokenBudget"], json!(100));
        assert!(goal_json.get("token_budget").is_none());

        let fix = serde_json::from_value::<ChatWorkflow>(json!({
            "type": "codex_fix_suggestion",
            "target": { "type": "baseBranch", "branch": "main" },
            "instructions": "只给建议",
            "mode": "suggest",
        }))
        .unwrap();
        let ChatWorkflow::CodexFixSuggestion { mode, .. } = &fix else {
            panic!("unexpected workflow: {fix:?}");
        };
        assert_eq!(mode.as_deref(), Some("suggest"));
        let fix_json = serde_json::to_value(&fix).unwrap();
        assert_eq!(fix_json["type"], json!("codex_fix_suggestion"));
        assert_eq!(fix_json["target"]["branch"], json!("main"));

        let batch_apply = serde_json::from_value::<ChatWorkflow>(json!({
            "type": "codex_batch_apply",
            "sourceTurnId": "turn-source",
            "sourceKind": "fix_suggestion",
            "sourceSummary": "建议修复权限边界",
            "instructions": "应用最小改动",
        }))
        .unwrap();
        let ChatWorkflow::CodexBatchApply {
            source_turn_id,
            source_kind,
            source_summary,
            instructions,
        } = &batch_apply
        else {
            panic!("unexpected workflow: {batch_apply:?}");
        };
        assert_eq!(source_turn_id, "turn-source");
        assert_eq!(source_kind, "fix_suggestion");
        assert_eq!(source_summary, "建议修复权限边界");
        assert_eq!(instructions.as_deref(), Some("应用最小改动"));
        let batch_apply_json = serde_json::to_value(&batch_apply).unwrap();
        assert_eq!(batch_apply_json["sourceTurnId"], json!("turn-source"));
        assert_eq!(batch_apply_json["sourceKind"], json!("fix_suggestion"));
        assert_eq!(batch_apply_json["sourceSummary"], json!("建议修复权限边界"));
        assert!(batch_apply_json.get("source_turn_id").is_none());

        let fork = serde_json::from_value::<ChatWorkflow>(json!({
            "type": "codex_thread_fork",
            "excludeTurns": false,
        }))
        .unwrap();
        let ChatWorkflow::CodexThreadFork { exclude_turns } = &fork else {
            panic!("unexpected workflow: {fork:?}");
        };
        assert_eq!(*exclude_turns, Some(false));
        let fork_json = serde_json::to_value(&fork).unwrap();
        assert_eq!(fork_json["excludeTurns"], json!(false));
        assert!(fork_json.get("exclude_turns").is_none());

        let diagnostics = serde_json::from_value::<ChatWorkflow>(json!({
            "type": "codex_config_diagnostics",
            "includeLayers": false,
        }))
        .unwrap();
        let ChatWorkflow::CodexConfigDiagnostics { include_layers } = &diagnostics else {
            panic!("unexpected workflow: {diagnostics:?}");
        };
        assert_eq!(*include_layers, Some(false));
        let diagnostics_json = serde_json::to_value(&diagnostics).unwrap();
        assert_eq!(diagnostics_json["includeLayers"], json!(false));
        assert!(diagnostics_json.get("include_layers").is_none());
    }

    #[test]
    fn runner_stdin_payload_keeps_ui_to_runner_contract() {
        let composer = ChatComposerState {
            task_id: "task-1".to_string(),
            backend: BACKEND_CODEX.to_string(),
            model: "gpt-5.5".to_string(),
            plan_mode: true,
            permission: "ask".to_string(),
            codex_settings: CodexComposerSettings::default(),
        };
        let workflow = ChatWorkflow::CodexFixSuggestion {
            target: CodexReviewTarget::UncommittedChanges,
            instructions: Some("重点看全链路".to_string()),
            mode: Some("suggest".to_string()),
        };

        let payload = build_runner_stdin_payload(
            BACKEND_CODEX,
            "C:\\Files\\workspace\\Lilia",
            "重点看全链路",
            &[],
            Some(&workflow),
            &composer,
            Some("thread-1"),
            &json!({ "mcpServers": [], "warnings": [] }),
        );

        assert_eq!(payload["backend"], json!("codex"));
        assert_eq!(payload["cwd"], json!("C:\\Files\\workspace\\Lilia"));
        assert_eq!(payload["prompt"], json!("重点看全链路"));
        assert_eq!(payload["resumeSessionId"], json!("thread-1"));
        assert_eq!(payload["planMode"], json!(true));
        assert_eq!(payload["permission"], json!("ask"));
        assert_eq!(payload["workflow"]["type"], json!("codex_fix_suggestion"));
        assert_eq!(payload["workflow"]["mode"], json!("suggest"));
        assert_eq!(payload["workflow"]["instructions"], json!("重点看全链路"));
        assert_eq!(payload["extensions"]["mcpServers"], json!([]));
        assert_eq!(payload["model"], json!("gpt-5.5"));
    }

    #[test]
    fn runner_stdin_payload_keeps_codex_batch_apply_workflow() {
        let composer = ChatComposerState {
            task_id: "task-1".to_string(),
            backend: BACKEND_CODEX.to_string(),
            model: "gpt-5.5".to_string(),
            plan_mode: false,
            permission: "ask".to_string(),
            codex_settings: CodexComposerSettings::default(),
        };
        let workflow = ChatWorkflow::CodexBatchApply {
            source_turn_id: "turn-source".to_string(),
            source_kind: "fix_suggestion".to_string(),
            source_summary: "建议修复权限边界".to_string(),
            instructions: None,
        };

        let payload = build_runner_stdin_payload(
            BACKEND_CODEX,
            "C:\\Files\\workspace\\Lilia",
            "",
            &[],
            Some(&workflow),
            &composer,
            Some("thread-1"),
            &json!({ "mcpServers": [], "warnings": [] }),
        );

        assert_eq!(payload["backend"], json!("codex"));
        assert_eq!(payload["prompt"], json!(""));
        assert_eq!(payload["workflow"]["type"], json!("codex_batch_apply"));
        assert_eq!(payload["workflow"]["sourceTurnId"], json!("turn-source"));
        assert_eq!(payload["workflow"]["sourceKind"], json!("fix_suggestion"));
        assert_eq!(
            payload["workflow"]["sourceSummary"],
            json!("建议修复权限边界")
        );
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
            workflow: None,
            message: ChatMessage {
                id: format!("u-{id}"),
                task_id: "task-1".to_string(),
                role: "user".to_string(),
                content: format!("content {id}"),
                attachments: Vec::new(),
                created_at: 100,
            },
            turn_id: format!("turn-{id}"),
            runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
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
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
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
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
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
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
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
        store
            .running_process_sessions
            .lock()
            .unwrap()
            .insert("task-1".to_string(), "proc-1".to_string());
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-stop".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
            },
        );
        store.interrupted_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-stop".to_string(),
                backend: BACKEND_CLAUDE.to_string(),
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
            },
        );

        let finished = finish_running_turn_handles(&store, "task-1", "turn-stop", BACKEND_CLAUDE);

        assert!(finished.interrupted);
        assert!(!finished.reset);
        assert!(store
            .running_process_sessions
            .lock()
            .unwrap()
            .get("task-1")
            .is_none());
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
                runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
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
    fn interrupted_finish_does_not_advance_pending_queue() {
        let store = ChatStore::default();
        store
            .running_tasks
            .lock()
            .unwrap()
            .insert("task-1".to_string(), true);
        store.interrupted_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-interrupt".to_string(),
                backend: BACKEND_CODEX.to_string(),
                runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
            },
        );
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(pending_turn("queued"));

        let finished =
            finish_running_turn_handles(&store, "task-1", "turn-interrupt", BACKEND_CODEX);
        assert!(finished.interrupted);
        assert!(!finished.reset);
        assert!(
            take_next_pending_turn(&store, "task-1", !finished.interrupted && !finished.reset)
                .is_none()
        );

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
    fn pending_reset_cleanup_is_consumed_once() {
        let store = ChatStore::default();

        mark_pending_reset_cleanup(&store, "task-1");

        assert!(take_pending_reset_cleanup(&store, "task-1"));
        assert!(!take_pending_reset_cleanup(&store, "task-1"));
    }

    #[test]
    fn runtime_finalization_persists_and_consumes_rollback_once() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);
        let rollback = ChatRollbackResult {
            rolled_back: true,
            restored_content: "restore".to_string(),
            restored_attachments: vec![ChatAttachment {
                id: "att-1".to_string(),
                kind: "file".to_string(),
                path: "C:\\Files\\workspace\\Lilia\\README.md".to_string(),
                name: "README.md".to_string(),
                size: Some(12),
                exists: true,
                mime: Some("text/markdown".to_string()),
                directory: None,
            }],
            removed_event_ids: vec!["evt-1".to_string()],
        };

        persist_pending_rollback(&conn, "task-1", &rollback).unwrap();
        let stored = take_persisted_pending_rollback(&conn, "task-1")
            .unwrap()
            .expect("pending rollback");

        assert_eq!(stored.restored_content, rollback.restored_content);
        assert_eq!(stored.restored_attachments.len(), 1);
        assert_eq!(stored.restored_attachments[0].id, "att-1");
        assert_eq!(
            stored.restored_attachments[0].path,
            rollback.restored_attachments[0].path
        );
        assert_eq!(
            stored.restored_attachments[0].mime.as_deref(),
            Some("text/markdown")
        );
        assert_eq!(stored.removed_event_ids, rollback.removed_event_ids);
        assert!(take_persisted_pending_rollback(&conn, "task-1")
            .unwrap()
            .is_none());
    }

    #[test]
    fn runtime_finalization_persists_and_consumes_reset_cleanup_once() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);

        persist_pending_reset_cleanup(&conn, "task-1").unwrap();

        assert!(take_persisted_pending_reset_cleanup(&conn, "task-1").unwrap());
        assert!(!take_persisted_pending_reset_cleanup(&conn, "task-1").unwrap());
    }

    #[test]
    fn runtime_state_clear_keeps_pending_finalization_until_explicit_cleanup() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);
        let running_turn = RunningTurn {
            turn_id: "turn-reset".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };
        let rollback = ChatRollbackResult {
            rolled_back: true,
            restored_content: "restore after restart".to_string(),
            restored_attachments: Vec::new(),
            removed_event_ids: vec!["evt-1".to_string()],
        };
        persist_runtime_state(
            &conn,
            &store,
            "task-1",
            &running_turn,
            "reset_pending_finish",
            Some("process-1"),
            None,
        )
        .unwrap();
        persist_pending_rollback(&conn, "task-1", &rollback).unwrap();
        persist_pending_reset_cleanup(&conn, "task-1").unwrap();

        clear_runtime_state(&conn, "task-1").unwrap();

        assert!(load_runtime_state(&conn, &store, "task-1")
            .unwrap()
            .is_none());
        let stored = take_persisted_pending_rollback(&conn, "task-1")
            .unwrap()
            .expect("pending rollback");
        assert_eq!(stored.restored_content, rollback.restored_content);
        assert!(take_persisted_pending_reset_cleanup(&conn, "task-1").unwrap());

        persist_pending_rollback(&conn, "task-1", &rollback).unwrap();
        persist_pending_reset_cleanup(&conn, "task-1").unwrap();
        clear_runtime_finalization(&conn, "task-1").unwrap();

        assert!(take_persisted_pending_rollback(&conn, "task-1")
            .unwrap()
            .is_none());
        assert!(!take_persisted_pending_reset_cleanup(&conn, "task-1").unwrap());
    }

    #[test]
    fn nanobot_turn_stop_stages_interrupt_and_returns_guide_ids() {
        let store = ChatStore::default();
        let running_turn = RunningTurn {
            turn_id: "turn-1".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };
        store
            .running_turns
            .lock()
            .unwrap()
            .insert("task-1".to_string(), running_turn.clone());
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-1".to_string()),
                ..pending_turn("queued")
            });

        let cleared = stage_nanobot_turn_stop(
            &store,
            "task-1",
            &running_turn,
            NanobotTurnStopKind::Interrupt,
        );

        assert_eq!(cleared, vec!["guide-1".to_string()]);
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
        assert_eq!(
            store
                .interrupted_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|turn| turn.turn_id.as_str()),
            Some("turn-1")
        );
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn nanobot_turn_stop_stages_reset_without_interrupt_mark() {
        let store = ChatStore::default();
        let running_turn = RunningTurn {
            turn_id: "turn-reset".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-reset".to_string()),
                ..pending_turn("queued")
            });

        let cleared =
            stage_nanobot_turn_stop(&store, "task-1", &running_turn, NanobotTurnStopKind::Reset);

        assert_eq!(cleared, vec!["guide-reset".to_string()]);
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
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
    fn nanobot_interrupt_turn_reset_stashes_pending_rollback() {
        let store = ChatStore::default();
        let running_turn = RunningTurn {
            turn_id: "turn-reset".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };
        let rollback = ChatRollbackResult {
            rolled_back: true,
            restored_content: "restore".to_string(),
            restored_attachments: Vec::new(),
            removed_event_ids: vec!["evt-1".to_string()],
        };

        let (_cleared, result) = stage_nanobot_interrupt_turn(
            &store,
            "task-1",
            &running_turn,
            NanobotTurnStopKind::Reset,
            Some(rollback.clone()),
        );

        assert!(!result.rolled_back);
        assert!(result.restored_content.is_empty());
        assert!(result.restored_attachments.is_empty());
        assert!(result.removed_event_ids.is_empty());
        let stored = take_pending_rollback(&store, "task-1").expect("pending rollback");
        assert!(stored.rolled_back);
        assert_eq!(stored.restored_content, rollback.restored_content);
        assert_eq!(
            stored.restored_attachments.len(),
            rollback.restored_attachments.len()
        );
        assert_eq!(stored.removed_event_ids, rollback.removed_event_ids);
    }

    #[test]
    fn nanobot_interrupt_turn_interrupt_does_not_stash_rollback() {
        let store = ChatStore::default();
        let running_turn = RunningTurn {
            turn_id: "turn-interrupt".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };

        let (_cleared, result) = stage_nanobot_interrupt_turn(
            &store,
            "task-1",
            &running_turn,
            NanobotTurnStopKind::Interrupt,
            Some(ChatRollbackResult {
                rolled_back: true,
                restored_content: "ignored".to_string(),
                restored_attachments: Vec::new(),
                removed_event_ids: vec!["evt-ignored".to_string()],
            }),
        );

        assert!(!result.rolled_back);
        assert!(result.restored_content.is_empty());
        assert!(result.restored_attachments.is_empty());
        assert!(result.removed_event_ids.is_empty());
        assert!(take_pending_rollback(&store, "task-1").is_none());
    }

    #[test]
    fn reset_session_plan_without_running_turn_clears_queue_and_requests_immediate_cleanup() {
        let store = ChatStore::default();
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-free".to_string()),
                ..pending_turn("queued")
            });

        let plan = plan_reset_session(&store, "task-1", None);

        assert_eq!(
            plan,
            ResetSessionPlan {
                cleared_guide_ids: vec!["guide-free".to_string()],
                stopped_running: false,
                immediate_cleanup: true,
            }
        );
        assert!(store.pending_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn reset_session_plan_for_nanobot_running_turn_stages_reset() {
        let store = ChatStore::default();
        let running_turn = RunningTurn {
            turn_id: "turn-reset".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-nanobot".to_string()),
                ..pending_turn("queued")
            });

        let plan = plan_reset_session(&store, "task-1", Some(&running_turn));

        assert_eq!(
            plan,
            ResetSessionPlan {
                cleared_guide_ids: vec!["guide-nanobot".to_string()],
                stopped_running: true,
                immediate_cleanup: false,
            }
        );
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
    fn reset_session_plan_for_builtin_running_turn_defers_to_stop_running_turn() {
        let store = ChatStore::default();
        let running_turn = RunningTurn {
            turn_id: "turn-builtin".to_string(),
            backend: BACKEND_CLAUDE.to_string(),
            runtime_channel: RUNTIME_CHANNEL_BUILTIN.to_string(),
        };
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-builtin".to_string()),
                ..pending_turn("queued")
            });

        let plan = plan_reset_session(&store, "task-1", Some(&running_turn));

        assert_eq!(plan, ResetSessionPlan::default());
        assert_eq!(
            store
                .pending_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|queue| queue.len()),
            Some(1)
        );
        assert!(store.reset_turns.lock().unwrap().get("task-1").is_none());
    }

    #[test]
    fn runtime_snapshot_reports_idle_state() {
        let store = ChatStore::default();

        let snapshot = chat_runtime_snapshot(&store, "task-1");

        assert_eq!(snapshot.task_id, "task-1");
        assert_eq!(snapshot.phase, "idle");
        assert!(snapshot.runtime_channel.is_none());
        assert!(snapshot.backend.is_none());
        assert!(snapshot.turn_id.is_none());
        assert_eq!(snapshot.queued_count, 0);
        assert_eq!(snapshot.pending_control_count, 0);
        assert!(!snapshot.pending_rollback);
        assert!(!snapshot.pending_reset_cleanup);
    }

    #[test]
    fn runtime_snapshot_reports_running_and_queued_state() {
        let store = ChatStore::default();
        store.running_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-1".to_string(),
                backend: BACKEND_CODEX.to_string(),
                runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
            },
        );
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(PendingChatTurn {
                guide_id: Some("guide-1".to_string()),
                ..pending_turn("queued")
            });
        record_runtime_control_event(
            &store,
            "task-1",
            "interrupt_requested",
            control_event_attributes([("mode", "user_interrupt".to_string())]),
            None,
        );

        let snapshot = chat_runtime_snapshot(&store, "task-1");

        assert_eq!(snapshot.phase, "running_and_queued");
        assert_eq!(
            snapshot.runtime_channel.as_deref(),
            Some(RUNTIME_CHANNEL_NANOBOT)
        );
        assert_eq!(snapshot.backend.as_deref(), Some(BACKEND_CODEX));
        assert_eq!(snapshot.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(snapshot.queued_count, 1);
        assert_eq!(snapshot.pending_control_count, 1);
    }

    #[test]
    fn runtime_snapshot_prefers_reset_pending_finish_phase() {
        let store = ChatStore::default();
        store.reset_turns.lock().unwrap().insert(
            "task-1".to_string(),
            RunningTurn {
                turn_id: "turn-reset".to_string(),
                backend: BACKEND_CODEX.to_string(),
                runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
            },
        );
        mark_pending_reset_cleanup(&store, "task-1");
        set_pending_rollback(
            &store,
            "task-1",
            ChatRollbackResult {
                rolled_back: true,
                restored_content: "restore".to_string(),
                restored_attachments: Vec::new(),
                removed_event_ids: vec!["evt-1".to_string()],
            },
        );

        let snapshot = chat_runtime_snapshot(&store, "task-1");

        assert_eq!(snapshot.phase, "reset_pending_finish");
        assert!(snapshot.pending_rollback);
        assert!(snapshot.pending_reset_cleanup);
    }

    #[test]
    fn runtime_snapshot_falls_back_to_current_epoch_persisted_state() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        let running_turn = RunningTurn {
            turn_id: "turn-persisted".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };

        persist_runtime_state(
            &conn,
            &store,
            "task-1",
            &running_turn,
            "running",
            None,
            None,
        )
        .unwrap();
        store
            .pending_turns
            .lock()
            .unwrap()
            .entry("task-1".to_string())
            .or_default()
            .push_back(pending_turn("queued"));
        let snapshot = chat_runtime_snapshot_with_persisted(Some(&conn), &store, "task-1");

        assert_eq!(snapshot.phase, "running_and_queued");
        assert_eq!(
            snapshot.runtime_channel.as_deref(),
            Some(RUNTIME_CHANNEL_NANOBOT)
        );
        assert_eq!(snapshot.backend.as_deref(), Some(BACKEND_CODEX));
        assert_eq!(snapshot.turn_id.as_deref(), Some("turn-persisted"));
    }

    #[test]
    fn runtime_snapshot_counts_persisted_control_events() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        persist_runtime_control_event(
            &conn,
            "task-1",
            &RuntimeControlEvent {
                name: "permission_update".to_string(),
                attributes: control_event_attributes([("permission", "readonly".to_string())]),
                payload: Some(json!({ "type": "settings_update", "permission": "readonly" })),
            },
        )
        .unwrap();
        record_runtime_control_event(
            &store,
            "task-1",
            "interrupt_requested",
            control_event_attributes([("mode", "user_interrupt".to_string())]),
            None,
        );

        let snapshot = chat_runtime_snapshot_with_persisted(Some(&conn), &store, "task-1");

        assert_eq!(snapshot.pending_control_count, 2);
    }

    #[test]
    fn persisted_runtime_state_reports_stale_epoch_as_abandoned() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, runtime_epoch, updated_at)
               VALUES ('task-1', 'turn-old', 'codex', 'nanobot', 'running', 'stale-epoch', 1)"#,
            [],
        )
        .unwrap();

        let snapshot = chat_runtime_snapshot_with_persisted(Some(&conn), &store, "task-1");

        assert_eq!(snapshot.phase, "abandoned");
        assert_eq!(
            snapshot.runtime_channel.as_deref(),
            Some(RUNTIME_CHANNEL_NANOBOT)
        );
        assert_eq!(snapshot.backend.as_deref(), Some(BACKEND_CODEX));
        assert_eq!(snapshot.turn_id.as_deref(), Some("turn-old"));
    }

    #[test]
    fn pending_turn_recovery_clears_abandoned_runtime_state() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, runtime_epoch, updated_at)
               VALUES ('task-1', 'turn-old', 'codex', 'nanobot', 'running', 'stale-epoch', 1)"#,
            [],
        )
        .unwrap();

        assert!(prepare_pending_turn_recovery(&conn, &store, "task-1").unwrap());
        assert!(load_any_runtime_state(&conn, "task-1").unwrap().is_none());
    }

    #[test]
    fn recoverable_pending_turn_drains_queue_after_clearing_abandoned_runtime_state() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, runtime_epoch, updated_at)
               VALUES ('task-1', 'turn-old', 'codex', 'nanobot', 'running', 'stale-epoch', 1)"#,
            [],
        )
        .unwrap();
        persist_pending_turn(
            &conn,
            "task-1",
            &PendingChatTurn {
                runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
                ..pending_turn("recover")
            },
        )
        .unwrap();

        let turn = take_next_recoverable_pending_turn(&conn, &store, "task-1")
            .unwrap()
            .expect("recoverable turn");

        assert_eq!(turn.turn_id, "turn-recover");
        assert_eq!(turn.runtime_channel, RUNTIME_CHANNEL_NANOBOT);
        assert!(load_any_runtime_state(&conn, "task-1").unwrap().is_none());
        assert_eq!(count_pending_turns(&conn, "task-1").unwrap(), 0);
    }

    #[test]
    fn pending_turn_recovery_keeps_live_runtime_state_blocking() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        persist_runtime_state(
            &conn,
            &store,
            "task-1",
            &RunningTurn {
                turn_id: "turn-live".to_string(),
                backend: BACKEND_CODEX.to_string(),
                runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
            },
            "running",
            None,
            None,
        )
        .unwrap();

        assert!(!prepare_pending_turn_recovery(&conn, &store, "task-1").unwrap());
        assert!(load_any_runtime_state(&conn, "task-1").unwrap().is_some());
    }

    #[test]
    fn runtime_state_phase_update_preserves_live_process_session() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);
        let running_turn = RunningTurn {
            turn_id: "turn-live".to_string(),
            backend: BACKEND_CODEX.to_string(),
            runtime_channel: RUNTIME_CHANNEL_NANOBOT.to_string(),
        };

        persist_runtime_state(
            &conn,
            &store,
            "task-1",
            &running_turn,
            "running",
            Some("process-live"),
            None,
        )
        .unwrap();
        persist_runtime_state(
            &conn,
            &store,
            "task-1",
            &running_turn,
            "reset_pending_finish",
            None,
            None,
        )
        .unwrap();

        let persisted = load_runtime_state(&conn, &store, "task-1")
            .unwrap()
            .expect("runtime state");
        assert_eq!(persisted.phase, "reset_pending_finish");
        assert_eq!(
            persisted.process_session_id.as_deref(),
            Some("process-live")
        );
    }

    #[test]
    fn queued_user_message_recovery_updates_existing_timeline_row() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        let message = ChatMessage {
            id: "msg-recover".to_string(),
            task_id: "task-1".to_string(),
            role: "user".to_string(),
            content: "resume queued".to_string(),
            attachments: Vec::new(),
            created_at: 100,
        };
        let queued_input = AgentTimelineEventInput {
            id: Some(message.id.clone()),
            task_id: message.task_id.clone(),
            turn_id: Some("turn-recover".to_string()),
            backend: BACKEND_CODEX.to_string(),
            kind: "message".to_string(),
            status: "pending".to_string(),
            title: "用户输入".to_string(),
            summary: Some(message.content.clone()),
            payload: json!({
                "role": message.role,
                "content": message.content,
                "attachments": message.attachments,
                "queued": true,
            }),
            created_at: Some(message.created_at as i64),
            updated_at: Some(101),
        };
        agent_timeline::insert(&conn, queued_input).unwrap();
        let recovered_input = AgentTimelineEventInput {
            id: Some(message.id.clone()),
            task_id: message.task_id.clone(),
            turn_id: Some("turn-recover".to_string()),
            backend: BACKEND_CODEX.to_string(),
            kind: "message".to_string(),
            status: "success".to_string(),
            title: "用户输入".to_string(),
            summary: Some(message.content.clone()),
            payload: json!({
                "role": message.role,
                "content": message.content,
                "attachments": message.attachments,
                "queued": false,
            }),
            created_at: Some(message.created_at as i64),
            updated_at: Some(102),
        };

        let recovered = agent_timeline::insert(&conn, recovered_input).unwrap();

        assert_eq!(recovered.id, "msg-recover");
        assert_eq!(recovered.status, "success");
        assert_eq!(recovered.payload["queued"], json!(false));
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_timeline_events WHERE task_id = 'task-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn persisted_runtime_state_with_live_process_session_survives_epoch_change() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();

        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".into());
        let child = match std::process::Command::new(python)
            .args([
                "-c",
                "import sys,time; print('{\"kind\":\"timeline\"}'); sys.stdout.flush(); time.sleep(0.2)",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                eprintln!("skip live process session snapshot test: {err}");
                return;
            }
        };
        let process_session_id =
            crate::chat::runner::start_test_process_session(child, &json!({"boot": true})).unwrap();

        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, process_session_id, runtime_epoch, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            rusqlite::params![
                "task-1",
                "turn-live",
                "codex",
                "nanobot",
                "running",
                process_session_id.clone(),
                "stale-epoch",
                1_i64
            ],
        )
        .unwrap();

        let snapshot = chat_runtime_snapshot_with_persisted(Some(&conn), &store, "task-1");

        assert_eq!(snapshot.phase, "running");
        assert_eq!(snapshot.turn_id.as_deref(), Some("turn-live"));
        crate::chat::runner::remove_test_process_session(&process_session_id);
    }

    #[test]
    fn restore_active_runtime_sessions_rehydrates_running_turns_from_live_process_session() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();

        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".into());
        let child = match std::process::Command::new(python)
            .args(["-c", "import time; time.sleep(0.2)"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                eprintln!("skip runtime restore test: {err}");
                return;
            }
        };
        let process_session_id =
            crate::chat::runner::start_test_process_session(child, &json!({"boot": true})).unwrap();

        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, process_session_id, runtime_epoch, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            rusqlite::params![
                "task-1",
                "turn-restore",
                "codex",
                "nanobot",
                "running",
                process_session_id.clone(),
                "stale-epoch",
                1_i64
            ],
        )
        .unwrap();

        restore_active_runtime_sessions(&conn, &store);

        assert!(store
            .running_tasks
            .lock()
            .unwrap()
            .get("task-1")
            .copied()
            .unwrap_or(false));
        assert_eq!(
            store
                .running_turns
                .lock()
                .unwrap()
                .get("task-1")
                .map(|turn| turn.turn_id.as_str()),
            Some("turn-restore")
        );
        assert_eq!(
            store
                .running_process_sessions
                .lock()
                .unwrap()
                .get("task-1")
                .map(String::as_str),
            Some(process_session_id.as_str())
        );
        crate::chat::runner::remove_test_process_session(&process_session_id);
    }

    #[test]
    fn restore_active_runtime_sessions_skips_dead_process_session() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();

        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".into());
        let child = match std::process::Command::new(python)
            .args(["-c", "pass"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                eprintln!("skip dead runtime restore test: {err}");
                return;
            }
        };
        let process_session_id =
            crate::chat::runner::start_test_process_session(child, &json!({"boot": true})).unwrap();
        for _ in 0..100 {
            if !crate::chat::runner::process_session_is_active(&process_session_id) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, process_session_id, runtime_epoch, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            rusqlite::params![
                "task-1",
                "turn-dead",
                "codex",
                "nanobot",
                "running",
                process_session_id.clone(),
                "stale-epoch",
                1_i64
            ],
        )
        .unwrap();

        restore_active_runtime_sessions(&conn, &store);

        assert!(!store
            .running_tasks
            .lock()
            .unwrap()
            .get("task-1")
            .copied()
            .unwrap_or(false));
        assert!(store.running_turns.lock().unwrap().get("task-1").is_none());
        assert!(store
            .running_process_sessions
            .lock()
            .unwrap()
            .get("task-1")
            .is_none());
        crate::chat::runner::remove_test_process_session(&process_session_id);
    }

    #[test]
    fn restore_active_runtime_sessions_preserves_runtime_channel_for_resume_dispatch() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-2', 'session-2')"#,
            [],
        )
        .unwrap();

        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".into());
        let child_1 = std::process::Command::new(&python)
            .args(["-c", "import time; time.sleep(0.2)"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let child_2 = std::process::Command::new(&python)
            .args(["-c", "import time; time.sleep(0.2)"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let proc_builtin =
            crate::chat::runner::start_test_process_session(child_1, &json!({"boot": true}))
                .unwrap();
        let proc_nanobot =
            crate::chat::runner::start_test_process_session(child_2, &json!({"boot": true}))
                .unwrap();

        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, process_session_id, runtime_epoch, updated_at)
               VALUES ('task-1', 'turn-builtin', 'claude', 'builtin', 'running', ?1, 'stale-1', 1)"#,
            rusqlite::params![proc_builtin.clone()],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, process_session_id, runtime_epoch, updated_at)
               VALUES ('task-2', 'turn-nanobot', 'codex', 'nanobot', 'running', ?1, 'stale-2', 2)"#,
            rusqlite::params![proc_nanobot.clone()],
        )
        .unwrap();

        let restored = restore_active_runtime_sessions(&conn, &store);

        assert_eq!(restored.len(), 2);
        assert!(restored.iter().any(|state| state.task_id == "task-1"
            && state.turn.runtime_channel == RUNTIME_CHANNEL_BUILTIN));
        assert!(restored.iter().any(|state| state.task_id == "task-2"
            && state.turn.runtime_channel == RUNTIME_CHANNEL_NANOBOT));

        crate::chat::runner::remove_test_process_session(&proc_builtin);
        crate::chat::runner::remove_test_process_session(&proc_nanobot);
    }

    #[test]
    fn restore_active_runtime_sessions_only_returns_still_reattachable_sessions() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        conn.execute(
            r#"INSERT INTO tasks (id, session_id) VALUES ('task-1', 'session-1')"#,
            [],
        )
        .unwrap();

        let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".into());
        let child = std::process::Command::new(python)
            .args(["-c", "pass"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let process_session_id =
            crate::chat::runner::start_test_process_session(child, &json!({"boot": true})).unwrap();
        for _ in 0..100 {
            if !crate::chat::runner::process_session_is_active(&process_session_id) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        conn.execute(
            r#"INSERT INTO task_runtime_states
               (task_id, turn_id, backend, runtime_channel, phase, process_session_id, runtime_epoch, updated_at)
               VALUES ('task-1', 'turn-finished', 'claude', 'builtin', 'running', ?1, 'stale-x', 1)"#,
            rusqlite::params![process_session_id.clone()],
        )
        .unwrap();

        let restored = restore_active_runtime_sessions(&conn, &store);

        assert!(restored.is_empty());
        assert!(store.running_turns.lock().unwrap().get("task-1").is_none());
        assert!(store
            .running_process_sessions
            .lock()
            .unwrap()
            .get("task-1")
            .is_none());
        crate::chat::runner::remove_test_process_session(&process_session_id);
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
            CREATE TABLE task_agent_sessions (
              task_id         TEXT NOT NULL,
              backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                              CHECK (runtime_channel IN ('builtin','nanobot')),
              session_id      TEXT NOT NULL,
              updated_at      INTEGER NOT NULL,
              PRIMARY KEY (task_id, backend, runtime_channel)
            );
            CREATE TABLE task_runtime_states (
              task_id         TEXT PRIMARY KEY,
              turn_id         TEXT NOT NULL,
              backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              runtime_channel TEXT NOT NULL CHECK (runtime_channel IN ('builtin','nanobot')),
              phase           TEXT NOT NULL CHECK (phase IN
                                ('running','interrupted_pending_finish','reset_pending_finish')),
              process_session_id TEXT,
              runtime_epoch   TEXT NOT NULL,
              context_json    TEXT,
              updated_at      INTEGER NOT NULL
            );
            CREATE TABLE task_runtime_control_events (
              id              INTEGER PRIMARY KEY AUTOINCREMENT,
              task_id         TEXT NOT NULL,
              name            TEXT NOT NULL,
              attributes_json TEXT NOT NULL DEFAULT '{}',
              payload_json    TEXT,
              created_at      INTEGER NOT NULL
            );
            CREATE TABLE task_runtime_finalizations (
              task_id               TEXT PRIMARY KEY,
              pending_reset_cleanup INTEGER NOT NULL DEFAULT 0
                                    CHECK (pending_reset_cleanup IN (0, 1)),
              rollback_json         TEXT,
              updated_at            INTEGER NOT NULL
            );
            CREATE TABLE task_pending_turns (
              id              INTEGER PRIMARY KEY AUTOINCREMENT,
              task_id         TEXT NOT NULL,
              content         TEXT NOT NULL,
              composer_json   TEXT NOT NULL,
              project_cwd     TEXT NOT NULL,
              attachments_json TEXT NOT NULL DEFAULT '[]',
              workflow_json   TEXT,
              message_json    TEXT NOT NULL,
              turn_id         TEXT NOT NULL,
              runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                              CHECK (runtime_channel IN ('builtin','nanobot')),
              guide_id        TEXT,
              created_at      INTEGER NOT NULL
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

    fn insert_resume_task(conn: &Connection) {
        conn.execute(
            "INSERT INTO tasks (id, session_id) VALUES ('task-1', 'legacy-session')",
            [],
        )
        .unwrap();
    }

    fn insert_codex_timeline_session(conn: &Connection, event_id: &str, session_id: &str, at: i64) {
        agent_timeline::insert(
            conn,
            AgentTimelineEventInput {
                id: Some(event_id.to_string()),
                task_id: "task-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                backend: BACKEND_CODEX.to_string(),
                kind: "turn".to_string(),
                status: "success".to_string(),
                title: "Codex done".to_string(),
                summary: None,
                payload: json!({ "sessionId": session_id }),
                created_at: Some(at),
                updated_at: Some(at),
            },
        )
        .unwrap();
    }

    fn assert_resume_session(conn: &Connection, backend: &str, expected: Option<&str>) {
        assert_eq!(
            load_persisted_resume_session_id(
                conn,
                "task-1",
                backend,
                crate::RUNTIME_CHANNEL_BUILTIN
            ),
            expected.map(|sid| sid.to_string())
        );
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
            load_persisted_resume_session_id(
                &conn,
                "task-1",
                BACKEND_CODEX,
                crate::RUNTIME_CHANNEL_BUILTIN,
            ),
            Some("codex-thread".to_string())
        );
        assert_eq!(
            load_persisted_resume_session_id(
                &conn,
                "task-1",
                BACKEND_CLAUDE,
                crate::RUNTIME_CHANNEL_BUILTIN,
            ),
            None
        );
    }

    #[test]
    fn persisted_resume_session_id_reads_backend_scoped_checkpoint() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);

        persist_agent_session_id(
            &conn,
            "task-1",
            BACKEND_CLAUDE,
            crate::RUNTIME_CHANNEL_BUILTIN,
            "claude-session",
        )
        .unwrap();
        persist_agent_session_id(
            &conn,
            "task-1",
            BACKEND_CODEX,
            crate::RUNTIME_CHANNEL_BUILTIN,
            "codex-thread",
        )
        .unwrap();

        assert_resume_session(&conn, BACKEND_CLAUDE, Some("claude-session"));
        assert_resume_session(&conn, BACKEND_CODEX, Some("codex-thread"));
    }

    #[test]
    fn persisted_resume_session_id_isolated_by_runtime_channel() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);
        insert_codex_timeline_session(&conn, "codex-turn", "timeline-thread", 200);

        persist_agent_session_id(
            &conn,
            "task-1",
            BACKEND_CODEX,
            crate::RUNTIME_CHANNEL_BUILTIN,
            "builtin-thread",
        )
        .unwrap();
        persist_agent_session_id(
            &conn,
            "task-1",
            BACKEND_CODEX,
            crate::RUNTIME_CHANNEL_NANOBOT,
            "nanobot-thread",
        )
        .unwrap();

        assert_eq!(
            load_persisted_resume_session_id(
                &conn,
                "task-1",
                BACKEND_CODEX,
                crate::RUNTIME_CHANNEL_BUILTIN,
            ),
            Some("builtin-thread".to_string())
        );
        assert_eq!(
            load_persisted_resume_session_id(
                &conn,
                "task-1",
                BACKEND_CODEX,
                crate::RUNTIME_CHANNEL_NANOBOT,
            ),
            Some("nanobot-thread".to_string())
        );
        assert_eq!(
            load_persisted_resume_session_id(
                &conn,
                "task-1",
                BACKEND_CLAUDE,
                crate::RUNTIME_CHANNEL_NANOBOT,
            ),
            None
        );
    }

    #[test]
    fn persisted_resume_session_id_prefers_checkpoint_over_timeline() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);
        insert_codex_timeline_session(&conn, "codex-turn-old", "timeline-thread", 100);
        persist_agent_session_id(
            &conn,
            "task-1",
            BACKEND_CODEX,
            crate::RUNTIME_CHANNEL_BUILTIN,
            "checkpoint-thread",
        )
        .unwrap();

        assert_resume_session(&conn, BACKEND_CODEX, Some("checkpoint-thread"));
    }

    #[test]
    fn persisted_resume_session_id_falls_back_to_timeline_without_checkpoint() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);
        insert_codex_timeline_session(&conn, "codex-turn", "timeline-thread", 200);

        assert_resume_session(&conn, BACKEND_CODEX, Some("timeline-thread"));
    }

    #[test]
    fn clear_agent_sessions_for_task_removes_backend_checkpoints() {
        let conn = Connection::open_in_memory().unwrap();
        create_resume_schema(&conn);
        insert_resume_task(&conn);
        persist_agent_session_id(
            &conn,
            "task-1",
            BACKEND_CLAUDE,
            crate::RUNTIME_CHANNEL_BUILTIN,
            "claude-session",
        )
        .unwrap();
        persist_agent_session_id(
            &conn,
            "task-1",
            BACKEND_CODEX,
            crate::RUNTIME_CHANNEL_BUILTIN,
            "codex-thread",
        )
        .unwrap();

        clear_agent_sessions_for_task(&conn, "task-1").unwrap();

        assert_resume_session(&conn, BACKEND_CLAUDE, None);
        assert_resume_session(&conn, BACKEND_CODEX, None);
    }
}
