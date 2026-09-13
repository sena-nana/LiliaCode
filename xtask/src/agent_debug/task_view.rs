use super::{capture_named_window, require_ok, write_json, Session};
use crate::{Result, XtaskError};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn wait(session: &Session, name: &str, predicate: impl Fn(&Value) -> bool) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let state = session.request(&json!({"command":"observe"}))?;
        require_ok(&state, name)?;
        if predicate(&state) {
            return Ok(state);
        }
        if Instant::now() >= deadline {
            write_json(
                &session
                    .run_dir
                    .join(format!("task-popup-{name}-blocked.json")),
                &state,
            )?;
            return Err(XtaskError::failure(
                "task_popup_state_timeout",
                format!("task popup did not reach {name}"),
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn run(session: &Session) -> Result<Value> {
    require_ok(
        &session
            .request(&json!({"command":"click", "targetId":"lilia.task-session.popup.open"}))?,
        "open task window",
    )?;
    let opened = wait(session, "ready", |state| {
        state
            .pointer("/observation/taskPopupReadyCount")
            .and_then(Value::as_u64)
            == Some(1)
    })?;
    let composer = opened
        .pointer("/observation/visibleTargetIds")
        .and_then(Value::as_array)
        .and_then(|targets| {
            targets
                .iter()
                .filter_map(Value::as_str)
                .find(|id| id.starts_with("lilia.task-popup.") && id.ends_with(".composer.input"))
        })
        .ok_or_else(|| {
            XtaskError::failure(
                "task_popup_input_missing",
                "the task window has no composer target",
            )
        })?;
    let root = composer.strip_suffix(".composer.input").unwrap();
    let click = |target: &str| -> Result {
        require_ok(
            &session.request(&json!({"command":"click", "targetId":target}))?,
            target,
        )
    };
    let shared_text = "Shared task window draft";
    let isolated_text = "Isolated popup draft after task switch";
    require_ok(
        &session.request(&json!({"command":"input", "targetId":composer, "text":shared_text}))?,
        "edit task window draft",
    )?;
    let shared = wait(session, "shared-draft", |state| {
        state
            .pointer("/observation/composerLength")
            .and_then(Value::as_u64)
            == Some(shared_text.len() as u64)
            && state
                .pointer("/observation/taskPopupComposerLengths/0")
                .and_then(Value::as_u64)
                == Some(shared_text.len() as u64)
    })?;
    write_json(
        &session.run_dir.join("task-popup-shared-draft.json"),
        &shared,
    )?;
    click("lilia.sidebar.task.native-agent-debug-draft-switch-task")?;
    require_ok(
        &session.request(&json!({"command":"input", "targetId":composer, "text":isolated_text}))?,
        "edit popup after main task switch",
    )?;
    let isolated = wait(session, "isolated-draft", |state| {
        state
            .pointer("/observation/selectedTask")
            .and_then(Value::as_str)
            == Some("native-agent-debug-draft-switch-task")
            && state
                .pointer("/observation/composerLength")
                .and_then(Value::as_u64)
                == Some(11)
            && state
                .pointer("/observation/taskPopupComposerLengths/0")
                .and_then(Value::as_u64)
                == Some(isolated_text.len() as u64)
    })?;
    write_json(
        &session.run_dir.join("task-popup-isolated-draft.json"),
        &isolated,
    )?;
    click(root)?;
    capture_named_window(
        session.pid(),
        &session.run_dir.join("task-popup.png"),
        "LiliaCode · 验证 Native Composer 与时间线",
    )?;
    click(&format!("{root}.close"))?;
    wait(session, "closed", |state| {
        state
            .pointer("/observation/taskPopupWindowCount")
            .and_then(Value::as_u64)
            == Some(0)
    })?;
    click("lilia.sidebar.task.native-agent-debug-task")?;
    let restored = wait(session, "restored", |state| {
        state
            .pointer("/observation/composerLength")
            .and_then(Value::as_u64)
            == Some(isolated_text.len() as u64)
    })?;
    write_json(&session.run_dir.join("task-popup-restored.json"), &restored)?;
    Ok(
        json!({"passed":true, "sharedTaskDraft":true, "differentTaskIsolation":true, "nativePopupCapture":true, "closePreservesDraft":true}),
    )
}
