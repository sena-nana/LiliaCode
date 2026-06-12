use rusqlite::params;
use tauri::{AppHandle, Runtime, State};

use super::{
    emit_changed, execute_graph, finish_run, outputs_from_node_states, run_workflow,
    update_node_state, update_run_status, validate_workflow_graph, waiting_node_state,
};
use crate::automation::repository::{
    list_runs, list_workflows, node_states_for_run, publish_workflow, run_by_id, save_draft,
    version_by_id, workflow_by_id,
};
use crate::automation::signals::manual_signal;
use crate::automation::types::{
    AutomationRun, AutomationRunDetail, AutomationRunOnceInput, AutomationRunStatus,
    AutomationResumeRunInput, AutomationSaveDraftInput, AutomationWorkflow,
    AutomationWorkflowVersion, GraphExecution,
};
use crate::chat::state::ChatStore;
use crate::store::LiliaStore;
use crate::util::now_millis;

#[tauri::command]
pub fn automation_list_workflows(
    store: State<'_, LiliaStore>,
) -> Result<Vec<AutomationWorkflow>, String> {
    let conn = store.conn()?;
    list_workflows(&conn)
}

#[tauri::command]
pub fn automation_get_workflow(
    id: String,
    store: State<'_, LiliaStore>,
) -> Result<Option<AutomationWorkflow>, String> {
    let conn = store.conn()?;
    workflow_by_id(&conn, &id)
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

#[tauri::command]
pub fn automation_list_runs(
    workflow_id: Option<String>,
    store: State<'_, LiliaStore>,
) -> Result<Vec<AutomationRun>, String> {
    let conn = store.conn()?;
    list_runs(&conn, workflow_id.as_deref())
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
