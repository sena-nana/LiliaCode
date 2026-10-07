use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::application::{
    DesktopApplicationConfig, DesktopEventSubscription, DesktopGoalSnapshot, DesktopGoalStatus,
    DesktopHost, DesktopHostAction, DesktopHostContext, DesktopHostError, DesktopHostResult,
    DesktopTaskSessionSnapshot,
};
use crate::task_session::TaskSessionView;
use lilia_agent::ProductCredentialLoginInput;
use lilia_contracts::{
    AgentSessionRef, ChatAttachment, ChatAttachmentKind, ChatConversationReference,
    LiliaAgentWorkflow, ProductEntity, ProjectionEventId, TimelineProjectionEvent,
};
use lilia_feature_agent_session::ClaimWorkerOutcome;
use lilia_service::ServiceAuthority;
use mutsuki_agent_contracts::{CredentialKind, OPENAI_CREDENTIAL_PROVIDER_ID};

static NEXT_AGENT_APPLICATION_ID: AtomicU64 = AtomicU64::new(1);

struct NoopHost;

impl DesktopHost for NoopHost {
    fn execute(
        &self,
        _context: &DesktopHostContext,
        _action: DesktopHostAction,
    ) -> Result<DesktopHostResult, DesktopHostError> {
        Ok(DesktopHostResult::Completed)
    }
}

fn mcp_pending() -> PendingProjection {
    PendingProjection {
        id: "pending-mcp".to_owned(),
        task_id: TaskId::new("task-mcp").unwrap(),
        agent_session: AgentSessionRef::new("session-mcp").unwrap(),
        sequence: 1,
        turn_id: Some("turn-mcp".to_owned()),
        request_id: "request-mcp".to_owned(),
        kind: "mcp_elicitation".to_owned(),
        status: PendingProjectionStatus::Open,
        prompt: Some("选择项目".to_owned()),
        action_revision: Some(1),
        payload: json!({
            "threadId": "thread-mcp",
            "turnId": "turn-mcp",
            "serverName": "linear",
            "mode": "form",
            "message": "选择项目",
            "requestedSchema": {
                "type": "object",
                "required": ["project"],
                "properties": {
                    "project": {"type": "string", "enum": ["A", "B"]}
                }
            }
        }),
    }
}

#[test]
fn session_fork_replaces_the_task_binding_without_leaving_the_parent_preferred() {
    let id = NEXT_AGENT_APPLICATION_ID.fetch_add(1, Ordering::Relaxed);
    let authority = ServiceAuthority::bootstrap_in_memory_named(
        format!("test:desktop-agent-session-fork:{id}"),
        format!("desktop-agent-session-fork:{id}"),
    )
    .unwrap();
    let task_id = TaskId::new(format!("session-fork-task-{id}")).unwrap();
    authority
        .client()
        .unwrap()
        .products()
        .create_entity(ProductEntity::Task(
            ProductTask::new(task_id.clone(), None, "Session fork").unwrap(),
        ))
        .unwrap();
    let application = DesktopApplication::from_authority(
        DesktopApplicationConfig::new(
            "C:/lilia/native-session-fork-test",
            format!("liliacode.native-session-fork-test.{id}"),
        )
        .unwrap(),
        authority,
        Arc::new(NoopHost),
    )
    .unwrap();

    application
        .persist_session_binding(&task_id, "parent-session", "profile")
        .unwrap();
    application
        .replace_session_binding(&task_id, "forked-session", "profile")
        .unwrap();

    let bindings = application
        .authority()
        .list_session_bindings(&task_id)
        .unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].agent_session.as_str(), "forked-session");
}

fn fork_binding_application(label: &str) -> (DesktopApplication, TaskId) {
    let id = NEXT_AGENT_APPLICATION_ID.fetch_add(1, Ordering::Relaxed);
    let authority = ServiceAuthority::bootstrap_in_memory_named(
        format!("test:desktop-agent-fork-binding-{label}:{id}"),
        format!("desktop-agent-fork-binding-{label}:{id}"),
    )
    .unwrap();
    let task_id = TaskId::new(format!("fork-binding-{label}-{id}")).unwrap();
    authority
        .client()
        .unwrap()
        .products()
        .create_entity(ProductEntity::Task(
            ProductTask::new(task_id.clone(), None, "Fork binding").unwrap(),
        ))
        .unwrap();
    let application = DesktopApplication::from_authority(
        DesktopApplicationConfig::new(
            "C:/lilia/native-fork-binding-test",
            format!("liliacode.native-fork-binding-test.{id}"),
        )
        .unwrap(),
        authority,
        Arc::new(NoopHost),
    )
    .unwrap();
    let runtime = application.authority().shared_runtime();
    runtime
        .inner()
        .credentials()
        .login(ProductCredentialLoginInput {
            provider_id: OPENAI_CREDENTIAL_PROVIDER_ID.into(),
            kind: CredentialKind::ApiKey,
            secret_material: "sk-test-openai-api-key-0123456789abcdef".into(),
            account_label: None,
            source: Some("user_api_key".into()),
        })
        .unwrap();
    runtime.inner().refresh_product_profile(None).unwrap();
    (application, task_id)
}

fn bound_session(application: &DesktopApplication, task_id: &TaskId) -> String {
    let bindings = application
        .authority()
        .list_session_bindings(task_id)
        .unwrap();
    assert_eq!(bindings.len(), 1);
    bindings[0].agent_session.as_str().to_owned()
}

#[test]
fn fork_through_turn_binds_the_task_only_to_the_target_session() {
    let (application, task_id) = fork_binding_application("success");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 16_384];
        let _ = stream.read(&mut request).unwrap();
        let body = json!({
            "choices": [{
                "finish_reason": "stop",
                "message": {"role": "assistant", "content": "done"}
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        })
        .to_string();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    application
        .authority()
        .shared_runtime()
        .inner()
        .set_model_endpoint_override(Some(format!("http://{address}/v1/chat/completions")));
    let mut request = DesktopTurnRequest::new(task_id.clone(), "first");
    request.permission = DesktopExecutionPermission::Full;
    let started = application.start_task_turn(request).unwrap();
    server.join().unwrap();
    assert_eq!(
        application.task_runtime_snapshot(&task_id).phase,
        "idle",
        "fork requires an idle task"
    );

    let source = bound_session(&application, &task_id);
    let target = application
        .fork_task_agent_session_through_turn(&task_id, &started.turn_id)
        .unwrap();

    assert_ne!(target, source);
    assert_eq!(bound_session(&application, &task_id), target);
    assert_eq!(
        application
            .authority()
            .shared_runtime()
            .inner()
            .session_ids_for_task(&task_id),
        vec![target.clone()]
    );
    let runtime = application.authority().shared_runtime();
    assert_eq!(
        runtime
            .inner()
            .session_snapshot(&target)
            .unwrap()
            .session_id,
        target
    );
    assert!(runtime
        .inner()
        .session_snapshot(&source)
        .unwrap()
        .events
        .iter()
        .any(|event| event.meta.turn_id.as_deref() == Some(started.turn_id.as_str())));
}

#[test]
fn failed_fork_binding_keeps_the_task_on_the_original_session() {
    let (application, task_id) = fork_binding_application("failure");
    let opened = application.open_task_agent_wire_session(&task_id).unwrap();
    let source = bound_session(&application, &task_id);
    assert_eq!(source, opened.session_id);

    let failed = application.bind_forked_task_session(&task_id, "not-ready-session", "profile");

    assert!(failed.is_err());
    assert_eq!(bound_session(&application, &task_id), source);
    assert_eq!(
        application
            .authority()
            .shared_runtime()
            .inner()
            .session_ids_for_task(&task_id),
        vec![source]
    );
}

#[test]
fn typed_workflow_can_start_without_message_content_and_reaches_turn_context() {
    let id = NEXT_AGENT_APPLICATION_ID.fetch_add(1, Ordering::Relaxed);
    let authority = ServiceAuthority::bootstrap_in_memory_named(
        format!("test:desktop-agent-workflow:{id}"),
        format!("desktop-agent-workflow:{id}"),
    )
    .unwrap();
    let task_id = TaskId::new(format!("workflow-task-{id}")).unwrap();
    authority
        .client()
        .unwrap()
        .products()
        .create_entity(ProductEntity::Task(
            ProductTask::new(task_id.clone(), None, "Workflow turn").unwrap(),
        ))
        .unwrap();
    let application = DesktopApplication::from_authority(
        DesktopApplicationConfig::new(
            "C:/lilia/native-workflow-test",
            format!("liliacode.native-workflow-test.{id}"),
        )
        .unwrap(),
        authority,
        Arc::new(NoopHost),
    )
    .unwrap();
    let mut request = DesktopTurnRequest::new(task_id.clone(), "");
    request.workflow = Some(LiliaAgentWorkflow::LiliaCompact);

    let prepared = application.prepare_task_turn_request(request).unwrap();
    let context = turn_context(&task_id, "turn-workflow", &prepared, None, None, None, None);

    assert_eq!(context["workflow"]["type"], "lilia_compact");
}

#[test]
fn mcp_interaction_response_preserves_actions_and_validates_form_content() {
    assert!(supported_pending_interaction_kind("mcp_elicitation"));
    let pending = mcp_pending();
    let accepted = normalized_pending_interaction_response(
        &pending,
        true,
        json!({"action": "accept", "content": {"project": "B"}}),
    )
    .unwrap();
    assert!(accepted.0);
    assert_eq!(accepted.1["content"]["project"], "B");
    assert!(normalized_pending_interaction_response(
        &pending,
        true,
        json!({"action": "accept", "content": {}}),
    )
    .is_err());
    assert_eq!(
        normalized_pending_interaction_response(&pending, false, json!({"action": "decline"}),)
            .unwrap(),
        (false, json!({"action": "decline"}))
    );
    assert!(
        normalized_pending_interaction_response(&pending, true, json!({"action": "cancel"}),)
            .is_err()
    );
}

#[test]
fn tool_consent_response_is_supported_and_decision_fenced() {
    assert!(supported_pending_interaction_kind("tool_consent"));
    assert!(!supported_pending_interaction_kind("agent_interaction"));
    let mut pending = mcp_pending();
    pending.kind = "tool_consent".to_owned();
    pending.payload = json!({
        "toolName": "shell",
        "input": {"command": "cargo test"}
    });

    let response = json!({
        "taskId": "task-1",
        "requestId": pending.request_id.clone(),
        "decision": "allow",
        "message": null,
        "updatedInput": {"command": "cargo test --locked"}
    });
    assert_eq!(
        normalized_pending_interaction_response(&pending, true, response.clone()).unwrap(),
        (true, response)
    );
    assert!(
        normalized_pending_interaction_response(&pending, false, json!({"decision": "allow"}),)
            .is_err()
    );
    assert!(normalized_pending_interaction_response(
        &pending,
        false,
        json!({"decision": "deny", "updatedInput": "invalid"}),
    )
    .is_err());
}

#[test]
fn native_turn_context_preserves_structured_attachments() {
    let task_id = TaskId::new("task-attachment").unwrap();
    let request = DesktopTurnRequest::new(task_id.clone(), "inspect").with_attachments(vec![
        ChatAttachment {
            id: "att-1".to_owned(),
            name: "README.md".to_owned(),
            path: "C:/repo/README.md".to_owned(),
            kind: ChatAttachmentKind::File,
            size: Some(42),
            exists: true,
            mime: None,
            directory: None,
        },
    ]);

    let context = turn_context(&task_id, "turn-1", &request, None, None, None, None);

    assert_eq!(context["attachments"][0]["id"], "att-1");
    assert_eq!(context["attachments"][0]["kind"], "file");
    assert_eq!(context["attachments"][0]["path"], "C:/repo/README.md");
}

#[test]
fn conversation_references_are_structured_and_serialized_once() {
    let task_id = TaskId::new("task-reference").unwrap();
    let reference = ChatConversationReference {
        task_id: "related-task".to_owned(),
        title: "相关设计".to_owned(),
        route: "/chats/related-task".to_owned(),
        project_id: None,
        project_name: None,
    };
    let request = DesktopTurnRequest::new(task_id.clone(), "inspect")
        .with_conversation_references(vec![reference.clone()]);

    assert_eq!(
        turn_content_with_references(&request),
        "inspect\n[对话引用: 相关设计 | related-task]"
    );
    let already_referenced = DesktopTurnRequest::new(
        task_id.clone(),
        "inspect\n[对话引用: 相关设计 | related-task]",
    )
    .with_conversation_references(vec![reference]);
    assert_eq!(
        turn_content_with_references(&already_referenced),
        "inspect\n[对话引用: 相关设计 | related-task]"
    );
    let context = turn_context(&task_id, "turn-reference", &request, None, None, None, None);
    assert_eq!(
        context["conversationReferences"][0]["taskId"],
        "related-task"
    );
}

#[test]
fn native_turn_context_includes_the_task_goal_snapshot() {
    let task_id = TaskId::new("task-goal").unwrap();
    let request = DesktopTurnRequest::new(task_id.clone(), "continue");
    let goal = DesktopGoalSnapshot {
        thread_id: task_id.as_str().to_owned(),
        objective: "finish Native parity".to_owned(),
        status: DesktopGoalStatus::Active,
        token_budget: Some(4_096),
        tokens_used: 512,
        time_used_seconds: 30,
        created_at: 100,
        updated_at: 200,
    };

    let context = turn_context(
        &task_id,
        "turn-goal",
        &request,
        Some(&goal),
        None,
        None,
        None,
    );

    assert_eq!(context["goal"]["objective"], "finish Native parity");
    assert_eq!(context["goal"]["status"], "active");
    assert_eq!(context["goal"]["tokenBudget"], 4_096);
}

#[test]
fn attachment_references_are_appended_once_and_support_attachment_only_turns() {
    let attachment = ChatAttachment {
        id: "att-1".to_owned(),
        name: "src".to_owned(),
        path: "C:/repo/src".to_owned(),
        kind: ChatAttachmentKind::Directory,
        size: None,
        exists: true,
        mime: None,
        directory: None,
    };
    let task_id = TaskId::new("task-attachment").unwrap();
    let only_attachment =
        DesktopTurnRequest::new(task_id.clone(), "").with_attachments(vec![attachment.clone()]);
    assert_eq!(
        turn_content_with_references(&only_attachment),
        "[目录引用: src | C:/repo/src]"
    );

    let referenced = DesktopTurnRequest::new(task_id, "Inspect\n[目录引用: src | C:/repo/src]")
        .with_attachments(vec![attachment]);
    assert_eq!(
        turn_content_with_references(&referenced),
        "Inspect\n[目录引用: src | C:/repo/src]"
    );
}

#[cfg(debug_assertions)]
#[test]
fn cancelling_a_turn_appends_the_visible_timeline() {
    let (application, task_id) = cancel_timeline_application("append");
    let turn_id = "turn-cancel-append";
    application
        .seed_interrupted_tool_for_debug(&task_id, turn_id)
        .unwrap();
    let session_id = bound_session(&application, &task_id);
    claim_prepared_turn(&application, &task_id, turn_id, &session_id);

    let mut visible = TaskSessionView::from_snapshot({
        let mut snapshot = application.task_session_snapshot(&task_id).unwrap();
        assert!(
            !snapshot.timeline.is_empty(),
            "seeded turn should already have projected events"
        );
        snapshot
            .timeline
            .insert(0, unsaved_history_row(&task_id, &session_id));
        snapshot
    });
    let kept_id = visible.timeline[0].id.clone();
    let projected_ids: Vec<_> = visible
        .timeline
        .iter()
        .skip(1)
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();

    let events = application.subscribe_events();
    application.interrupt_task_turn(&task_id).unwrap();
    let changed = next_timeline_changed(&events);
    assert_eq!(changed.task_id, task_id);

    let sequence = application
        .authority()
        .shared_runtime()
        .inner()
        .session_snapshot(&session_id)
        .unwrap()
        .events
        .last()
        .unwrap()
        .sequence;
    assert_eq!(changed.cursor, Some(sequence));

    let stored = application.task_session_snapshot(&task_id).unwrap();
    assert_eq!(stored.timeline.last().unwrap().sequence, sequence);
    apply_published_timeline(&mut visible, stored, changed.cursor);
    assert!(
        visible.timeline.iter().any(|item| item.id == kept_id),
        "incremental update keeps history that a full replace would drop"
    );
    for id in projected_ids {
        assert!(visible.timeline.iter().any(|item| item.id == id));
    }
    assert_eq!(
        visible.timeline.last().map(|item| item.sequence),
        Some(sequence)
    );
}

#[test]
fn cancelling_without_a_session_sequence_rereads_the_timeline() {
    let (application, task_id) = cancel_timeline_application("reload");
    let session_id = application
        .open_task_agent_wire_session(&task_id)
        .unwrap()
        .session_id;
    assert!(application
        .authority()
        .shared_runtime()
        .inner()
        .session_snapshot(&session_id)
        .unwrap()
        .events
        .is_empty());
    let turn_id = "turn-cancel-reload";
    start_and_claim_turn(&application, &task_id, turn_id, &session_id);

    let mut visible = TaskSessionView::from_snapshot({
        let mut snapshot = application.task_session_snapshot(&task_id).unwrap();
        snapshot
            .timeline
            .push(unsaved_history_row(&task_id, &session_id));
        snapshot
    });
    let kept_id = visible.timeline[0].id.clone();

    let events = application.subscribe_events();
    application.interrupt_task_turn(&task_id).unwrap();
    let changed = next_timeline_changed(&events);
    assert_eq!(changed.task_id, task_id);
    assert!(changed.cursor.is_none());

    let reread = application
        .task_session_snapshot_page(&task_id, 100)
        .unwrap();
    let expected_ids: Vec<_> = reread
        .timeline
        .iter()
        .map(|event| event.id.as_str().to_owned())
        .collect::<Vec<_>>();
    apply_published_timeline(&mut visible, reread, changed.cursor);
    let visible_ids: Vec<_> = visible
        .timeline
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(visible_ids, expected_ids);
    assert!(
        visible_ids.iter().all(|id| id != &kept_id),
        "a missing sequence replaces the visible timeline with a fresh read"
    );
}

fn cancel_timeline_application(label: &str) -> (DesktopApplication, TaskId) {
    let id = NEXT_AGENT_APPLICATION_ID.fetch_add(1, Ordering::Relaxed);
    let authority = ServiceAuthority::bootstrap_in_memory_named(
        format!("test:desktop-agent-cancel-timeline:{label}:{id}"),
        format!("desktop-agent-cancel-timeline:{label}:{id}"),
    )
    .unwrap();
    authority
        .shared_runtime()
        .inner()
        .credentials()
        .login(ProductCredentialLoginInput {
            provider_id: OPENAI_CREDENTIAL_PROVIDER_ID.into(),
            kind: CredentialKind::ApiKey,
            secret_material: "sk-test-cancel-timeline-0123456789abcdef".into(),
            account_label: None,
            source: Some("cancel-timeline-test".into()),
        })
        .unwrap();
    let task_id = TaskId::new(format!("cancel-timeline-{label}-{id}")).unwrap();
    authority
        .client()
        .unwrap()
        .products()
        .create_entity(ProductEntity::Task(
            ProductTask::new(task_id.clone(), None, "Cancel timeline").unwrap(),
        ))
        .unwrap();
    let application = DesktopApplication::from_authority(
        DesktopApplicationConfig::new(
            "C:/lilia/native-cancel-timeline-test",
            format!("liliacode.native-cancel-timeline.{label}.{id}"),
        )
        .unwrap(),
        authority,
        Arc::new(NoopHost),
    )
    .unwrap();
    (application, task_id)
}

fn claim_prepared_turn(
    application: &DesktopApplication,
    task_id: &TaskId,
    turn_id: &str,
    session_id: &str,
) {
    application
        .inner
        .agent
        .attach_session(task_id, turn_id, session_id.to_owned());
    assert_eq!(
        application
            .inner
            .agent
            .snapshot(task_id)
            .session_id
            .as_deref(),
        Some(session_id)
    );
    let mut queue = application.inner.turn_submissions.queue().unwrap();
    let outcome = lilia_feature_agent_session::claim_turn_for_worker(
        &mut queue,
        &application.inner.agent,
        task_id,
        turn_id,
    )
    .unwrap();
    assert!(matches!(outcome, Some(ClaimWorkerOutcome::Submit { .. })));
}

fn start_and_claim_turn(
    application: &DesktopApplication,
    task_id: &TaskId,
    turn_id: &str,
    session_id: &str,
) {
    let request = application
        .prepare_task_turn_request(DesktopTurnRequest::new(task_id.clone(), "停止这一轮"))
        .unwrap();
    {
        let submission = application
            .inner
            .turn_submissions
            .submission_guard()
            .unwrap();
        application
            .inner
            .turn_submissions
            .queue()
            .unwrap()
            .enqueue(turn_id, &request)
            .unwrap();
        application
            .accept_persisted_task_turn(request, turn_id.to_owned(), false)
            .unwrap();
        drop(submission);
    }
    claim_prepared_turn(application, task_id, turn_id, session_id);
}

fn unsaved_history_row(task_id: &TaskId, session_id: &str) -> TimelineProjectionEvent {
    TimelineProjectionEvent {
        id: ProjectionEventId::new("local-history-row"),
        task_id: task_id.clone(),
        agent_session: AgentSessionRef::new(session_id).unwrap(),
        sequence: 0,
        turn_id: None,
        kind: "message".into(),
        status: "success".into(),
        title: "earlier context".into(),
        summary: Some("already on screen".into()),
        payload: json!({ "role": "user", "text": "already on screen" }),
        projected: true,
    }
}

fn apply_published_timeline(
    visible: &mut TaskSessionView,
    snapshot: DesktopTaskSessionSnapshot,
    cursor: Option<u64>,
) {
    if cursor.is_none() {
        *visible = TaskSessionView::from_snapshot(snapshot);
    } else {
        visible.apply_projection_delta(snapshot);
    }
}

fn next_timeline_changed(events: &DesktopEventSubscription) -> TimelineChanged {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let timeout = deadline.saturating_duration_since(Instant::now());
        assert!(!timeout.is_zero(), "timed out waiting for timeline change");
        let event = events.recv_timeout(timeout).expect("timeline change");
        if let Some(changed) = event.downcast::<TimelineChanged>() {
            return changed.clone();
        }
    }
}
