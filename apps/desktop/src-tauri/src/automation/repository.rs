use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use uuid::Uuid;

use super::{signals::manual_signal, validate_workflow_graph};
use crate::automation::types::{
    AutomationDraft, AutomationRun, AutomationRunNodeState, AutomationRunStatus,
    AutomationRunSummary, AutomationSaveDraftInput, AutomationScopeFilter, AutomationWorkflow,
    AutomationWorkflowVersion,
};
use crate::util::now_millis;

pub(crate) fn json_text<T: Serialize>(value: &T, label: &str) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("automation: 序列化 {label} 失败：{e}"))
}

pub(crate) fn row_to_workflow(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationWorkflow> {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutomationRunTriggerSummary {
    #[serde(default = "default_manual_trigger_kind")]
    kind: String,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    event_kind: Option<String>,
}

impl Default for AutomationRunTriggerSummary {
    fn default() -> Self {
        Self {
            kind: default_manual_trigger_kind(),
            project_id: None,
            task_id: None,
            backend: None,
            event_kind: None,
        }
    }
}

fn default_manual_trigger_kind() -> String {
    "manual".to_string()
}

fn row_to_run_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationRunSummary> {
    let status: String = row.get(3)?;
    let trigger_json: String = row.get(4)?;
    let trigger =
        serde_json::from_str::<AutomationRunTriggerSummary>(&trigger_json).unwrap_or_default();
    Ok(AutomationRunSummary {
        id: row.get(0)?,
        workflow_id: row.get(1)?,
        workflow_version_id: row.get(2)?,
        status: AutomationRunStatus::from_str(&status),
        trigger_kind: trigger.kind,
        project_id: trigger.project_id,
        task_id: trigger.task_id,
        backend: trigger.backend,
        event_kind: trigger.event_kind,
        started_at: row.get(5)?,
        finished_at: row.get(6)?,
        error: row.get(7)?,
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

pub(crate) fn list_workflows(conn: &Connection) -> Result<Vec<AutomationWorkflow>, String> {
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

pub(crate) fn workflow_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<AutomationWorkflow>, String> {
    conn.query_row(
        r#"SELECT id, name, enabled, scope_json, draft_json, published_version_id, created_at, updated_at
           FROM automation_workflows
           WHERE id = ?1"#,
        params![id],
        row_to_workflow,
    )
    .optional()
    .map_err(|e| format!("automation_workflow_by_id: {e}"))
}

pub(crate) fn save_draft(
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

pub(crate) fn delete_workflow(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        r#"DELETE FROM automation_run_nodes
           WHERE run_id IN (SELECT id FROM automation_runs WHERE workflow_id = ?1)"#,
        params![id],
    )
    .map_err(|e| format!("automation_delete_workflow: delete run nodes 失败：{e}"))?;
    conn.execute("DELETE FROM automation_runs WHERE workflow_id = ?1", params![id])
        .map_err(|e| format!("automation_delete_workflow: delete runs 失败：{e}"))?;
    conn.execute(
        "DELETE FROM automation_workflow_versions WHERE workflow_id = ?1",
        params![id],
    )
    .map_err(|e| format!("automation_delete_workflow: delete versions 失败：{e}"))?;
    let deleted = conn
        .execute("DELETE FROM automation_workflows WHERE id = ?1", params![id])
        .map_err(|e| format!("automation_delete_workflow: {e}"))?;
    if deleted == 0 {
        return Err("automation_delete_workflow: 自动化不存在".to_string());
    }
    Ok(())
}

pub(crate) fn publish_workflow(
    conn: &Connection,
    id: &str,
) -> Result<AutomationWorkflowVersion, String> {
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

pub(crate) fn version_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<AutomationWorkflowVersion>, String> {
    conn.query_row(
        r#"SELECT id, workflow_id, version, snapshot_json, created_at
           FROM automation_workflow_versions WHERE id = ?1"#,
        params![id],
        row_to_version,
    )
    .optional()
    .map_err(|e| format!("automation_version_by_id: {e}"))
}

pub(crate) fn list_runs(
    conn: &Connection,
    workflow_id: Option<&str>,
) -> Result<Vec<AutomationRunSummary>, String> {
    let sql = if workflow_id.is_some() {
        r#"SELECT id, workflow_id, workflow_version_id, status, trigger_json, started_at, finished_at, error
           FROM automation_runs WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT 100"#
    } else {
        r#"SELECT id, workflow_id, workflow_version_id, status, trigger_json, started_at, finished_at, error
           FROM automation_runs ORDER BY started_at DESC LIMIT 100"#
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("automation_list_runs: prepare 失败：{e}"))?;
    let mut out = Vec::new();
    if let Some(workflow_id) = workflow_id {
        let rows = stmt
            .query_map(params![workflow_id], row_to_run_summary)
            .map_err(|e| format!("automation_list_runs: query 失败：{e}"))?;
        for row in rows {
            out.push(row.map_err(|e| format!("automation_list_runs: row 失败：{e}"))?);
        }
    } else {
        let rows = stmt
            .query_map([], row_to_run_summary)
            .map_err(|e| format!("automation_list_runs: query 失败：{e}"))?;
        for row in rows {
            out.push(row.map_err(|e| format!("automation_list_runs: row 失败：{e}"))?);
        }
    }
    Ok(out)
}

pub(crate) fn run_by_id(conn: &Connection, run_id: &str) -> Result<Option<AutomationRun>, String> {
    conn.query_row(
        r#"SELECT id, workflow_id, workflow_version_id, status, trigger_json, scope_json, started_at, finished_at, error
           FROM automation_runs WHERE id = ?1"#,
        params![run_id],
        row_to_run,
    )
    .optional()
    .map_err(|e| format!("automation_get_run: {e}"))
}

pub(crate) fn node_states_for_run(
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
