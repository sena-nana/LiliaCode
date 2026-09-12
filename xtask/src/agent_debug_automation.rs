use super::*;

fn select_relative(session: &Session, id: &str, delta: i32) -> Result {
    reveal_automation_control(session, id)?;
    let target = format!("lilia.ui.automation.{id}");
    ui_click(session, &target)?;
    let key = if delta < 0 { "ArrowUp" } else { "ArrowDown" };
    for key in
        std::iter::repeat_n(key, delta.unsigned_abs() as usize).chain(std::iter::once("Enter"))
    {
        require_ok(
            &session
                .request(&serde_json::json!({"command":"ui-key","targetId":target,"key":key}))?,
            "select automation option",
        )?;
    }
    Ok(())
}

fn workflow_runs<'a>(state: &'a Value, workflow: &str) -> Vec<&'a Value> {
    state
        .pointer("/observation/automationAuthority/runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|detail| detail["run"]["workflowId"] == workflow)
        .collect()
}

fn normalize_details(mut details: Vec<Value>) -> Vec<Value> {
    for detail in &mut details {
        if let Some(nodes) = detail["nodes"].as_array_mut() {
            nodes.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        }
    }
    details.sort_by(|a, b| a["run"]["id"].as_str().cmp(&b["run"]["id"].as_str()));
    details
}

fn normalized_runs(state: &Value, workflow: &str) -> Vec<Value> {
    normalize_details(
        workflow_runs(state, workflow)
            .into_iter()
            .cloned()
            .collect(),
    )
}

fn processed_task_creation(state: &Value, task: &str) -> bool {
    let authority = &state["observation"]["automationAuthority"];
    let cursor = authority["productCursor"].as_u64().unwrap_or(0);
    authority["taskCreatedEvents"]
        .as_array()
        .is_some_and(|events| {
            events.iter().any(|event| {
                event["taskId"] == task
                    && event["sequence"]
                        .as_u64()
                        .is_some_and(|sequence| sequence <= cursor)
            })
        })
}

fn selected_task(session: &Session) -> Result<String> {
    let state = wait_observation(session, |state| {
        state["observation"]["selectedTask"].is_string()
    })?;
    Ok(state["observation"]["selectedTask"]
        .as_str()
        .unwrap()
        .to_owned())
}

fn leave_automation_page(session: &Session) -> Result {
    let ui = wait_ui(session)?;
    if ui
        .pointer("/snapshot/targets")
        .and_then(Value::as_array)
        .is_some_and(|targets| {
            targets
                .iter()
                .any(|target| target["id"] == "lilia.ui.automation.back")
        })
    {
        ui_click(session, "lilia.ui.automation.back")?;
    }
    Ok(())
}

fn reopen_workflow(session: &Session, navigation: &str, workflow: &str) -> Result {
    leave_automation_page(session)?;
    ui_click(session, navigation)?;
    ui_click(session, &format!("lilia.ui.pane.auto-{workflow}"))?;
    wait_observation(session, |state| {
        state["observation"]["selectedAutomation"] == workflow
    })?;
    Ok(())
}

pub(super) fn replay(session: &mut Session) -> Result {
    leave_automation_page(session)?;
    let navigation = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.navigation.") && id.contains("automation")
    })?;
    ui_click(session, &navigation)?;
    ui_click(session, "lilia.ui.automation.new")?;
    let created = wait_observation(session, |state| {
        state["observation"]["automationNodeCount"] == 1
    })?;
    let workflow = created["observation"]["selectedAutomation"]
        .as_str()
        .ok_or_else(|| {
            XtaskError::failure(
                "automation_selection_missing",
                "new workflow has no identity",
            )
        })?
        .to_owned();
    let ui = wait_ui(session)?;
    let trigger = ui
        .pointer("/snapshot/targets")
        .and_then(Value::as_array)
        .and_then(|targets| {
            targets
                .iter()
                .find(|target| target["id"] == "lilia.ui.automation.canvas")
        })
        .and_then(|canvas| canvas["graphNodes"].as_array())
        .and_then(|nodes| nodes.iter().find(|node| node["id"] == "trigger"))
        .ok_or_else(|| {
            XtaskError::failure(
                "automation_trigger_not_exposed",
                "initial trigger has no visible canvas geometry",
            )
        })?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-click-at", "targetId":"lilia.ui.automation.canvas", "x":trigger["x"].to_string(),"y":trigger["y"].to_string()}))?, "select trigger on canvas")?;
    wait_observation(session, |state| {
        state["observation"]["automationSelectedNodeKind"] == "trigger"
    })?;
    select_relative(session, "auto-node-triggerKind", 1)?;
    wait_observation(session, |state| {
        state["observation"]["automationSelectedNodeConfigDraft"]["triggerKind"] == "task_changed"
    })?;
    select_relative(session, "auto-inspector-panel", 1)?;
    reveal_automation_control(session, "auto-name")?;
    ui_input(
        session,
        "lilia.ui.automation.auto-name",
        "自动事件与重启验收",
    )?;
    reveal_automation_control(session, "auto-add-human")?;
    ui_click(session, "lilia.ui.automation.auto-add-human")?;
    reveal_automation_control(session, "auto-node-prompt")?;
    ui_input(
        session,
        "lilia.ui.automation.auto-node-prompt",
        "重启后继续同一运行",
    )?;
    wait_observation(session, |state| {
        state["observation"]["automationSelectedNodeConfigDraft"]["prompt"] == "重启后继续同一运行"
    })?;
    select_relative(session, "auto-inspector-panel", 2)?;
    for id in [
        "auto-project-native-agent-debug-project",
        "auto-scope-event-kind-task_created",
    ] {
        reveal_automation_control(session, id)?;
        ui_click(session, &format!("lilia.ui.automation.{id}"))?;
    }
    wait_observation(session, |state| {
        let scope = &state["observation"]["automationScope"];
        scope["projectIds"] == serde_json::json!(["native-agent-debug-project"])
            && scope["eventKinds"] == serde_json::json!(["task_created"])
            && scope["includeInbox"] == false
    })?;
    select_relative(session, "auto-inspector-panel", -1)?;
    reveal_automation_control(session, "auto-publish")?;
    ui_click(session, "lilia.ui.automation.auto-publish")?;
    wait_observation(session, |state| {
        state["observation"]["automationPublished"] == true
    })?;
    reveal_automation_control(session, "auto-toggle")?;
    ui_click(session, "lilia.ui.automation.auto-toggle")?;
    let enabled = wait_observation(session, |state| {
        state["observation"]["automationEnabled"] == true
    })?;
    write_json(
        &session.run_dir.join("automation-event-enabled.json"),
        &enabled,
    )?;

    leave_automation_page(session)?;
    ui_click(session, "lilia.ui.new-conversation")?;
    wait_observation(session, |state| {
        state["observation"]["selectedProject"].is_null()
    })?;
    send_marked_turn(
        session,
        "NATIVE_AUTOMATION_UNMATCHED_INBOX",
        "automation-unmatched",
    )?;
    let unmatched_task = selected_task(session)?;
    let unmatched = wait_observation(session, |state| {
        processed_task_creation(state, &unmatched_task)
    })?;
    if !workflow_runs(&unmatched, &workflow).is_empty() {
        return Err(XtaskError::failure(
            "automation_scope_leak",
            "out-of-scope task created an automation run",
        ));
    }
    write_json(
        &session.run_dir.join("automation-event-unmatched.json"),
        &unmatched,
    )?;

    ui_click(session, "lilia.ui.sidebar.row.native-agent-debug-project")?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-hover", "targetId":"lilia.ui.sidebar.row.native-agent-debug-project"}))?, "hover project row to reveal its normal draft action")?;
    ui_click(
        session,
        "lilia.ui.sidebar.tool.native-agent-debug-project-draft",
    )?;
    wait_observation(session, |state| {
        state["observation"]["selectedProject"] == "native-agent-debug-project"
            && state["observation"]["selectedTask"].is_null()
    })?;
    send_marked_turn(
        session,
        "NATIVE_AUTOMATION_MATCHED_PROJECT",
        "automation-matched",
    )?;
    let task = selected_task(session)?;
    let waiting = wait_observation(session, |state| {
        processed_task_creation(state, &task)
            && workflow_runs(state, &workflow)
                .iter()
                .any(|detail| detail["run"]["status"] == "waiting_user")
    })?;
    let runs = workflow_runs(&waiting, &workflow);
    if runs.len() != 1 || runs[0]["run"]["trigger"]["taskId"] != task {
        return Err(XtaskError::failure(
            "automation_event_duplicate",
            "matching task must create exactly one attributed run",
        ));
    }
    let initial = (*runs[0]).clone();
    let run_id = initial["run"]["id"].as_str().unwrap().to_owned();
    write_json(
        &session.run_dir.join("automation-event-waiting.json"),
        &waiting,
    )?;
    session.restart()?;
    let restored = wait_observation(session, |state| !workflow_runs(state, &workflow).is_empty())?;
    if normalized_runs(&restored, &workflow) != normalize_details(vec![initial.clone()]) {
        return Err(XtaskError::failure(
            "automation_restart_changed_wait",
            "restart changed the persisted waiting run or nodes",
        ));
    }
    reopen_workflow(session, &navigation, &workflow)?;
    select_relative(session, "auto-inspector-panel", 2)?;
    wait_observation(session, |state| {
        state["observation"]["selectedAutomationRun"] == run_id
            && state["observation"]["automationRunStatus"] == "waiting_user"
    })?;
    session.capture(&session.run_dir.join("automation-restart-waiting.png"))?;
    reveal_automation_control(session, "auto-response")?;
    ui_input(session, "lilia.ui.automation.auto-response", "重启后确认")?;
    reveal_automation_control(session, "auto-resume")?;
    ui_click(session, "lilia.ui.automation.auto-resume")?;
    let completed = wait_observation(session, |state| {
        workflow_runs(state, &workflow)
            .iter()
            .any(|detail| detail["run"]["id"] == run_id && detail["run"]["status"] == "succeeded")
    })?;
    write_json(
        &session.run_dir.join("automation-restart-completed.json"),
        &completed,
    )?;
    let completed_runs = normalized_runs(&completed, &workflow);
    session.restart()?;
    let stable = wait_observation(session, |state| !workflow_runs(state, &workflow).is_empty())?;
    if normalized_runs(&stable, &workflow) != completed_runs {
        return Err(XtaskError::failure(
            "automation_restart_replayed",
            "completed run changed or was duplicated after restart",
        ));
    }
    write_json(
        &session
            .run_dir
            .join("automation-event-restart-verified.json"),
        &serde_json::json!({
            "workflowId":workflow,"runId":run_id,"unmatchedTaskId":unmatched_task,"matchedTaskId":task,
            "beforeRestart":initial,"completedRuns":completed_runs,"afterRestart":stable,
            "humanOnly":true,"externalSideEffectsVerified":false
        }),
    )?;
    reopen_workflow(session, &navigation, &workflow)?;
    reveal_automation_control(session, "auto-toggle")?;
    ui_click(session, "lilia.ui.automation.auto-toggle")?;
    wait_observation(session, |state| {
        state["observation"]["automationEnabled"] == false
    })?;
    session.capture(&session.run_dir.join("automation-event-restart.png"))
}
