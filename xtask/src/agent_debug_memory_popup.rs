use super::*;

const TASK: &str = "native-agent-debug-task";
const PROJECT: &str = "native-agent-debug-project";
const SCROLL: &str = "lilia.ui.project.scroll";

fn click_project_control(session: &Session, target: &str) -> Result {
    reveal_scrolled_control(session, SCROLL, target)?;
    ui_click(session, target)
}

fn popup_target(session: &Session, window: u64, target: &str) -> Result {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        let state = session
            .request(&serde_json::json!({"command":"ui-observe","windowId":window.to_string()}))?;
        require_ok(&state, "observe actual popup controls")?;
        if state
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .is_some_and(|targets| targets.iter().any(|entry| entry["id"] == target))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(XtaskError::failure(
                "popup_control_missing",
                format!("window {window}: {target}"),
            ));
        }
        thread::sleep(Duration::from_millis(80));
    }
}

fn send(session: &Session, window: u64, marker: &str) -> Result<Value> {
    popup_target(session, window, "lilia.ui.popup.composer")?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-input","windowId":window.to_string(),"targetId":"lilia.ui.popup.composer","text":marker}))?, "type in actual popup composer")?;
    wait_observation(session, |state| {
        state["observation"]["taskPopupTaskIds"]
            .as_array()
            .and_then(|tasks| tasks.iter().position(|task| task == TASK))
            .is_some_and(|index| {
                state["observation"]["taskPopupComposerLengths"][index].as_u64()
                    == Some(marker.len() as u64)
            })
    })?;
    popup_target(session, window, "lilia.ui.popup.send")?;
    let after = session.model_fixture.as_ref().unwrap().requests().len();
    require_ok(&session.request(&serde_json::json!({"command":"ui-click","windowId":window.to_string(),"targetId":"lilia.ui.popup.send"}))?, "send from actual popup")?;
    wait_model_request(session, after, |request| {
        let last_user = request["messages"].as_array().and_then(|messages| {
            messages
                .iter()
                .rev()
                .find(|message| message["role"] == "user")
        });
        last_user.is_some_and(|message| message["content"] == marker)
            && turn_context_from_request(request)
                .is_some_and(|context| context["workspace"]["metadata"]["productTaskId"] == TASK)
    })
}

pub(super) fn replay(session: &mut Session) -> Result {
    ui_click(session, &format!("lilia.ui.sidebar.row.{TASK}"))?;
    ui_click(session, "lilia.ui.sidebar.more")?;
    let menu = wait_target(session, "lilia.ui.menu.window")?;
    let item = menu
        .pointer("/snapshot/menu/items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find(|item| item["value"] == "more-popup"))
        .ok_or_else(|| {
            XtaskError::failure("popup_menu_missing", "task menu has no normal popup entry")
        })?;
    let native_windows_before = if session.capture_enabled {
        visible_native_windows(session.pid())?
    } else {
        Vec::new()
    };
    let before = session.request(&serde_json::json!({"command":"observe"}))?;
    require_ok(&before, "observe runtime windows before opening popup")?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-click-at","targetId":"lilia.ui.menu.window","x":item["x"].to_string(),"y":item["y"].to_string()}))?, "open normal task popup")?;
    let state = wait_observation(session, |state| {
        state["observation"]["workspaceWindows"]
            .as_array()
            .is_some_and(|windows| {
                windows.iter().any(|window| {
                    window["itemIds"].as_array().is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| item.as_str().is_some_and(|id| id.contains(TASK)))
                    })
                })
            })
    })?;
    let window = state["observation"]["workspaceWindows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| {
            window["itemIds"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.as_str().is_some_and(|id| id.contains(TASK)))
            })
        })
        .unwrap()["windowId"]
        .as_u64()
        .unwrap();
    if before["observation"]["workspaceWindows"]
        .as_array()
        .is_some_and(|windows| windows.iter().any(|entry| entry["windowId"] == window))
    {
        return Err(XtaskError::failure(
            "popup_runtime_window_not_new",
            "the normal popup action reused an existing Runtime window; native creation cannot identify it",
        ));
    }
    popup_target(session, window, "lilia.ui.popup.composer")?;
    capture_task_popup(session, window, TASK, &native_windows_before)?;

    let project_row = format!("lilia.ui.sidebar.row.{PROJECT}");
    ui_click(session, &project_row)?;
    wait_target(session, SCROLL)?;
    require_ok(
        &session.request(
            &serde_json::json!({"command":"ui-hover","targetId":"lilia.ui.new-conversation"}),
        )?,
        "move pointer out of the project row",
    )?;
    require_ok(
        &session.request(&serde_json::json!({"command":"ui-hover","targetId":project_row}))?,
        "reveal project menu",
    )?;
    ui_click(session, &format!("lilia.ui.sidebar.tool.{PROJECT}-menu"))?;
    let menu = wait_target(session, "lilia.ui.menu.project")?;
    let item = menu
        .pointer("/snapshot/projectMenu/items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item["value"]
                    .as_str()
                    .is_some_and(|id| id.ends_with("open-project-memory"))
            })
        })
        .ok_or_else(|| {
            XtaskError::failure("memory_menu_missing", "project menu has no memory entry")
        })?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-click-at","targetId":"lilia.ui.menu.project","x":item["x"].to_string(),"y":item["y"].to_string()}))?,"open shared memory page")?;
    let state = wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["memories"]
            .as_array()
            .is_some_and(|memories| {
                memories
                    .iter()
                    .any(|memory| memory["title"] == "正常入口多行记忆")
            })
    })?;
    let memory = state["observation"]["knowledgeAuthority"]["memories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|memory| memory["title"] == "正常入口多行记忆")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    click_project_control(session, &format!("lilia.ui.project-card.memory-{memory}"))?;
    let body = "memory-popup-updated-493\n主窗口保存，已打开弹窗随后读取。";
    reveal_scrolled_control(session, SCROLL, "lilia.ui.field.memory-body")?;
    ui_input(session, "lilia.ui.field.memory-body", body)?;
    click_project_control(session, "lilia.ui.composer.memory-save")?;
    wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["memories"]
            .as_array()
            .is_some_and(|memories| {
                memories.iter().any(|entry| {
                    entry["id"] == memory && entry["body"] == body && entry["enabled"] == true
                })
            })
    })?;
    click_project_control(session, "lilia.ui.memory.task-menu")?;
    ui_click(session, &format!("lilia.ui.memory.task.{TASK}"))?;
    click_project_control(session, "lilia.ui.composer.memory-task-reset")?;
    wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["injection"]["taskId"] == TASK
            && state["observation"]["knowledgeAuthority"]["injection"]["lastInjectedTurnSeq"]
                .is_null()
    })?;
    let first = send(session, window, "memory-popup-first-493")?;
    let first_context = turn_context_from_request(&first).unwrap();
    let first_turn = first_context["turnId"].as_str().unwrap();
    if first_context["memoryInjection"]["taskId"] != TASK
        || first_context["memoryInjection"]["turnId"] != first_turn
        || !first_context["memoryInjection"]["baseline"]
            .as_str()
            .is_some_and(|text| text.contains("memory-popup-updated-493"))
    {
        return Err(XtaskError::failure(
            "popup_memory_update_missing",
            first_context.to_string(),
        ));
    }
    click_project_control(session, "lilia.ui.switch.memory-enabled")?;
    click_project_control(session, "lilia.ui.composer.memory-save")?;
    wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["memories"]
            .as_array()
            .is_some_and(|memories| {
                memories
                    .iter()
                    .any(|entry| entry["id"] == memory && entry["enabled"] == false)
            })
    })?;
    click_project_control(session, "lilia.ui.composer.memory-task-reset")?;
    wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["injection"]["lastInjectedTurnSeq"].is_null()
    })?;
    let second = send(session, window, "memory-popup-disabled-493")?;
    let second_context = turn_context_from_request(&second).unwrap();
    if second_context["turnId"] == first_turn
        || second_context["memoryInjection"]["turnId"] != second_context["turnId"]
        || second_context["memoryInjection"]["memoryIds"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id == &memory))
        || second_context["memoryInjection"]["baseline"]
            .as_str()
            .is_some_and(|text| text.contains("memory-popup-updated-493"))
    {
        return Err(XtaskError::failure(
            "popup_memory_disable_ignored",
            second_context.to_string(),
        ));
    }
    write_json(
        &session.run_dir.join("memory-popup-sync-verified.json"),
        &serde_json::json!({"windowId":window,"memoryId":memory,"updatedRequest":first,"disabledRequest":second}),
    )?;
    Ok(())
}
