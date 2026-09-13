use super::*;
use std::collections::BTreeMap;

impl ShellHandles {
    pub(crate) fn drag_retained_ui_at(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        target: &str,
        start: (f32, f32),
        end: (f32, f32),
    ) -> Result<bool, FrameworkError> {
        if !end.0.is_finite()
            || !end.1.is_finite()
            || !document
                .context()
                .world()
                .layout_box(self.shell.stable_id())
                .is_some_and(|bounds| bounds.contains(end.0, end.1))
            || !self.hover_retained_ui(document, target, Some(start))?
        {
            return Ok(false);
        }
        let doc = document.document();
        let context = document.context_mut();
        let mut adapter = nana_ui::RuntimeInputAdapter::default();
        let mut handled = false;
        for (phase, point, buttons) in [
            (nana_ui_platform::PointerPhase::Down, start, 1),
            (nana_ui_platform::PointerPhase::Move, end, 1),
            (nana_ui_platform::PointerPhase::Up, end, 0),
        ] {
            let mut event = nana_ui_platform::InputEvent::Pointer {
                phase,
                pointer_id: 1,
                pointer_type: nana_ui_platform::PointerType::Mouse,
                x: point.0,
                y: point.1,
                screen_x: point.0,
                screen_y: point.1,
                button: 0,
                buttons,
                pressure: if buttons != 0 { 1.0 } else { 0.0 },
                tangential_pressure: 0.0,
                tilt_x: 0,
                tilt_y: 0,
                twist: 0,
                is_primary: true,
                activation_click: false,
                modifiers: Default::default(),
            };
            match adapter.dispatch(context, doc, &event) {
                Ok(result) => handled |= result.prevent_default,
                Err(error) => {
                    if let nana_ui_platform::InputEvent::Pointer { phase, buttons, .. } = &mut event
                    {
                        *phase = nana_ui_platform::PointerPhase::Cancel;
                        *buttons = 0;
                    }
                    let _ = adapter.dispatch(context, doc, &event);
                    return Err(error);
                }
            }
        }
        Ok(handled)
    }

    pub(crate) fn hover_retained_ui(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        target: &str,
        point: Option<(f32, f32)>,
    ) -> Result<bool, FrameworkError> {
        let Some(id) = self
            .retained_targets(document.context())
            .get(target)
            .copied()
        else {
            return Ok(false);
        };
        let context = document.context_mut();
        let doc = DocumentId::new(PRIMARY_DOCUMENT).expect("primary document");
        context.rebuild_hit_test(doc);
        if !self.target_is_mounted(context, id) {
            return Ok(false);
        }
        let mut ancestor = Some(id);
        while let Some(node) = ancestor {
            if context
                .world()
                .accessibility(node)
                .is_some_and(|state| state.disabled)
            {
                return Ok(false);
            }
            ancestor = context.world().node(node).and_then(|node| node.parent);
        }
        let Some((x, y)) = point.or_else(|| self.target_exposed_point(context, id)) else {
            return Ok(false);
        };
        if !x.is_finite()
            || !y.is_finite()
            || !context
                .world()
                .layout_box(self.shell.stable_id())
                .is_some_and(|bounds| bounds.contains(x, y))
        {
            return Ok(false);
        }
        let Some((layout_x, layout_y)) = context.world().pointer_layout_position(id, x, y) else {
            return Ok(false);
        };
        if !context
            .world()
            .layout_box(id)
            .is_some_and(|bounds| bounds.contains(layout_x, layout_y))
        {
            return Ok(false);
        }
        let mut hit = context.world().hit_test(doc, x, y);
        while hit.is_some() && hit != Some(id) {
            hit = hit.and_then(|node| context.world().node(node).and_then(|node| node.parent));
        }
        if hit != Some(id) {
            return Ok(false);
        }
        nana_ui::RuntimeInputAdapter::default().dispatch(
            context,
            doc,
            &nana_ui_platform::InputEvent::Pointer {
                phase: nana_ui_platform::PointerPhase::Move,
                pointer_id: 1,
                pointer_type: nana_ui_platform::PointerType::Mouse,
                x,
                y,
                screen_x: x,
                screen_y: y,
                button: 0,
                buttons: 0,
                pressure: 0.0,
                tangential_pressure: 0.0,
                tilt_x: 0,
                tilt_y: 0,
                twist: 0,
                is_primary: true,
                activation_click: false,
                modifiers: Default::default(),
            },
        )?;
        Ok(true)
    }

    pub(crate) fn click_retained_ui_at(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        target: &str,
        x: f32,
        y: f32,
    ) -> Result<bool, FrameworkError> {
        let Some(id) = self
            .retained_targets(document.context())
            .get(target)
            .copied()
        else {
            return Ok(false);
        };
        let context = document.context_mut();
        let document_id = DocumentId::new(PRIMARY_DOCUMENT).expect("primary document");
        context.rebuild_hit_test(document_id);
        if !self.target_is_mounted(context, id)
            || !context
                .world()
                .layout_box(self.shell.stable_id())
                .is_some_and(|bounds| bounds.contains(x, y))
        {
            return Ok(false);
        }
        let mut hit = context.world().hit_test(document_id, x, y);
        while hit.is_some() && hit != Some(id) {
            hit = hit.and_then(|node| context.world().node(node).and_then(|node| node.parent));
        }
        if hit != Some(id) {
            return Ok(false);
        }
        let mut adapter = nana_ui::RuntimeInputAdapter::default();
        let mut handled = false;
        for (phase, buttons) in [
            (nana_ui_platform::PointerPhase::Down, 1),
            (nana_ui_platform::PointerPhase::Up, 0),
        ] {
            handled |= adapter
                .dispatch(
                    context,
                    document_id,
                    &nana_ui_platform::InputEvent::Pointer {
                        phase,
                        pointer_id: 1,
                        pointer_type: nana_ui_platform::PointerType::Mouse,
                        x,
                        y,
                        screen_x: x,
                        screen_y: y,
                        button: 0,
                        buttons,
                        pressure: 0.0,
                        tangential_pressure: 0.0,
                        tilt_x: 0,
                        tilt_y: 0,
                        twist: 0,
                        is_primary: true,
                        activation_click: false,
                        modifiers: Default::default(),
                    },
                )?
                .prevent_default;
        }
        Ok(handled)
    }
    pub(crate) fn scroll_retained_ui(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        target: &str,
        delta_y: f32,
    ) -> Result<bool, FrameworkError> {
        let Some(id) = self
            .retained_targets(document.context())
            .get(target)
            .copied()
        else {
            return Ok(false);
        };
        let context = document.context_mut();
        let document_id = DocumentId::new(PRIMARY_DOCUMENT).expect("primary document");
        context.rebuild_hit_test(document_id);
        let Some((x, y)) = self.target_exposed_point(context, id) else {
            return Ok(false);
        };
        let event = nana_ui_platform::InputEvent::Wheel {
            x,
            y,
            delta_x: 0.0,
            delta_y,
            line_delta: false,
            modifiers: nana_ui_platform::InputModifiers::default(),
        };
        nana_ui::RuntimeInputAdapter::default().dispatch(context, document_id, &event)?;
        Ok(true)
    }
    pub(crate) fn key_retained_ui(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        target: &str,
        key: &str,
    ) -> Result<bool, FrameworkError> {
        let Some(id) = self
            .retained_targets(document.context())
            .get(target)
            .copied()
        else {
            return Ok(false);
        };
        let context = document.context_mut();
        let document_id = DocumentId::new(PRIMARY_DOCUMENT).expect("primary document");
        context.rebuild_hit_test(document_id);
        if !self.target_is_mounted(context, id) {
            return Ok(false);
        }
        let overlay_key = matches!(key, "Escape" | "Tab")
            && context
                .active_runtime_overlay(document_id)
                .is_some_and(|overlay| overlay.root == id);
        if !overlay_key
            && context.world().focused(document_id) != Some(id)
            && (!self.target_is_exposed(context, id) || !context.focus_node(document_id, id)?)
        {
            return Ok(false);
        }
        let event = crate::agent_debug::retained_key_event(key);
        Ok(nana_ui::RuntimeInputAdapter::default()
            .dispatch(context, document_id, &event)?
            .prevent_default)
    }

    fn retained_targets(&self, context: &AppContext) -> BTreeMap<String, StableNodeId> {
        let composer = &self.task_view.composer_view;
        let mut targets = BTreeMap::from([
            (
                "lilia.ui.timeline.scroll".into(),
                self.task_view.timeline_view.timeline_scroll.stable_id(),
            ),
            (
                target_ids::COMPOSER_INPUT.to_owned(),
                composer.composer.stable_id(),
            ),
            (
                "lilia.ui.new-conversation".into(),
                self.new_conversation.stable_id(),
            ),
            ("lilia.ui.send".into(), composer.send.stable_id()),
            (
                "lilia.ui.sidebar.search".into(),
                self.search_toggle.stable_id(),
            ),
            (
                "lilia.ui.sidebar.query".into(),
                self.search_input.stable_id(),
            ),
            ("lilia.ui.sidebar.more".into(), self.footer_more.stable_id()),
            (
                "lilia.ui.sidebar.scroll".into(),
                self.sidebar_scroll.stable_id(),
            ),
            (
                "lilia.ui.composer.plus".into(),
                composer.plus_menu.stable_id(),
            ),
            (
                "lilia.ui.composer.permission".into(),
                composer.permission_menu.stable_id(),
            ),
            ("lilia.ui.composer.model".into(), composer.model.stable_id()),
            (
                "lilia.ui.composer.reasoning".into(),
                composer.reasoning.stable_id(),
            ),
            (
                "lilia.ui.composer.review-target".into(),
                composer.review_target.stable_id(),
            ),
            (
                "lilia.ui.composer.review-value".into(),
                composer.review_value.stable_id(),
            ),
            (
                "lilia.ui.composer.review-submit".into(),
                composer.review_submit.stable_id(),
            ),
            (
                "lilia.ui.composer.review-cancel".into(),
                composer.review_cancel.stable_id(),
            ),
            (
                "lilia.ui.composer.worktree".into(),
                composer.worktree_menu.stable_id(),
            ),
            (
                "lilia.ui.project.scroll".into(),
                self.project_page.stable_id(),
            ),
            (
                "lilia.ui.todo.input".into(),
                self.todo_panel.input.stable_id(),
            ),
        ]);
        for (id, node) in &self.footer_nav {
            targets.insert(format!("lilia.ui.navigation.{id}"), node.stable_id());
        }
        for (id, node) in &self.row_tool_buttons {
            targets.insert(format!("lilia.ui.sidebar.tool.{id}"), node.stable_id());
        }
        if let Some(chart) = self.settings_view.quota_chart {
            targets.insert("lilia.ui.quota.trend".into(), chart.stable_id());
        }
        for (name, chart) in &self.settings_view.quota_donuts {
            targets.insert(
                format!("lilia.ui.settings.quota-{name}-chart"),
                chart.stable_id(),
            );
        }
        for (id, node) in &self.todo_panel.controls {
            targets.insert(format!("lilia.ui.todo.{id}"), node.stable_id());
        }
        if let Some(menu) = self.titlebar_menu {
            targets.insert("lilia.ui.menu.window".into(), menu.stable_id());
        }
        if let Some(menu) = self.more_menu {
            targets.insert("lilia.ui.menu.project".into(), menu.stable_id());
        }
        if let Some(button) = self.confirm_commit {
            targets.insert("lilia.ui.confirm.commit".into(), button.stable_id());
        }
        if let Some(button) = self.confirm_cancel {
            targets.insert("lilia.ui.confirm.cancel".into(), button.stable_id());
        }
        if let Some(viewer) = self.image_viewer {
            targets.insert("lilia.ui.image.viewer".into(), viewer.stable_id());
        }
        for (id, field) in &self.project_fields.editors {
            targets.insert(format!("lilia.ui.field.{id}"), field.stable_id());
        }
        for (id, field) in &self.settings_view.fields.editors {
            targets.insert(format!("lilia.ui.field.{id}"), field.stable_id());
            targets.insert(format!("lilia.ui.settings.{id}"), field.stable_id());
        }
        for (id, node) in &self.settings_view.form_switches {
            targets.insert(format!("lilia.ui.switch.{id}"), node.stable_id());
            targets.insert(format!("lilia.ui.settings.{id}"), node.stable_id());
        }
        for (id, node) in &self.settings_view.product_actions {
            targets.insert(format!("lilia.ui.action.{id}"), node.stable_id());
            targets.insert(format!("lilia.ui.settings.{id}"), node.stable_id());
            if let Some(rest) = id.strip_prefix("action:") {
                targets.insert(format!("lilia.ui.settings.{rest}"), node.stable_id());
            }
        }
        for (id, node) in self.settings_view.extensions.debug_nodes() {
            targets.insert(format!("lilia.ui.settings.{id}"), node);
        }
        for (id, node) in &self.extra_buttons {
            targets.insert(format!("lilia.ui.composer.{id}"), node.stable_id());
        }
        for (id, node) in &composer.extra_buttons {
            targets.insert(format!("lilia.ui.composer.{id}"), node.stable_id());
        }
        for (id, node) in &composer.completion_items {
            targets.insert(format!("lilia.ui.completion.{id}"), node.stable_id());
            targets.insert(format!("lilia.ui.composer.{id}"), node.stable_id());
        }
        for (id, node) in &self.task_view.timeline_view.timeline_markdown {
            targets.insert(format!("lilia.ui.timeline.markdown.{id}"), node.stable_id());
        }
        for (id, node) in &self.task_view.timeline_view.timeline_actions {
            if let Some(event_id) = id.strip_prefix("fork-") {
                targets.insert(
                    format!("lilia.ui.timeline.{event_id}.fork"),
                    node.stable_id(),
                );
            } else if let Some(event_id) = id.strip_prefix("continue-") {
                targets.insert(
                    format!("lilia.ui.timeline.{event_id}.continue"),
                    node.stable_id(),
                );
            }
        }
        for (id, node) in &self.pane_buttons {
            targets.insert(format!("lilia.ui.pane.{id}"), node.stable_id());
        }
        for (id, node) in &self.project_cards {
            targets.insert(format!("lilia.ui.project-card.{id}"), node.stable_id());
        }
        for (id, node) in &composer.plus_items {
            targets.insert(format!("lilia.ui.plus.{id}"), node.stable_id());
        }
        for (id, node) in self.task_view.pending_view.debug_nodes() {
            targets.insert(format!("lilia.ui.pending.{id}"), node);
        }
        for (id, node) in self.automation_view.debug_nodes() {
            targets.insert(format!("lilia.ui.automation.{id}"), node);
        }
        for (id, row) in &self.automation_view.rows {
            targets.insert(format!("lilia.ui.pane.auto-{id}"), row.stable_id());
        }
        if let Some(memory) = &self.memory_view {
            for (id, node) in memory.debug_nodes() {
                targets.insert(format!("lilia.ui.{id}"), node);
                match id.as_str() {
                    "switch.memory-enabled" => {
                        targets.insert(target_ids::MEMORY_TOGGLE.to_owned(), node);
                    }
                    "switch.memory-global" => {
                        targets.insert(target_ids::MEMORY_SETTINGS_GLOBAL.to_owned(), node);
                    }
                    "switch.memory-baseline" => {
                        targets.insert(target_ids::MEMORY_SETTINGS_BASELINE.to_owned(), node);
                    }
                    "switch.memory-task-enabled" => {
                        targets.insert(target_ids::TASK_MEMORY_TOGGLE.to_owned(), node);
                    }
                    "composer.memory-task-reset" => {
                        targets.insert(target_ids::TASK_MEMORY_RESET_COOLDOWN.to_owned(), node);
                    }
                    _ => {}
                }
            }
        }
        if let Some(roadmap) = &self.roadmap_view {
            for (id, node) in roadmap.debug_nodes() {
                targets.insert(format!("lilia.ui.{id}"), node);
            }
        }
        for (id, node) in &self.task_rows {
            targets.insert(format!("lilia.ui.sidebar.row.{id}"), node.stable_id());
        }
        let _ = context.read(self.settings_view.settings_sidebar, |sidebar| {
            let Some(assembly) = &sidebar.assembly else {
                return;
            };
            if let Some(body) = assembly.body {
                targets.insert("lilia.ui.settings.scroll".into(), body);
            }
            if let Some(back) = assembly.back_row {
                targets.insert("lilia.ui.settings.back".into(), back);
            }
            for (id, node) in &assembly.tab_rows {
                targets.insert(format!("lilia.ui.settings.tab.{id}"), *node);
            }
        });
        let _ = context.read(self.settings_view.settings_page, |page| {
            if let Some(scroll) = page.assembly.as_ref().and_then(|assembly| assembly.scroll) {
                targets.insert("lilia.ui.settings.content-scroll".into(), scroll);
            }
        });
        targets.insert(
            "lilia.ui.settings.appearance-sidebar".into(),
            self.settings_view.sidebar_mode.stable_id(),
        );
        targets.insert(
            "lilia.ui.settings.worktree-mode".into(),
            self.settings_view.worktree_mode.stable_id(),
        );
        if let Some(qr) = self.settings_view.remote_qr {
            targets.insert("lilia.ui.settings.remote-qr".into(), qr.stable_id());
        }
        targets
    }

    fn target_is_mounted(&self, context: &AppContext, id: StableNodeId) -> bool {
        let Some(bounds) = context.world().layout_box(id) else {
            return false;
        };
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return false;
        }
        let mut cursor = Some(id);
        while let Some(node) = cursor {
            if node == self.shell.stable_id() {
                return true;
            }
            cursor = context.world().node(node).and_then(|node| node.parent);
        }
        false
    }

    fn target_is_exposed(&self, context: &AppContext, id: StableNodeId) -> bool {
        self.target_exposed_point(context, id).is_some()
    }

    fn target_exposed_point(&self, context: &AppContext, id: StableNodeId) -> Option<(f32, f32)> {
        if !self.target_is_mounted(context, id) {
            return None;
        }
        let bounds = context.world().layout_box(id).expect("mounted target");
        let Some(viewport) = context.world().layout_box(self.shell.stable_id()) else {
            return None;
        };
        let document = DocumentId::new(PRIMARY_DOCUMENT).expect("primary document");
        for (x, y) in [
            (0.5, 0.5),
            (0.15, 0.5),
            (0.85, 0.5),
            (0.5, 0.15),
            (0.5, 0.85),
        ] {
            let Some((point_x, point_y)) = context.world().layout_pointer_position(
                id,
                bounds.x + bounds.width * x,
                bounds.y + bounds.height * y,
            ) else {
                continue;
            };
            if point_x < viewport.x
                || point_y < viewport.y
                || point_x >= viewport.x + viewport.width
                || point_y >= viewport.y + viewport.height
            {
                continue;
            }
            let mut hit = context.world().hit_test(document, point_x, point_y);
            while let Some(node) = hit {
                if node == id {
                    return Some((point_x, point_y));
                }
                hit = context.world().node(node).and_then(|node| node.parent);
            }
        }
        None
    }

    pub(crate) fn observe_retained_ui(
        &self,
        document: &nana_ui::runtime::RuntimeDocument,
    ) -> serde_json::Value {
        let context = document.context();
        let targets = self
            .retained_targets(context)
            .into_iter()
            .filter(|(_, id)| self.target_is_exposed(context, *id))
            .map(|(id, node)| {
                let bounds = context.world().layout_box(node).expect("mounted target");
                let dropdown = context.read(Entity::<nana_ui::Dropdown>::from_stable_id(node), |field| {
                    let selection = match &field.selection {
                        nana_ui::DropdownSelection::Single(value) => serde_json::json!(value.as_deref()),
                        nana_ui::DropdownSelection::Multiple(values) => serde_json::json!(values.iter().map(AsRef::as_ref).collect::<Vec<&str>>()),
                    };
                    serde_json::json!({"opened":field.opened,"highlighted":field.highlighted,
                        "selection":selection,"focused":context.world().focused(document.document()) == Some(node),
                        "options":field.options.iter().map(|option| serde_json::json!({
                            "value":option.value.as_ref(),"label":option.label.as_ref(),"disabled":option.disabled,
                        })).collect::<Vec<_>>()})
                }).ok();
                let chart = context.read(Entity::<nana_ui::runtime::DonutChart>::from_stable_id(node), |chart| {
                    (
                        chart.active,
                        chart.active.and_then(|i| chart.slices.get(i).map(|slice| slice.value)),
                        chart.active.and_then(|i| chart.labels.get(i).map(|label| label.to_string())),
                        chart.active.and_then(|i| chart.tooltip(i)),
                    )
                }).ok().or_else(|| context.read(Entity::<nana_ui::runtime::TimeSeriesChart>::from_stable_id(node), |chart| {
                    (
                        chart.active,
                        chart.active.and_then(|i| chart.values.get(i).copied()),
                        chart.active.and_then(|i| chart.axis_labels.get(i).map(|label| label.to_string())),
                        chart.active.and_then(|i| chart.tooltip(i)),
                    )
                }).ok());
                let chart_hover = chart.map(|(active, value, label, fallback)| {
                    let tooltip = context.world().node(node).into_iter().flat_map(|entry| entry.children)
                        .find_map(|child| context.read(Entity::<nana_ui::runtime::Tooltip>::from_stable_id(child), |tip| {
                            (!tip.style.layout.hidden && self.target_is_mounted(context, child))
                                .then(|| tip.label.to_string())
                        }).ok().flatten())
                        .or(fallback);
                    serde_json::json!({"active":active,"value":value,"label":label,"tooltip":tooltip})
                });
                let images = match context.world().component_geometry(node) {
                    Some(nana_ui::runtime::ComponentGeometry::NativeMarkdown { drawing, .. }) => drawing.commands.iter().filter_map(|command| {
                        let nana_ui::runtime::MarkdownDrawingCommand::Image { bounds, .. } = command else { return None; };
                        let (x, y) = context.world().layout_pointer_position(node, bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0)?;
                        let mut hit = context.world().hit_test(DocumentId::new(PRIMARY_DOCUMENT).expect("primary document"), x, y);
                        while hit.is_some() && hit != Some(node) {
                            hit = hit.and_then(|id| context.world().node(id).and_then(|entry| entry.parent));
                        }
                        (hit == Some(node)).then(|| serde_json::json!({"x": x, "y": y, "width":bounds.width, "height":bounds.height}))
                    }).collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                let graph_nodes = context
                    .read(
                        Entity::<nana_ui::runtime::GraphCanvas>::from_stable_id(node),
                        |canvas| {
                            let Some(bounds) = context.world().layout_box(node) else {
                                return Vec::new();
                            };
                            canvas
                                .model
                                .nodes()
                                .iter()
                                .zip(canvas.paint_nodes().iter())
                                .filter_map(|(model, paint)| {
                                    let (x, y) = context.world().layout_pointer_position(
                                        node,
                                        bounds.x + paint.x + paint.width / 2.0,
                                        bounds.y + paint.y + paint.title_height.max(1.0) / 2.0,
                                    )?;
                                    let document_id = DocumentId::new(PRIMARY_DOCUMENT)
                                        .expect("primary document");
                                    let mut hit = context.world().hit_test(document_id, x, y);
                                    while hit.is_some() && hit != Some(node) {
                                        hit = hit.and_then(|id| {
                                            context
                                                .world()
                                                .node(id)
                                                .and_then(|entry| entry.parent)
                                        });
                                    }
                                    (hit == Some(node)).then(|| {
                                        serde_json::json!({
                                            "id": model.id.as_str(),
                                            "x": x,
                                            "y": y,
                                        })
                                    })
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .ok()
                    .unwrap_or_default();
                let resize_grip = match context.world().component_geometry(node) {
                    Some(nana_ui::runtime::ComponentGeometry::TextInput { resize_grip: Some(grip), .. }) => {
                        context.world().layout_pointer_position(node, grip.x + grip.width / 2.0, grip.y + grip.height / 2.0)
                            .filter(|(x, y)| context.world().hit_test(document.document(), *x, *y) == Some(node))
                            .map(|(x, y)| serde_json::json!({"x": x, "y": y}))
                    }
                    _ => None,
                };
                let scroll = context.read(Entity::<ScrollView>::from_stable_id(node), |view| format!("{:?}", view.axes)).ok().map(|axes| {
                    let offset = context.world().scroll_offset(node);
                    let point = self.target_exposed_point(context, node).map(|(x, y)| serde_json::json!({
                        "x":x, "y":y, "hit":format!("{:?}", context.world().hit_test(document.document(), x, y)),
                    }));
                    let metrics = context.world().scroll_metrics(node).map(|metrics| serde_json::json!({
                        "viewportWidth": metrics.viewport_width, "viewportHeight": metrics.viewport_height,
                        "contentWidth": metrics.content_width, "contentHeight": metrics.content_height,
                    }));
                    serde_json::json!({"axes": axes, "eventPoint":point,
                        "offset": offset.map(|offset| serde_json::json!({"x": offset.x, "y": offset.y})),
                        "metrics": metrics,
                        "overflow": context.world().node_style(node).map(|style| serde_json::json!({
                            "x": format!("{:?}", style.layout.overflow_x), "y": format!("{:?}", style.layout.overflow_y),
                        })),
                    })
                });
                let (x, y) = context.world().layout_pointer_position(node, bounds.x, bounds.y).unwrap_or((bounds.x, bounds.y));
                serde_json::json!({"id": id, "bounds": {"x": x, "y": y,
                    "width": bounds.width, "height": bounds.height}, "images":images,"graphNodes":graph_nodes,"chartHover":chart_hover,"resizeGrip":resize_grip,"scroll":scroll,"dropdown":dropdown})
            })
            .collect::<Vec<_>>();
        let menu = self.titlebar_menu.and_then(|entity| {
            let Some(nana_ui::runtime::ComponentGeometry::MenuSurface { options, .. }) = context.world().component_geometry(entity.stable_id()) else { return None; };
            context.read(entity, |menu| {
                serde_json::json!({"targetId":"lilia.ui.menu.window", "highlighted":menu.highlighted,
                    "items":menu.visible_items().iter().zip(options.iter()).map(|(item, geometry)| serde_json::json!({
                        "value":item.value.as_ref(), "disabled": item.disabled,
                        "x": geometry.bounds.x + geometry.bounds.width / 2.0,
                        "y": geometry.bounds.y + geometry.bounds.height / 2.0,
                    })).collect::<Vec<_>>()})
            }).ok()
        });
        let project_menu = self.more_menu.and_then(|entity| {
            let Some(nana_ui::runtime::ComponentGeometry::MenuSurface { options, .. }) = context.world().component_geometry(entity.stable_id()) else { return None; };
            context.read(entity, |menu| {
                serde_json::json!({"targetId":"lilia.ui.menu.project",
                    "items":menu.visible_items().iter().zip(options.iter()).filter_map(|(item, geometry)| {
                        let (x,y) = context.world().layout_pointer_position(entity.stable_id(), geometry.bounds.x + geometry.bounds.width / 2.0, geometry.bounds.y + geometry.bounds.height / 2.0)?;
                        Some(serde_json::json!({"value":item.value.as_ref(), "disabled":item.disabled,"x":x,"y":y}))
                    }).collect::<Vec<_>>()})
            }).ok()
        });
        let project_scroll = self.project_page.stable_id();
        let scroll_diagnostic = context.world().layout_box(project_scroll).map(|bounds| {
            let samples = [
                (0.5, 0.5),
                (0.15, 0.5),
                (0.85, 0.5),
                (0.5, 0.15),
                (0.5, 0.85),
            ]
            .into_iter()
            .map(|(x, y)| {
                let point = context.world().layout_pointer_position(
                    project_scroll,
                    bounds.x + bounds.width * x,
                    bounds.y + bounds.height * y,
                );
                let mut ancestors = Vec::new();
                let mut hit =
                    point.and_then(|(x, y)| context.world().hit_test(document.document(), x, y));
                while let Some(node) = hit {
                    ancestors.push(format!("{node:?}"));
                    hit = context.world().node(node).and_then(|node| node.parent);
                }
                serde_json::json!({"point": point,"hitAncestors": ancestors})
            })
            .collect::<Vec<_>>();
            serde_json::json!({"id":format!("{project_scroll:?}"),
                "bounds":{"x":bounds.x,"y":bounds.y,"width":bounds.width,"height":bounds.height},
                "mounted":self.target_is_mounted(context,project_scroll),
                "samples":samples})
        });
        serde_json::json!({"windowId": "main", "targets": targets, "menu": menu, "projectMenu":project_menu,
            "diagnostics":{"projectScroll":scroll_diagnostic}})
    }

    pub(crate) fn act_retained_ui(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        target: &str,
        text: Option<&str>,
    ) -> Result<bool, FrameworkError> {
        let Some(id) = self
            .retained_targets(document.context())
            .get(target)
            .copied()
        else {
            return Ok(false);
        };
        let context = document.context_mut();
        context.rebuild_hit_test(DocumentId::new(PRIMARY_DOCUMENT).expect("primary document"));
        let document_id = DocumentId::new(PRIMARY_DOCUMENT).expect("primary document");
        let focused_input = text.is_some() && context.world().focused(document_id) == Some(id);
        if !self.target_is_mounted(context, id)
            || (!focused_input && !self.target_is_exposed(context, id))
        {
            return Ok(false);
        }
        if let Some(text) = text {
            if context.world().focused(document_id) != Some(id)
                && !context.focus_node(document_id, id)?
            {
                return Ok(false);
            }
            context.select_all_focused_text(document_id)?;
            context.replace_focused_text(document_id, text)
        } else {
            let _ = context.focus_node(
                DocumentId::new(PRIMARY_DOCUMENT).expect("primary document"),
                id,
            )?;
            context.activate_node(id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::DesktopComposerTurnRequest;

    fn reference_composer() -> crate::application::DesktopComposerState {
        let mut state = crate::application::DesktopComposerState::transient(
            lilia_contracts::TaskId::new("composer-ui-regression").unwrap(),
        );
        state
            .apply_transient_command(
                crate::application::DesktopComposerCommand::ApplyConversationReference {
                    expected_revision: 0,
                    content: "中文草稿\n第二行\n".into(),
                    reference: lilia_contracts::ChatConversationReference {
                        task_id: "related-task".into(),
                        title: "相关任务".into(),
                        route: "/chats/related-task".into(),
                        project_id: None,
                        project_name: None,
                    },
                },
            )
            .unwrap();
        state
    }

    fn composer_snapshot(state: &crate::application::DesktopComposerState) -> PrimaryShellSnapshot {
        let mut snapshot = empty_snapshot();
        snapshot.composer.composer_disabled = false;
        snapshot.composer.composer = state.content.clone();
        snapshot.composer.composer_task_id = Some(state.task_id.to_string());
        snapshot.composer.composer_revision = state.revision;
        snapshot.composer.composer_atom_spans = state.content_atom_spans();
        snapshot
    }

    fn refresh_composer(
        handles: &mut ShellHandles,
        document: &mut nana_ui::runtime::RuntimeDocument,
        state: &crate::application::DesktopComposerState,
    ) {
        handles.sync(document, &composer_snapshot(state)).unwrap();
        let document_id = document.document();
        document
            .context_mut()
            .layout_document(
                document_id,
                nana_ui::runtime::LayoutViewport::new(960.0, 720.0),
            )
            .unwrap();
    }

    fn composer_set_content(event: &ShellIntent) -> Option<&str> {
        match event {
            ShellIntent::AddressedComposer {
                action: ComposerInputAction::SetContent { value, .. },
                ..
            } => Some(value.as_str()),
            _ => None,
        }
    }

    #[test]
    fn deleting_reference_atom_and_undo_redo_updates_visible_and_submitted_context() {
        use crate::application::DesktopComposerCommand;
        let mut state = reference_composer();
        let reference = state.conversation_references[0].clone();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink_events = events.clone();
        let (mut document, mut handles) = mount_primary_shell(
            &composer_snapshot(&state),
            Arc::new(move |intent| sink_events.lock().unwrap().push(intent)),
        )
        .unwrap();
        refresh_composer(&mut handles, &mut document, &state);
        assert_eq!(
            document
                .context()
                .read(handles.task_view.composer_view.composer, |area| area
                    .atom_spans
                    .len())
                .unwrap(),
            1
        );
        for (key, expected) in [(None, 0), (Some("Meta+z"), 1), (Some("Meta+Shift+z"), 0)] {
            events.lock().unwrap().clear();
            if let Some(key) = key {
                assert!(
                    handles
                        .key_retained_ui(&mut document, target_ids::COMPOSER_INPUT, key)
                        .unwrap()
                );
            } else {
                assert!(
                    handles
                        .act_retained_ui(
                            &mut document,
                            target_ids::COMPOSER_INPUT,
                            Some("中文草稿\n第二行")
                        )
                        .unwrap()
                );
            }
            let value = events
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find_map(composer_set_content)
                .unwrap()
                .to_owned();
            state
                .apply_transient_command(DesktopComposerCommand::SetContent(value))
                .unwrap();
            refresh_composer(&mut handles, &mut document, &state);
            assert_eq!(
                document
                    .context()
                    .read(handles.task_view.composer_view.composer, |area| area
                        .atom_spans
                        .len())
                    .unwrap(),
                expected
            );
            assert_eq!(state.turn_request().conversation_references.len(), expected);
            assert_eq!(
                state.conversation_references,
                vec![reference.clone()],
                "undo metadata stays retained"
            );
            assert!(
                handles
                    .retained_targets(document.context())
                    .keys()
                    .all(|id| !id.contains("reference-remove-"))
            );
        }
    }

    #[test]
    fn retained_actions_emit_normal_intents_and_reject_parked_controls() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink_events = events.clone();
        let mut snapshot = empty_snapshot();
        snapshot.composer.composer_disabled = false;
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| sink_events.lock().unwrap().push(intent)),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let layout = |document: &mut nana_ui::runtime::RuntimeDocument| {
            document
                .context_mut()
                .layout_document(
                    DocumentId::new(PRIMARY_DOCUMENT).unwrap(),
                    nana_ui::runtime::LayoutViewport::new(1180.0, 760.0),
                )
                .unwrap();
        };
        layout(&mut document);
        assert!(
            handles
                .act_retained_ui(
                    &mut document,
                    target_ids::COMPOSER_INPUT,
                    Some("草稿\n中文")
                )
                .unwrap()
        );
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|event| composer_set_content(event) == Some("草稿\n中文"))
        );
        assert!(
            handles
                .act_retained_ui(
                    &mut document,
                    target_ids::COMPOSER_INPUT,
                    Some("第二次输入")
                )
                .unwrap()
        );
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|event| composer_set_content(event) == Some("第二次输入"))
        );
        assert!(
            !handles
                .act_retained_ui(&mut document, "lilia.ui.send", None)
                .unwrap()
        );
        snapshot.navigation = WindowRoute::Settings;
        handles.sync(&mut document, &snapshot).unwrap();
        layout(&mut document);
        assert!(
            !handles
                .act_retained_ui(
                    &mut document,
                    target_ids::COMPOSER_INPUT,
                    Some("must not replace")
                )
                .unwrap()
        );
        assert!(
            !handles
                .act_retained_ui(&mut document, "missing", None)
                .unwrap()
        );
    }
}
