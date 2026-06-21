use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

use super::run_workflow;
use crate::automation::contract;
use crate::automation::repository::{row_to_workflow, version_by_id};
use crate::automation::types::{
    AutomationDraft, AutomationScopeFilter, AutomationSignalEnvelope, AutomationWorkflow,
};
use crate::chat::state::ChatStore;
use crate::store::LiliaStore;
use crate::task_contract;
use crate::todos::contract as todo_contract;
use crate::util::now_millis;

pub fn emit_task_changed_signal<R: Runtime>(
    app: &AppHandle<R>,
    project_id: Option<String>,
    task_id: Option<String>,
    task_status: Option<String>,
    event_kind: &str,
    automation_run_id: Option<String>,
) {
    let signal = task_changed_signal(
        project_id,
        task_id,
        task_status,
        event_kind,
        automation_run_id,
    );
    dispatch_signal(app.clone(), signal);
}

pub fn emit_timeline_signal<R: Runtime>(
    app: &AppHandle<R>,
    event: &crate::agent_timeline::AgentTimelineEvent,
) {
    dispatch_signal(app.clone(), timeline_signal(event));
}

pub(crate) fn timeline_signal(
    event: &crate::agent_timeline::AgentTimelineEvent,
) -> AutomationSignalEnvelope {
    AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: contract::timeline_trigger_kind().to_string(),
        project_id: None,
        task_id: Some(event.task_id.clone()),
        backend: Some(event.backend.clone()),
        event_kind: Some(contract::timeline_trigger_kind().to_string()),
        automation_run_id: automation_run_id_from_payload(&event.payload),
        payload: serde_json::json!({
            "timelineEvent": event,
            "timelineEventKind": event.kind,
        }),
        created_at: now_millis(),
    }
}

pub fn emit_todo_signal<R: Runtime>(
    app: &AppHandle<R>,
    task_id: String,
    automation_run_id: Option<String>,
) {
    let signal = AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: contract::todo_trigger_kind().to_string(),
        project_id: None,
        task_id: Some(task_id),
        backend: None,
        event_kind: Some(contract::todo_trigger_kind().to_string()),
        automation_run_id,
        payload: serde_json::json!({ "source": todo_contract::changed_event_name() }),
        created_at: now_millis(),
    };
    dispatch_signal(app.clone(), signal);
}

pub fn emit_interaction_signal<R: Runtime>(
    app: &AppHandle<R>,
    task_id: String,
    turn_id: String,
    backend: String,
    request_id: String,
    interaction_kind: String,
    payload: JsonValue,
    automation_run_id: Option<String>,
) {
    dispatch_signal(
        app.clone(),
        interaction_signal(
            task_id,
            turn_id,
            backend,
            request_id,
            interaction_kind,
            payload,
            automation_run_id,
        ),
    );
}

pub(crate) fn interaction_signal(
    task_id: String,
    turn_id: String,
    backend: String,
    request_id: String,
    interaction_kind: String,
    payload: JsonValue,
    automation_run_id: Option<String>,
) -> AutomationSignalEnvelope {
    AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: contract::interaction_trigger_kind().to_string(),
        project_id: None,
        task_id: Some(task_id),
        backend: Some(backend),
        event_kind: Some(contract::interaction_trigger_kind().to_string()),
        automation_run_id,
        payload: serde_json::json!({
            "turnId": turn_id,
            "requestId": request_id,
            "interactionKind": interaction_kind,
            "payload": payload,
        }),
        created_at: now_millis(),
    }
}

fn dispatch_signal<R: Runtime>(app: AppHandle<R>, signal: AutomationSignalEnvelope) {
    if signal.automation_run_id.is_some() {
        return;
    }
    std::thread::spawn(move || {
        if let Err(err) = run_matching_workflows(&app, signal) {
            eprintln!("[automation] signal dispatch failed: {err}");
        }
    });
}

fn automation_run_id_from_payload(payload: &JsonValue) -> Option<String> {
    payload
        .get("automationRunId")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub(crate) fn task_changed_signal(
    project_id: Option<String>,
    task_id: Option<String>,
    task_status: Option<String>,
    event_kind: &str,
    automation_run_id: Option<String>,
) -> AutomationSignalEnvelope {
    let event_kind = if event_kind.trim().is_empty() {
        contract::task_changed_trigger_kind()
    } else {
        event_kind.trim()
    };
    AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: contract::task_changed_trigger_kind().to_string(),
        project_id,
        task_id,
        backend: None,
        event_kind: Some(event_kind.to_string()),
        automation_run_id,
        payload: serde_json::json!({
            "source": task_contract::tasks_changed_event_name(),
            "eventKind": event_kind,
            "taskStatus": task_status,
        }),
        created_at: now_millis(),
    }
}

fn run_matching_workflows<R: Runtime>(
    app: &AppHandle<R>,
    mut signal: AutomationSignalEnvelope,
) -> Result<(), String> {
    let Some(store) = app.try_state::<LiliaStore>() else {
        return Ok(());
    };
    let conn = store.conn()?;
    enrich_signal_context(&conn, &mut signal)?;
    let workflows = enabled_workflows_for_signal(&conn, &signal)?;
    drop(conn);
    for workflow in workflows {
        let app_handle = app.clone();
        let signal = signal.clone();
        std::thread::spawn(move || {
            let chat_store = app_handle.state::<ChatStore>();
            if let Err(err) = run_workflow(&app_handle, &chat_store, &workflow.id, signal) {
                eprintln!("[automation] workflow {} failed: {err}", workflow.id);
            }
        });
    }
    Ok(())
}

pub(crate) fn enrich_signal_context(
    conn: &Connection,
    signal: &mut AutomationSignalEnvelope,
) -> Result<(), String> {
    let Some(task_id) = signal.task_id.as_deref() else {
        return Ok(());
    };
    let context = conn
        .query_row(
            "SELECT project_id, status FROM tasks WHERE id = ?1 AND archived = 0",
            params![task_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| format!("automation_signal_context: {e}"))?;
    let Some((project_id, status)) = context else {
        return Ok(());
    };
    if signal.project_id.is_none() {
        signal.project_id = project_id;
    }
    if signal.payload.get("taskStatus").is_none() {
        let mut payload = signal.payload.as_object().cloned().unwrap_or_default();
        payload.insert("taskStatus".to_string(), JsonValue::String(status));
        signal.payload = JsonValue::Object(payload);
    }
    Ok(())
}

pub(crate) fn enabled_workflows_for_signal(
    conn: &Connection,
    signal: &AutomationSignalEnvelope,
) -> Result<Vec<AutomationWorkflow>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, name, enabled, scope_json, draft_json, published_version_id, created_at, updated_at
               FROM automation_workflows
               WHERE enabled = 1 AND published_version_id IS NOT NULL
               ORDER BY updated_at DESC"#,
        )
        .map_err(|e| format!("automation_list_enabled: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map([], row_to_workflow)
        .map_err(|e| format!("automation_list_enabled: query 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let workflow = row.map_err(|e| format!("automation_list_enabled: row 失败：{e}"))?;
        if workflow_matches_published_snapshot(conn, &workflow, signal)? {
            out.push(workflow);
        }
    }
    Ok(out)
}

pub(crate) fn workflow_matches_published_snapshot(
    conn: &Connection,
    workflow: &AutomationWorkflow,
    signal: &AutomationSignalEnvelope,
) -> Result<bool, String> {
    let Some(version_id) = workflow.published_version_id.as_deref() else {
        return Ok(false);
    };
    let Some(version) = version_by_id(conn, version_id)? else {
        return Ok(false);
    };
    Ok(
        scope_matches(&version.snapshot.scope, signal)
            && trigger_matches(&version.snapshot, signal),
    )
}

pub(crate) fn scope_matches(
    scope: &AutomationScopeFilter,
    signal: &AutomationSignalEnvelope,
) -> bool {
    if !scope.project_ids.is_empty() {
        let Some(project_id) = signal.project_id.as_deref() else {
            return scope.include_inbox;
        };
        if project_id.is_empty() {
            return scope.include_inbox;
        }
        if !scope.project_ids.iter().any(|id| id == project_id) {
            return false;
        }
    } else if !scope.include_inbox && matches!(signal.project_id.as_deref(), None | Some("")) {
        return false;
    }
    if !scope.backends.is_empty() {
        let Some(backend) = signal.backend.as_deref() else {
            return false;
        };
        if !scope.backends.iter().any(|item| item == backend) {
            return false;
        }
    }
    if !scope.event_kinds.is_empty() {
        let Some(kind) = signal.event_kind.as_deref() else {
            return false;
        };
        if !scope.event_kinds.iter().any(|item| item == kind) {
            return false;
        }
    }
    if !scope.task_statuses.is_empty() {
        let Some(status) = signal
            .payload
            .get("taskStatus")
            .and_then(|value| value.as_str())
        else {
            return false;
        };
        if !scope.task_statuses.iter().any(|item| item == status) {
            return false;
        }
    }
    true
}

fn trigger_matches(draft: &AutomationDraft, signal: &AutomationSignalEnvelope) -> bool {
    draft.nodes.iter().any(|node| {
        let trigger_kind = node
            .config
            .get("triggerKind")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| contract::default_trigger_kind().to_string());
        node.kind == "trigger" && trigger_kind == signal.kind
    })
}

pub(crate) fn manual_signal(payload: Option<JsonValue>) -> AutomationSignalEnvelope {
    AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: contract::default_trigger_kind().to_string(),
        project_id: None,
        task_id: None,
        backend: None,
        event_kind: Some(contract::default_trigger_kind().to_string()),
        automation_run_id: None,
        payload: payload.unwrap_or_else(|| serde_json::json!({})),
        created_at: now_millis(),
    }
}
