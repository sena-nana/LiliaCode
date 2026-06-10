use std::collections::BTreeMap;

use mutsuki_runtime_contracts::{
    AgentParticipation, AgentSpec, Envelope, OperationHandlerKey, OperationSnapshot,
    OperationStatus, RuntimeError, ScopeRuleSpec, SideEffectPolicy, SourceDescriptor, SourceRef,
    SourceSnapshot, StrategyResult, StrategyResultStatus,
};
use mutsuki_runtime_core::{
    AgentRuntime, BackendPayload, OperationBackend, RuntimeFailure, RuntimeResult, StrategyBackend,
};
use serde_json::json;
use tauri::AppHandle;

use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::runner::{
    finish_agent_turn, run_node_agent_runner, RunnerInvocation, RunnerOutput,
};
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::chat::types::ChatWorkflow;
use crate::util::now_millis;
use crate::RUNTIME_CHANNEL_NANOBOT;

pub(crate) fn supervise_turn(app: AppHandle, invocation: RunnerInvocation) -> Result<(), String> {
    let task_id = invocation.task_id.clone();
    let turn_id = invocation.turn_id.clone();
    let backend = invocation.composer.backend.clone();
    let project_cwd = invocation.project_cwd.clone();
    let prompt_length = invocation.content.chars().count();
    let attachment_count = invocation.attachments.len();
    let workflow_type = invocation.workflow.as_ref().and_then(workflow_type);
    let source_id = format!("lilia:{backend}");
    let agent_id = format!("lilia-{backend}-agent");
    let source = SourceSnapshot {
        descriptor: SourceDescriptor {
            source_id: source_id.clone(),
            kind: "lilia.chat".to_string(),
            capabilities: Vec::new(),
            description: "Lilia chat turn source".to_string(),
        },
        plugin_id: "lilia".to_string(),
        plugin_generation: 0,
    };
    let mut runtime = AgentRuntime::new();
    let mut host = LiliaNodeRunnerBackend::new(app.clone(), source, invocation);
    runtime
        .register_agent(AgentSpec {
            agent_id: agent_id.clone(),
            owner: Some("lilia".to_string()),
            priority: 0,
            participation: AgentParticipation::PrimaryCandidate,
            accepts: vec![ScopeRuleSpec::BySourceId {
                source_id: source_id.clone(),
            }],
            strategy_id: "lilia-node-runner".to_string(),
            side_effect_policy: SideEffectPolicy::AllowExternal,
        })
        .map_err(|err| format_runtime_error(err.error()))?;
    runtime
        .start_agent(&agent_id, &mut host)
        .map_err(|err| format_runtime_error(err.error()))?;

    runtime
        .publish(Envelope {
            id: format!("{}:input", turn_id),
            timestamp: now_millis() as f64 / 1000.0,
            source: SourceRef {
                source_id,
                kind: "lilia.chat".to_string(),
                metadata: BTreeMap::new(),
            },
            payload_schema_id: "lilia.chat.input".to_string(),
            capabilities_required: Vec::new(),
            payload: json!({
                "backend": backend,
                "cwd": project_cwd,
                "promptLength": prompt_length,
                "attachmentCount": attachment_count,
                "workflowType": workflow_type,
            }),
        })
        .map_err(|err| format_runtime_error(err.error()))?;
    runtime
        .tick_once(&agent_id, &mut host)
        .map_err(|err| format_runtime_error(err.error()))?;
    runtime
        .stop_agent(&agent_id, &mut host)
        .map_err(|err| format_runtime_error(err.error()))?;

    let runner_result = host.take_output();
    let events = runtime.events();
    let now = now_millis() as i64;
    persist_and_emit_input(
        &app,
        AgentTimelineEventInput {
            id: Some(format!("{}:{}:nanobot-runtime", task_id, turn_id)),
            task_id: task_id.clone(),
            turn_id: Some(turn_id.clone()),
            backend: backend.clone(),
            kind: "diagnostic".to_string(),
            status: "success".to_string(),
            title: "NanoBot Runtime".to_string(),
            summary: Some("NanoBot Rust Core 已接管本轮运行时调度".to_string()),
            payload: json!({
                "runtimeChannel": RUNTIME_CHANNEL_NANOBOT,
                "agentId": agent_id,
                "eventCount": events.len(),
                "events": events,
            }),
            created_at: Some(now),
            updated_at: Some(now),
        },
    );
    let output = runner_result.unwrap_or_default();
    finish_agent_turn(
        app,
        task_id,
        backend,
        RUNTIME_CHANNEL_NANOBOT.to_string(),
        output.last_session_id,
        !output.interrupted && !output.reset,
    );
    Ok(())
}

struct LiliaNodeRunnerBackend {
    app: AppHandle,
    source: SourceSnapshot,
    invocation: Option<RunnerInvocation>,
    output: Option<RunnerOutput>,
}

impl LiliaNodeRunnerBackend {
    fn new(app: AppHandle, source: SourceSnapshot, invocation: RunnerInvocation) -> Self {
        Self {
            app,
            source,
            invocation: Some(invocation),
            output: None,
        }
    }

    fn take_output(&mut self) -> Option<RunnerOutput> {
        self.output.take()
    }
}

impl StrategyBackend for LiliaNodeRunnerBackend {
    fn on_awake(&mut self, _agent_id: &str) -> RuntimeResult<()> {
        Ok(())
    }

    fn on_input(&mut self, _agent_id: &str, _envelope: &Envelope) -> RuntimeResult<StrategyResult> {
        let Some(invocation) = self.invocation.take() else {
            return Ok(StrategyResult::wait_input());
        };
        let task_id = invocation.task_id.clone();
        let turn_id = invocation.turn_id.clone();
        let backend = invocation.composer.backend.clone();
        match run_node_agent_runner(&self.app, invocation) {
            Ok(output) => {
                self.output = Some(output);
                Ok(StrategyResult {
                    status: StrategyResultStatus::Completed,
                    decision: None,
                    emitted: Vec::new(),
                    error: None,
                })
            }
            Err(message) => {
                let error = RuntimeError::new(
                    mutsuki_runtime_contracts::ERR_RUNTIME_BACKEND_FAILED,
                    "lilia-node-runner",
                    "lilia-node-runner.on_input",
                );
                crate::chat::timeline_sink::persist_and_emit_error_timeline_event(
                    &self.app,
                    &task_id,
                    &backend,
                    Some(&turn_id),
                    message,
                );
                self.output = Some(RunnerOutput::default());
                Ok(StrategyResult {
                    status: StrategyResultStatus::Failed,
                    decision: None,
                    emitted: Vec::new(),
                    error: Some(error),
                })
            }
        }
    }

    fn next_step(&mut self, _agent_id: &str) -> RuntimeResult<StrategyResult> {
        Ok(StrategyResult::wait_input())
    }

    fn on_stop(&mut self, _agent_id: &str) -> RuntimeResult<()> {
        Ok(())
    }
}

impl OperationBackend for LiliaNodeRunnerBackend {
    fn list_operations(&self, _agent_id: &str) -> RuntimeResult<Vec<OperationSnapshot>> {
        Ok(Vec::new())
    }

    fn list_sources(&self, _agent_id: &str) -> RuntimeResult<Vec<SourceSnapshot>> {
        Ok(vec![self.source.clone()])
    }

    fn invoke(
        &mut self,
        _agent_id: &str,
        _key: &OperationHandlerKey,
        _payload: serde_json::Value,
    ) -> RuntimeResult<BackendPayload> {
        Err(RuntimeFailure::new(RuntimeError::new(
            mutsuki_runtime_contracts::ERR_OPERATION_NOT_FOUND,
            "lilia-node-runner",
            "lilia-node-runner.invoke",
        )))
    }

    fn operation_status(&self, _agent_id: &str, _key: &OperationHandlerKey) -> OperationStatus {
        OperationStatus::NotFound
    }
}

fn workflow_type(workflow: &ChatWorkflow) -> Option<&'static str> {
    match workflow {
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
    }
}

fn format_runtime_error(error: &RuntimeError) -> String {
    format!("{} at {} ({})", error.code, error.route, error.source)
}
