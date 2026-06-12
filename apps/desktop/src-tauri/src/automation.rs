use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use uuid::Uuid;

use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::runner::spawn_agent_turn;
use crate::chat::state::{
    new_chat_message_id, normalize_composer_for_backend, queue_pending_turn_for_app,
    should_persist_user_message, ChatStore,
};
use crate::chat::timeline_sink::{persist_and_emit_input, persist_and_emit_message_timeline_event};
use crate::chat::types::{ChatComposerState, ChatMessage, ChatSendResult, ChatWorkflow};
use crate::store::LiliaStore;
use crate::util::now_millis;
use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    WaitingUser,
}

impl AutomationRunStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::WaitingUser => "waiting_user",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            "waiting_user" => Self::WaitingUser,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationScopeFilter {
    #[serde(default)]
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub include_inbox: bool,
    #[serde(default)]
    pub task_statuses: Vec<String>,
    #[serde(default)]
    pub backends: Vec<String>,
    #[serde(default)]
    pub event_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationNodePosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationNode {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub position: AutomationNodePosition,
    #[serde(default)]
    pub config: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub source_handle: Option<String>,
    #[serde(default)]
    pub target_handle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationDraft {
    #[serde(default)]
    pub nodes: Vec<AutomationNode>,
    #[serde(default)]
    pub edges: Vec<AutomationEdge>,
    #[serde(default)]
    pub scope: AutomationScopeFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationWorkflow {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub scope: AutomationScopeFilter,
    pub draft: AutomationDraft,
    pub published_version_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationWorkflowVersion {
    pub id: String,
    pub workflow_id: String,
    pub version: i64,
    pub snapshot: AutomationDraft,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationSignalEnvelope {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub event_kind: Option<String>,
    #[serde(default)]
    pub automation_run_id: Option<String>,
    #[serde(default)]
    pub payload: JsonValue,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRun {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub status: AutomationRunStatus,
    pub trigger: AutomationSignalEnvelope,
    pub scope: AutomationScopeFilter,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunNodeState {
    pub id: String,
    pub run_id: String,
    pub node_id: String,
    pub status: AutomationRunStatus,
    #[serde(default)]
    pub input: JsonValue,
    #[serde(default)]
    pub output: Option<JsonValue>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationSaveDraftInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub scope: AutomationScopeFilter,
    #[serde(default)]
    pub nodes: Vec<AutomationNode>,
    #[serde(default)]
    pub edges: Vec<AutomationEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunOnceInput {
    #[serde(default)]
    pub payload: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutomationResumeRunInput {
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub payload: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunDetail {
    pub run: AutomationRun,
    pub nodes: Vec<AutomationRunNodeState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutomationChangedEvent {
    workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AutomationRunEvent {
    run: AutomationRun,
}

enum GraphExecution {
    Finished,
    WaitingUser,
    WaitingAgent,
}

fn json_text<T: Serialize>(value: &T, label: &str) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("automation: 序列化 {label} 失败：{e}"))
}

fn merge_json_objects(base: JsonValue, patch: JsonValue) -> JsonValue {
    let mut out = base.as_object().cloned().unwrap_or_default();
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            out.insert(key.clone(), value.clone());
        }
    }
    JsonValue::Object(out)
}

fn row_to_workflow(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationWorkflow> {
    let scope_json: String = row.get(3)?;
    let draft_json: String = row.get(4)?;
    let scope: AutomationScopeFilter = serde_json::from_str(&scope_json).unwrap_or_default();
    let draft = serde_json::from_str(&draft_json).unwrap_or(AutomationDraft {
        nodes: Vec::new(),
        edges: Vec::new(),
        scope: scope.clone(),
    });
    Ok(AutomationWorkflow {
        id: row.get(0)?,
        name: row.get(1)?,
        enabled: row.get::<_, i64>(2)? != 0,
        scope,
        draft,
        published_version_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn row_to_version(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationWorkflowVersion> {
    let snapshot_json: String = row.get(3)?;
    Ok(AutomationWorkflowVersion {
        id: row.get(0)?,
        workflow_id: row.get(1)?,
        version: row.get(2)?,
        snapshot: serde_json::from_str(&snapshot_json).unwrap_or_default(),
        created_at: row.get(4)?,
    })
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationRun> {
    let trigger_json: String = row.get(4)?;
    let scope_json: String = row.get(5)?;
    let status: String = row.get(3)?;
    Ok(AutomationRun {
        id: row.get(0)?,
        workflow_id: row.get(1)?,
        workflow_version_id: row.get(2)?,
        status: AutomationRunStatus::from_str(&status),
        trigger: serde_json::from_str(&trigger_json).unwrap_or_else(|_| manual_signal(None)),
        scope: serde_json::from_str(&scope_json).unwrap_or_default(),
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        error: row.get(8)?,
    })
}

fn row_to_node_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationRunNodeState> {
    let status: String = row.get(3)?;
    let input_json: String = row.get(4)?;
    let output_json: Option<String> = row.get(5)?;
    Ok(AutomationRunNodeState {
        id: row.get(0)?,
        run_id: row.get(1)?,
        node_id: row.get(2)?,
        status: AutomationRunStatus::from_str(&status),
        input: serde_json::from_str(&input_json).unwrap_or(JsonValue::Object(JsonMap::new())),
        output: output_json.and_then(|text| serde_json::from_str(&text).ok()),
        error: row.get(6)?,
        started_at: row.get(7)?,
        finished_at: row.get(8)?,
    })
}

fn emit_changed<R: Runtime>(app: &AppHandle<R>, workflow_id: Option<String>) {
    let _ = app.emit("automation:changed", AutomationChangedEvent { workflow_id });
}

fn emit_run<R: Runtime>(app: &AppHandle<R>, event: &str, run: AutomationRun) {
    let _ = app.emit(event, AutomationRunEvent { run });
}

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
    let signal = AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: "timeline_event".to_string(),
        project_id: None,
        task_id: Some(event.task_id.clone()),
        backend: Some(event.backend.clone()),
        event_kind: Some(event.kind.clone()),
        automation_run_id: automation_run_id_from_payload(&event.payload),
        payload: serde_json::json!({ "timelineEvent": event }),
        created_at: now_millis(),
    };
    dispatch_signal(app.clone(), signal);
}

pub fn emit_todo_signal<R: Runtime>(
    app: &AppHandle<R>,
    task_id: String,
    automation_run_id: Option<String>,
) {
    let signal = AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: "todo_changed".to_string(),
        project_id: None,
        task_id: Some(task_id),
        backend: None,
        event_kind: Some("todo_changed".to_string()),
        automation_run_id,
        payload: serde_json::json!({ "source": "todo-changed" }),
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
    let signal = AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: "interaction_request".to_string(),
        project_id: None,
        task_id: Some(task_id),
        backend: Some(backend),
        event_kind: Some(interaction_kind.clone()),
        automation_run_id,
        payload: serde_json::json!({
            "turnId": turn_id,
            "requestId": request_id,
            "interactionKind": interaction_kind,
            "payload": payload,
        }),
        created_at: now_millis(),
    };
    dispatch_signal(app.clone(), signal);
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

fn task_changed_signal(
    project_id: Option<String>,
    task_id: Option<String>,
    task_status: Option<String>,
    event_kind: &str,
    automation_run_id: Option<String>,
) -> AutomationSignalEnvelope {
    let event_kind = if event_kind.trim().is_empty() {
        "task_changed"
    } else {
        event_kind.trim()
    };
    AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: "task_changed".to_string(),
        project_id,
        task_id,
        backend: None,
        event_kind: Some(event_kind.to_string()),
        automation_run_id,
        payload: serde_json::json!({
            "source": "tasks:changed",
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

fn enrich_signal_context(
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

fn enabled_workflows_for_signal(
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

fn workflow_matches_published_snapshot(
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

fn scope_matches(scope: &AutomationScopeFilter, signal: &AutomationSignalEnvelope) -> bool {
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
        node.kind == "trigger"
            && node
                .config
                .get("triggerKind")
                .and_then(|value| value.as_str())
                .unwrap_or("manual")
                == signal.kind
    })
}

fn manual_signal(payload: Option<JsonValue>) -> AutomationSignalEnvelope {
    AutomationSignalEnvelope {
        id: Uuid::new_v4().to_string(),
        kind: "manual".to_string(),
        project_id: None,
        task_id: None,
        backend: None,
        event_kind: Some("manual".to_string()),
        automation_run_id: None,
        payload: payload.unwrap_or_else(|| serde_json::json!({})),
        created_at: now_millis(),
    }
}

#[tauri::command]
pub fn automation_list_workflows(
    store: State<'_, LiliaStore>,
) -> Result<Vec<AutomationWorkflow>, String> {
    let conn = store.conn()?;
    list_workflows(&conn)
}

fn list_workflows(conn: &Connection) -> Result<Vec<AutomationWorkflow>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, name, enabled, scope_json, draft_json, published_version_id, created_at, updated_at
               FROM automation_workflows
               ORDER BY updated_at DESC"#,
        )
        .map_err(|e| format!("automation_list_workflows: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map([], row_to_workflow)
        .map_err(|e| format!("automation_list_workflows: query 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("automation_list_workflows: row 失败：{e}"))?);
    }
    Ok(out)
}

#[tauri::command]
pub fn automation_get_workflow(
    id: String,
    store: State<'_, LiliaStore>,
) -> Result<Option<AutomationWorkflow>, String> {
    let conn = store.conn()?;
    workflow_by_id(&conn, &id)
}

fn workflow_by_id(conn: &Connection, id: &str) -> Result<Option<AutomationWorkflow>, String> {
    conn.query_row(
        r#"SELECT id, name, enabled, scope_json, draft_json, published_version_id, created_at, updated_at
           FROM automation_workflows
           WHERE id = ?1"#,
        params![id],
        row_to_workflow,
    )
    .optional()
    .map_err(|e| format!("automation_get_workflow: {e}"))
}

#[tauri::command]
pub fn automation_save_draft<R: Runtime>(
    input: AutomationSaveDraftInput,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
) -> Result<AutomationWorkflow, String> {
    let conn = store.conn()?;
    let workflow = save_draft(&conn, input)?;
    emit_changed(&app, Some(workflow.id.clone()));
    Ok(workflow)
}

fn save_draft(
    conn: &Connection,
    input: AutomationSaveDraftInput,
) -> Result<AutomationWorkflow, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("automation_save_draft: 名称不能为空".to_string());
    }
    validate_workflow_graph(&input.nodes, &input.edges)?;
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = now_millis();
    let scope = input.scope;
    let draft = AutomationDraft {
        nodes: input.nodes,
        edges: input.edges,
        scope: scope.clone(),
    };
    let scope_json = json_text(&scope, "scope")?;
    let draft_json = json_text(&draft, "draft")?;
    let existing = workflow_by_id(conn, &id)?;
    if existing.is_some() {
        conn.execute(
            r#"UPDATE automation_workflows
               SET name = ?1, scope_json = ?2, draft_json = ?3, updated_at = ?4
               WHERE id = ?5"#,
            params![name, scope_json, draft_json, now, id],
        )
        .map_err(|e| format!("automation_save_draft: update 失败：{e}"))?;
    } else {
        conn.execute(
            r#"INSERT INTO automation_workflows
               (id, name, enabled, scope_json, draft_json, published_version_id, created_at, updated_at)
               VALUES (?1, ?2, 0, ?3, ?4, NULL, ?5, ?6)"#,
            params![id, name, scope_json, draft_json, now, now],
        )
        .map_err(|e| format!("automation_save_draft: insert 失败：{e}"))?;
    }
    workflow_by_id(conn, &id)?.ok_or_else(|| "automation_save_draft: 保存后读取失败".to_string())
}

#[tauri::command]
pub fn automation_publish<R: Runtime>(
    id: String,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
) -> Result<AutomationWorkflowVersion, String> {
    let conn = store.conn()?;
    let version = publish_workflow(&conn, &id)?;
    emit_changed(&app, Some(id));
    Ok(version)
}

fn publish_workflow(conn: &Connection, id: &str) -> Result<AutomationWorkflowVersion, String> {
    let workflow =
        workflow_by_id(conn, id)?.ok_or_else(|| "automation_publish: 自动化不存在".to_string())?;
    validate_workflow_graph(&workflow.draft.nodes, &workflow.draft.edges)?;
    let next_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM automation_workflow_versions WHERE workflow_id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("automation_publish: 查询版本失败：{e}"))?;
    let version_id = Uuid::new_v4().to_string();
    let now = now_millis();
    let snapshot_json = json_text(&workflow.draft, "snapshot")?;
    conn.execute(
        r#"INSERT INTO automation_workflow_versions
           (id, workflow_id, version, snapshot_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5)"#,
        params![version_id, id, next_version, snapshot_json, now],
    )
    .map_err(|e| format!("automation_publish: insert version 失败：{e}"))?;
    conn.execute(
        "UPDATE automation_workflows SET published_version_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![version_id, now, id],
    )
    .map_err(|e| format!("automation_publish: update workflow 失败：{e}"))?;
    version_by_id(conn, &version_id)?
        .ok_or_else(|| "automation_publish: 发布后读取失败".to_string())
}

fn version_by_id(conn: &Connection, id: &str) -> Result<Option<AutomationWorkflowVersion>, String> {
    conn.query_row(
        r#"SELECT id, workflow_id, version, snapshot_json, created_at
           FROM automation_workflow_versions WHERE id = ?1"#,
        params![id],
        row_to_version,
    )
    .optional()
    .map_err(|e| format!("automation_version_by_id: {e}"))
}

#[tauri::command]
pub fn automation_set_enabled<R: Runtime>(
    id: String,
    enabled: bool,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
) -> Result<(), String> {
    let conn = store.conn()?;
    let workflow = workflow_by_id(&conn, &id)?
        .ok_or_else(|| "automation_set_enabled: 自动化不存在".to_string())?;
    if enabled && workflow.published_version_id.is_none() {
        return Err("automation_set_enabled: 启用前需要先发布".to_string());
    }
    conn.execute(
        "UPDATE automation_workflows SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
        params![if enabled { 1 } else { 0 }, now_millis(), id],
    )
    .map_err(|e| format!("automation_set_enabled: {e}"))?;
    emit_changed(&app, Some(id));
    Ok(())
}

#[tauri::command]
pub fn automation_run_once<R: Runtime>(
    id: String,
    input: Option<AutomationRunOnceInput>,
    app: AppHandle<R>,
    _store: State<'_, LiliaStore>,
    chat_store: State<'_, ChatStore>,
) -> Result<AutomationRun, String> {
    let payload = input.and_then(|input| input.payload);
    run_workflow(&app, &chat_store, &id, manual_signal(payload))
}

#[tauri::command]
pub fn automation_resume_run<R: Runtime>(
    run_id: String,
    input: Option<AutomationResumeRunInput>,
    app: AppHandle<R>,
    store: State<'_, LiliaStore>,
    chat_store: State<'_, ChatStore>,
) -> Result<AutomationRun, String> {
    let conn = store.conn()?;
    let run = run_by_id(&conn, &run_id)?
        .ok_or_else(|| "automation_resume_run: run 不存在".to_string())?;
    if run.status != AutomationRunStatus::WaitingUser {
        return Err("automation_resume_run: 只能恢复等待用户的运行".to_string());
    }
    let version = version_by_id(&conn, &run.workflow_version_id)?
        .ok_or_else(|| "automation_resume_run: 发布版本不存在".to_string())?;
    validate_workflow_graph(&version.snapshot.nodes, &version.snapshot.edges)?;
    let input = input.unwrap_or_default();
    let node_states = node_states_for_run(&conn, &run_id)?;
    let waiting_state = waiting_node_state(&node_states, input.node_id.as_deref())?;
    let mut outputs = outputs_from_node_states(&node_states);
    let user_payload = input
        .payload
        .unwrap_or_else(|| serde_json::json!({ "confirmed": true }));
    let node_output = serde_json::json!({
        "waitingUser": false,
        "confirmed": true,
        "user": user_payload,
        "selectedHandle": "success",
    });
    update_node_state(
        &conn,
        &run.id,
        &waiting_state.node_id,
        AutomationRunStatus::Succeeded,
        waiting_state.input.clone(),
        Some(node_output.clone()),
        None,
    )?;
    outputs.insert(
        waiting_state.node_id.clone(),
        serde_json::json!({
            "status": AutomationRunStatus::Succeeded.as_str(),
            "output": node_output,
        }),
    );
    let running = update_run_status(
        &conn,
        &app,
        &run.id,
        AutomationRunStatus::Running,
        None,
        false,
    )?;
    let result = execute_graph(
        &app,
        &chat_store,
        &conn,
        &running,
        &version.snapshot,
        Some(waiting_state.node_id),
        outputs,
    );
    match result {
        Ok(GraphExecution::Finished) => finish_run(
            &conn,
            &app,
            &running.id,
            AutomationRunStatus::Succeeded,
            None,
        ),
        Ok(GraphExecution::WaitingUser) => update_run_status(
            &conn,
            &app,
            &running.id,
            AutomationRunStatus::WaitingUser,
            None,
            false,
        ),
        Ok(GraphExecution::WaitingAgent) => update_run_status(
            &conn,
            &app,
            &running.id,
            AutomationRunStatus::Running,
            None,
            false,
        ),
        Err(err) => finish_run(
            &conn,
            &app,
            &running.id,
            AutomationRunStatus::Failed,
            Some(err),
        ),
    }
}

pub fn automation_complete_agent_turn<R: Runtime>(
    app: &AppHandle<R>,
    chat_store: &ChatStore,
    automation_run_id: Option<String>,
    turn_id: &str,
    success: bool,
) {
    let Some(run_id) = automation_run_id else {
        return;
    };
    if let Err(err) = complete_agent_turn(app, chat_store, &run_id, turn_id, success) {
        eprintln!("[automation] complete agent turn failed for run {run_id}: {err}");
    }
}

#[tauri::command]
pub fn automation_list_runs(
    workflow_id: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<Vec<AutomationRun>, String> {
    let conn = store.conn()?;
    list_runs(&conn, workflow_id.as_deref())
}

fn list_runs(conn: &Connection, workflow_id: Option<&str>) -> Result<Vec<AutomationRun>, String> {
    let sql = if workflow_id.is_some() {
        r#"SELECT id, workflow_id, workflow_version_id, status, trigger_json, scope_json, started_at, finished_at, error
           FROM automation_runs WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT 100"#
    } else {
        r#"SELECT id, workflow_id, workflow_version_id, status, trigger_json, scope_json, started_at, finished_at, error
           FROM automation_runs ORDER BY started_at DESC LIMIT 100"#
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("automation_list_runs: prepare 失败：{e}"))?;
    let mut out = Vec::new();
    if let Some(workflow_id) = workflow_id {
        let rows = stmt
            .query_map(params![workflow_id], row_to_run)
            .map_err(|e| format!("automation_list_runs: query 失败：{e}"))?;
        for row in rows {
            out.push(row.map_err(|e| format!("automation_list_runs: row 失败：{e}"))?);
        }
    } else {
        let rows = stmt
            .query_map([], row_to_run)
            .map_err(|e| format!("automation_list_runs: query 失败：{e}"))?;
        for row in rows {
            out.push(row.map_err(|e| format!("automation_list_runs: row 失败：{e}"))?);
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn automation_get_run(
    run_id: String,
    store: State<'_, LiliaStore>,
) -> Result<Option<AutomationRunDetail>, String> {
    let conn = store.conn()?;
    let Some(run) = run_by_id(&conn, &run_id)? else {
        return Ok(None);
    };
    Ok(Some(AutomationRunDetail {
        nodes: node_states_for_run(&conn, &run_id)?,
        run,
    }))
}

fn run_by_id(conn: &Connection, run_id: &str) -> Result<Option<AutomationRun>, String> {
    conn.query_row(
        r#"SELECT id, workflow_id, workflow_version_id, status, trigger_json, scope_json, started_at, finished_at, error
           FROM automation_runs WHERE id = ?1"#,
        params![run_id],
        row_to_run,
    )
    .optional()
    .map_err(|e| format!("automation_get_run: {e}"))
}

fn node_states_for_run(
    conn: &Connection,
    run_id: &str,
) -> Result<Vec<AutomationRunNodeState>, String> {
    let mut stmt = conn
        .prepare(
            r#"SELECT id, run_id, node_id, status, input_json, output_json, error, started_at, finished_at
               FROM automation_run_nodes
               WHERE run_id = ?1
               ORDER BY id ASC"#,
        )
        .map_err(|e| format!("automation_get_run_nodes: prepare 失败：{e}"))?;
    let rows = stmt
        .query_map(params![run_id], row_to_node_state)
        .map_err(|e| format!("automation_get_run_nodes: query 失败：{e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("automation_get_run_nodes: row 失败：{e}"))?);
    }
    Ok(out)
}

fn waiting_node_state(
    states: &[AutomationRunNodeState],
    node_id: Option<&str>,
) -> Result<AutomationRunNodeState, String> {
    states
        .iter()
        .find(|state| {
            state.status == AutomationRunStatus::WaitingUser
                && node_id.map(|id| id == state.node_id).unwrap_or(true)
        })
        .cloned()
        .ok_or_else(|| "automation_resume_run: 未找到等待中的人工节点".to_string())
}

fn outputs_from_node_states(states: &[AutomationRunNodeState]) -> BTreeMap<String, JsonValue> {
    states
        .iter()
        .filter_map(|state| match state.status {
            AutomationRunStatus::Succeeded | AutomationRunStatus::Skipped => {
                state.output.clone().map(|output| {
                    (
                        state.node_id.clone(),
                        serde_json::json!({
                            "status": state.status.as_str(),
                            "output": output,
                        }),
                    )
                })
            }
            _ => None,
        })
        .collect()
}

fn running_agent_node_state(
    states: &[AutomationRunNodeState],
    turn_id: &str,
) -> Result<AutomationRunNodeState, String> {
    states
        .iter()
        .find(|state| {
            state.status == AutomationRunStatus::Running
                && state
                    .output
                    .as_ref()
                    .and_then(|output| output.get("waitingAgent"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
                && state
                    .output
                    .as_ref()
                    .and_then(|output| output.get("turnId"))
                    .and_then(|value| value.as_str())
                    == Some(turn_id)
        })
        .cloned()
        .ok_or_else(|| "automation_complete_agent_turn: 未找到等待中的 Agent 节点".to_string())
}

fn complete_agent_turn<R: Runtime>(
    app: &AppHandle<R>,
    chat_store: &ChatStore,
    run_id: &str,
    turn_id: &str,
    success: bool,
) -> Result<AutomationRun, String> {
    let conn = app.state::<LiliaStore>().conn()?;
    let run = run_by_id(&conn, run_id)?
        .ok_or_else(|| "automation_complete_agent_turn: run 不存在".to_string())?;
    if run.status != AutomationRunStatus::Running {
        return Ok(run);
    }
    let version = version_by_id(&conn, &run.workflow_version_id)?
        .ok_or_else(|| "automation_complete_agent_turn: 发布版本不存在".to_string())?;
    validate_workflow_graph(&version.snapshot.nodes, &version.snapshot.edges)?;
    let node_states = node_states_for_run(&conn, run_id)?;
    let agent_state = running_agent_node_state(&node_states, turn_id)?;
    let mut outputs = outputs_from_node_states(&node_states);
    let prior_output = agent_state
        .output
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    let node_output = merge_json_objects(
        prior_output,
        serde_json::json!({
            "waitingAgent": false,
            "completed": success,
            "selectedHandle": if success { "success" } else { "error" },
        }),
    );
    let node_status = if success {
        AutomationRunStatus::Succeeded
    } else {
        AutomationRunStatus::Failed
    };
    update_node_state(
        &conn,
        &run.id,
        &agent_state.node_id,
        node_status.clone(),
        agent_state.input.clone(),
        Some(node_output.clone()),
        None,
    )?;
    if !success {
        return finish_run(
            &conn,
            app,
            &run.id,
            AutomationRunStatus::Failed,
            Some("Agent 节点运行失败或被中断".to_string()),
        );
    }
    outputs.insert(
        agent_state.node_id.clone(),
        serde_json::json!({
            "status": node_status.as_str(),
            "output": node_output,
        }),
    );
    let result = execute_graph(
        app,
        chat_store,
        &conn,
        &run,
        &version.snapshot,
        Some(agent_state.node_id),
        outputs,
    );
    match result {
        Ok(GraphExecution::Finished) => {
            finish_run(&conn, app, &run.id, AutomationRunStatus::Succeeded, None)
        }
        Ok(GraphExecution::WaitingUser) => update_run_status(
            &conn,
            app,
            &run.id,
            AutomationRunStatus::WaitingUser,
            None,
            false,
        ),
        Ok(GraphExecution::WaitingAgent) => update_run_status(
            &conn,
            app,
            &run.id,
            AutomationRunStatus::Running,
            None,
            false,
        ),
        Err(err) => finish_run(&conn, app, &run.id, AutomationRunStatus::Failed, Some(err)),
    }
}

fn run_workflow<R: Runtime>(
    app: &AppHandle<R>,
    chat_store: &ChatStore,
    workflow_id: &str,
    signal: AutomationSignalEnvelope,
) -> Result<AutomationRun, String> {
    let conn = app.state::<LiliaStore>().conn()?;
    let workflow = workflow_by_id(&conn, workflow_id)?
        .ok_or_else(|| "automation_run_once: 自动化不存在".to_string())?;
    let version_id = workflow
        .published_version_id
        .clone()
        .ok_or_else(|| "automation_run_once: 运行前需要先发布".to_string())?;
    let version = version_by_id(&conn, &version_id)?
        .ok_or_else(|| "automation_run_once: 发布版本不存在".to_string())?;
    validate_workflow_graph(&version.snapshot.nodes, &version.snapshot.edges)?;
    if workflow_has_active_run(&conn, workflow_id)? {
        return create_skipped_run(&conn, app, &workflow, &version, signal);
    }
    let run = create_run(&conn, app, &workflow, &version, signal)?;
    emit_run(app, "automation:run-started", run.clone());
    let result = execute_graph(
        app,
        chat_store,
        &conn,
        &run,
        &version.snapshot,
        None,
        BTreeMap::new(),
    );
    match result {
        Ok(GraphExecution::Finished) => {
            finish_run(&conn, app, &run.id, AutomationRunStatus::Succeeded, None)
        }
        Ok(GraphExecution::WaitingUser) => update_run_status(
            &conn,
            app,
            &run.id,
            AutomationRunStatus::WaitingUser,
            None,
            false,
        ),
        Ok(GraphExecution::WaitingAgent) => update_run_status(
            &conn,
            app,
            &run.id,
            AutomationRunStatus::Running,
            None,
            false,
        ),
        Err(err) => finish_run(&conn, app, &run.id, AutomationRunStatus::Failed, Some(err)),
    }
}

fn workflow_has_active_run(conn: &Connection, workflow_id: &str) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            r#"SELECT COUNT(*) FROM automation_runs
               WHERE workflow_id = ?1 AND status IN ('pending', 'running', 'waiting_user')"#,
            params![workflow_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("automation_active_run: {e}"))?;
    Ok(count > 0)
}

fn create_skipped_run<R: Runtime>(
    conn: &Connection,
    app: &AppHandle<R>,
    workflow: &AutomationWorkflow,
    version: &AutomationWorkflowVersion,
    signal: AutomationSignalEnvelope,
) -> Result<AutomationRun, String> {
    let mut run = insert_run(
        conn,
        workflow,
        version,
        signal,
        AutomationRunStatus::Skipped,
        Some("同一自动化已有运行中实例".to_string()),
    )?;
    run.finished_at = Some(now_millis());
    conn.execute(
        "UPDATE automation_runs SET finished_at = ?1 WHERE id = ?2",
        params![run.finished_at, run.id],
    )
    .map_err(|e| format!("automation_skipped_run: update 失败：{e}"))?;
    emit_run(app, "automation:run-finished", run.clone());
    Ok(run)
}

fn create_run<R: Runtime>(
    conn: &Connection,
    app: &AppHandle<R>,
    workflow: &AutomationWorkflow,
    version: &AutomationWorkflowVersion,
    signal: AutomationSignalEnvelope,
) -> Result<AutomationRun, String> {
    let run = insert_run(
        conn,
        workflow,
        version,
        signal,
        AutomationRunStatus::Running,
        None,
    )?;
    for node in &version.snapshot.nodes {
        insert_node_state(
            conn,
            &run.id,
            &node.id,
            AutomationRunStatus::Pending,
            serde_json::json!({}),
            None,
            None,
            None,
        )?;
    }
    emit_run(app, "automation:run-updated", run.clone());
    Ok(run)
}

fn insert_run(
    conn: &Connection,
    workflow: &AutomationWorkflow,
    version: &AutomationWorkflowVersion,
    signal: AutomationSignalEnvelope,
    status: AutomationRunStatus,
    error: Option<String>,
) -> Result<AutomationRun, String> {
    let now = now_millis();
    let run = AutomationRun {
        id: Uuid::new_v4().to_string(),
        workflow_id: workflow.id.clone(),
        workflow_version_id: version.id.clone(),
        status,
        trigger: signal,
        scope: version.snapshot.scope.clone(),
        started_at: now,
        finished_at: None,
        error,
    };
    conn.execute(
        r#"INSERT INTO automation_runs
           (id, workflow_id, workflow_version_id, status, trigger_json, scope_json, started_at, finished_at, error)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)"#,
        params![
            run.id,
            run.workflow_id,
            run.workflow_version_id,
            run.status.as_str(),
            json_text(&run.trigger, "trigger")?,
            json_text(&run.scope, "scope")?,
            run.started_at,
            run.error,
        ],
    )
    .map_err(|e| format!("automation_insert_run: {e}"))?;
    Ok(run)
}

fn finish_run<R: Runtime>(
    conn: &Connection,
    app: &AppHandle<R>,
    run_id: &str,
    status: AutomationRunStatus,
    error: Option<String>,
) -> Result<AutomationRun, String> {
    update_run_status(conn, app, run_id, status, error, true)
}

fn update_run_status<R: Runtime>(
    conn: &Connection,
    app: &AppHandle<R>,
    run_id: &str,
    status: AutomationRunStatus,
    error: Option<String>,
    finished: bool,
) -> Result<AutomationRun, String> {
    let now = now_millis();
    let finished_at = finished.then_some(now);
    conn.execute(
        r#"UPDATE automation_runs
           SET status = ?1, finished_at = ?2, error = ?3
           WHERE id = ?4"#,
        params![status.as_str(), finished_at, error, run_id],
    )
    .map_err(|e| format!("automation_update_run_status: {e}"))?;
    let run = run_by_id(conn, run_id)?
        .ok_or_else(|| "automation_update_run_status: run 不存在".to_string())?;
    emit_run(
        app,
        if finished {
            "automation:run-finished"
        } else {
            "automation:run-updated"
        },
        run.clone(),
    );
    Ok(run)
}

fn insert_node_state(
    conn: &Connection,
    run_id: &str,
    node_id: &str,
    status: AutomationRunStatus,
    input: JsonValue,
    output: Option<JsonValue>,
    error: Option<String>,
    finished_at: Option<i64>,
) -> Result<(), String> {
    let now = now_millis();
    conn.execute(
        r#"INSERT OR REPLACE INTO automation_run_nodes
           (id, run_id, node_id, status, input_json, output_json, error, started_at, finished_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        params![
            format!("{run_id}:{node_id}"),
            run_id,
            node_id,
            status.as_str(),
            json_text(&input, "node input")?,
            output
                .map(|value| json_text(&value, "node output"))
                .transpose()?,
            error,
            now,
            finished_at,
        ],
    )
    .map_err(|e| format!("automation_node_state: {e}"))?;
    Ok(())
}

fn update_node_state(
    conn: &Connection,
    run_id: &str,
    node_id: &str,
    status: AutomationRunStatus,
    input: JsonValue,
    output: Option<JsonValue>,
    error: Option<String>,
) -> Result<(), String> {
    insert_node_state(
        conn,
        run_id,
        node_id,
        status,
        input,
        output,
        error,
        Some(now_millis()),
    )
}

fn execute_graph<R: Runtime>(
    app: &AppHandle<R>,
    chat_store: &ChatStore,
    conn: &Connection,
    run: &AutomationRun,
    draft: &AutomationDraft,
    resume_from_node: Option<String>,
    mut outputs: BTreeMap<String, JsonValue>,
) -> Result<GraphExecution, String> {
    let ordered = topological_order(&draft.nodes, &draft.edges)?;
    let node_map: BTreeMap<String, AutomationNode> = draft
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.clone()))
        .collect();
    let mut active_nodes = resume_from_node
        .as_deref()
        .and_then(|node_id| node_map.get(node_id))
        .map(|node| {
            active_outgoing_edges(
                &draft.edges,
                &node.id,
                outputs
                    .get(&node.id)
                    .and_then(|value| value.get("output"))
                    .unwrap_or(&JsonValue::Null),
            )
            .into_iter()
            .map(|edge| edge.target.clone())
            .collect()
        })
        .unwrap_or_else(|| initial_active_nodes(&draft.nodes, &draft.edges));
    let mut reached_nodes: BTreeSet<String> = outputs.keys().cloned().collect();
    for node_id in ordered {
        let Some(node) = node_map.get(&node_id) else {
            continue;
        };
        if outputs.contains_key(&node.id) {
            continue;
        }
        if !active_nodes.contains(&node.id) {
            continue;
        }
        let input = serde_json::json!({
            "trigger": run.trigger,
            "nodes": outputs,
            "config": node.config,
        });
        update_node_state(
            conn,
            &run.id,
            &node.id,
            AutomationRunStatus::Running,
            input.clone(),
            None,
            None,
        )?;
        let output = match execute_node(app, chat_store, conn, run, node, &input) {
            Ok(output) => output,
            Err(err) => {
                update_node_state(
                    conn,
                    &run.id,
                    &node.id,
                    AutomationRunStatus::Failed,
                    input,
                    None,
                    Some(err.clone()),
                )?;
                return Err(err);
            }
        };
        if output
            .get("waitingUser")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            update_node_state(
                conn,
                &run.id,
                &node.id,
                AutomationRunStatus::WaitingUser,
                input,
                Some(output),
                None,
            )?;
            return Ok(GraphExecution::WaitingUser);
        }
        if output
            .get("waitingAgent")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            update_node_state(
                conn,
                &run.id,
                &node.id,
                AutomationRunStatus::Running,
                input,
                Some(output),
                None,
            )?;
            return Ok(GraphExecution::WaitingAgent);
        }
        let next_status = if output
            .get("skipped")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            AutomationRunStatus::Skipped
        } else {
            AutomationRunStatus::Succeeded
        };
        reached_nodes.insert(node.id.clone());
        outputs.insert(
            node.id.clone(),
            serde_json::json!({
                "status": next_status.as_str(),
                "output": output.clone(),
            }),
        );
        update_node_state(
            conn,
            &run.id,
            &node.id,
            next_status,
            input,
            Some(output.clone()),
            None,
        )?;
        if output
            .get("stopped")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            break;
        }
        for edge in active_outgoing_edges(&draft.edges, &node.id, &output) {
            active_nodes.insert(edge.target.clone());
        }
    }
    let skipped_input = serde_json::json!({
        "trigger": run.trigger,
        "nodes": outputs,
    });
    for node in &draft.nodes {
        if !reached_nodes.contains(&node.id) {
            update_node_state(
                conn,
                &run.id,
                &node.id,
                AutomationRunStatus::Skipped,
                skipped_input.clone(),
                Some(serde_json::json!({ "skipped": true, "reason": "分支未命中" })),
                None,
            )?;
        }
    }
    Ok(GraphExecution::Finished)
}

fn execute_node<R: Runtime>(
    app: &AppHandle<R>,
    chat_store: &ChatStore,
    conn: &Connection,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    match node.kind.as_str() {
        "trigger" => Ok(serde_json::json!({ "triggered": true })),
        "logic" => execute_logic_node(node, input),
        "human" => execute_human_node(node, input),
        "tool" => execute_tool_node(app, conn, run, node, input),
        "agent" => execute_agent_node(app, chat_store, run, node, input),
        other => Err(format!("未知自动化节点类型：{other}")),
    }
}

fn execute_human_node(node: &AutomationNode, input: &JsonValue) -> Result<JsonValue, String> {
    Ok(serde_json::json!({
        "waitingUser": true,
        "prompt": render_template(
            node.config
                .get("prompt")
                .and_then(|value| value.as_str())
                .unwrap_or("确认后继续执行自动化。"),
            input,
        ),
    }))
}

fn execute_logic_node(node: &AutomationNode, input: &JsonValue) -> Result<JsonValue, String> {
    let logic = node
        .config
        .get("logic")
        .and_then(|value| value.as_str())
        .unwrap_or("condition");
    match logic {
        "stop" => Ok(serde_json::json!({ "stopped": true })),
        "condition" => {
            let path = node
                .config
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or("trigger.kind");
            let expected = node.config.get("equals").and_then(|value| value.as_str());
            let actual = json_path(input, path);
            let passed = match expected {
                Some(expected) => actual
                    .map(|value| json_value_to_string(value) == expected)
                    .unwrap_or(false),
                None => actual.map(json_value_truthy).unwrap_or(false),
            };
            Ok(serde_json::json!({
                "passed": passed,
                "selectedHandle": if passed { "true" } else { "false" },
            }))
        }
        "switch" => {
            let path = node
                .config
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or("trigger.kind");
            let value = json_path(input, path).cloned().unwrap_or(JsonValue::Null);
            let selected_handle = json_value_to_port(&value);
            Ok(serde_json::json!({
                "value": value,
                "selectedHandle": selected_handle,
                "routeKind": "switch",
            }))
        }
        other => Err(format!("未知逻辑节点类型：{other}")),
    }
}

fn execute_tool_node<R: Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    let action = node
        .config
        .get("action")
        .and_then(|value| value.as_str())
        .unwrap_or("record_timeline");
    match action {
        "create_task" => create_task_from_tool(app, conn, run, node, input),
        "update_task_status" => update_task_status_from_tool(app, conn, run, node, input),
        "add_todo" => add_todo_from_tool(app, conn, run, node, input),
        "send_guide" => send_guide_from_tool(app, conn, run, node, input),
        "record_timeline" => record_timeline_from_tool(app, run, node, input),
        other => Err(format!("未知工具节点动作：{other}")),
    }
}

fn create_task_from_tool<R: Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    let id = Uuid::new_v4().to_string();
    let title = render_template(
        node.config
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("自动化任务"),
        input,
    );
    let project_id = optional_rendered_string(node.config.get("projectId"), input)
        .or_else(|| run.trigger.project_id.clone());
    let status = optional_rendered_string(node.config.get("status"), input)
        .unwrap_or_else(|| "waiting".to_string());
    let now = now_millis();
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks WHERE project_id IS ?1",
            params![project_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("automation create_task: sort_order 失败：{e}"))?;
    conn.execute(
        r#"INSERT INTO tasks
           (id, project_id, session_id, title, title_source, status, created_at, parent_id, archived, sort_order, pinned)
           VALUES (?1, ?2, ?3, ?4, 'auto', ?5, ?6, NULL, 0, ?7, 0)"#,
        params![id, project_id, id, title, status, now, sort_order],
    )
    .map_err(|e| format!("automation create_task: insert 失败：{e}"))?;
    let _ = app.emit(
        "tasks:changed",
        serde_json::json!({ "projectId": project_id.clone() }),
    );
    emit_task_changed_signal(
        app,
        project_id.clone(),
        Some(id.clone()),
        Some(status.clone()),
        "task_created",
        Some(run.id.clone()),
    );
    Ok(serde_json::json!({ "taskId": id, "projectId": project_id, "status": status }))
}

fn update_task_status_from_tool<R: Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    let task_id = optional_rendered_string(node.config.get("taskId"), input)
        .or_else(|| run.trigger.task_id.clone())
        .ok_or_else(|| "update_task_status 需要 taskId".to_string())?;
    let status = render_template(
        node.config
            .get("status")
            .and_then(|value| value.as_str())
            .unwrap_or("waiting"),
        input,
    );
    conn.execute(
        "UPDATE tasks SET status = ?1 WHERE id = ?2",
        params![status, task_id],
    )
    .map_err(|e| format!("automation update_task_status: {e}"))?;
    let project_id = conn
        .query_row(
            "SELECT project_id FROM tasks WHERE id = ?1 AND archived = 0",
            params![task_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| format!("automation update_task_status: 查询项目失败：{e}"))?
        .flatten();
    let _ = app.emit(
        "tasks:changed",
        serde_json::json!({ "projectId": project_id.clone() }),
    );
    emit_task_changed_signal(
        app,
        project_id.clone(),
        Some(task_id.clone()),
        Some(status.clone()),
        "task_status_changed",
        Some(run.id.clone()),
    );
    Ok(serde_json::json!({ "taskId": task_id, "projectId": project_id, "status": status }))
}

fn add_todo_from_tool<R: Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    let task_id = optional_rendered_string(node.config.get("taskId"), input)
        .or_else(|| run.trigger.task_id.clone())
        .ok_or_else(|| "add_todo 需要 taskId".to_string())?;
    let text = render_template(
        node.config
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or("自动化 Todo"),
        input,
    );
    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    insert_task_todo_row(conn, &id, &task_id, &text, "agent", "normal", None, now)?;
    let _ = app.emit(
        "todo-changed",
        serde_json::json!({ "taskId": task_id.clone() }),
    );
    emit_todo_signal(app, task_id.clone(), Some(run.id.clone()));
    Ok(serde_json::json!({ "todoId": id, "taskId": task_id }))
}

fn send_guide_from_tool<R: Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    let task_id = optional_rendered_string(node.config.get("taskId"), input)
        .or_else(|| run.trigger.task_id.clone())
        .ok_or_else(|| "send_guide 需要 taskId".to_string())?;
    let text = render_template(
        node.config
            .get("text")
            .or_else(|| node.config.get("title"))
            .and_then(|value| value.as_str())
            .unwrap_or("自动化引导"),
        input,
    );
    let priority = normalize_task_priority(
        node.config
            .get("priority")
            .and_then(|value| value.as_str())
            .unwrap_or("normal"),
    );
    let id = Uuid::new_v4().to_string();
    let now = now_millis();
    insert_task_todo_row(
        conn,
        &id,
        &task_id,
        &text,
        "lilia",
        priority,
        Some("pending"),
        now,
    )?;
    let _ = app.emit(
        "todo-changed",
        serde_json::json!({ "taskId": task_id.clone() }),
    );
    emit_todo_signal(app, task_id.clone(), Some(run.id.clone()));
    Ok(serde_json::json!({
        "guideId": id,
        "taskId": task_id,
        "priority": priority,
        "guideStatus": "pending",
    }))
}

fn insert_task_todo_row(
    conn: &Connection,
    id: &str,
    task_id: &str,
    text: &str,
    source: &str,
    priority: &str,
    guide_status: Option<&str>,
    now: i64,
) -> Result<(), String> {
    let order: i64 = conn
        .query_row(
            r#"SELECT COALESCE(MAX("order"), -1) + 1 FROM task_todos WHERE task_id = ?1"#,
            params![task_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("automation todo: order 失败：{e}"))?;
    conn.execute(
        r#"INSERT INTO task_todos
           (id, task_id, text, done, "order", source, priority, guide_status, attachments_json, created_at, updated_at)
           VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, '[]', ?8, ?9)"#,
        params![id, task_id, text, order, source, priority, guide_status, now, now],
    )
    .map_err(|e| format!("automation todo: insert 失败：{e}"))?;
    Ok(())
}

fn normalize_task_priority(value: &str) -> &'static str {
    match value {
        "high" => "high",
        "low" => "low",
        _ => "normal",
    }
}

fn record_timeline_from_tool<R: Runtime>(
    app: &AppHandle<R>,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    let timeline_input = timeline_input_from_tool(run, node, input)?;
    let event_id = timeline_input.id.clone();
    persist_and_emit_input(app, timeline_input);
    Ok(serde_json::json!({
        "recorded": true,
        "timelineEventId": event_id,
        "automationRunId": run.id,
    }))
}

fn timeline_input_from_tool(
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<AgentTimelineEventInput, String> {
    let task_id = optional_rendered_string(node.config.get("taskId"), input)
        .or_else(|| run.trigger.task_id.clone())
        .ok_or_else(|| "record_timeline 需要 taskId".to_string())?;
    let title = render_template(
        node.config
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("自动化记录"),
        input,
    );
    let summary =
        optional_rendered_string(node.config.get("summary"), input).or(Some(title.clone()));
    let status = optional_rendered_string(node.config.get("status"), input)
        .unwrap_or_else(|| "info".to_string());
    let backend = optional_rendered_string(node.config.get("backend"), input)
        .or_else(|| run.trigger.backend.clone())
        .filter(|value| value == BACKEND_CODEX || value == BACKEND_CLAUDE)
        .unwrap_or_else(|| BACKEND_CLAUDE.to_string());
    let now = now_millis();
    Ok(AgentTimelineEventInput {
        id: Some(format!("automation:{}:{}", run.id, node.id)),
        task_id,
        turn_id: None,
        backend,
        kind: "automation".to_string(),
        status,
        title,
        summary,
        payload: serde_json::json!({
            "automationRunId": run.id,
            "workflowId": run.workflow_id,
            "workflowVersionId": run.workflow_version_id,
            "nodeId": node.id,
        }),
        created_at: Some(now),
        updated_at: Some(now),
    })
}

fn execute_agent_node<R: Runtime>(
    app: &AppHandle<R>,
    chat_store: &ChatStore,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<JsonValue, String> {
    let create_task = node
        .config
        .get("createTask")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let task_id = if create_task {
        create_agent_target_task(app, run, node, input)?
    } else {
        optional_rendered_string(node.config.get("taskId"), input)
            .or_else(|| run.trigger.task_id.clone())
            .ok_or_else(|| "Agent 节点需要 taskId".to_string())?
    };
    let content = render_template(
        node.config
            .get("prompt")
            .and_then(|value| value.as_str())
            .unwrap_or("请根据当前上下文继续推进。"),
        input,
    );
    let backend = node
        .config
        .get("backend")
        .and_then(|value| value.as_str())
        .filter(|value| *value == BACKEND_CODEX || *value == BACKEND_CLAUDE)
        .unwrap_or(BACKEND_CLAUDE)
        .to_string();
    let model = node
        .config
        .get("model")
        .and_then(|value| value.as_str())
        .unwrap_or(if backend == BACKEND_CODEX {
            "gpt-5.5"
        } else {
            "claude-sonnet-4-6"
        })
        .to_string();
    let permission = node
        .config
        .get("permission")
        .and_then(|value| value.as_str())
        .unwrap_or("ask")
        .to_string();
    let project_cwd = node
        .config
        .get("projectCwd")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let composer = ChatComposerState {
        task_id: task_id.clone(),
        backend,
        model,
        plan_mode: false,
        permission,
        codex_settings: Default::default(),
    };
    let result = dispatch_agent_turn(
        app,
        chat_store,
        run.id.clone(),
        task_id.clone(),
        content,
        composer,
        project_cwd,
    )?;
    Ok(serde_json::json!({
        "waitingAgent": true,
        "taskId": task_id,
        "turnId": result.turn_id,
        "dispatch": result.dispatch,
        "queuedCount": result.queued_count,
        "messageId": result.message.id,
    }))
}

fn create_agent_target_task<R: Runtime>(
    app: &AppHandle<R>,
    run: &AutomationRun,
    node: &AutomationNode,
    input: &JsonValue,
) -> Result<String, String> {
    let conn = app.state::<LiliaStore>().conn()?;
    let id = Uuid::new_v4().to_string();
    let title = render_template(
        node.config
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("自动化 Agent 任务"),
        input,
    );
    let project_id = optional_rendered_string(node.config.get("projectId"), input)
        .or_else(|| run.trigger.project_id.clone());
    let now = now_millis();
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks WHERE project_id IS ?1",
            params![project_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("automation agent create_task: sort_order 失败：{e}"))?;
    conn.execute(
        r#"INSERT INTO tasks
           (id, project_id, session_id, title, title_source, status, created_at, parent_id, archived, sort_order, pinned)
           VALUES (?1, ?2, ?3, ?4, 'auto', 'running', ?5, NULL, 0, ?6, 0)"#,
        params![id, project_id, id, title, now, sort_order],
    )
    .map_err(|e| format!("automation agent create_task: insert 失败：{e}"))?;
    let _ = app.emit(
        "tasks:changed",
        serde_json::json!({ "projectId": project_id.clone() }),
    );
    emit_task_changed_signal(
        app,
        project_id,
        Some(id.clone()),
        Some("running".to_string()),
        "task_created",
        Some(run.id.clone()),
    );
    Ok(id)
}

fn dispatch_agent_turn<R: Runtime>(
    app: &AppHandle<R>,
    store: &ChatStore,
    automation_run_id: String,
    task_id: String,
    content: String,
    composer: ChatComposerState,
    project_cwd: String,
) -> Result<ChatSendResult, String> {
    let backend = composer.backend.clone();
    let composer = normalize_composer_for_backend(composer, &task_id, &backend);
    let workflow = Some(ChatWorkflow::Automation {
        automation_run_id: automation_run_id.clone(),
    });
    let message = ChatMessage {
        id: new_chat_message_id(),
        task_id: task_id.clone(),
        role: "user".to_string(),
        content: content.clone(),
        attachments: Vec::new(),
        created_at: now_millis() as u64,
    };
    let turn_id = format!("turn-{}", now_millis());
    store
        .composers
        .lock()
        .unwrap()
        .insert(task_id.clone(), composer.clone());

    {
        let mut running = store.running_tasks.lock().unwrap();
        if running.contains_key(&task_id) {
            let runtime_channel = store
                .running_turns
                .lock()
                .unwrap()
                .get(&task_id)
                .map(|turn| turn.runtime_channel.clone())
                .unwrap_or_else(|| crate::RUNTIME_CHANNEL_BUILTIN.to_string());
            drop(running);
            let queued_count = queue_pending_turn_for_app(
                app,
                store,
                &task_id,
                content,
                composer.clone(),
                project_cwd,
                Vec::new(),
                workflow.clone(),
                message.clone(),
                turn_id.clone(),
                runtime_channel,
                None,
            );
            if should_persist_user_message(&message.content, &workflow) {
                persist_and_emit_message_timeline_event(
                    app,
                    &message,
                    &composer.backend,
                    &turn_id,
                    true,
                    Some(&automation_run_id),
                );
            }
            return Ok(ChatSendResult {
                message,
                dispatch: "queued".to_string(),
                queued_count,
                turn_id,
            });
        }
        running.insert(task_id.clone(), true);
    }

    if should_persist_user_message(&message.content, &workflow) {
        persist_and_emit_message_timeline_event(
            app,
            &message,
            &composer.backend,
            &turn_id,
            false,
            Some(&automation_run_id),
        );
    }
    spawn_agent_turn(
        app.clone(),
        task_id,
        content,
        composer,
        project_cwd,
        Vec::new(),
        workflow,
        turn_id.clone(),
    );
    Ok(ChatSendResult {
        message,
        dispatch: "started".to_string(),
        queued_count: 0,
        turn_id,
    })
}

fn optional_rendered_string(value: Option<&JsonValue>, input: &JsonValue) -> Option<String> {
    value
        .and_then(|value| value.as_str())
        .map(|value| render_template(value, input))
        .filter(|value| !value.trim().is_empty())
}

fn render_template(template: &str, input: &JsonValue) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        let (prefix, tail) = rest.split_at(start);
        out.push_str(prefix);
        let Some(end) = tail.find('}') else {
            out.push_str(tail);
            return out;
        };
        let path = &tail[2..end];
        if let Some(value) = json_path(input, path) {
            out.push_str(&json_value_to_string(value));
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

fn json_path<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn json_value_to_string(value: &JsonValue) -> String {
    value
        .as_str()
        .map(|value| value.to_string())
        .unwrap_or_else(|| value.to_string())
}

fn json_value_to_port(value: &JsonValue) -> String {
    json_value_to_string(value)
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn json_value_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Bool(value) => *value,
        JsonValue::Number(value) => value.as_i64().unwrap_or(0) != 0,
        JsonValue::String(value) => !value.trim().is_empty() && value != "false",
        JsonValue::Array(value) => !value.is_empty(),
        JsonValue::Object(value) => !value.is_empty(),
        JsonValue::Null => false,
    }
}

fn initial_active_nodes(nodes: &[AutomationNode], edges: &[AutomationEdge]) -> BTreeSet<String> {
    let trigger_nodes: BTreeSet<String> = nodes
        .iter()
        .filter(|node| node.kind == "trigger")
        .map(|node| node.id.clone())
        .collect();
    if !trigger_nodes.is_empty() {
        return trigger_nodes;
    }
    let targets: BTreeSet<String> = edges.iter().map(|edge| edge.target.clone()).collect();
    nodes
        .iter()
        .filter(|node| !targets.contains(&node.id))
        .map(|node| node.id.clone())
        .collect()
}

fn active_outgoing_edges<'a>(
    edges: &'a [AutomationEdge],
    node_id: &str,
    output: &JsonValue,
) -> Vec<&'a AutomationEdge> {
    let outgoing: Vec<&AutomationEdge> =
        edges.iter().filter(|edge| edge.source == node_id).collect();
    if outgoing.is_empty() {
        return Vec::new();
    }
    let selected_handles = selected_output_handles(output);
    let has_exact_selected_edge = outgoing.iter().any(|edge| {
        edge.source_handle
            .as_deref()
            .map(normalize_handle)
            .filter(|handle| handle != "default")
            .map(|handle| selected_handles.contains(&handle))
            .unwrap_or(false)
    });
    outgoing
        .into_iter()
        .filter(|edge| {
            let handle = edge.source_handle.as_deref().map(normalize_handle);
            match handle.as_deref() {
                Some("default") => {
                    output.get("routeKind").and_then(|value| value.as_str()) == Some("switch")
                        && !has_exact_selected_edge
                }
                Some(handle) => selected_handles.contains(handle),
                None => {
                    selected_handles.is_empty()
                        || selected_handles.contains("success")
                        || selected_handles.contains("true")
                }
            }
        })
        .collect()
}

fn selected_output_handles(output: &JsonValue) -> BTreeSet<String> {
    let mut handles = BTreeSet::new();
    if let Some(handle) = output
        .get("selectedHandle")
        .and_then(|value| value.as_str())
    {
        let normalized = normalize_handle(handle);
        if !normalized.is_empty() {
            handles.insert(normalized);
        }
    }
    if let Some(items) = output
        .get("selectedHandles")
        .and_then(|value| value.as_array())
    {
        for item in items {
            if let Some(handle) = item.as_str() {
                let normalized = normalize_handle(handle);
                if !normalized.is_empty() {
                    handles.insert(normalized);
                }
            }
        }
    }
    handles
}

fn normalize_handle(handle: &str) -> String {
    handle.trim().to_ascii_lowercase()
}

fn validate_workflow_graph(
    nodes: &[AutomationNode],
    edges: &[AutomationEdge],
) -> Result<(), String> {
    let ids: BTreeSet<String> = nodes.iter().map(|node| node.id.clone()).collect();
    if ids.len() != nodes.len() {
        return Err("自动化节点 id 不能重复".to_string());
    }
    let trigger_count = nodes.iter().filter(|node| node.kind == "trigger").count();
    if !nodes.is_empty() && trigger_count == 0 {
        return Err("MVP 自动化需要一个触发节点".to_string());
    }
    if trigger_count > 1 {
        return Err("MVP 自动化只允许一个触发节点".to_string());
    }
    for edge in edges {
        if !ids.contains(&edge.source) || !ids.contains(&edge.target) {
            return Err("自动化连线引用了不存在的节点".to_string());
        }
    }
    topological_order(nodes, edges).map(|_| ())
}

fn topological_order(
    nodes: &[AutomationNode],
    edges: &[AutomationEdge],
) -> Result<Vec<String>, String> {
    let mut incoming: BTreeMap<String, usize> =
        nodes.iter().map(|node| (node.id.clone(), 0)).collect();
    let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in edges {
        *incoming.entry(edge.target.clone()).or_default() += 1;
        outgoing
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
    }
    let mut ready: VecDeque<String> = incoming
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect();
    let mut ordered = Vec::new();
    while let Some(id) = ready.pop_front() {
        ordered.push(id.clone());
        for target in outgoing.get(&id).cloned().unwrap_or_default() {
            if let Some(count) = incoming.get_mut(&target) {
                *count -= 1;
                if *count == 0 {
                    ready.push_back(target);
                }
            }
        }
    }
    if ordered.len() != nodes.len() {
        return Err("自动化工作流不能包含循环".to_string());
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn node(id: &str, kind: &str) -> AutomationNode {
        AutomationNode {
            id: id.to_string(),
            kind: kind.to_string(),
            title: id.to_string(),
            position: AutomationNodePosition { x: 0.0, y: 0.0 },
            config: serde_json::json!({}),
        }
    }

    fn edge(source: &str, target: &str) -> AutomationEdge {
        AutomationEdge {
            id: format!("{source}-{target}"),
            source: source.to_string(),
            target: target.to_string(),
            source_handle: None,
            target_handle: None,
        }
    }

    fn edge_with_handle(source: &str, target: &str, source_handle: &str) -> AutomationEdge {
        AutomationEdge {
            id: format!("{source}-{source_handle}-{target}"),
            source: source.to_string(),
            target: target.to_string(),
            source_handle: Some(source_handle.to_string()),
            target_handle: None,
        }
    }

    fn create_workflow_tables(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE automation_workflows (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                scope_json TEXT NOT NULL,
                draft_json TEXT NOT NULL,
                published_version_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE automation_workflow_versions (
                id TEXT PRIMARY KEY NOT NULL,
                workflow_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                snapshot_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE automation_runs (
                id TEXT PRIMARY KEY NOT NULL,
                workflow_id TEXT NOT NULL,
                workflow_version_id TEXT NOT NULL,
                status TEXT NOT NULL,
                trigger_json TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                error TEXT
            );
            "#,
        )
        .unwrap();
    }

    #[test]
    fn graph_validation_rejects_cycles() {
        let nodes = vec![node("a", "trigger"), node("b", "tool")];
        let edges = vec![edge("a", "b"), edge("b", "a")];

        assert!(validate_workflow_graph(&nodes, &edges)
            .unwrap_err()
            .contains("循环"));
    }

    #[test]
    fn graph_validation_requires_single_trigger_for_non_empty_workflow() {
        assert!(validate_workflow_graph(&[], &[]).is_ok());

        let nodes = vec![node("a", "tool"), node("b", "logic")];
        assert!(validate_workflow_graph(&nodes, &[])
            .unwrap_err()
            .contains("触发节点"));

        let nodes = vec![
            node("first", "trigger"),
            node("second", "trigger"),
            node("tool", "tool"),
        ];
        assert!(validate_workflow_graph(&nodes, &[])
            .unwrap_err()
            .contains("只允许一个触发节点"));
    }

    #[test]
    fn scope_matches_backend_and_event_kind() {
        let scope = AutomationScopeFilter {
            project_ids: Vec::new(),
            include_inbox: true,
            task_statuses: Vec::new(),
            backends: vec!["codex".to_string()],
            event_kinds: vec!["tool".to_string()],
        };
        let signal = AutomationSignalEnvelope {
            id: "s1".to_string(),
            kind: "timeline_event".to_string(),
            project_id: None,
            task_id: Some("task-1".to_string()),
            backend: Some("codex".to_string()),
            event_kind: Some("tool".to_string()),
            automation_run_id: None,
            payload: serde_json::json!({}),
            created_at: 1,
        };

        assert!(scope_matches(&scope, &signal));
    }

    #[test]
    fn task_changed_signal_keeps_trigger_kind_and_splits_event_kind() {
        let signal = task_changed_signal(
            Some("project-1".to_string()),
            Some("task-1".to_string()),
            Some("done".to_string()),
            "task_status_changed",
            None,
        );

        assert_eq!(signal.kind, "task_changed");
        assert_eq!(signal.event_kind.as_deref(), Some("task_status_changed"));
        assert_eq!(signal.payload["eventKind"], "task_status_changed");
        assert_eq!(signal.payload["taskStatus"], "done");

        let fallback = task_changed_signal(None, None, None, " ", None);
        assert_eq!(fallback.event_kind.as_deref(), Some("task_changed"));
    }

    #[test]
    fn enabled_workflows_match_published_snapshot_not_draft() {
        let conn = Connection::open_in_memory().unwrap();
        create_workflow_tables(&conn);
        let draft = AutomationDraft {
            nodes: vec![AutomationNode {
                id: "draft-trigger".to_string(),
                kind: "trigger".to_string(),
                title: "Draft".to_string(),
                position: AutomationNodePosition { x: 0.0, y: 0.0 },
                config: serde_json::json!({ "triggerKind": "manual" }),
            }],
            edges: Vec::new(),
            scope: AutomationScopeFilter {
                include_inbox: true,
                backends: vec!["codex".to_string()],
                ..Default::default()
            },
        };
        let snapshot = AutomationDraft {
            nodes: vec![AutomationNode {
                id: "published-trigger".to_string(),
                kind: "trigger".to_string(),
                title: "Published".to_string(),
                position: AutomationNodePosition { x: 0.0, y: 0.0 },
                config: serde_json::json!({ "triggerKind": "task_changed" }),
            }],
            edges: Vec::new(),
            scope: AutomationScopeFilter {
                include_inbox: true,
                ..Default::default()
            },
        };
        conn.execute(
            r#"INSERT INTO automation_workflows
               (id, name, enabled, scope_json, draft_json, published_version_id, created_at, updated_at)
               VALUES ('wf-1', 'Workflow', 1, ?1, ?2, 'ver-1', 1, 1)"#,
            params![
                json_text(&draft.scope, "scope").unwrap(),
                json_text(&draft, "draft").unwrap()
            ],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO automation_workflow_versions
               (id, workflow_id, version, snapshot_json, created_at)
               VALUES ('ver-1', 'wf-1', 1, ?1, 1)"#,
            params![json_text(&snapshot, "snapshot").unwrap()],
        )
        .unwrap();
        let signal = AutomationSignalEnvelope {
            id: "signal-1".to_string(),
            kind: "task_changed".to_string(),
            project_id: None,
            task_id: Some("task-1".to_string()),
            backend: None,
            event_kind: Some("task_changed".to_string()),
            automation_run_id: None,
            payload: serde_json::json!({ "taskStatus": "waiting" }),
            created_at: 1,
        };

        let workflows = enabled_workflows_for_signal(&conn, &signal).unwrap();

        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].id, "wf-1");
    }

    #[test]
    fn task_status_signal_enriches_context_and_matches_global_scope() {
        let conn = Connection::open_in_memory().unwrap();
        create_workflow_tables(&conn);
        conn.execute_batch(
            r#"
            CREATE TABLE tasks (
                id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT,
                status TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO tasks (id, project_id, status, archived)
            VALUES ('task-1', 'project-1', 'done', 0);
            "#,
        )
        .unwrap();
        let draft = AutomationDraft {
            nodes: vec![AutomationNode {
                id: "trigger-1".to_string(),
                kind: "trigger".to_string(),
                title: "任务变化".to_string(),
                position: AutomationNodePosition { x: 0.0, y: 0.0 },
                config: serde_json::json!({ "triggerKind": "task_changed" }),
            }],
            edges: Vec::new(),
            scope: AutomationScopeFilter {
                include_inbox: false,
                project_ids: vec!["project-1".to_string()],
                task_statuses: vec!["done".to_string()],
                event_kinds: vec!["task_status_changed".to_string()],
                ..Default::default()
            },
        };
        conn.execute(
            r#"INSERT INTO automation_workflows
               (id, name, enabled, scope_json, draft_json, published_version_id, created_at, updated_at)
               VALUES ('wf-1', 'Done workflow', 1, ?1, ?2, 'ver-1', 1, 1)"#,
            params![
                json_text(&draft.scope, "scope").unwrap(),
                json_text(&draft, "draft").unwrap()
            ],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO automation_workflow_versions
               (id, workflow_id, version, snapshot_json, created_at)
               VALUES ('ver-1', 'wf-1', 1, ?1, 1)"#,
            params![json_text(&draft, "snapshot").unwrap()],
        )
        .unwrap();
        let mut signal = AutomationSignalEnvelope {
            id: "signal-1".to_string(),
            kind: "task_changed".to_string(),
            project_id: None,
            task_id: Some("task-1".to_string()),
            backend: None,
            event_kind: Some("task_status_changed".to_string()),
            automation_run_id: None,
            payload: serde_json::json!({}),
            created_at: 1,
        };

        enrich_signal_context(&conn, &mut signal).unwrap();
        let workflows = enabled_workflows_for_signal(&conn, &signal).unwrap();

        assert_eq!(signal.project_id.as_deref(), Some("project-1"));
        assert_eq!(signal.payload["taskStatus"], "done");
        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].id, "wf-1");
    }

    #[test]
    fn inserted_run_scope_uses_published_snapshot() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE automation_runs (
                id TEXT PRIMARY KEY NOT NULL,
                workflow_id TEXT NOT NULL,
                workflow_version_id TEXT NOT NULL,
                status TEXT NOT NULL,
                trigger_json TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                error TEXT
            );
            "#,
        )
        .unwrap();
        let workflow = AutomationWorkflow {
            id: "wf-1".to_string(),
            name: "Workflow".to_string(),
            enabled: true,
            scope: AutomationScopeFilter {
                include_inbox: true,
                backends: vec!["codex".to_string()],
                ..Default::default()
            },
            draft: AutomationDraft::default(),
            published_version_id: Some("ver-1".to_string()),
            created_at: 1,
            updated_at: 1,
        };
        let version = AutomationWorkflowVersion {
            id: "ver-1".to_string(),
            workflow_id: "wf-1".to_string(),
            version: 1,
            snapshot: AutomationDraft {
                nodes: Vec::new(),
                edges: Vec::new(),
                scope: AutomationScopeFilter {
                    include_inbox: false,
                    project_ids: vec!["project-1".to_string()],
                    ..Default::default()
                },
            },
            created_at: 1,
        };

        let run = insert_run(
            &conn,
            &workflow,
            &version,
            manual_signal(None),
            AutomationRunStatus::Running,
            None,
        )
        .unwrap();

        assert_eq!(run.scope, version.snapshot.scope);
        assert_ne!(run.scope, workflow.scope);
    }

    #[test]
    fn tool_todo_rows_distinguish_agent_todos_from_guides() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE task_todos (
                id TEXT PRIMARY KEY NOT NULL,
                task_id TEXT NOT NULL,
                text TEXT NOT NULL,
                done INTEGER NOT NULL,
                "order" INTEGER NOT NULL,
                source TEXT NOT NULL CHECK (source IN ('lilia','agent')),
                priority TEXT NOT NULL,
                guide_status TEXT,
                attachments_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();

        insert_task_todo_row(
            &conn,
            "todo-1",
            "task-1",
            "普通 Todo",
            "agent",
            "normal",
            None,
            1,
        )
        .unwrap();
        insert_task_todo_row(
            &conn,
            "guide-1",
            "task-1",
            "引导消息",
            "lilia",
            "high",
            Some("pending"),
            2,
        )
        .unwrap();

        let todo = conn
            .query_row(
                "SELECT source, priority, guide_status FROM task_todos WHERE id = 'todo-1'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();
        let guide = conn
            .query_row(
                "SELECT source, priority, guide_status FROM task_todos WHERE id = 'guide-1'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(todo, ("agent".to_string(), "normal".to_string(), None));
        assert_eq!(
            guide,
            (
                "lilia".to_string(),
                "high".to_string(),
                Some("pending".to_string())
            )
        );
    }

    #[test]
    fn record_timeline_tool_targets_task_and_marks_automation_origin() {
        let run = AutomationRun {
            id: "run-1".to_string(),
            workflow_id: "wf-1".to_string(),
            workflow_version_id: "ver-1".to_string(),
            status: AutomationRunStatus::Running,
            trigger: AutomationSignalEnvelope {
                id: "signal-1".to_string(),
                kind: "manual".to_string(),
                project_id: None,
                task_id: Some("task-1".to_string()),
                backend: Some("codex".to_string()),
                event_kind: Some("manual".to_string()),
                automation_run_id: None,
                payload: serde_json::json!({}),
                created_at: 1,
            },
            scope: AutomationScopeFilter::default(),
            started_at: 1,
            finished_at: None,
            error: None,
        };
        let node = AutomationNode {
            id: "tool-1".to_string(),
            kind: "tool".to_string(),
            title: "记录".to_string(),
            position: AutomationNodePosition { x: 0.0, y: 0.0 },
            config: serde_json::json!({
                "title": "记录 ${trigger.kind}",
                "summary": "来自 ${trigger.taskId}",
                "status": "success"
            }),
        };
        let input = serde_json::json!({
            "trigger": run.trigger,
            "nodes": {},
            "config": node.config,
        });

        let event = timeline_input_from_tool(&run, &node, &input).unwrap();

        assert_eq!(event.id.as_deref(), Some("automation:run-1:tool-1"));
        assert_eq!(event.task_id, "task-1");
        assert_eq!(event.backend, "codex");
        assert_eq!(event.kind, "automation");
        assert_eq!(event.status, "success");
        assert_eq!(event.title, "记录 manual");
        assert_eq!(event.summary.as_deref(), Some("来自 task-1"));
        assert_eq!(event.payload["automationRunId"], "run-1");
    }

    #[test]
    fn template_replaces_json_paths() {
        let value = serde_json::json!({
            "trigger": { "taskId": "task-1" },
            "nodes": { "a": { "output": { "summary": "done" } } }
        });

        assert_eq!(
            render_template("任务 ${trigger.taskId}: ${nodes.a.output.summary}", &value),
            "任务 task-1: done"
        );
    }

    #[test]
    fn condition_routes_only_selected_handle() {
        let edges = vec![
            edge_with_handle("logic", "yes", "true"),
            edge_with_handle("logic", "no", "false"),
            edge("logic", "implicit"),
        ];
        let output = serde_json::json!({ "selectedHandle": "false" });
        let targets: BTreeSet<String> = active_outgoing_edges(&edges, "logic", &output)
            .into_iter()
            .map(|edge| edge.target.clone())
            .collect();

        assert!(targets.contains("no"));
        assert!(!targets.contains("yes"));
        assert!(!targets.contains("implicit"));
    }

    #[test]
    fn switch_routes_default_when_no_case_matches() {
        let edges = vec![
            edge_with_handle("switch", "matched", "done"),
            edge_with_handle("switch", "fallback", "default"),
        ];
        let output = serde_json::json!({
            "routeKind": "switch",
            "selectedHandle": "blocked",
        });
        let targets: BTreeSet<String> = active_outgoing_edges(&edges, "switch", &output)
            .into_iter()
            .map(|edge| edge.target.clone())
            .collect();

        assert!(targets.contains("fallback"));
        assert!(!targets.contains("matched"));
    }
}
