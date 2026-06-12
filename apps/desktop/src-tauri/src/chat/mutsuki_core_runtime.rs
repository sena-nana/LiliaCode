use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

mod diagnostic;

use mutsuki_runtime_contracts::{
    AgentParticipation, AgentSpec, Envelope, LeaseToken, OperationDescriptor, OperationHandlerKey,
    OperationSnapshot, OperationStatus, RefDescriptor, RuntimeError, ScalarValue, ScopeRuleSpec,
    SideEffectPolicy, SourceDescriptor, SourceRef, SourceSnapshot, StrategyResult,
    StrategyResultStatus,
};
use mutsuki_runtime_core::{
    AgentRuntime, AgentScheduler, BackendEventSink, BackendPayload, OperationBackend,
    RuntimeFailure, RuntimeResult, RuntimeTickOutcome, SchedulerDecision, SchedulerDriver,
    SchedulerOptions, SchedulerStopReason, StrategyBackend,
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use tauri::{AppHandle, Manager, Runtime};

use crate::chat::runner::{
    finish_agent_turn, poll_runner_session, reattach_runner_session, start_runner_session,
    write_runner_stdin_payload, RunnerInvocation, RunnerLifecycleEvent, RunnerLifecycleObserver,
    RunnerOutput, RunnerSession, RunnerSessionPoll,
};
use crate::chat::state::{
    clear_runtime_control_event_ids_for_app, persist_agent_session_id,
    take_runtime_control_deliveries_for_app, ChatStore, RuntimeControlDelivery,
    RuntimeControlEvent,
};
use crate::chat::state::{session_key, PersistedRuntimeState};
use crate::chat::types::ChatWorkflow;
use crate::util::now_millis;
use crate::RUNTIME_CHANNEL_MUTSUKI_CORE;

#[cfg(test)]
use diagnostic::{diagnostic_payload, diagnostic_status, record_runner_lifecycle_events};
use diagnostic::{
    format_runtime_error, persist_runtime_diagnostic, record_missing_runner_operation,
    record_runner_lifecycle_event, record_runner_watchdog_idle, record_runtime_control_events,
    runtime_backend_error,
};

const PLUGIN_ID: &str = "lilia.local-runtime";
const STRATEGY_ID: &str = "lilia-local-runtime";
const OP_RUNNER_EXECUTE: &str = "lilia.runner.execute";
const OP_RUNNER_POLL: &str = "lilia.runner.poll";
const OP_INTERACTION_RESPONSE: &str = "lilia.runner.interaction_response";
const OP_PERMISSION_UPDATE: &str = "lilia.runtime.permission_update";
const OP_INTERRUPT_REQUEST: &str = "lilia.runtime.interrupt_request";
const OP_RESET_REQUEST: &str = "lilia.runtime.reset_request";
const OP_STOP_MARKS_READ: &str = "lilia.runtime.stop_marks_read";
const OP_SESSION_CHECKPOINT: &str = "lilia.runtime.session_checkpoint";
const CONTROL_PAYLOAD_SCHEMA_ID: &str = "lilia.runtime.control";
const RUNTIME_SCHEDULER_MAX_TICKS: usize = 30_000;
const RUNTIME_SCHEDULER_IDLE_TICK_LIMIT: usize = 18_750;
const RUNTIME_SCHEDULER_POLL_SLEEP_MS: u64 = 16;

pub(crate) fn supervise_turn<R: Runtime>(
    app: AppHandle<R>,
    invocation: RunnerInvocation,
) -> Result<(), String> {
    let context = RuntimeTurnContext::from_invocation(&invocation);
    supervise_runtime(app, context, Some(invocation), None)
}

pub(crate) fn resume_supervised_turn<R: Runtime>(
    app: AppHandle<R>,
    persisted: PersistedRuntimeState,
) -> Result<(), String> {
    let context = RuntimeTurnContext::from_persisted(&persisted);
    supervise_runtime(app, context, None, persisted.process_session_id)
}

fn supervise_runtime<R: Runtime>(
    app: AppHandle<R>,
    context: RuntimeTurnContext,
    invocation: Option<RunnerInvocation>,
    resumed_process_session_id: Option<String>,
) -> Result<(), String> {
    let source_id = context.source_id.clone();
    let agent_id = context.agent_id.clone();
    let mut runtime = AgentRuntime::new();
    register_runtime_resources(&mut runtime, &context);
    let mut backend = LiliaRuntimeBackend::new(
        app.clone(),
        context.clone(),
        invocation,
        resumed_process_session_id,
    )?;
    let mut fatal_error: Option<String> = None;
    let mut leases = Vec::new();

    if let Err(err) = runtime.register_agent(AgentSpec {
        agent_id: agent_id.clone(),
        owner: Some("lilia".to_string()),
        priority: 0,
        participation: AgentParticipation::PrimaryCandidate,
        accepts: vec![
            ScopeRuleSpec::BySourceId {
                source_id: source_id.clone(),
            },
            ScopeRuleSpec::BySourceId {
                source_id: control_source_id(&context),
            },
        ],
        strategy_id: STRATEGY_ID.to_string(),
        side_effect_policy: SideEffectPolicy::AllowExternal,
    }) {
        fatal_error = Some(format_runtime_error(err.error()));
    } else if let Err(err) = runtime.start_agent(&agent_id, &mut backend) {
        fatal_error = Some(format_runtime_error(err.error()));
    } else if let Err(err) = runtime.publish(build_turn_envelope(&context)) {
        fatal_error = Some(format_runtime_error(err.error()));
    } else {
        match acquire_runtime_resources(&mut runtime, &context) {
            Ok(acquired) => {
                leases = acquired;
                let scheduler = AgentScheduler::new(SchedulerOptions {
                    max_ticks: RUNTIME_SCHEDULER_MAX_TICKS,
                    stop_on_wait_input: false,
                });
                let mut driver = LiliaRuntimeSchedulerDriver {
                    context: &context,
                    idle_ticks: 0,
                };
                match scheduler.run_with_driver(&mut runtime, &agent_id, &mut backend, &mut driver)
                {
                    Ok(report) => match report.stop_reason {
                        SchedulerStopReason::Halted | SchedulerStopReason::Completed => {}
                        SchedulerStopReason::MaxTicks => {
                            fatal_error =
                                Some("MutsukiCore runtime scheduler reached max ticks".to_string());
                        }
                        SchedulerStopReason::Failed => {
                            fatal_error = Some("MutsukiCore runtime strategy failed".to_string());
                        }
                        SchedulerStopReason::WaitInput => {
                            fatal_error = Some(
                                "MutsukiCore runtime stopped while waiting for input".to_string(),
                            );
                        }
                    },
                    Err(err) => {
                        fatal_error = Some(format_runtime_error(err.error()));
                    }
                }
            }
            Err(err) => {
                fatal_error = Some(format_runtime_error(err.error()));
            }
        }
    }
    if fatal_error.is_none() {
        fatal_error = backend.fatal_error.clone();
    }
    let operation_count = backend.operation_count();
    let source_count = backend.source_count();
    let lifecycle_events = backend.lifecycle_events().to_vec();
    let output = backend.take_output().unwrap_or_default();
    let control_events = {
        let events = backend.control_events();
        let mut events = events.lock().unwrap();
        std::mem::take(&mut *events)
    };
    if !leases.is_empty() {
        release_runtime_resources(&mut runtime, leases);
    }
    if let Err(err) = runtime.stop_agent(&agent_id, &mut backend) {
        fatal_error.get_or_insert_with(|| format_runtime_error(err.error()));
    }
    record_runtime_control_events(&mut runtime, &context, &control_events);
    if let Some(err) = &fatal_error {
        runtime.record_backend_event(
            Some(&context.agent_id),
            "runner.runtime.fatal",
            BTreeMap::new(),
            Some(runtime_backend_error(err.clone())),
        );
    }
    persist_runtime_diagnostic(
        &app,
        &context,
        &runtime,
        operation_count,
        source_count,
        &lifecycle_events,
        &output,
    );
    let advance_queue = should_advance_queue_after_runtime(&output, fatal_error.as_deref());
    let agent_success = advance_queue;
    let automation_run_id = context.automation_run_id.clone();
    {
        let store = app.state::<ChatStore>();
        crate::automation::automation_complete_agent_turn(
            &app,
            &store,
            automation_run_id,
            &context.turn_id,
            agent_success,
        );
    }
    finish_agent_turn(
        app,
        context.task_id,
        context.backend,
        RUNTIME_CHANNEL_MUTSUKI_CORE.to_string(),
        output.last_session_id,
        agent_success,
        None,
    );
    if let Some(err) = fatal_error {
        return Err(err);
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct RuntimeTurnContext {
    task_id: String,
    turn_id: String,
    backend: String,
    project_cwd: String,
    prompt_length: usize,
    attachment_count: usize,
    workflow_type: Option<&'static str>,
    automation_run_id: Option<String>,
    source_id: String,
    agent_id: String,
    resume_session_id: Option<String>,
    permission: String,
    composer_runtime_workspace_roots: Vec<String>,
}

impl RuntimeTurnContext {
    fn from_invocation(invocation: &RunnerInvocation) -> Self {
        let backend = invocation.composer.backend.clone();
        Self {
            task_id: invocation.task_id.clone(),
            turn_id: invocation.turn_id.clone(),
            backend: backend.clone(),
            project_cwd: invocation.project_cwd.clone(),
            prompt_length: invocation.content.chars().count(),
            attachment_count: invocation.attachments.len(),
            workflow_type: invocation.workflow.as_ref().and_then(workflow_type),
            automation_run_id: automation_run_id_from_workflow(invocation.workflow.as_ref()),
            source_id: format!("lilia:{backend}"),
            agent_id: format!("lilia-{backend}-agent"),
            resume_session_id: invocation.resume_session_id.clone(),
            permission: invocation.composer.permission.clone(),
            composer_runtime_workspace_roots: invocation
                .composer
                .codex_settings
                .runtime_workspace_roots
                .clone()
                .unwrap_or_default(),
        }
    }

    fn from_persisted(persisted: &PersistedRuntimeState) -> Self {
        let context = persisted
            .context_json
            .as_deref()
            .and_then(RuntimeStateContext::from_json_str)
            .unwrap_or_default();
        let backend = persisted.turn.backend.clone();
        Self {
            task_id: persisted.task_id.clone(),
            turn_id: persisted.turn.turn_id.clone(),
            backend: backend.clone(),
            project_cwd: context.project_cwd,
            prompt_length: context.prompt_length,
            attachment_count: context.attachment_count,
            workflow_type: context
                .workflow_type
                .as_deref()
                .and_then(parse_workflow_type),
            automation_run_id: context.automation_run_id,
            source_id: format!("lilia:{backend}"),
            agent_id: format!("lilia-{backend}-agent"),
            resume_session_id: context.resume_session_id,
            permission: if context.permission.trim().is_empty() {
                "ask".to_string()
            } else {
                context.permission
            },
            composer_runtime_workspace_roots: context.composer_runtime_workspace_roots,
        }
    }
}

struct LiliaRuntimeSchedulerDriver<'a> {
    context: &'a RuntimeTurnContext,
    idle_ticks: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStateContext {
    #[serde(default)]
    project_cwd: String,
    #[serde(default)]
    prompt_length: usize,
    #[serde(default)]
    attachment_count: usize,
    #[serde(default)]
    workflow_type: Option<String>,
    #[serde(default)]
    automation_run_id: Option<String>,
    #[serde(default)]
    resume_session_id: Option<String>,
    #[serde(default)]
    permission: String,
    #[serde(default)]
    composer_runtime_workspace_roots: Vec<String>,
}

impl RuntimeStateContext {
    fn from_json_str(value: &str) -> Option<Self> {
        serde_json::from_str(value).ok()
    }
}

impl<R: Runtime> SchedulerDriver<LiliaRuntimeBackend<R>> for LiliaRuntimeSchedulerDriver<'_> {
    fn before_tick(
        &mut self,
        runtime: &mut AgentRuntime,
        _agent_id: &str,
        backend: &mut LiliaRuntimeBackend<R>,
    ) -> RuntimeResult<SchedulerDecision> {
        let incoming = take_runtime_control_deliveries_for_app(
            &backend.app,
            &backend.app.state::<ChatStore>(),
            &self.context.task_id,
            &backend.delivered_control_ids,
        );
        if !incoming.is_empty() {
            let events = incoming
                .iter()
                .map(|delivery| delivery.event.clone())
                .collect::<Vec<_>>();
            backend.control_events.lock().unwrap().extend(events);
            for delivery in &incoming {
                if publish_runtime_control_delivery(runtime, self.context, delivery) {
                    if let Some(id) = delivery.persisted_id {
                        backend.delivered_control_ids.insert(id);
                    }
                }
            }
        }
        Ok(SchedulerDecision::Continue)
    }

    fn after_tick(
        &mut self,
        runtime: &mut AgentRuntime,
        _agent_id: &str,
        backend: &mut LiliaRuntimeBackend<R>,
        outcome: &RuntimeTickOutcome,
    ) -> RuntimeResult<SchedulerDecision> {
        if should_record_missing_operation(backend.step, outcome) {
            let expected = backend
                .step
                .expected_operation()
                .unwrap_or(OP_RUNNER_EXECUTE);
            backend.fatal_error = Some(format!("missing runtime operation: {expected}"));
            record_missing_runner_operation(runtime, self.context, backend.step);
            return Ok(SchedulerDecision::Halt);
        }
        if should_watchdog_idle_tick(backend.step, outcome) {
            self.idle_ticks += 1;
            if self.idle_ticks >= RUNTIME_SCHEDULER_IDLE_TICK_LIMIT {
                let message = format!(
                    "MutsukiCore runtime runner poll idle limit reached after {} ticks",
                    self.idle_ticks
                );
                backend.fatal_error = Some(message);
                record_runner_watchdog_idle(runtime, self.context, self.idle_ticks);
                return Ok(SchedulerDecision::Halt);
            }
        } else {
            self.idle_ticks = 0;
        }
        if backend.step == RuntimeStep::Executed {
            return Ok(SchedulerDecision::Halt);
        }
        thread::sleep(Duration::from_millis(RUNTIME_SCHEDULER_POLL_SLEEP_MS));
        Ok(SchedulerDecision::Continue)
    }
}

struct LiliaRuntimeBackend<R: Runtime> {
    app: AppHandle<R>,
    context: RuntimeTurnContext,
    invocation: Option<RunnerInvocation>,
    runner_session: Option<RunnerSession>,
    step: RuntimeStep,
    operations: Vec<OperationSnapshot>,
    sources: Vec<SourceSnapshot>,
    lifecycle_events: Vec<RunnerLifecycleEvent>,
    control_events: Arc<Mutex<Vec<RuntimeControlEvent>>>,
    delivered_control_ids: HashSet<i64>,
    fatal_error: Option<String>,
    output: Option<RunnerOutput>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeStep {
    PendingExecute,
    RunnerRunning,
    PendingCompletionStop,
    PendingCompletionCheckpoint,
    Executed,
    Completed,
}

impl RuntimeStep {
    fn expects_operation(self) -> bool {
        self.expected_operation().is_some()
    }

    fn expected_operation(self) -> Option<&'static str> {
        match self {
            RuntimeStep::PendingExecute => Some(OP_RUNNER_EXECUTE),
            RuntimeStep::RunnerRunning => Some(OP_RUNNER_POLL),
            RuntimeStep::PendingCompletionStop => Some(OP_STOP_MARKS_READ),
            RuntimeStep::PendingCompletionCheckpoint => Some(OP_SESSION_CHECKPOINT),
            RuntimeStep::Executed | RuntimeStep::Completed => None,
        }
    }
}

fn should_record_missing_operation(step: RuntimeStep, outcome: &RuntimeTickOutcome) -> bool {
    outcome.operation.is_none()
        && outcome.strategy.status == StrategyResultStatus::Continue
        && step.expects_operation()
}

fn should_watchdog_idle_tick(step: RuntimeStep, outcome: &RuntimeTickOutcome) -> bool {
    if step != RuntimeStep::RunnerRunning
        || outcome.strategy.status != StrategyResultStatus::Continue
    {
        return false;
    }
    let Some(BackendPayload::Json(payload)) = &outcome.operation else {
        return false;
    };
    payload.get("operation").and_then(|value| value.as_str()) == Some(OP_RUNNER_POLL)
        && payload.get("status").and_then(|value| value.as_str()) == Some("running")
}

impl<R: Runtime> LiliaRuntimeBackend<R> {
    fn new(
        app: AppHandle<R>,
        context: RuntimeTurnContext,
        invocation: Option<RunnerInvocation>,
        resumed_process_session_id: Option<String>,
    ) -> Result<Self, String> {
        let runner_session = if let Some(process_session_id) = resumed_process_session_id.clone() {
            Some(reattach_runner_session(
                &app,
                context.task_id.clone(),
                context.turn_id.clone(),
                context.backend.clone(),
                process_session_id,
            )?)
        } else {
            None
        };
        let has_runner_session = runner_session.is_some();
        Ok(Self {
            app,
            operations: runtime_operations(),
            sources: runtime_sources(&context),
            context,
            invocation,
            runner_session,
            step: if has_runner_session {
                RuntimeStep::RunnerRunning
            } else {
                RuntimeStep::PendingExecute
            },
            lifecycle_events: Vec::new(),
            control_events: Arc::new(Mutex::new(Vec::new())),
            delivered_control_ids: HashSet::new(),
            fatal_error: None,
            output: None,
        })
    }

    fn take_output(&mut self) -> Option<RunnerOutput> {
        self.output.take()
    }

    fn operation_count(&self) -> usize {
        self.operations.len()
    }

    fn source_count(&self) -> usize {
        self.sources.len()
    }

    fn lifecycle_events(&self) -> &[RunnerLifecycleEvent] {
        &self.lifecycle_events
    }

    fn control_events(&self) -> Arc<Mutex<Vec<RuntimeControlEvent>>> {
        self.control_events.clone()
    }

    fn forward_control_payload(&mut self, payload: &JsonValue) -> Result<bool, String> {
        let Some(session) = self.runner_session.as_mut() else {
            return Ok(false);
        };
        write_runner_stdin_payload(&session.process_session_id, payload.clone())
    }

    fn terminate_runner_session(&mut self) -> Result<bool, String> {
        let Some(session) = self.runner_session.as_ref() else {
            return Ok(false);
        };
        session.terminate()
    }

    fn ack_runtime_control_payload(&mut self, payload: &JsonValue) {
        let Some(id) = persisted_control_ack_id(payload, true) else {
            return;
        };
        match clear_runtime_control_event_ids_for_app(&self.app, &[id]) {
            Ok(()) => {
                self.delivered_control_ids.remove(&id);
            }
            Err(err) => {
                eprintln!("[chat-runtime] ack runtime control event failed: {err}");
            }
        }
    }

    fn ack_runtime_control_payload_if_delivered(&mut self, payload: &JsonValue, delivered: bool) {
        if delivered {
            self.ack_runtime_control_payload(payload);
        }
    }

    fn execute_runner(
        &mut self,
        events: &mut dyn BackendEventSink,
    ) -> RuntimeResult<BackendPayload> {
        let Some(invocation) = self.invocation.take() else {
            return Ok(BackendPayload::Json(json!({
                "status": "wait_input",
                "reason": "runner_invocation_consumed",
            })));
        };
        let app = self.app.clone();
        let mut observer = RuntimeLifecycleObserver {
            context: &self.context,
            lifecycle_events: &mut self.lifecycle_events,
            events,
        };
        match start_runner_session(&app, invocation, &mut observer) {
            Ok(session) => {
                self.runner_session = Some(session);
                self.step = RuntimeStep::RunnerRunning;
                Ok(BackendPayload::Json(json!({
                    "status": "running",
                    "operation": OP_RUNNER_EXECUTE,
                })))
            }
            Err(message) => {
                let error = runtime_backend_error("lilia.runner.execute");
                self.fatal_error = Some(format!("{}: {message}", error.route));
                crate::chat::timeline_sink::persist_and_emit_error_timeline_event(
                    &self.app,
                    &self.context.task_id,
                    &self.context.backend,
                    Some(&self.context.turn_id),
                    message,
                );
                self.output = Some(RunnerOutput::default());
                self.step = RuntimeStep::PendingCompletionStop;
                events.record_backend_event(
                    Some(&self.context.agent_id),
                    "runner.operation.execute_error",
                    BTreeMap::new(),
                    Some(error),
                );
                Ok(BackendPayload::Json(json!({
                    "status": "error",
                    "operation": OP_RUNNER_EXECUTE,
                })))
            }
        }
    }

    fn poll_runner(&mut self, events: &mut dyn BackendEventSink) -> RuntimeResult<BackendPayload> {
        let Some(session) = self.runner_session.as_mut() else {
            return Ok(BackendPayload::Json(json!({
                "status": "wait_input",
                "reason": "runner_session_missing",
            })));
        };
        let mut observer = RuntimeLifecycleObserver {
            context: &self.context,
            lifecycle_events: &mut self.lifecycle_events,
            events,
        };
        match poll_runner_session(&self.app, session, &mut observer) {
            Ok(RunnerSessionPoll::Running) => Ok(BackendPayload::Json(json!({
                "status": "running",
                "operation": OP_RUNNER_POLL,
            }))),
            Ok(RunnerSessionPoll::Completed(output)) => {
                self.runner_session = None;
                let payload = runner_output_payload(&output, &self.lifecycle_events);
                self.output = Some(output);
                self.step = RuntimeStep::PendingCompletionStop;
                Ok(BackendPayload::Json(payload))
            }
            Err(message) => {
                self.runner_session = None;
                self.output = Some(RunnerOutput::default());
                self.step = RuntimeStep::PendingCompletionStop;
                let error = runtime_backend_error("lilia.runner.poll");
                self.fatal_error = Some(format!("{}: {message}", error.route));
                crate::chat::timeline_sink::persist_and_emit_error_timeline_event(
                    &self.app,
                    &self.context.task_id,
                    &self.context.backend,
                    Some(&self.context.turn_id),
                    message,
                );
                events.record_backend_event(
                    Some(&self.context.agent_id),
                    "runner.operation.poll_error",
                    BTreeMap::new(),
                    Some(error),
                );
                Ok(BackendPayload::Json(json!({
                    "status": "error",
                    "operation": OP_RUNNER_POLL,
                })))
            }
        }
    }

    fn mark_stop_marks_read(&mut self) {
        let has_session = self
            .output
            .as_ref()
            .and_then(|output| output.last_session_id.as_ref())
            .is_some();
        self.step = if has_session {
            RuntimeStep::PendingCompletionCheckpoint
        } else {
            RuntimeStep::Executed
        };
    }

    fn mark_session_checkpointed(&mut self) {
        self.step = RuntimeStep::Executed;
    }

    fn persist_session_checkpoint(&mut self, payload: &JsonValue) -> Result<bool, String> {
        let Some(session_id) = payload
            .get("sessionId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(false);
        };
        let backend = payload
            .get("backend")
            .and_then(|value| value.as_str())
            .unwrap_or(&self.context.backend);
        let runtime_channel = payload
            .get("runtimeChannel")
            .and_then(|value| value.as_str())
            .unwrap_or(RUNTIME_CHANNEL_MUTSUKI_CORE);
        let store = self.app.state::<ChatStore>();
        if let Some(lilia_store) = self.app.try_state::<crate::store::LiliaStore>() {
            let conn = lilia_store.conn()?;
            persist_session_checkpoint_state(
                &store,
                Some(&conn),
                &self.context.task_id,
                backend,
                runtime_channel,
                session_id,
            )?;
        } else {
            persist_session_checkpoint_state(
                &store,
                None,
                &self.context.task_id,
                backend,
                runtime_channel,
                session_id,
            )?;
        }
        Ok(true)
    }
}

fn persist_session_checkpoint_state(
    chat_store: &ChatStore,
    conn: Option<&Connection>,
    task_id: &str,
    backend: &str,
    runtime_channel: &str,
    session_id: &str,
) -> Result<bool, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Ok(false);
    }
    chat_store.sdk_sessions.lock().unwrap().insert(
        session_key(runtime_channel, backend, task_id),
        session_id.to_string(),
    );
    if let Some(conn) = conn {
        persist_agent_session_id(conn, task_id, backend, runtime_channel, session_id)?;
    }
    Ok(true)
}

struct RuntimeLifecycleObserver<'a> {
    context: &'a RuntimeTurnContext,
    lifecycle_events: &'a mut Vec<RunnerLifecycleEvent>,
    events: &'a mut dyn BackendEventSink,
}

impl RunnerLifecycleObserver for RuntimeLifecycleObserver<'_> {
    fn record(&mut self, event: RunnerLifecycleEvent) {
        record_runner_lifecycle_event(self.events, self.context, &event);
        self.lifecycle_events.push(event);
    }
}

impl<R: Runtime> StrategyBackend for LiliaRuntimeBackend<R> {
    fn on_awake(&mut self, _agent_id: &str) -> RuntimeResult<()> {
        Ok(())
    }

    fn on_input(&mut self, agent_id: &str, envelope: &Envelope) -> RuntimeResult<StrategyResult> {
        if let Some(control) = runtime_control_delivery_from_envelope(&self.context, envelope) {
            return Ok(control_delivery_decision(agent_id, &control));
        }
        if self.step != RuntimeStep::PendingExecute {
            return Ok(StrategyResult::wait_input());
        }
        Ok(runner_execute_decision(agent_id))
    }

    fn next_step(&mut self, agent_id: &str) -> RuntimeResult<StrategyResult> {
        match self.step {
            RuntimeStep::PendingExecute => Ok(runner_execute_decision(agent_id)),
            RuntimeStep::RunnerRunning => Ok(runner_poll_decision(agent_id)),
            RuntimeStep::PendingCompletionStop => {
                Ok(completion_stop_decision(agent_id, &self.output))
            }
            RuntimeStep::PendingCompletionCheckpoint => Ok(completion_checkpoint_decision(
                agent_id,
                &self.context,
                &self.output,
            )),
            RuntimeStep::Executed | RuntimeStep::Completed => Ok(StrategyResult::wait_input()),
        }
    }

    fn on_stop(&mut self, _agent_id: &str) -> RuntimeResult<()> {
        self.step = RuntimeStep::Completed;
        Ok(())
    }
}

fn runner_execute_decision(agent_id: &str) -> StrategyResult {
    StrategyResult {
        status: StrategyResultStatus::Continue,
        decision: Some(json!({
            "agentId": agent_id,
            "operation": OP_RUNNER_EXECUTE,
        })),
        emitted: Vec::new(),
        error: None,
    }
}

fn runner_poll_decision(agent_id: &str) -> StrategyResult {
    StrategyResult {
        status: StrategyResultStatus::Continue,
        decision: Some(json!({
            "agentId": agent_id,
            "operation": OP_RUNNER_POLL,
        })),
        emitted: Vec::new(),
        error: None,
    }
}

fn completion_stop_decision(agent_id: &str, output: &Option<RunnerOutput>) -> StrategyResult {
    let interrupted = output
        .as_ref()
        .map(|output| output.interrupted)
        .unwrap_or_default();
    let reset = output
        .as_ref()
        .map(|output| output.reset)
        .unwrap_or_default();
    StrategyResult {
        status: StrategyResultStatus::Continue,
        decision: Some(json!({
            "agentId": agent_id,
            "operation": OP_STOP_MARKS_READ,
            "payload": {
                "interrupted": interrupted,
                "reset": reset,
            },
        })),
        emitted: Vec::new(),
        error: None,
    }
}

fn completion_checkpoint_decision(
    agent_id: &str,
    context: &RuntimeTurnContext,
    output: &Option<RunnerOutput>,
) -> StrategyResult {
    let session_id = output
        .as_ref()
        .and_then(|output| output.last_session_id.clone());
    StrategyResult {
        status: StrategyResultStatus::Continue,
        decision: Some(json!({
            "agentId": agent_id,
            "operation": OP_SESSION_CHECKPOINT,
            "payload": {
                "runtimeChannel": RUNTIME_CHANNEL_MUTSUKI_CORE,
                "backend": context.backend.clone(),
                "sessionId": session_id,
            },
        })),
        emitted: Vec::new(),
        error: None,
    }
}

fn control_delivery_decision(agent_id: &str, delivery: &RuntimeControlDelivery) -> StrategyResult {
    let event = &delivery.event;
    let Some(op_id) = control_event_operation(&event.name) else {
        return StrategyResult::wait_input();
    };
    StrategyResult {
        status: StrategyResultStatus::Continue,
        decision: Some(json!({
            "agentId": agent_id,
            "operation": op_id,
            "payload": control_event_operation_payload(delivery),
        })),
        emitted: Vec::new(),
        error: None,
    }
}

impl<R: Runtime> OperationBackend for LiliaRuntimeBackend<R> {
    fn list_operations(&self, _agent_id: &str) -> RuntimeResult<Vec<OperationSnapshot>> {
        Ok(self.operations.clone())
    }

    fn list_sources(&self, _agent_id: &str) -> RuntimeResult<Vec<SourceSnapshot>> {
        Ok(self.sources.clone())
    }

    fn invoke(
        &mut self,
        _agent_id: &str,
        key: &OperationHandlerKey,
        payload: JsonValue,
        events: &mut dyn BackendEventSink,
    ) -> RuntimeResult<BackendPayload> {
        match key.op_id.as_str() {
            OP_RUNNER_EXECUTE => self.execute_runner(events),
            OP_RUNNER_POLL => self.poll_runner(events),
            OP_INTERACTION_RESPONSE | OP_PERMISSION_UPDATE => {
                let stdin_payload = payload.get("payload").cloned().unwrap_or(JsonValue::Null);
                match self.forward_control_payload(&stdin_payload) {
                    Ok(forwarded) => {
                        self.ack_runtime_control_payload_if_delivered(&payload, forwarded);
                        Ok(BackendPayload::Json(json!({
                            "forwarded": forwarded,
                            "opId": key.op_id,
                        })))
                    }
                    Err(message) => {
                        events.record_backend_event(
                            Some(&self.context.agent_id),
                            &format!("runner.control.{}.forward_error", key.op_id),
                            BTreeMap::new(),
                            Some(runtime_backend_error(key.op_id.clone())),
                        );
                        Ok(BackendPayload::Json(json!({
                            "forwarded": false,
                            "opId": key.op_id,
                            "error": message,
                        })))
                    }
                }
            }
            OP_INTERRUPT_REQUEST | OP_RESET_REQUEST => match self.terminate_runner_session() {
                Ok(terminated) => {
                    self.ack_runtime_control_payload_if_delivered(&payload, terminated);
                    Ok(BackendPayload::Json(json!({
                        "terminated": terminated,
                        "opId": key.op_id,
                    })))
                }
                Err(message) => {
                    events.record_backend_event(
                        Some(&self.context.agent_id),
                        &format!("runner.control.{}.terminate_error", key.op_id),
                        BTreeMap::new(),
                        Some(runtime_backend_error(key.op_id.clone())),
                    );
                    Ok(BackendPayload::Json(json!({
                        "terminated": false,
                        "opId": key.op_id,
                        "error": message,
                    })))
                }
            },
            OP_STOP_MARKS_READ => {
                self.mark_stop_marks_read();
                Ok(BackendPayload::Json(json!({
                    "registered": true,
                    "opId": key.op_id,
                    "payload": payload,
                })))
            }
            OP_SESSION_CHECKPOINT => {
                let persisted = match self.persist_session_checkpoint(&payload) {
                    Ok(persisted) => persisted,
                    Err(message) => {
                        self.fatal_error = Some(format!("{}: {message}", key.op_id));
                        events.record_backend_event(
                            Some(&self.context.agent_id),
                            "runner.completion.session_checkpoint.persist_error",
                            BTreeMap::new(),
                            Some(runtime_backend_error(key.op_id.clone())),
                        );
                        false
                    }
                };
                self.mark_session_checkpointed();
                Ok(BackendPayload::Json(json!({
                    "registered": true,
                    "opId": key.op_id,
                    "persisted": persisted,
                    "payload": payload,
                })))
            }
            _ => Err(operation_not_found_failure(&key.op_id)),
        }
    }

    fn operation_status(&self, _agent_id: &str, key: &OperationHandlerKey) -> OperationStatus {
        self.operations
            .iter()
            .find(|operation| operation.key == *key)
            .map(|operation| operation.status.clone())
            .unwrap_or(OperationStatus::NotFound)
    }
}

fn build_turn_envelope(context: &RuntimeTurnContext) -> Envelope {
    Envelope {
        id: format!("{}:input", context.turn_id),
        timestamp: now_millis() as f64 / 1000.0,
        source: SourceRef {
            source_id: context.source_id.clone(),
            kind: "lilia.chat".to_string(),
            metadata: BTreeMap::new(),
        },
        payload_schema_id: "lilia.chat.input".to_string(),
        capabilities_required: Vec::new(),
        payload: json!({
            "backend": context.backend,
            "cwd": context.project_cwd,
            "promptLength": context.prompt_length,
            "attachmentCount": context.attachment_count,
            "workflowType": context.workflow_type,
            "runtimeChannel": RUNTIME_CHANNEL_MUTSUKI_CORE,
            "resumeSessionId": context.resume_session_id,
            "permission": context.permission,
            "composerRuntimeWorkspaceRoots": context.composer_runtime_workspace_roots,
        }),
    }
}

#[cfg(test)]
fn build_control_envelope(context: &RuntimeTurnContext, event: &RuntimeControlEvent) -> Envelope {
    build_control_delivery_envelope(
        context,
        &RuntimeControlDelivery {
            persisted_id: None,
            event: event.clone(),
        },
    )
}

fn build_control_delivery_envelope(
    context: &RuntimeTurnContext,
    delivery: &RuntimeControlDelivery,
) -> Envelope {
    let event = &delivery.event;
    Envelope {
        id: format!(
            "{}:control:{}:{}",
            context.turn_id,
            event.name,
            now_millis()
        ),
        timestamp: now_millis() as f64 / 1000.0,
        source: SourceRef {
            source_id: control_source_id(context),
            kind: "lilia.runner.control".to_string(),
            metadata: BTreeMap::new(),
        },
        payload_schema_id: CONTROL_PAYLOAD_SCHEMA_ID.to_string(),
        capabilities_required: Vec::new(),
        payload: json!({
            "name": event.name,
            "attributes": event.attributes,
            "payload": event.payload,
            "persistedControlId": delivery.persisted_id,
        }),
    }
}

fn control_source_id(context: &RuntimeTurnContext) -> String {
    format!("{}:control", context.source_id)
}

#[cfg(test)]
fn runtime_control_from_envelope(
    context: &RuntimeTurnContext,
    envelope: &Envelope,
) -> Option<RuntimeControlEvent> {
    runtime_control_delivery_from_envelope(context, envelope).map(|delivery| delivery.event)
}

fn runtime_control_delivery_from_envelope(
    context: &RuntimeTurnContext,
    envelope: &Envelope,
) -> Option<RuntimeControlDelivery> {
    if envelope.source.source_id != control_source_id(context)
        || envelope.payload_schema_id != CONTROL_PAYLOAD_SCHEMA_ID
    {
        return None;
    }
    let name = envelope.payload.get("name")?.as_str()?.to_string();
    let attributes = envelope
        .payload
        .get("attributes")
        .and_then(|value| value.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let payload = envelope
        .payload
        .get("payload")
        .cloned()
        .filter(|value| !value.is_null());
    Some(RuntimeControlDelivery {
        persisted_id: envelope
            .payload
            .get("persistedControlId")
            .and_then(|value| value.as_i64()),
        event: RuntimeControlEvent {
            name,
            attributes,
            payload,
        },
    })
}

fn publish_runtime_control_delivery(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
    delivery: &RuntimeControlDelivery,
) -> bool {
    let event = &delivery.event;
    if let Err(err) = runtime.publish(build_control_delivery_envelope(context, delivery)) {
        runtime.record_backend_event(
            Some(&context.agent_id),
            &format!("runner.control.{}.publish_error", event.name),
            BTreeMap::new(),
            Some(err.error().clone()),
        );
        return false;
    }
    true
}

fn control_event_operation_payload(delivery: &RuntimeControlDelivery) -> JsonValue {
    let event = &delivery.event;
    json!({
        "name": event.name,
        "attributes": event.attributes,
        "payload": event.payload,
        "persistedControlId": delivery.persisted_id,
    })
}

fn persisted_control_ack_id(payload: &JsonValue, delivered: bool) -> Option<i64> {
    delivered
        .then(|| {
            payload
                .get("persistedControlId")
                .and_then(|value| value.as_i64())
        })
        .flatten()
}

fn runtime_operations() -> Vec<OperationSnapshot> {
    [
        (
            OP_RUNNER_EXECUTE,
            "Execute runner",
            "Start the local Node agent runner adapter for one Lilia turn.",
            true,
        ),
        (
            OP_RUNNER_POLL,
            "Poll runner",
            "Advance the active local Node runner session until it completes.",
            false,
        ),
        (
            OP_INTERACTION_RESPONSE,
            "Forward interaction response",
            "Register the existing stdin response path for Ask User, tool consent, MCP elicitation, and permission approval.",
            false,
        ),
        (
            OP_PERMISSION_UPDATE,
            "Update runtime permission",
            "Register runtime permission changes forwarded to the active runner.",
            false,
        ),
        (
            OP_INTERRUPT_REQUEST,
            "Request interrupt",
            "Register user interrupt requests forwarded to the active runner.",
            false,
        ),
        (
            OP_RESET_REQUEST,
            "Request reset",
            "Register user reset requests forwarded to the active runner.",
            false,
        ),
        (
            OP_STOP_MARKS_READ,
            "Read stop marks",
            "Register interrupt and reset mark observation for the active turn.",
            false,
        ),
        (
            OP_SESSION_CHECKPOINT,
            "Persist session checkpoint",
            "Register session checkpoint writes isolated by runtime channel.",
            false,
        ),
    ]
    .into_iter()
    .map(|(op_id, name, description, is_tool)| operation_snapshot(op_id, name, description, is_tool))
    .collect()
}

fn operation_snapshot(
    op_id: &str,
    name: &str,
    description: &str,
    is_tool: bool,
) -> OperationSnapshot {
    OperationSnapshot {
        descriptor: OperationDescriptor {
            op_id: op_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            plugin_id: PLUGIN_ID.to_string(),
            func_qualname: format!("{STRATEGY_ID}.{op_id}"),
            parameters_schema: json!({ "type": "object" }),
            return_schema: json!({ "type": "object" }),
            perms_rule_id: None,
            requires_capabilities: Vec::new(),
            is_tool,
        },
        status: OperationStatus::Active,
        key: OperationHandlerKey {
            plugin_id: PLUGIN_ID.to_string(),
            plugin_generation: 0,
            op_id: op_id.to_string(),
            handler_id: format!("{PLUGIN_ID}:{op_id}:0"),
        },
    }
}

fn runtime_sources(context: &RuntimeTurnContext) -> Vec<SourceSnapshot> {
    vec![
        SourceSnapshot {
            descriptor: SourceDescriptor {
                source_id: context.source_id.clone(),
                kind: "lilia.chat".to_string(),
                capabilities: vec!["lilia.chat.turn".to_string()],
                description: "Lilia chat turn source".to_string(),
            },
            plugin_id: PLUGIN_ID.to_string(),
            plugin_generation: 0,
        },
        SourceSnapshot {
            descriptor: SourceDescriptor {
                source_id: format!("{}:runner", context.source_id),
                kind: "lilia.runner".to_string(),
                capabilities: vec![
                    "lilia.runner.events".to_string(),
                    "lilia.runner.interactions".to_string(),
                ],
                description: "Local Node runner adapter source".to_string(),
            },
            plugin_id: PLUGIN_ID.to_string(),
            plugin_generation: 0,
        },
        SourceSnapshot {
            descriptor: SourceDescriptor {
                source_id: control_source_id(context),
                kind: "lilia.runner.control".to_string(),
                capabilities: vec!["lilia.runner.control".to_string()],
                description: "Lilia runtime control source".to_string(),
            },
            plugin_id: PLUGIN_ID.to_string(),
            plugin_generation: 0,
        },
    ]
}

fn register_runtime_resources(runtime: &mut AgentRuntime, context: &RuntimeTurnContext) {
    let refs = runtime_resource_refs(context);
    runtime.resources_mut().register(
        resource_descriptor(
            refs.workspace,
            "lilia.workspace",
            json!({
                "cwd": context.project_cwd,
                "composerRuntimeWorkspaceRoots": context.composer_runtime_workspace_roots,
            }),
        ),
        &context.agent_id,
    );
    runtime.resources_mut().register(
        resource_descriptor(
            refs.permission,
            "lilia.permission",
            json!({
                "backend": context.backend,
                "permission": context.permission,
            }),
        ),
        &context.agent_id,
    );
    runtime.resources_mut().register(
        resource_descriptor(
            refs.extensions,
            "lilia.runtime_extensions",
            json!({
                "mcp": true,
                "runnerProtocol": "stdin-jsonl",
            }),
        ),
        &context.agent_id,
    );
}

struct RuntimeResourceRefs {
    workspace: String,
    permission: String,
    extensions: String,
}

fn runtime_resource_refs(context: &RuntimeTurnContext) -> RuntimeResourceRefs {
    RuntimeResourceRefs {
        workspace: format!("lilia:{}:workspace", context.turn_id),
        permission: format!("lilia:{}:permission", context.turn_id),
        extensions: format!("lilia:{}:extensions", context.turn_id),
    }
}

fn acquire_runtime_resources(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
) -> RuntimeResult<Vec<LeaseToken>> {
    let refs = runtime_resource_refs(context);
    let mut leases = Vec::new();
    for ref_id in [&refs.workspace, &refs.permission, &refs.extensions] {
        leases.push(runtime.resources_mut().acquire(ref_id, &context.agent_id)?);
    }
    Ok(leases)
}

fn release_runtime_resources(runtime: &mut AgentRuntime, leases: Vec<LeaseToken>) {
    for lease in leases {
        if let Err(err) = runtime.resources_mut().release(&lease) {
            runtime.record_backend_event(
                None,
                "runner.resource.release.error",
                BTreeMap::new(),
                Some(err.error().clone()),
            );
        }
    }
}

fn resource_descriptor(ref_id: String, kind: &str, attributes: JsonValue) -> RefDescriptor {
    RefDescriptor {
        ref_id,
        kind: kind.to_string(),
        schema_id_target: kind.to_string(),
        schema_version_target: "1.0.0".to_string(),
        attributes: scalar_attributes(attributes),
        lineage: Vec::new(),
    }
}

fn scalar_attributes(value: JsonValue) -> BTreeMap<String, ScalarValue> {
    let mut attributes = BTreeMap::new();
    let JsonValue::Object(map) = value else {
        return attributes;
    };
    for (key, value) in map {
        if let Some(scalar) = json_to_scalar(value) {
            attributes.insert(key, scalar);
        }
    }
    attributes
}

fn json_to_scalar(value: JsonValue) -> Option<ScalarValue> {
    match value {
        JsonValue::String(value) => Some(ScalarValue::String(value)),
        JsonValue::Number(value) => value
            .as_i64()
            .map(ScalarValue::Int)
            .or_else(|| value.as_f64().map(ScalarValue::Float)),
        JsonValue::Bool(value) => Some(ScalarValue::Bool(value)),
        JsonValue::Array(values) => Some(ScalarValue::String(JsonValue::Array(values).to_string())),
        JsonValue::Object(values) => {
            Some(ScalarValue::String(JsonValue::Object(values).to_string()))
        }
        JsonValue::Null => None,
    }
}

fn runner_output_payload(
    output: &RunnerOutput,
    lifecycle_events: &[RunnerLifecycleEvent],
) -> JsonValue {
    json!({
        "runnerSessionId": output.last_session_id,
        "interrupted": output.interrupted,
        "reset": output.reset,
        "lifecycleEvents": lifecycle_events,
    })
}

fn should_advance_queue_after_runtime(output: &RunnerOutput, fatal_error: Option<&str>) -> bool {
    fatal_error.is_none() && !output.interrupted && !output.reset
}

#[cfg(test)]
fn drive_runtime_control_events<B: OperationBackend>(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
    backend: &mut B,
    control_events: &[RuntimeControlEvent],
) -> bool {
    let mut ok = true;
    for event in control_events {
        let Some(op_id) = control_event_operation(&event.name) else {
            ok = false;
            let event_name = format!("runner.control.{}.unhandled", event.name);
            runtime.record_backend_event(
                Some(&context.agent_id),
                &event_name,
                event
                    .attributes
                    .iter()
                    .map(|(key, value)| (key.clone(), ScalarValue::String(value.clone())))
                    .collect(),
                Some(runtime_backend_error(format!(
                    "lilia.runner.control.{}",
                    event.name
                ))),
            );
            continue;
        };
        if let Err(err) = runtime.invoke_operation(
            &context.agent_id,
            op_id,
            json!({
                "name": event.name,
                "attributes": event.attributes.clone(),
            }),
            backend,
        ) {
            ok = false;
            let event_name = format!("runner.control.{}.invoke_error", event.name);
            runtime.record_backend_event(
                Some(&context.agent_id),
                &event_name,
                BTreeMap::from([(
                    "operation".to_string(),
                    ScalarValue::String(op_id.to_string()),
                )]),
                Some(err.error().clone()),
            );
        }
    }
    ok
}

#[cfg(test)]
fn drive_runtime_completion_operations<B: OperationBackend>(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
    backend: &mut B,
    output: &RunnerOutput,
) -> bool {
    let mut ok = true;
    if let Err(err) = runtime.invoke_operation(
        &context.agent_id,
        OP_STOP_MARKS_READ,
        json!({
            "interrupted": output.interrupted,
            "reset": output.reset,
        }),
        backend,
    ) {
        ok = false;
        runtime.record_backend_event(
            Some(&context.agent_id),
            "runner.completion.stop_marks_read.invoke_error",
            BTreeMap::new(),
            Some(err.error().clone()),
        );
    }
    if let Some(session_id) = &output.last_session_id {
        if let Err(err) = runtime.invoke_operation(
            &context.agent_id,
            OP_SESSION_CHECKPOINT,
            json!({
                "runtimeChannel": RUNTIME_CHANNEL_MUTSUKI_CORE,
                "backend": context.backend.clone(),
                "sessionId": session_id,
            }),
            backend,
        ) {
            ok = false;
            runtime.record_backend_event(
                Some(&context.agent_id),
                "runner.completion.session_checkpoint.invoke_error",
                BTreeMap::new(),
                Some(err.error().clone()),
            );
        }
    }
    ok
}

fn control_event_operation(name: &str) -> Option<&'static str> {
    match name {
        "interaction_response" => Some(OP_INTERACTION_RESPONSE),
        "permission_update" => Some(OP_PERMISSION_UPDATE),
        "interrupt_requested" => Some(OP_INTERRUPT_REQUEST),
        "reset_requested" => Some(OP_RESET_REQUEST),
        _ => None,
    }
}

fn operation_not_found_failure(op_id: &str) -> RuntimeFailure {
    RuntimeFailure::new(RuntimeError::new(
        mutsuki_runtime_contracts::ERR_OPERATION_NOT_FOUND,
        PLUGIN_ID,
        format!("{STRATEGY_ID}.{op_id}"),
    ))
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
        ChatWorkflow::Automation { .. } => Some("automation"),
    }
}

fn automation_run_id_from_workflow(workflow: Option<&ChatWorkflow>) -> Option<String> {
    match workflow {
        Some(ChatWorkflow::Automation { automation_run_id }) => Some(automation_run_id.clone()),
        _ => None,
    }
}

fn parse_workflow_type(value: &str) -> Option<&'static str> {
    match value {
        "codex_review" => Some("codex_review"),
        "codex_fix_suggestion" => Some("codex_fix_suggestion"),
        "codex_batch_apply" => Some("codex_batch_apply"),
        "codex_goal" => Some("codex_goal"),
        "codex_compact" => Some("codex_compact"),
        "codex_background_terminals_clean" => Some("codex_background_terminals_clean"),
        "codex_memory_mode" => Some("codex_memory_mode"),
        "codex_memory_reset" => Some("codex_memory_reset"),
        "codex_thread_fork" => Some("codex_thread_fork"),
        "codex_config_diagnostics" => Some("codex_config_diagnostics"),
        "automation" => Some("automation"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> RuntimeTurnContext {
        RuntimeTurnContext {
            task_id: "task-1".to_string(),
            turn_id: "turn-1".to_string(),
            backend: "codex".to_string(),
            project_cwd: "C:/repo".to_string(),
            prompt_length: 12,
            attachment_count: 2,
            workflow_type: Some("codex_goal"),
            automation_run_id: None,
            source_id: "lilia:codex".to_string(),
            agent_id: "lilia-codex-agent".to_string(),
            resume_session_id: Some("thread-1".to_string()),
            permission: "ask".to_string(),
            composer_runtime_workspace_roots: vec!["C:/repo".to_string(), "D:/shared".to_string()],
        }
    }

    #[test]
    fn runtime_operations_register_local_boundaries() {
        let operations = runtime_operations();
        let op_ids: Vec<_> = operations
            .iter()
            .map(|operation| operation.descriptor.op_id.as_str())
            .collect();

        assert_eq!(
            op_ids,
            vec![
                OP_RUNNER_EXECUTE,
                OP_RUNNER_POLL,
                OP_INTERACTION_RESPONSE,
                OP_PERMISSION_UPDATE,
                OP_INTERRUPT_REQUEST,
                OP_RESET_REQUEST,
                OP_STOP_MARKS_READ,
                OP_SESSION_CHECKPOINT
            ]
        );
        assert!(operations
            .iter()
            .all(|op| op.status == OperationStatus::Active));
    }

    #[test]
    fn runtime_sources_expose_chat_and_runner_adapter() {
        let sources = runtime_sources(&context());

        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0].descriptor.source_id, "lilia:codex");
        assert_eq!(sources[0].descriptor.kind, "lilia.chat");
        assert_eq!(sources[1].descriptor.kind, "lilia.runner");
        assert!(sources[1]
            .descriptor
            .capabilities
            .contains(&"lilia.runner.interactions".to_string()));
        assert_eq!(sources[2].descriptor.source_id, "lilia:codex:control");
        assert_eq!(sources[2].descriptor.kind, "lilia.runner.control");
        assert!(sources[2]
            .descriptor
            .capabilities
            .contains(&"lilia.runner.control".to_string()));
    }

    #[test]
    fn runtime_resources_capture_workspace_permission_and_extensions() {
        let context = context();
        let mut runtime = AgentRuntime::new();

        register_runtime_resources(&mut runtime, &context);
        let records = runtime.resources().list_records();

        assert_eq!(records.len(), 3);
        assert!(records
            .iter()
            .any(|record| record.descriptor.kind == "lilia.workspace"));
        assert!(records
            .iter()
            .any(|record| record.descriptor.kind == "lilia.permission"));
        assert!(records
            .iter()
            .any(|record| record.descriptor.kind == "lilia.runtime_extensions"));
    }

    #[test]
    fn persisted_runtime_context_restores_original_runner_context() {
        let persisted = PersistedRuntimeState {
            task_id: "task-restore".to_string(),
            turn: crate::chat::state::RunningTurn {
                turn_id: "turn-restore".to_string(),
                backend: "codex".to_string(),
                runtime_channel: RUNTIME_CHANNEL_MUTSUKI_CORE.to_string(),
            },
            phase: "running".to_string(),
            process_session_id: Some("process-1".to_string()),
            runtime_epoch: "epoch-1".to_string(),
            context_json: Some(
                json!({
                    "projectCwd": "D:/repo",
                    "promptLength": 23,
                    "attachmentCount": 3,
                    "workflowType": "codex_goal",
                    "resumeSessionId": "thread-restore",
                    "permission": "workspace-write",
                    "composerRuntimeWorkspaceRoots": ["D:/repo", "E:/shared"],
                })
                .to_string(),
            ),
        };

        let context = RuntimeTurnContext::from_persisted(&persisted);

        assert_eq!(context.task_id, "task-restore");
        assert_eq!(context.turn_id, "turn-restore");
        assert_eq!(context.project_cwd, "D:/repo");
        assert_eq!(context.prompt_length, 23);
        assert_eq!(context.attachment_count, 3);
        assert_eq!(context.workflow_type, Some("codex_goal"));
        assert_eq!(context.resume_session_id.as_deref(), Some("thread-restore"));
        assert_eq!(context.permission, "workspace-write");
        assert_eq!(
            context.composer_runtime_workspace_roots,
            vec!["D:/repo".to_string(), "E:/shared".to_string()]
        );
    }

    #[test]
    fn persisted_runtime_context_falls_back_to_reattach_only_defaults() {
        let persisted = PersistedRuntimeState {
            task_id: "task-restore".to_string(),
            turn: crate::chat::state::RunningTurn {
                turn_id: "turn-restore".to_string(),
                backend: "codex".to_string(),
                runtime_channel: RUNTIME_CHANNEL_MUTSUKI_CORE.to_string(),
            },
            phase: "running".to_string(),
            process_session_id: Some("process-1".to_string()),
            runtime_epoch: "epoch-1".to_string(),
            context_json: Some("{invalid".to_string()),
        };

        let context = RuntimeTurnContext::from_persisted(&persisted);

        assert_eq!(context.project_cwd, "");
        assert_eq!(context.prompt_length, 0);
        assert_eq!(context.workflow_type, None);
        assert_eq!(context.resume_session_id, None);
        assert_eq!(context.permission, "ask");
        assert!(context.composer_runtime_workspace_roots.is_empty());
    }

    #[test]
    fn runtime_resource_leases_capture_execution_window() {
        let context = context();
        let mut runtime = AgentRuntime::new();

        register_runtime_resources(&mut runtime, &context);
        let leases = acquire_runtime_resources(&mut runtime, &context).unwrap();

        assert_eq!(leases.len(), 3);
        assert!(runtime
            .resources()
            .list_records()
            .iter()
            .all(|record| record.lease_count == 1));

        release_runtime_resources(&mut runtime, leases);

        assert!(runtime
            .resources()
            .list_records()
            .iter()
            .all(|record| record.lease_count == 0));
        let events = runtime.events();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.name == "resource.acquire")
                .count(),
            3
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.name == "resource.release")
                .count(),
            3
        );
    }

    #[test]
    fn runner_lifecycle_events_are_recorded_as_runtime_backend_events() {
        let context = context();
        let mut runtime = AgentRuntime::new();
        let lifecycle_events = vec![
            RunnerLifecycleEvent {
                stage: "process_spawned",
                detail: json!({ "queuedCount": 0 }),
            },
            RunnerLifecycleEvent {
                stage: "runner_error_event",
                detail: json!({ "message": "failed" }),
            },
        ];

        record_runner_lifecycle_events(&mut runtime, &context, &lifecycle_events);
        let events = runtime.events();

        let spawned = events
            .iter()
            .find(|event| event.name == "runner.lifecycle.process_spawned")
            .unwrap();
        assert_eq!(
            spawned.kind,
            mutsuki_runtime_contracts::RuntimeEventKind::Backend
        );
        assert_eq!(spawned.agent_id.as_deref(), Some("lilia-codex-agent"));
        assert_eq!(
            spawned.attributes.get("stage"),
            Some(&ScalarValue::String("process_spawned".to_string()))
        );
        assert_eq!(
            spawned.attributes.get("queuedCount"),
            Some(&ScalarValue::Int(0))
        );
        assert!(spawned.error.is_none());

        let failed = events
            .iter()
            .find(|event| event.name == "runner.lifecycle.runner_error_event")
            .unwrap();
        assert_eq!(
            failed.kind,
            mutsuki_runtime_contracts::RuntimeEventKind::Backend
        );
        assert_eq!(
            failed.error.as_ref().map(|error| error.route.as_str()),
            Some("lilia.runner.lifecycle.runner_error_event")
        );
    }

    #[test]
    fn missing_runner_operation_records_runtime_error() {
        let context = context();
        let mut runtime = AgentRuntime::new();

        record_missing_runner_operation(&mut runtime, &context, RuntimeStep::RunnerRunning);

        let events = runtime.events();
        let event = events
            .iter()
            .find(|event| event.name == "runner.operation.missing")
            .unwrap();
        assert_eq!(
            event.kind,
            mutsuki_runtime_contracts::RuntimeEventKind::Backend
        );
        assert_eq!(event.agent_id.as_deref(), Some("lilia-codex-agent"));
        assert_eq!(
            event.attributes.get("expectedOperation"),
            Some(&ScalarValue::String(OP_RUNNER_POLL.to_string()))
        );
        assert_eq!(
            event.error.as_ref().map(|error| error.route.as_str()),
            Some(OP_RUNNER_POLL)
        );
    }

    #[test]
    fn missing_operation_detection_ignores_wait_input_ticks() {
        let wait_input = RuntimeTickOutcome {
            strategy: StrategyResult::wait_input(),
            operation: None,
        };
        let continue_without_op = RuntimeTickOutcome {
            strategy: StrategyResult {
                status: StrategyResultStatus::Continue,
                decision: None,
                emitted: Vec::new(),
                error: None,
            },
            operation: None,
        };

        assert!(!should_record_missing_operation(
            RuntimeStep::RunnerRunning,
            &wait_input
        ));
        assert!(should_record_missing_operation(
            RuntimeStep::RunnerRunning,
            &continue_without_op
        ));
        assert!(!should_record_missing_operation(
            RuntimeStep::Executed,
            &continue_without_op
        ));
    }

    #[test]
    fn watchdog_idle_detection_only_tracks_stuck_runner_poll() {
        let wait_input = RuntimeTickOutcome {
            strategy: StrategyResult::wait_input(),
            operation: None,
        };
        let continue_without_op = RuntimeTickOutcome {
            strategy: StrategyResult {
                status: StrategyResultStatus::Continue,
                decision: None,
                emitted: Vec::new(),
                error: None,
            },
            operation: None,
        };
        let continue_with_op = RuntimeTickOutcome {
            strategy: StrategyResult {
                status: StrategyResultStatus::Continue,
                decision: None,
                emitted: Vec::new(),
                error: None,
            },
            operation: Some(BackendPayload::Json(json!({
                "status": "running",
                "operation": OP_RUNNER_POLL,
            }))),
        };

        assert!(should_watchdog_idle_tick(
            RuntimeStep::RunnerRunning,
            &continue_with_op
        ));
        assert!(!should_watchdog_idle_tick(
            RuntimeStep::RunnerRunning,
            &continue_without_op
        ));
        assert!(!should_watchdog_idle_tick(
            RuntimeStep::RunnerRunning,
            &wait_input
        ));
        assert!(!should_watchdog_idle_tick(
            RuntimeStep::PendingCompletionStop,
            &continue_with_op
        ));
        assert!(!should_watchdog_idle_tick(
            RuntimeStep::RunnerRunning,
            &continue_without_op
        ));
    }

    #[test]
    fn watchdog_idle_event_is_visible_runtime_error() {
        let context = context();
        let mut runtime = AgentRuntime::new();

        record_runner_watchdog_idle(&mut runtime, &context, 7);

        let event = runtime
            .events()
            .into_iter()
            .find(|event| event.name == "runner.runtime.watchdog_idle")
            .unwrap();
        assert_eq!(event.agent_id.as_deref(), Some("lilia-codex-agent"));
        assert_eq!(
            event.attributes.get("idleTicks"),
            Some(&ScalarValue::Int(7))
        );
        assert_eq!(
            event.error.as_ref().map(|error| error.route.as_str()),
            Some("lilia.runner.poll.watchdog_idle")
        );
    }

    #[test]
    fn runtime_control_events_are_recorded_as_backend_events() {
        let context = context();
        let mut runtime = AgentRuntime::new();
        let control_events = vec![RuntimeControlEvent {
            name: "interaction_response".to_string(),
            attributes: BTreeMap::from([
                ("requestId".to_string(), "ask-1".to_string()),
                ("kind".to_string(), "plan_approval".to_string()),
            ]),
            payload: Some(json!({
                "type": "interaction_response",
                "id": "ask-1",
            })),
        }];

        record_runtime_control_events(&mut runtime, &context, &control_events);

        let events = runtime.events();
        let event = events
            .iter()
            .find(|event| event.name == "runner.control.interaction_response")
            .unwrap();
        assert_eq!(
            event.kind,
            mutsuki_runtime_contracts::RuntimeEventKind::Backend
        );
        assert_eq!(event.agent_id.as_deref(), Some("lilia-codex-agent"));
        assert_eq!(
            event.attributes.get("requestId"),
            Some(&ScalarValue::String("ask-1".to_string()))
        );
        assert_eq!(
            event.attributes.get("kind"),
            Some(&ScalarValue::String("plan_approval".to_string()))
        );
        assert!(event.error.is_none());
    }

    #[test]
    fn runtime_control_event_with_stdin_error_records_runtime_error() {
        let context = context();
        let mut runtime = AgentRuntime::new();
        let control_events = vec![RuntimeControlEvent {
            name: "permission_update".to_string(),
            attributes: BTreeMap::from([
                ("permission".to_string(), "readonly".to_string()),
                ("stdinError".to_string(), "pipe closed".to_string()),
            ]),
            payload: None,
        }];

        record_runtime_control_events(&mut runtime, &context, &control_events);

        let event = runtime
            .events()
            .into_iter()
            .find(|event| event.name == "runner.control.permission_update")
            .unwrap();
        assert_eq!(
            event.error.as_ref().map(|error| error.code.as_str()),
            Some(mutsuki_runtime_contracts::ERR_RUNTIME_BACKEND_FAILED)
        );
    }

    #[derive(Default)]
    struct ControlBackend {
        control_context: Option<RuntimeTurnContext>,
        invocations: Vec<(String, JsonValue)>,
    }

    impl StrategyBackend for ControlBackend {
        fn on_awake(&mut self, _agent_id: &str) -> RuntimeResult<()> {
            Ok(())
        }

        fn on_input(
            &mut self,
            agent_id: &str,
            envelope: &Envelope,
        ) -> RuntimeResult<StrategyResult> {
            if let Some(context) = &self.control_context {
                if let Some(control) = runtime_control_delivery_from_envelope(context, envelope) {
                    return Ok(control_delivery_decision(agent_id, &control));
                }
            }
            Ok(StrategyResult::wait_input())
        }

        fn next_step(&mut self, _agent_id: &str) -> RuntimeResult<StrategyResult> {
            Ok(StrategyResult::wait_input())
        }

        fn on_stop(&mut self, _agent_id: &str) -> RuntimeResult<()> {
            Ok(())
        }
    }

    impl OperationBackend for ControlBackend {
        fn list_operations(&self, _agent_id: &str) -> RuntimeResult<Vec<OperationSnapshot>> {
            Ok(runtime_operations())
        }

        fn list_sources(&self, _agent_id: &str) -> RuntimeResult<Vec<SourceSnapshot>> {
            Ok(self
                .control_context
                .as_ref()
                .map(runtime_sources)
                .unwrap_or_default())
        }

        fn invoke(
            &mut self,
            _agent_id: &str,
            key: &OperationHandlerKey,
            payload: JsonValue,
            _events: &mut dyn BackendEventSink,
        ) -> RuntimeResult<BackendPayload> {
            self.invocations.push((key.op_id.clone(), payload));
            Ok(BackendPayload::Json(json!({ "ok": true })))
        }

        fn operation_status(&self, _agent_id: &str, _key: &OperationHandlerKey) -> OperationStatus {
            OperationStatus::Active
        }
    }

    fn started_control_runtime(context: &RuntimeTurnContext) -> (AgentRuntime, ControlBackend) {
        let mut runtime = AgentRuntime::new();
        let mut backend = ControlBackend::default();
        runtime
            .register_agent(AgentSpec {
                agent_id: context.agent_id.clone(),
                owner: Some("lilia".to_string()),
                priority: 0,
                participation: AgentParticipation::PrimaryCandidate,
                accepts: Vec::new(),
                strategy_id: STRATEGY_ID.to_string(),
                side_effect_policy: SideEffectPolicy::AllowExternal,
            })
            .unwrap();
        runtime
            .start_agent(&context.agent_id, &mut backend)
            .unwrap();
        (runtime, backend)
    }

    fn started_control_runtime_for_envelopes(
        context: &RuntimeTurnContext,
    ) -> (AgentRuntime, ControlBackend) {
        let mut runtime = AgentRuntime::new();
        let mut backend = ControlBackend {
            control_context: Some(context.clone()),
            ..ControlBackend::default()
        };
        runtime
            .register_agent(AgentSpec {
                agent_id: context.agent_id.clone(),
                owner: Some("lilia".to_string()),
                priority: 0,
                participation: AgentParticipation::PrimaryCandidate,
                accepts: vec![ScopeRuleSpec::BySourceId {
                    source_id: control_source_id(context),
                }],
                strategy_id: STRATEGY_ID.to_string(),
                side_effect_policy: SideEffectPolicy::AllowExternal,
            })
            .unwrap();
        runtime
            .start_agent(&context.agent_id, &mut backend)
            .unwrap();
        (runtime, backend)
    }

    #[test]
    fn runtime_tick_drives_control_envelope_operation_once() {
        let context = context();
        let (mut runtime, mut backend) = started_control_runtime_for_envelopes(&context);
        let control = RuntimeControlEvent {
            name: "permission_update".to_string(),
            attributes: BTreeMap::from([("permission".to_string(), "readonly".to_string())]),
            payload: Some(json!({
                "type": "settings_update",
                "permission": "readonly"
            })),
        };

        assert_eq!(
            runtime
                .publish(build_control_envelope(&context, &control))
                .unwrap(),
            vec![context.agent_id.clone()]
        );
        let outcome = runtime
            .tick_once_and_drive(&context.agent_id, &mut backend)
            .unwrap();

        assert_eq!(outcome.strategy.status, StrategyResultStatus::Continue);
        assert_eq!(backend.invocations.len(), 1);
        assert_eq!(backend.invocations[0].0, OP_PERMISSION_UPDATE);
        assert_eq!(
            backend.invocations[0].1["payload"]["permission"],
            json!("readonly")
        );
        assert_eq!(
            runtime
                .events()
                .iter()
                .filter(|event| event.name == "operation.invoke")
                .count(),
            1
        );
    }

    #[test]
    fn persisted_control_delivery_id_reaches_operation_payload() {
        let context = context();
        let (mut runtime, mut backend) = started_control_runtime_for_envelopes(&context);
        let delivery = RuntimeControlDelivery {
            persisted_id: Some(42),
            event: RuntimeControlEvent {
                name: "permission_update".to_string(),
                attributes: BTreeMap::from([("permission".to_string(), "readonly".to_string())]),
                payload: Some(json!({
                    "type": "settings_update",
                    "permission": "readonly"
                })),
            },
        };

        assert_eq!(
            runtime
                .publish(build_control_delivery_envelope(&context, &delivery))
                .unwrap(),
            vec![context.agent_id.clone()]
        );
        runtime
            .tick_once_and_drive(&context.agent_id, &mut backend)
            .unwrap();

        assert_eq!(backend.invocations.len(), 1);
        assert_eq!(backend.invocations[0].0, OP_PERMISSION_UPDATE);
        assert_eq!(backend.invocations[0].1["persistedControlId"], json!(42));
        assert_eq!(
            backend.invocations[0].1["payload"]["permission"],
            json!("readonly")
        );
    }

    #[test]
    fn persisted_control_ack_id_requires_successful_delivery() {
        let payload = json!({ "persistedControlId": 42 });

        assert_eq!(persisted_control_ack_id(&payload, true), Some(42));
        assert_eq!(persisted_control_ack_id(&payload, false), None);
        assert_eq!(
            persisted_control_ack_id(&json!({ "persistedControlId": null }), true),
            None
        );
    }

    #[test]
    fn unknown_control_envelope_waits_without_missing_operation() {
        let context = context();
        let (mut runtime, mut backend) = started_control_runtime_for_envelopes(&context);
        let control = RuntimeControlEvent {
            name: "unknown_control".to_string(),
            attributes: BTreeMap::new(),
            payload: None,
        };

        runtime
            .publish(build_control_envelope(&context, &control))
            .unwrap();
        let outcome = runtime
            .tick_once_and_drive(&context.agent_id, &mut backend)
            .unwrap();

        assert_eq!(outcome.strategy.status, StrategyResultStatus::WaitInput);
        assert!(outcome.operation.is_none());
        assert!(!should_record_missing_operation(
            RuntimeStep::RunnerRunning,
            &outcome
        ));
        assert!(backend.invocations.is_empty());
    }

    #[test]
    fn runtime_control_events_drive_registered_operations() {
        let context = context();
        let (mut runtime, mut backend) = started_control_runtime(&context);
        let control_events = vec![
            RuntimeControlEvent {
                name: "interaction_response".to_string(),
                attributes: BTreeMap::from([("requestId".to_string(), "ask-1".to_string())]),
                payload: Some(json!({ "type": "interaction_response" })),
            },
            RuntimeControlEvent {
                name: "reset_requested".to_string(),
                attributes: BTreeMap::from([("mode".to_string(), "session_reset".to_string())]),
                payload: None,
            },
        ];

        assert!(drive_runtime_control_events(
            &mut runtime,
            &context,
            &mut backend,
            &control_events
        ));

        assert_eq!(
            backend
                .invocations
                .iter()
                .map(|(op_id, _)| op_id.as_str())
                .collect::<Vec<_>>(),
            vec![OP_INTERACTION_RESPONSE, OP_RESET_REQUEST]
        );
        assert_eq!(
            backend.invocations[0].1["attributes"]["requestId"],
            json!("ask-1")
        );
        assert_eq!(
            runtime
                .events()
                .iter()
                .filter(|event| event.name == "operation.invoke")
                .count(),
            2
        );
    }

    #[test]
    fn runtime_completion_operations_drive_stop_marks_and_checkpoint() {
        let context = context();
        let (mut runtime, mut backend) = started_control_runtime(&context);
        let output = RunnerOutput {
            last_session_id: Some("thread-2".to_string()),
            interrupted: true,
            reset: false,
        };

        assert!(drive_runtime_completion_operations(
            &mut runtime,
            &context,
            &mut backend,
            &output
        ));

        assert_eq!(
            backend
                .invocations
                .iter()
                .map(|(op_id, _)| op_id.as_str())
                .collect::<Vec<_>>(),
            vec![OP_STOP_MARKS_READ, OP_SESSION_CHECKPOINT]
        );
        assert_eq!(backend.invocations[0].1["interrupted"], json!(true));
        assert_eq!(backend.invocations[0].1["reset"], json!(false));
        assert_eq!(backend.invocations[1].1["sessionId"], json!("thread-2"));
        assert_eq!(
            runtime
                .events()
                .iter()
                .filter(|event| event.name == "operation.invoke")
                .count(),
            2
        );
    }

    #[test]
    fn completion_decisions_request_runtime_operations() {
        let context = context();
        let output = Some(RunnerOutput {
            last_session_id: Some("thread-2".to_string()),
            interrupted: true,
            reset: false,
        });

        let stop = completion_stop_decision(&context.agent_id, &output)
            .decision
            .unwrap();
        let checkpoint = completion_checkpoint_decision(&context.agent_id, &context, &output)
            .decision
            .unwrap();

        assert_eq!(stop["operation"], OP_STOP_MARKS_READ);
        assert_eq!(stop["payload"]["interrupted"], json!(true));
        assert_eq!(stop["payload"]["reset"], json!(false));
        assert_eq!(checkpoint["operation"], OP_SESSION_CHECKPOINT);
        assert_eq!(
            checkpoint["payload"]["runtimeChannel"],
            RUNTIME_CHANNEL_MUTSUKI_CORE
        );
        assert_eq!(checkpoint["payload"]["backend"], "codex");
        assert_eq!(checkpoint["payload"]["sessionId"], "thread-2");
    }

    #[test]
    fn session_checkpoint_persists_resume_state() {
        let store = ChatStore::default();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE task_agent_sessions (
              task_id         TEXT NOT NULL,
              backend         TEXT NOT NULL CHECK (backend IN ('claude','codex')),
              runtime_channel TEXT NOT NULL DEFAULT 'builtin'
                              CHECK (runtime_channel IN ('builtin','mutsuki_core')),
              session_id      TEXT NOT NULL,
              updated_at      INTEGER NOT NULL,
              PRIMARY KEY (task_id, backend, runtime_channel)
            );
            "#,
        )
        .unwrap();

        assert!(persist_session_checkpoint_state(
            &store,
            Some(&conn),
            "task-1",
            "codex",
            RUNTIME_CHANNEL_MUTSUKI_CORE,
            "thread-2",
        )
        .unwrap());

        assert_eq!(
            store
                .sdk_sessions
                .lock()
                .unwrap()
                .get(&session_key(
                    RUNTIME_CHANNEL_MUTSUKI_CORE,
                    "codex",
                    "task-1"
                ))
                .map(String::as_str),
            Some("thread-2")
        );
        assert_eq!(
            crate::chat::state::load_persisted_resume_session_id(
                &conn,
                "task-1",
                "codex",
                RUNTIME_CHANNEL_MUTSUKI_CORE,
            ),
            Some("thread-2".to_string())
        );
    }

    #[test]
    fn unknown_runtime_control_event_records_runtime_error() {
        let context = context();
        let (mut runtime, mut backend) = started_control_runtime(&context);
        let control_events = vec![RuntimeControlEvent {
            name: "unknown_control".to_string(),
            attributes: BTreeMap::new(),
            payload: None,
        }];

        assert!(!drive_runtime_control_events(
            &mut runtime,
            &context,
            &mut backend,
            &control_events
        ));
        assert!(backend.invocations.is_empty());
        let events = runtime.events();
        let event = events
            .iter()
            .find(|event| event.name == "runner.control.unknown_control.unhandled")
            .unwrap();
        assert_eq!(
            event.kind,
            mutsuki_runtime_contracts::RuntimeEventKind::Backend
        );
        assert_eq!(
            event.error.as_ref().map(|error| error.code.as_str()),
            Some(mutsuki_runtime_contracts::ERR_RUNTIME_BACKEND_FAILED)
        );
    }

    #[test]
    fn unknown_runtime_operation_returns_not_found() {
        let failure = operation_not_found_failure("missing.operation");

        assert_eq!(
            failure.error().code,
            mutsuki_runtime_contracts::ERR_OPERATION_NOT_FOUND
        );
        assert_eq!(failure.error().source, PLUGIN_ID);
    }

    #[test]
    fn diagnostic_payload_contains_stable_mutsuki_core_fields() {
        let context = context();
        let output = RunnerOutput {
            last_session_id: Some("thread-2".to_string()),
            interrupted: false,
            reset: false,
        };
        let lifecycle_events = vec![RunnerLifecycleEvent {
            stage: "process_spawned",
            detail: json!({}),
        }];
        let payload = diagnostic_payload(&context, 5, 2, 3, &[], &[], &lifecycle_events, &output);

        assert_eq!(payload["runtimeChannel"], RUNTIME_CHANNEL_MUTSUKI_CORE);
        assert_eq!(payload["agentId"], "lilia-codex-agent");
        assert_eq!(payload["operationCount"], 5);
        assert_eq!(payload["sourceCount"], 2);
        assert_eq!(payload["eventCount"], 3);
        assert_eq!(payload["events"], json!([]));
        assert_eq!(payload["traceSpans"], json!([]));
        assert_eq!(
            payload["lifecycleEvents"],
            json!([{ "stage": "process_spawned", "detail": {} }])
        );
        assert_eq!(payload["runnerSessionId"], "thread-2");
        assert_eq!(payload["interrupted"], false);
        assert_eq!(payload["reset"], false);
    }

    #[test]
    fn diagnostic_status_tracks_cancelled_and_runtime_errors() {
        let output = RunnerOutput {
            last_session_id: None,
            interrupted: false,
            reset: false,
        };
        assert_eq!(diagnostic_status(&[], &output), "success");

        let interrupted = RunnerOutput {
            interrupted: true,
            ..RunnerOutput::default()
        };
        assert_eq!(diagnostic_status(&[], &interrupted), "cancelled");

        let event = mutsuki_runtime_contracts::RuntimeEvent {
            sequence: 1,
            kind: mutsuki_runtime_contracts::RuntimeEventKind::Operation,
            name: "operation.invoke.error".to_string(),
            agent_id: Some("lilia-codex-agent".to_string()),
            attributes: BTreeMap::new(),
            error: Some(runtime_backend_error(OP_RUNNER_EXECUTE)),
        };
        assert_eq!(diagnostic_status(&[event], &output), "error");
    }

    #[test]
    fn runtime_queue_advances_only_after_successful_completion() {
        let success = RunnerOutput {
            last_session_id: Some("thread-2".to_string()),
            interrupted: false,
            reset: false,
        };
        let interrupted = RunnerOutput {
            interrupted: true,
            ..RunnerOutput::default()
        };
        let reset = RunnerOutput {
            reset: true,
            ..RunnerOutput::default()
        };

        assert!(should_advance_queue_after_runtime(&success, None));
        assert!(!should_advance_queue_after_runtime(&interrupted, None));
        assert!(!should_advance_queue_after_runtime(&reset, None));
        assert!(!should_advance_queue_after_runtime(
            &RunnerOutput::default(),
            Some("runtime failed")
        ));
    }

    #[test]
    fn turn_envelope_carries_runtime_context_without_contract_changes() {
        let envelope = build_turn_envelope(&context());

        assert_eq!(envelope.source.source_id, "lilia:codex");
        assert_eq!(
            envelope.payload["runtimeChannel"],
            RUNTIME_CHANNEL_MUTSUKI_CORE
        );
        assert_eq!(envelope.payload["resumeSessionId"], "thread-1");
        assert_eq!(envelope.payload["permission"], "ask");
        assert_eq!(
            envelope.payload["composerRuntimeWorkspaceRoots"],
            json!(["C:/repo", "D:/shared"])
        );
    }

    #[test]
    fn control_envelope_uses_runtime_control_source_and_payload() {
        let control = RuntimeControlEvent {
            name: "interaction_response".to_string(),
            attributes: BTreeMap::from([("requestId".to_string(), "ask-1".to_string())]),
            payload: Some(json!({ "type": "interaction_response", "id": "ask-1" })),
        };
        let envelope = build_control_envelope(&context(), &control);
        let decoded = runtime_control_from_envelope(&context(), &envelope).unwrap();

        assert_eq!(envelope.source.source_id, "lilia:codex:control");
        assert_eq!(envelope.payload_schema_id, CONTROL_PAYLOAD_SCHEMA_ID);
        assert_eq!(decoded.name, "interaction_response");
        assert_eq!(decoded.payload, control.payload);
    }

    #[test]
    fn runner_execute_decision_requests_runner_operation() {
        let result = runner_execute_decision("lilia-codex-agent");

        assert_eq!(result.status, StrategyResultStatus::Continue);
        assert_eq!(result.decision.unwrap()["operation"], OP_RUNNER_EXECUTE);
    }

    #[test]
    fn runner_poll_decision_requests_poll_operation() {
        let result = runner_poll_decision("lilia-codex-agent");

        assert_eq!(result.status, StrategyResultStatus::Continue);
        assert_eq!(result.decision.unwrap()["operation"], OP_RUNNER_POLL);
    }

    #[test]
    fn runtime_step_running_is_distinct_from_execute_and_completed() {
        assert_ne!(RuntimeStep::RunnerRunning, RuntimeStep::PendingExecute);
        assert_ne!(
            RuntimeStep::RunnerRunning,
            RuntimeStep::PendingCompletionStop
        );
        assert_ne!(
            RuntimeStep::PendingCompletionStop,
            RuntimeStep::PendingCompletionCheckpoint
        );
        assert_ne!(RuntimeStep::RunnerRunning, RuntimeStep::Executed);
        assert_ne!(RuntimeStep::RunnerRunning, RuntimeStep::Completed);
    }
}
