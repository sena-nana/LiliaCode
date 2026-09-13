use super::*;
use crate::application::{
    DesktopApplication, DesktopApplicationConfig, DesktopHost, DesktopHostAction,
    DesktopHostContext, DesktopHostError, DesktopHostResult,
};
use lilia_contracts::{ProductTask, TaskId};
use lilia_service::ServiceAuthority;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[test]
fn memory_injection_reaches_real_model_context_with_cooldown_and_retry() {
    let home = tempfile::tempdir().unwrap();
    let authority = ServiceAuthority::bootstrap_with_home(home.path()).unwrap();
    let application = DesktopApplication::from_authority(
        DesktopApplicationConfig::new(home.path(), "memory-wire-test").unwrap(),
        authority,
        Arc::new(NoopHost),
    )
    .unwrap();
    let runtime = application.authority().shared_runtime();
    runtime
        .inner()
        .credentials()
        .login(lilia_agent::ProductCredentialLoginInput {
            provider_id: mutsuki_agent_contracts::OPENAI_CREDENTIAL_PROVIDER_ID.into(),
            kind: mutsuki_agent_contracts::CredentialKind::ApiKey,
            secret_material: "sk-memory-loopback-test-0123456789abcdef".into(),
            account_label: None,
            source: Some("user_api_key".into()),
        })
        .unwrap();
    runtime.inner().refresh_product_profile(None).unwrap();
    application
        .save_memory(crate::application::MemoryUpsertInput {
            id: None,
            scope: crate::application::MemoryScope::User,
            project_id: None,
            title: "Wire memory".into(),
            body: "memory-wire-marker-729".into(),
            tags: vec![],
            enabled: true,
            source_task_id: None,
            expected_updated_at: None,
        })
        .unwrap();
    let mut settings = application.memory_settings().unwrap();
    settings.cooldown_turns = 2;
    application.save_memory_settings(settings.clone()).unwrap();
    let task = TaskId::new("memory-model-task").unwrap();
    application
        .authority()
        .client()
        .unwrap()
        .products()
        .create_entity(lilia_contracts::ProductEntity::Task(
            ProductTask::new(task.clone(), None, "Memory wire").unwrap(),
        ))
        .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    runtime.inner().set_model_endpoint_override(Some(format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    )));
    let (send, receive) = mpsc::channel();
    let server = std::thread::spawn(move || {
        for _ in 0..4 {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("memory model request missing: {error}"),
                }
            };
            let request = read_request(&mut stream);
            let body = json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"Memory tested."}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            send.send(request).unwrap();
        }
    });
    for index in 1..=4 {
        if index == 4 {
            settings.enabled = false;
            application.save_memory_settings(settings.clone()).unwrap();
        }
        let mut request = DesktopTurnRequest::new(task.clone(), format!("Memory turn {index}"));
        request.model = Some("gpt-5.4".into());
        let dispatched = application.start_task_turn(request).unwrap();
        let captured = receive.recv_timeout(Duration::from_secs(10)).unwrap();
        let context = captured["messages"].as_array().unwrap().iter().rev()
            .filter_map(|message| message["content"].as_str())
            .find_map(|content| content.strip_prefix("Product-provided workspace and turn context (authoritative for this turn): ").and_then(|text| serde_json::from_str::<Value>(text).ok())).unwrap();
        assert_eq!(context["memoryInjection"]["turnSequence"], index);
        let injected = index == 1 || index == 3;
        assert_eq!(
            context["memoryInjection"]["baseline"]
                .as_str()
                .is_some_and(|baseline| baseline.contains("memory-wire-marker-729")),
            injected
        );
        assert_eq!(
            context
                .to_string()
                .matches("memory-wire-marker-729")
                .count(),
            usize::from(injected)
        );
        let before = application.memory_injection_state(&task).unwrap();
        let retried = application
            .prepare_memory_for_turn(&task, &dispatched.turn_id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(retried).unwrap(),
            context["memoryInjection"]
        );
        assert_eq!(application.memory_injection_state(&task).unwrap(), before);
        let deadline = Instant::now() + Duration::from_secs(5);
        while application.task_runtime_snapshot(&task).turn_id.is_some()
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(application.task_runtime_snapshot(&task).turn_id.is_none());
    }
    server.join().unwrap();
}

struct NoopHost;
impl DesktopHost for NoopHost {
    fn execute(
        &self,
        _: &DesktopHostContext,
        _: DesktopHostAction,
    ) -> Result<DesktopHostResult, DesktopHostError> {
        Ok(DesktopHostResult::Completed)
    }
}

fn workflows() -> Vec<LiliaAgentWorkflow> {
    vec![
        LiliaAgentWorkflow::LiliaReview {
            target: LiliaReviewTarget::UncommittedChanges,
            instructions: Some("Check cancellation correctness".into()),
            delivery: Some("inline".into()),
        },
        LiliaAgentWorkflow::LiliaReview {
            target: LiliaReviewTarget::BaseBranch {
                branch: "release/review-base".into(),
            },
            instructions: None,
            delivery: Some("inline".into()),
        },
        LiliaAgentWorkflow::LiliaReview {
            target: LiliaReviewTarget::Commit {
                sha: "0123456789abcdef".into(),
            },
            instructions: None,
            delivery: Some("inline".into()),
        },
        LiliaAgentWorkflow::LiliaFixSuggestion {
            target: LiliaReviewTarget::UncommittedChanges,
            instructions: Some("Keep API compatibility".into()),
            mode: Some("suggest".into()),
        },
        LiliaAgentWorkflow::LiliaFixSuggestion {
            target: LiliaReviewTarget::UncommittedChanges,
            instructions: None,
            mode: Some("apply".into()),
        },
        LiliaAgentWorkflow::LiliaBatchApply {
            source_turn_id: "review-source-turn".into(),
            source_kind: "review".into(),
            source_summary: "Preserve pending input when a refresh completes.".into(),
            instructions: None,
        },
        LiliaAgentWorkflow::LiliaTaskWorkflow {
            kind: "refactor".into(),
            instructions: Some("Extract the parser without changing accepted syntax.".into()),
        },
    ]
}

#[test]
fn workflow_compilation_preserves_user_context_and_is_idempotent() {
    for workflow in workflows() {
        let mut request = DesktopTurnRequest::new(
            TaskId::new("workflow-compile").unwrap(),
            "Respect the existing public API.",
        );
        request.workflow = Some(workflow.clone());
        compile_turn_input(&mut request).unwrap();
        assert!(request.content.contains("Respect the existing public API."));
        assert_eq!(request.workflow, Some(workflow));
        let once = request.clone();
        compile_turn_input(&mut request).unwrap();
        assert_eq!(
            request, once,
            "queue preparation or recovery must not duplicate instructions"
        );
    }
    let mut compact = DesktopTurnRequest::new(TaskId::new("compact").unwrap(), "");
    compact.workflow = Some(LiliaAgentWorkflow::LiliaCompact);
    compile_turn_input(&mut compact).unwrap();
    assert!(
        compact.content.is_empty(),
        "compaction remains a dedicated operation"
    );
    compact.workflow = Some(LiliaAgentWorkflow::LiliaMemoryReset);
    assert!(
        compile_turn_input(&mut compact).is_err(),
        "do not turn local operations into model instructions"
    );
    compact.workflow = Some(LiliaAgentWorkflow::Automation {
        automation_run_id: "run-1".into(),
    });
    compact.content = "Inspect the release build result.".into();
    compile_turn_input(&mut compact).unwrap();
    assert_eq!(compact.content, "Inspect the release build result.");
}

fn read_request(stream: &mut std::net::TcpStream) -> Value {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0, "request ended early");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(split) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&bytes[..split]).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .expect("model request has Content-Length");
            if bytes.len() >= split + 4 + length {
                return serde_json::from_slice(&bytes[split + 4..split + 4 + length]).unwrap();
            }
        }
        assert!(bytes.len() < 2 * 1024 * 1024, "bounded model request");
    }
}

#[test]
fn workflow_only_turns_reach_real_wire_and_loopback_model_with_typed_context() {
    let authority =
        ServiceAuthority::bootstrap_in_memory_named("test:workflow-wire", "workflow-wire").unwrap();
    let runtime = authority.shared_runtime();
    runtime
        .inner()
        .credentials()
        .login(lilia_agent::ProductCredentialLoginInput {
            provider_id: mutsuki_agent_contracts::OPENAI_CREDENTIAL_PROVIDER_ID.into(),
            kind: mutsuki_agent_contracts::CredentialKind::ApiKey,
            secret_material: "sk-workflow-local-test-0123456789abcdef".into(),
            account_label: None,
            source: Some("user_api_key".into()),
        })
        .unwrap();
    runtime.inner().refresh_product_profile(None).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!(
        "http://{}/v1/chat/completions",
        listener.local_addr().unwrap()
    );
    let (send, receive) = mpsc::channel();
    let count = workflows().len();
    let server = std::thread::spawn(move || {
        for _ in 0..count {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("local model request missing: {error}"),
                }
            };
            let request = read_request(&mut stream);
            let body = json!({"choices":[{"finish_reason":"stop", "message":{"role":"assistant", "content":"Workflow completed."}}], "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            send.send(request).unwrap();
        }
    });
    let home = tempfile::tempdir().unwrap();
    let application = DesktopApplication::from_authority(
        DesktopApplicationConfig::new(home.path(), "workflow-wire-test").unwrap(),
        authority,
        Arc::new(NoopHost),
    )
    .unwrap();
    application
        .authority()
        .shared_runtime()
        .inner()
        .set_model_endpoint_override(Some(endpoint));
    for (index, workflow) in workflows().into_iter().enumerate() {
        let task = TaskId::new(format!("workflow-model-{index}")).unwrap();
        application
            .authority()
            .client()
            .unwrap()
            .products()
            .create_entity(lilia_contracts::ProductEntity::Task(
                ProductTask::new(task.clone(), None, title(&workflow)).unwrap(),
            ))
            .unwrap();
        let mut request = DesktopTurnRequest::new(task.clone(), "");
        request.workflow = Some(workflow.clone());
        request.model = Some("gpt-5.4".into());
        application.start_task_turn(request).unwrap();
        let captured = receive
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|error| {
                panic!(
                    "workflow must reach model: {error:?}; runtime={:?}",
                    application
                        .authority()
                        .shared_runtime()
                        .inner()
                        .session_snapshot(&format!("native-{}-1", task.as_str()))
                )
            });
        let messages = captured["messages"].as_array().unwrap();
        let user = messages
            .iter()
            .rev()
            .find(|message| message["role"] == "user")
            .unwrap()["content"]
            .as_str()
            .unwrap();
        assert_eq!(
            user,
            model_instruction(&workflow).unwrap(),
            "the provider receives executable workflow input"
        );
        let context = messages.iter().filter_map(|message| message["content"].as_str()).find_map(|content| content.strip_prefix("Product-provided workspace and turn context (authoritative for this turn): ").and_then(|json| serde_json::from_str::<Value>(json).ok())).expect("actual provider context message");
        assert_eq!(
            context["workflow"],
            serde_json::to_value(&workflow).unwrap()
        );
        assert_eq!(
            context["workspace"]["metadata"]["productTaskId"],
            task.as_str()
        );
        assert_ne!(title(&workflow), "附件");
    }
    server.join().unwrap();
}
