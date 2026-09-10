use super::*;

fn target(ui: &Value, id: &str) -> Option<Value> {
    ui.pointer("/snapshot/targets")?
        .as_array()?
        .iter()
        .find(|entry| entry["id"] == id)
        .cloned()
}

pub(super) fn replay(session: &Session) -> Result {
    let mut evidence = Vec::new();
    for (name, id) in [
        ("trend", "lilia.ui.quota.trend"),
        ("projects", "lilia.ui.settings.quota-project-chart"),
        (
            "conversations",
            "lilia.ui.settings.quota-conversation-chart",
        ),
        ("tools", "lilia.ui.settings.quota-tool-chart"),
    ] {
        reveal_scrolled_control(session, "lilia.ui.settings.content-scroll", id)?;
        let mut selected = None;
        // A partly visible chart is a valid scroll target; find an exposed datum
        // through normal pointer events, scrolling further if its ring is clipped.
        'scroll: for _ in 0..8 {
            let ui = wait_ui(session)?;
            if let Some(chart) = target(&ui, id) {
                let bounds = &chart["bounds"];
                let bx = bounds["x"].as_f64().unwrap_or(0.0);
                let by = bounds["y"].as_f64().unwrap_or(0.0);
                let width = bounds["width"].as_f64().unwrap_or(0.0);
                let height = bounds["height"].as_f64().unwrap_or(0.0);
                let points: Vec<(f64, f64)> = if name == "trend" {
                    // Cover every displayed day rather than assuming a fixture date.
                    (0..60)
                        .rev()
                        .map(|i| {
                            (
                                48.0 + (width - 60.0) * (i as f64 + 0.5) / 60.0,
                                12.0 + (height - 66.0) * 0.5,
                            )
                        })
                        .collect()
                } else {
                    let radius = width.min(height) * 0.42;
                    (0..12)
                        .map(|i| {
                            let angle = i as f64 * std::f64::consts::TAU / 12.0;
                            (
                                width / 2.0 + radius * angle.cos(),
                                height / 2.0 + radius * angle.sin(),
                            )
                        })
                        .collect()
                };
                for (x, y) in points {
                    let response = session.request(&serde_json::json!({"command":"ui-hover","targetId":id,"x":(bx+x).to_string(),"y":(by+y).to_string()}))?;
                    if response["ok"] != true {
                        continue;
                    }
                    let deadline = Instant::now() + Duration::from_millis(400);
                    loop {
                        let hovered = wait_ui(session)?;
                        if let Some(current) = target(&hovered, id) {
                            let hover = &current["chartHover"];
                            if hover["value"].as_f64().is_some_and(|value| value > 0.0)
                                && hover["tooltip"]
                                    .as_str()
                                    .is_some_and(|text| !text.is_empty())
                            {
                                selected = Some(current);
                                break 'scroll;
                            }
                        }
                        if Instant::now() >= deadline {
                            break;
                        }
                        thread::sleep(Duration::from_millis(30));
                    }
                }
            }
            require_ok(&session.request(&serde_json::json!({"command":"ui-scroll","targetId":"lilia.ui.settings.content-scroll","deltaY":"-140"}))?, "reveal quota chart datum")?;
            thread::sleep(Duration::from_millis(80));
        }
        let selected = selected.ok_or_else(|| {
            XtaskError::failure(
                "quota_hover_missing",
                format!("{name}: no exposed nonzero datum produced a mounted tooltip"),
            )
        })?;
        let hover = &selected["chartHover"];
        let tooltip = hover["tooltip"].as_str().unwrap_or_default();
        let label = hover["label"].as_str().unwrap_or_default();
        let value = hover["value"].as_f64().unwrap_or_default();
        if label.is_empty()
            || !tooltip.contains(label)
            || !tooltip.contains(&format!("{value:.0}"))
            || (name != "trend" && !tooltip.contains('%'))
            || (name == "trend"
                && [
                    "输入:",
                    "输出:",
                    "缓存命中:",
                    "缓存写入:",
                    "总量:",
                    "成本",
                    "记录",
                ]
                .iter()
                .any(|field| !tooltip.contains(field)))
        {
            return Err(XtaskError::failure(
                "quota_tooltip_content_mismatch",
                format!("{name}: rendered tooltip does not describe the selected nonzero datum"),
            ));
        }
        session.capture(&session.run_dir.join(format!("quota-hover-{name}.png")))?;
        require_ok(
            &session.request(
                &serde_json::json!({"command":"ui-hover","targetId":"lilia.ui.settings.tab.quota"}),
            )?,
            "leave quota chart",
        )?;
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let cleared = loop {
            let ui = wait_ui(session)?;
            if let Some(chart) = target(&ui, id) {
                if chart["chartHover"]["active"].is_null()
                    && chart["chartHover"]["tooltip"].is_null()
                {
                    break chart;
                }
            }
            if Instant::now() >= deadline {
                return Err(XtaskError::failure(
                    "quota_tooltip_not_cleared",
                    format!("{name}: tooltip remained after moving to the settings tab"),
                ));
            }
            thread::sleep(Duration::from_millis(50));
        };
        evidence.push(serde_json::json!({"chart":name,"hovered":selected,"afterLeave":cleared}));
    }
    write_json(
        &session.run_dir.join("quota-hover-verified.json"),
        &Value::Array(evidence),
    )
}
