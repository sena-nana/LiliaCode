use std::collections::BTreeMap;

use mutsuki_runtime_contracts::{RuntimeError, ScalarValue};
use mutsuki_runtime_core::{AgentRuntime, BackendEventSink};
use serde_json::{json, Value as JsonValue};
use tauri::{AppHandle, Runtime};

use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::runner::RunnerLifecycleEvent;
use crate::chat::state::RuntimeControlEvent;
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::util::now_millis;
use crate::RUNTIME_CHANNEL_NANOBOT;

use super::{RuntimeStep, RuntimeTurnContext};

pub(super) fn record_missing_runner_operation(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
    step: RuntimeStep,
) {
    let expected_operation = step
        .expected_operation()
        .unwrap_or(super::OP_RUNNER_EXECUTE);
    runtime.record_backend_event(
        Some(&context.agent_id),
        "runner.operation.missing",
        BTreeMap::from([(
            "expectedOperation".to_string(),
            ScalarValue::String(expected_operation.to_string()),
        )]),
        Some(runtime_backend_error(expected_operation)),
    );
}

pub(super) fn record_runner_watchdog_idle(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
    idle_ticks: usize,
) {
    runtime.record_backend_event(
        Some(&context.agent_id),
        "runner.runtime.watchdog_idle",
        BTreeMap::from([("idleTicks".to_string(), ScalarValue::Int(idle_ticks as i64))]),
        Some(runtime_backend_error("lilia.runner.poll.watchdog_idle")),
    );
}

#[cfg(test)]
pub(super) fn record_runner_lifecycle_events(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
    lifecycle_events: &[RunnerLifecycleEvent],
) {
    for event in lifecycle_events {
        record_runner_lifecycle_event(runtime, context, event);
    }
}

pub(super) fn record_runner_lifecycle_event(
    events: &mut dyn BackendEventSink,
    context: &RuntimeTurnContext,
    event: &RunnerLifecycleEvent,
) {
    let mut attributes = super::scalar_attributes(event.detail.clone());
    attributes.insert(
        "stage".to_string(),
        ScalarValue::String(event.stage.to_string()),
    );
    events.record_backend_event(
        Some(&context.agent_id),
        &format!("runner.lifecycle.{}", event.stage),
        attributes,
        lifecycle_runtime_error(event),
    );
}

pub(super) fn record_runtime_control_events(
    runtime: &mut AgentRuntime,
    context: &RuntimeTurnContext,
    control_events: &[RuntimeControlEvent],
) {
    for event in control_events {
        record_runtime_control_event(runtime, context, event);
    }
}

pub(super) fn record_runtime_control_event(
    events: &mut dyn BackendEventSink,
    context: &RuntimeTurnContext,
    event: &RuntimeControlEvent,
) {
    let attributes = event
        .attributes
        .iter()
        .map(|(key, value)| (key.clone(), ScalarValue::String(value.clone())))
        .collect();
    events.record_backend_event(
        Some(&context.agent_id),
        &format!("runner.control.{}", event.name),
        attributes,
        runtime_control_runtime_error(event),
    );
}

pub(super) fn persist_runtime_diagnostic<R: Runtime>(
    app: &AppHandle<R>,
    context: &RuntimeTurnContext,
    runtime: &AgentRuntime,
    operation_count: usize,
    source_count: usize,
    lifecycle_events: &[RunnerLifecycleEvent],
    output: &crate::chat::runner::RunnerOutput,
) {
    let events = runtime.events();
    let trace_spans = runtime.trace_spans();
    let now = now_millis() as i64;
    let status = diagnostic_status(&events, output);
    persist_and_emit_input(
        app,
        AgentTimelineEventInput {
            id: Some(format!(
                "{}:{}:nanobot-runtime",
                context.task_id, context.turn_id
            )),
            task_id: context.task_id.clone(),
            turn_id: Some(context.turn_id.clone()),
            backend: context.backend.clone(),
            kind: "diagnostic".to_string(),
            status,
            title: "NanoBot Runtime".to_string(),
            summary: Some("NanoBot Rust Core 已接管本轮本地运行时边界".to_string()),
            payload: diagnostic_payload(
                context,
                operation_count,
                source_count,
                events.len(),
                &events,
                trace_spans,
                lifecycle_events,
                output,
            ),
            created_at: Some(now),
            updated_at: Some(now),
        },
    );
}

pub(super) fn diagnostic_payload(
    context: &RuntimeTurnContext,
    operation_count: usize,
    source_count: usize,
    event_count: usize,
    events: &[mutsuki_runtime_contracts::RuntimeEvent],
    trace_spans: &[mutsuki_runtime_contracts::TraceSpan],
    lifecycle_events: &[RunnerLifecycleEvent],
    output: &crate::chat::runner::RunnerOutput,
) -> JsonValue {
    json!({
        "runtimeChannel": RUNTIME_CHANNEL_NANOBOT,
        "agentId": context.agent_id,
        "operationCount": operation_count,
        "sourceCount": source_count,
        "eventCount": event_count,
        "events": events,
        "traceSpans": trace_spans,
        "lifecycleEvents": lifecycle_events,
        "runnerSessionId": output.last_session_id,
        "interrupted": output.interrupted,
        "reset": output.reset,
    })
}

pub(super) fn runtime_backend_error(route: impl Into<String>) -> RuntimeError {
    RuntimeError::new(
        mutsuki_runtime_contracts::ERR_RUNTIME_BACKEND_FAILED,
        super::PLUGIN_ID,
        route.into(),
    )
}

pub(super) fn diagnostic_status(
    events: &[mutsuki_runtime_contracts::RuntimeEvent],
    output: &crate::chat::runner::RunnerOutput,
) -> String {
    if output.interrupted || output.reset {
        return "cancelled".to_string();
    }
    if events.iter().any(|event| event.error.is_some()) {
        return "error".to_string();
    }
    "success".to_string()
}

pub(super) fn format_runtime_error(error: &RuntimeError) -> String {
    format!("{} at {} ({})", error.code, error.route, error.source)
}

fn lifecycle_runtime_error(event: &RunnerLifecycleEvent) -> Option<RuntimeError> {
    matches!(
        event.stage,
        "process_spawn_failed"
            | "stdin_write_failed"
            | "runner_error_event"
            | "stdout_unavailable"
            | "process_exit_error_emitted"
    )
    .then(|| runtime_backend_error(format!("lilia.runner.lifecycle.{}", event.stage)))
}

fn runtime_control_runtime_error(event: &RuntimeControlEvent) -> Option<RuntimeError> {
    event
        .attributes
        .contains_key("stdinError")
        .then(|| runtime_backend_error(format!("lilia.runner.control.{}", event.name)))
}
