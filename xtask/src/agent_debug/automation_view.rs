use super::{capture_resized_window, capture_window, require_ok, write_json, Session};
use crate::{Result, XtaskError};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) fn run(session: &Session) -> Result<()> {
    for target in ["lilia.automations.create", "lilia.automations.add.human"] {
        click(session, target)?;
    }
    click(session, "lilia.automations.node.close")?;
    let state = session.request(&json!({"command":"observe"}))?;
    require_ok(&state, "observe workflow nodes")?;
    let target = state
        .pointer("/observation/visibleTargetIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find(|target| {
            target.starts_with("lilia.automations.graph.") && !target.ends_with(".node.trigger")
        })
        .ok_or_else(|| {
            XtaskError::failure(
                "automation_node_missing",
                "created confirmation node has no visible target",
            )
        })?;
    click(session, target)?;
    for (target, text) in [
        ("lilia.automations.node.title", "发布前确认"),
        (
            "lilia.automations.node.config.prompt",
            "请检查结果后确认继续。",
        ),
    ] {
        require_ok(
            &session.request(&json!({"command":"input", "targetId":target, "text":text}))?,
            "edit confirmation node",
        )?;
    }
    click(session, "lilia.automations.node.save")?;
    let editor = session.request(&json!({"command":"observe"}))?;
    require_ok(&editor, "observe saved node")?;
    if editor
        .pointer("/observation/automationSelectedNodeTitle")
        .and_then(Value::as_str)
        != Some("发布前确认")
        || editor
            .pointer("/observation/automationSelectedNodeConfig/prompt")
            .and_then(Value::as_str)
            != Some("请检查结果后确认继续。")
    {
        return Err(XtaskError::failure(
            "automation_node_not_saved",
            "confirmation node draft was not saved",
        ));
    }
    write_json(
        &session.run_dir.join("automation-node-editor.json"),
        &editor,
    )?;
    capture_window(
        session.pid(),
        &session.run_dir.join("automation-node-editor.png"),
    )?;
    capture_resized_window(
        session.pid(),
        &session.run_dir.join("automation-node-editor-narrow.png"),
        [780, 760],
    )?;
    let narrow = session.request(&json!({"command":"observe"}))?;
    require_ok(&narrow, "observe narrow node editor")?;
    let targets = narrow
        .pointer("/observation/visibleTargetIds")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            XtaskError::failure("automation_narrow_targets", "narrow editor targets missing")
        })?;
    if !targets
        .iter()
        .any(|id| id.as_str() == Some("lilia.automations.node.save"))
        || targets
            .iter()
            .filter_map(Value::as_str)
            .any(|id| id.starts_with("lilia.automations.graph."))
    {
        return Err(XtaskError::failure(
            "automation_narrow_editor",
            "narrow editor did not replace the graph",
        ));
    }
    write_json(
        &session.run_dir.join("automation-node-editor-narrow.json"),
        &narrow,
    )?;
    click(session, "lilia.automations.node.close")?;
    capture_resized_window(
        session.pid(),
        &session.run_dir.join("automation-graph-restored.png"),
        [1180, 760],
    )?;
    write_json(
        &session.run_dir.join("automation-graph-restored.json"),
        &session.request(&json!({"command":"observe"}))?,
    )?;
    click(session, "lilia.automations.publish")?;
    click(session, "lilia.automations.run")?;
    let waiting = expect_status(session, "waiting_user", "waiting")?;
    if waiting
        .pointer("/observation/automationWaitingHumanNode")
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(XtaskError::failure(
            "automation_missing_confirmation",
            "run is waiting without a human confirmation node",
        ));
    }
    require_ok(&session.request(&json!({"command":"input", "targetId":"lilia.automations.run.human-response", "text":"已检查，可以继续。"}))?, "enter automation response")?;
    click(session, "lilia.automations.run.resume")?;
    let completed = expect_status(session, "succeeded", "completed")?;
    if completed.pointer("/observation/selectedAutomationRun")
        != waiting.pointer("/observation/selectedAutomationRun")
    {
        return Err(XtaskError::failure(
            "automation_run_changed",
            "confirmation completed a different run",
        ));
    }
    click(session, "lilia.automations.run")?;
    let second = expect_status(session, "waiting_user", "second-run")?;
    if second.pointer("/observation/selectedAutomationRun")
        == waiting.pointer("/observation/selectedAutomationRun")
    {
        return Err(XtaskError::failure(
            "automation_run_reused",
            "starting a workflow did not create a new run",
        ));
    }
    click(session, "lilia.automations.run.cancel")?;
    expect_status(session, "cancelled", "cancelled")?;
    Ok(())
}

fn click(session: &Session, target: &str) -> Result<()> {
    require_ok(
        &session.request(&json!({"command":"click", "targetId":target}))?,
        "automation action",
    )
}

fn expect_status(session: &Session, status: &str, name: &str) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let state = session.request(&json!({"command":"observe"}))?;
        require_ok(&state, "observe automation")?;
        if state.to_string().contains("sk-native-agent-debug-fixture") {
            return Err(XtaskError::failure(
                "automation_secret_leak",
                "automation observation contains the secret canary",
            ));
        }
        if state
            .pointer("/observation/automationRunStatus")
            .and_then(Value::as_str)
            == Some(status)
            && state
                .pointer("/observation/visibleTargetIds")
                .and_then(Value::as_array)
                .is_some_and(|targets| {
                    targets.iter().any(|target| {
                        target.as_str()
                            == Some(if status == "waiting_user" {
                                "lilia.automations.run.resume"
                            } else {
                                "lilia.automations.run"
                            })
                    })
                })
        {
            write_json(
                &session.run_dir.join(format!("automation-{name}.json")),
                &state,
            )?;
            capture_window(
                session.pid(),
                &session.run_dir.join(format!("automation-{name}.png")),
            )?;
            return Ok(state);
        }
        if Instant::now() >= deadline {
            write_json(&session.run_dir.join("automation-failure.json"), &state)?;
            return Err(XtaskError::failure(
                "automation_status_timeout",
                format!("automation did not reach {status}"),
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
