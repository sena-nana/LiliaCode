use super::*;

const PROJECT: &str = "native-agent-debug-project";
const SCROLL: &str = "lilia.ui.project.scroll";

fn open(session: &Session, page: &str) -> Result {
    let ui = wait_ui(session)?;
    for back in ["lilia.ui.automation.back", "lilia.ui.settings.back"] {
        if ui
            .pointer("/snapshot/targets")
            .and_then(Value::as_array)
            .is_some_and(|targets| targets.iter().any(|target| target["id"] == back))
        {
            ui_click(session, back)?;
        }
    }
    let row = format!("lilia.ui.sidebar.row.{PROJECT}");
    ui_click(session, &row)?;
    wait_target(session, SCROLL)?;
    require_ok(
        &session.request(
            &serde_json::json!({"command":"ui-hover","targetId":"lilia.ui.new-conversation"}),
        )?,
        "move pointer out of the project row",
    )?;
    require_ok(
        &session.request(&serde_json::json!({"command":"ui-hover","targetId":row}))?,
        "reveal project menu",
    )?;
    ui_click(session, &format!("lilia.ui.sidebar.tool.{PROJECT}-menu"))?;
    let state = wait_target(session, "lilia.ui.menu.project")?;
    let suffix = format!("open-project-{page}");
    let item = state
        .pointer("/snapshot/projectMenu/items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item["value"]
                    .as_str()
                    .is_some_and(|value| value.ends_with(&suffix))
            })
        })
        .ok_or_else(|| {
            XtaskError::failure(
                "knowledge_menu_missing",
                format!("normal project menu lacks {page}"),
            )
        })?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-click-at","targetId":"lilia.ui.menu.project","x":item["x"].to_string(),"y":item["y"].to_string()}))?, "open project knowledge page")?;
    wait_target(session, SCROLL)?;
    Ok(())
}

fn click(session: &Session, target: &str) -> Result {
    reveal_scrolled_control(session, SCROLL, target)?;
    ui_click(session, target)
}

fn input(session: &Session, name: &str, value: &str) -> Result {
    let target = format!("lilia.ui.field.{name}");
    reveal_scrolled_control(session, SCROLL, &target)?;
    ui_input(session, &target, value)
}

fn record<'a>(state: &'a Value, collection: &str, id: &str) -> Option<&'a Value> {
    state
        .pointer(&format!("/observation/knowledgeAuthority/{collection}"))?
        .as_array()?
        .iter()
        .find(|row| row["id"] == id)
}

fn linked(state: &Value, milestone: &str, task: &str) -> bool {
    state
        .pointer("/observation/knowledgeAuthority/roadmap/links")
        .and_then(Value::as_array)
        .is_some_and(|links| {
            links
                .iter()
                .any(|link| link["milestoneId"] == milestone && link["taskId"] == task)
        })
}

fn task_status(session: &Session, task: &str, action: &str) -> Result {
    reveal_task(session, task)?;
    for target in [
        "lilia.ui.new-conversation".to_owned(),
        format!("lilia.ui.sidebar.row.{task}"),
    ] {
        wait_target(session, &target)?;
        require_ok(
            &session.request(&serde_json::json!({"command":"ui-hover","targetId":target}))?,
            "reveal task menu",
        )?;
    }
    ui_click(session, &format!("lilia.ui.sidebar.tool.{task}-menu"))?;
    let state = wait_target(session, "lilia.ui.menu.project")?;
    let item = state
        .pointer("/snapshot/projectMenu/items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                item["value"]
                    .as_str()
                    .is_some_and(|value| value.ends_with(action))
            })
        })
        .ok_or_else(|| {
            XtaskError::failure(
                "task_status_menu_missing",
                format!("normal task menu lacks {action}"),
            )
        })?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-click-at","targetId":"lilia.ui.menu.project","x":item["x"].to_string(),"y":item["y"].to_string()}))?, "change task completion from normal menu")
}

fn reveal_task(session: &Session, task: &str) -> Result {
    let row = format!("lilia.ui.sidebar.row.{task}");
    let ui = wait_ui(session)?;
    if !ui["snapshot"]["targets"]
        .as_array()
        .is_some_and(|targets| targets.iter().any(|target| target["id"] == row))
    {
        let reveal = format!("lilia.ui.sidebar.row.reveal-{PROJECT}");
        if ui["snapshot"]["targets"]
            .as_array()
            .is_some_and(|targets| targets.iter().any(|target| target["id"] == reveal))
        {
            ui_click(session, &reveal)?;
        }
    }
    reveal_scrolled_control(session, "lilia.ui.sidebar.scroll", &row)
}

fn roadmap_progress(state: &Value, milestone: &str, task: &str, done: bool) -> bool {
    let authority = &state["observation"]["knowledgeAuthority"];
    let expected = if done {
        "1/1 已完成"
    } else {
        "0/1 已完成"
    };
    authority["roadmapCards"].as_array().is_some_and(|cards| {
        cards.iter().any(|card| {
            card["id"] == milestone
                && card["status"]
                    .as_str()
                    .is_some_and(|status| status.contains(expected))
        })
    }) && authority["tasks"].as_array().is_some_and(|tasks| {
        tasks.iter().any(|row| {
            row["id"] == task
                && if done {
                    row["status"] == "done"
                } else {
                    row["status"] != "done"
                }
        })
    })
}

pub(super) fn replay(session: &mut Session) -> Result {
    open(session, "roadmap")?;
    let before = wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["projectId"] == PROJECT
            && state["observation"]["knowledgeAuthority"]["roadmap"]["milestones"].is_array()
    })?;
    let previous_ids = before["observation"]["knowledgeAuthority"]["roadmap"]["milestones"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|record| record["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    click(session, "lilia.ui.composer.project-milestone-create")?;
    let created = wait_observation(session, |state| {
        state["observation"]["selectedMilestone"]
            .as_str()
            .is_some_and(|id| {
                !previous_ids.iter().any(|previous| previous == id)
                    && record(state, "roadmap/milestones", id).is_some()
            })
    })?;
    let milestone = created["observation"]["selectedMilestone"]
        .as_str()
        .unwrap()
        .to_owned();
    input(session, "project-milestone-title", "正常入口路线图验收")?;
    input(
        session,
        "project-milestone-description",
        "第一阶段：建立关联\n第二阶段：验证重启保留",
    )?;
    click(session, "lilia.ui.composer.project-milestone-save")?;
    let saved = wait_observation(session, |state| {
        record(state, "roadmap/milestones", &milestone).is_some_and(|record| {
            record["title"] == "正常入口路线图验收"
                && record["description"] == "第一阶段：建立关联\n第二阶段：验证重启保留"
        })
    })?;
    let milestone_record = record(&saved, "roadmap/milestones", &milestone)
        .unwrap()
        .clone();
    let task = "native-agent-debug-task";
    let toggle = format!("lilia.ui.switch.milestone-task-{task}");
    click(session, &toggle)?;
    let associated = wait_observation(session, |state| {
        linked(state, &milestone, task) && state["observation"]["selectedMilestoneTaskCount"] == 1
    })?;
    write_json(
        &session.run_dir.join("knowledge-roadmap-linked.json"),
        &associated,
    )?;
    click(session, &toggle)?;
    wait_observation(session, |state| {
        !linked(state, &milestone, task) && state["observation"]["selectedMilestoneTaskCount"] == 0
    })?;
    click(session, &toggle)?;
    wait_observation(session, |state| {
        linked(state, &milestone, task)
            && state["observation"]["selectedMilestoneTaskCount"] == 1
            && record(state, "roadmap/milestones", &milestone) == Some(&milestone_record)
    })?;
    reveal_scrolled_control(session, SCROLL, "lilia.ui.field.project-milestone-title")?;
    session.capture(&session.run_dir.join("knowledge-roadmap.png"))?;

    task_status(session, task, "complete-task")?;
    let completed = wait_observation(session, |state| {
        roadmap_progress(state, &milestone, task, true)
    })?;
    task_status(session, task, "reopen-task")?;
    let reopened = wait_observation(session, |state| {
        roadmap_progress(state, &milestone, task, false)
    })?;
    write_json(
        &session.run_dir.join("knowledge-roadmap-completion.json"),
        &serde_json::json!({"completed":completed,"reopened":reopened}),
    )?;

    open(session, "memory")?;
    click(session, "lilia.ui.composer.memory-new")?;
    wait_observation(session, |state| {
        state["observation"]["selectedMemory"].is_null()
    })?;
    input(session, "memory-title", "正常入口多行记忆")?;
    let body =
        "项目约定：使用正常用户入口。\n验证要求：保存、禁用、重新启用。\n重启之后保留全部三行。";
    input(session, "memory-body", body)?;
    input(session, "memory-tags", "验收,持久化")?;
    click(session, "lilia.ui.composer.memory-save")?;
    let saved = wait_observation(session, |state| {
        state["observation"]["selectedMemory"]
            .as_str()
            .is_some_and(|id| {
                record(state, "memories", id).is_some_and(|record| {
                    record["title"] == "正常入口多行记忆"
                        && record["body"] == body
                        && record["projectId"] == PROJECT
                        && record["scope"] == "project"
                        && record["tags"].as_array().is_some_and(|tags| {
                            tags.len() == 2
                                && tags.contains(&Value::String("验收".into()))
                                && tags.contains(&Value::String("持久化".into()))
                        })
                })
            })
    })?;
    let memory = saved["observation"]["selectedMemory"]
        .as_str()
        .unwrap()
        .to_owned();
    input(session, "memory-title", "撤销重做临时标题")?;
    let mut history = Vec::new();
    let (undo, redo) = if cfg!(target_os = "macos") {
        ("Meta+z", "Meta+Shift+z")
    } else {
        ("Control+z", "Control+Shift+z")
    };
    for (key, expected) in [
        (undo, "正常入口多行记忆"),
        (redo, "撤销重做临时标题"),
        (undo, "正常入口多行记忆"),
    ] {
        reveal_scrolled_control(session, SCROLL, "lilia.ui.field.memory-title")?;
        let before = wait_ui(session)?;
        let response = session.request(&serde_json::json!({"command":"ui-key", "targetId":"lilia.ui.field.memory-title", "key":key}))?;
        write_json(
            &session.run_dir.join(format!(
                "knowledge-memory-history-step-{}.json",
                history.len()
            )),
            &serde_json::json!({"key":key,"expected":expected,"before":before,"response":response}),
        )?;
        require_ok(&response, "memory history through normal keyboard")?;
        click(session, "lilia.ui.composer.memory-save")?;
        history.push(wait_observation(session, |state| {
            record(state, "memories", &memory)
                .is_some_and(|record| record["title"] == expected && record["body"] == body)
        })?);
    }
    write_json(
        &session.run_dir.join("knowledge-memory-history.json"),
        &serde_json::json!({"savedStates":history}),
    )?;
    click(session, "lilia.ui.switch.memory-enabled")?;
    click(session, "lilia.ui.composer.memory-save")?;
    wait_observation(session, |state| {
        record(state, "memories", &memory).is_some_and(|record| record["enabled"] == false)
    })?;
    click(session, "lilia.ui.switch.memory-enabled")?;
    click(session, "lilia.ui.composer.memory-save")?;
    let enabled = wait_observation(session, |state| {
        record(state, "memories", &memory)
            .is_some_and(|record| record["enabled"] == true && record["body"] == body)
    })?;
    let memory_record = record(&enabled, "memories", &memory).unwrap().clone();
    input(session, "memory-cooldown", "3")?;
    require_ok(&session.request(&serde_json::json!({"command":"ui-key", "targetId":"lilia.ui.field.memory-cooldown", "key":"Enter"}))?, "commit memory cooldown input")?;
    wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["memorySettings"]["cooldownTurns"] == 3
    })?;
    reveal_scrolled_control(session, SCROLL, "lilia.ui.field.memory-body")?;
    session.capture(&session.run_dir.join("knowledge-memory.png"))?;
    replay_memory_resize(session, &memory, &memory_record)?;
    session.restart()?;
    open(session, "roadmap")?;
    let roadmap_restored = wait_observation(session, |state| {
        record(state, "roadmap/milestones", &milestone) == Some(&milestone_record)
            && linked(state, &milestone, task)
    })?;
    open(session, "memory")?;
    let memory_restored = wait_observation(session, |state| {
        record(state, "memories", &memory) == Some(&memory_record)
            && state["observation"]["knowledgeAuthority"]["memorySettings"]["cooldownTurns"] == 3
    })?;
    reveal_task(session, task)?;
    ui_click(session, &format!("lilia.ui.sidebar.row.{task}"))?;
    let mut injection_requests = Vec::new();
    for index in 1..=5 {
        if index == 5 {
            open(session, "memory")?;
            click(session, "lilia.ui.switch.memory-global")?;
            wait_observation(session, |state| {
                state["observation"]["knowledgeAuthority"]["memorySettings"]["enabled"] == false
            })?;
            reveal_task(session, task)?;
            ui_click(session, &format!("lilia.ui.sidebar.row.{task}"))?;
        }
        let request = send_marked_turn(
            session,
            &format!("NATIVE_PARITY_MEMORY_INJECTION_{index}"),
            &format!("memory-{index}"),
        )?;
        let context = turn_context_from_request(&request).ok_or_else(|| {
            XtaskError::failure(
                "memory_context_missing",
                "provider request has no turn context",
            )
        })?;
        let injected = index == 1 || index == 4;
        let decision = &context["memoryInjection"];
        let has_memory = decision["memoryIds"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id == &memory));
        let occurrences = context
            .to_string()
            .matches("项目约定：使用正常用户入口。")
            .count();
        if has_memory != injected || occurrences != usize::from(injected) {
            return Err(XtaskError::failure(
                "memory_injection_mismatch",
                format!(
                    "turn {index}: normal memory settings did not produce expected unique model context"
                ),
            ));
        }
        injection_requests.push(decision.clone());
    }
    open(session, "memory")?;
    click(session, "lilia.ui.switch.memory-global")?;
    wait_observation(session, |state| {
        state["observation"]["knowledgeAuthority"]["memorySettings"]["enabled"] == true
    })?;
    write_json(
        &session.run_dir.join("knowledge-model-injection.json"),
        &serde_json::json!({"memoryId":memory,"decisions":injection_requests,"normalUiSubmission":true}),
    )?;
    write_json(
        &session.run_dir.join("knowledge-restart-verified.json"),
        &serde_json::json!({"roadmap":roadmap_restored,"memory":memory_restored,"modelInjectionVerified":true}),
    )?;
    Ok(())
}

fn replay_memory_resize(session: &Session, memory: &str, expected: &Value) -> Result {
    const BODY: &str = "lilia.ui.field.memory-body";
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    let (before, x, y, height) = loop {
        let ui = wait_target(session, BODY)?;
        let targets = ui["snapshot"]["targets"]
            .as_array()
            .expect("mounted targets");
        let body = targets
            .iter()
            .find(|target| target["id"] == BODY)
            .expect("body target");
        let viewport = targets
            .iter()
            .find(|target| target["id"] == SCROLL)
            .ok_or_else(|| {
                XtaskError::failure(
                    "memory_scroll_missing",
                    "memory page scroll view is not mounted",
                )
            })?;
        let bottom = viewport["bounds"]["y"].as_f64().unwrap_or(0.0)
            + viewport["bounds"]["height"].as_f64().unwrap_or(0.0);
        if let (Some(x), Some(y), Some(height)) = (
            body["resizeGrip"]["x"].as_f64(),
            body["resizeGrip"]["y"].as_f64(),
            body["bounds"]["height"].as_f64(),
        ) {
            if y + 48.0 < bottom - 4.0 {
                break (ui, x, y, height);
            }
        }
        if Instant::now() >= deadline {
            write_json(&session.run_dir.join("memory-resize-failure.json"), &ui)?;
            return Err(XtaskError::failure(
                "memory_resize_grip_unreachable",
                "normal scrolling did not expose the memory resize handle",
            ));
        }
        require_ok(
            &session.request(
                &serde_json::json!({"command":"ui-scroll","targetId":SCROLL,"deltaY":"80"}),
            )?,
            "reveal memory resize handle",
        )?;
        thread::sleep(Duration::from_millis(100));
    };
    require_ok(&session.request(&serde_json::json!({"command":"ui-drag","targetId":BODY,
        "startX":x.to_string(),"startY":y.to_string(),"endX":x.to_string(),"endY":(y+48.0).to_string()}))?, "resize memory body through normal pointer events")?;
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    let after = loop {
        let ui = wait_target(session, BODY)?;
        let actual = ui["snapshot"]["targets"]
            .as_array()
            .and_then(|targets| targets.iter().find(|target| target["id"] == BODY))
            .and_then(|target| target["bounds"]["height"].as_f64());
        if actual.is_some_and(|actual| (actual - height - 48.0).abs() <= 1.0) {
            break ui;
        }
        if Instant::now() >= deadline {
            write_json(&session.run_dir.join("memory-resize-failure.json"), &ui)?;
            return Err(XtaskError::failure(
                "memory_resize_not_applied",
                "normal drag did not resize the memory editor by 48 logical pixels",
            ));
        }
        thread::sleep(Duration::from_millis(100));
    };
    let authority = wait_observation(session, |state| {
        record(state, "memories", memory) == Some(expected)
    })?;
    write_json(
        &session.run_dir.join("memory-resize-verified.json"),
        &serde_json::json!({"before":before,"after":after,"authority":authority}),
    )?;
    session.capture(&session.run_dir.join("knowledge-memory-resized.png"))
}
