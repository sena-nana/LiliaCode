use super::*;
use crate::application::MemoryScope;
use crate::module::memory::MemoryMessage;
use nana_ui::runtime::{Checkbox, NumberChanged, NumberInput};

fn message(message: MemoryMessage) -> ShellIntent {
    ShellIntent::MemoryCommand(message)
}
fn content_row(gap: f32) -> Stack {
    Stack::row(gap)
        .width(LengthSpec::Fill)
        .height(LengthSpec::Shrink)
        .grow(0.0)
        .shrink(0.0)
}

fn text_style(size: f32, weight: u16, muted: bool) -> NodeStyle {
    let mut style = NodeStyle::default();
    style.foreground = Some(if muted {
        SemanticColorRole::Muted
    } else {
        SemanticColorRole::Text
    });
    let layout = Arc::make_mut(&mut style.layout);
    layout.font_size = Some(size);
    layout.font_weight = Some(weight);
    layout.line_height = Some(nana_ui_core::LineHeightSpec::Absolute(18.0));
    style
}

fn memory_input_layout(style: &mut NodeStyle, multiline: bool) {
    let layout = Arc::make_mut(&mut style.layout);
    layout.font_size = Some(12.0);
    layout.font_weight = Some(400);
    layout.padding_top = Some(LengthSpec::Px(8.0));
    layout.padding_bottom = Some(LengthSpec::Px(8.0));
    layout.padding_left = Some(LengthSpec::Px(9.0));
    layout.padding_right = Some(LengthSpec::Px(9.0));
    layout.border_radius = Some(6.0);
    layout.border_width = Some(1.0);
    layout.min_height = multiline.then_some(LengthSpec::Px(148.0));
    layout.height = Some(if multiline {
        LengthSpec::Px(8.0 * 18.0 + 18.0)
    } else {
        LengthSpec::Shrink
    });
    layout.line_height = multiline.then_some(nana_ui_core::LineHeightSpec::Absolute(18.0));
}

fn memory_button_style(style: NodeStyle, primary: bool, disabled: bool) -> NodeStyle {
    let mut style = style
        .surface(SemanticColorRole::Subtle)
        .outline(SemanticColorRole::Border, 1.0);
    style.foreground = Some(SemanticColorRole::Text);
    style.interaction = nana_ui::runtime::InteractionStyle::default();
    style.interaction.hovered.background = Some(SemanticColorRole::Hover);
    style.interaction.pressed.background = Some(SemanticColorRole::Hover);
    if primary {
        style = style
            .surface_mix(nana_ui_core::SemanticColorMix::new(
                SemanticColorRole::Accent,
                SemanticColorRole::Surface,
                0.22,
            ))
            .outline_mix(
                nana_ui_core::SemanticColorMix::new(
                    SemanticColorRole::Accent,
                    SemanticColorRole::Border,
                    0.58,
                ),
                1.0,
            );
    }
    Arc::make_mut(&mut style.layout).opacity = Some(if disabled { 0.55 } else { 1.0 });
    style
}

fn memory_icon_data(name: &str) -> Icon {
    static PLUS: nana_ui_core::icon::IconData = nana_ui_core::icon::IconData {
        name: "memory-plus",
        shapes: &[],
        svg: include_str!("../../assets/icons/lucide/plus.svg"),
    };
    static SAVE: nana_ui_core::icon::IconData = nana_ui_core::icon::IconData {
        name: "memory-save",
        shapes: &[],
        svg: include_str!("../../assets/icons/lucide/save.svg"),
    };
    static REFRESH: nana_ui_core::icon::IconData = nana_ui_core::icon::IconData {
        name: "memory-refresh",
        shapes: &[],
        svg: r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 11a9 9 0 1 1 2 7M3 4v7h7"/></svg>"#,
    };
    static EDIT: nana_ui_core::icon::IconData = nana_ui_core::icon::IconData {
        name: "memory-edit",
        shapes: &[],
        svg: r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m16 3 5 5-12 12-6 1 1-6zM14 5l5 5"/></svg>"#,
    };
    static TOGGLE: nana_ui_core::icon::IconData = nana_ui_core::icon::IconData {
        name: "memory-toggle",
        shapes: &[],
        svg: r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m4 12 5 5L20 6"/></svg>"#,
    };
    static DELETE: nana_ui_core::icon::IconData = nana_ui_core::icon::IconData {
        name: "memory-delete",
        shapes: &[],
        svg: r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7"/></svg>"#,
    };
    Icon::from_data(match name {
        "plus" => &PLUS,
        "save" => &SAVE,
        "edit" => &EDIT,
        "toggle" => &TOGGLE,
        "delete" => &DELETE,
        _ => &REFRESH,
    })
}

fn updated_label(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "未记录".into();
    }
    #[cfg(unix)]
    {
        let seconds = timestamp.div_euclid(1000) as libc::time_t;
        let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
        if !unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) }.is_null() {
            let local = unsafe { local.assume_init() };
            return format!(
                "{:02}-{:02} {:02}:{:02}",
                local.tm_mon + 1,
                local.tm_mday,
                local.tm_hour,
                local.tm_min
            );
        }
    }
    crate::desktop::format_civil_date(timestamp)
}

impl ShellHandles {
    fn memory_icon(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        key: &str,
        name: &str,
        label: &str,
        intent: ShellIntent,
    ) -> Result<StableNodeId, FrameworkError> {
        let mut view = IconButton::new(memory_icon_data(name), label.to_owned())
            .kind(ButtonKind::Subtle)
            .size(ControlSize::Small)
            .with_tooltip(label.to_owned());
        let layout = Arc::make_mut(&mut view.style.layout);
        layout.width = Some(LengthSpec::Px(28.0));
        layout.min_width = Some(LengthSpec::Px(28.0));
        layout.height = Some(LengthSpec::Px(28.0));
        layout.min_height = Some(LengthSpec::Px(28.0));
        layout.border_radius = Some(6.0);
        view.style = memory_button_style(view.style, false, false);
        view.style.interaction.base.foreground = Some(SemanticColorRole::Text);
        let entity = if let Some(entity) = self.memory_icons.get(key).copied() {
            cx.update_component(entity, |old, _| *old = view)?;
            entity
        } else {
            let entity = cx.create_detached_component(doc, view)?;
            bind_activate(cx, entity, self.sink.clone(), intent)?;
            self.memory_icons.insert(key.to_owned(), entity);
            entity
        };
        Ok(entity.stable_id())
    }
    fn memory_stack(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        key: &str,
        stack: Stack,
        children: &[StableNodeId],
    ) -> Result<StableNodeId, FrameworkError> {
        let entity = if let Some(entity) = self.memory_layout.get(key).copied() {
            cx.update_component(entity, |view, _| *view = stack)?;
            entity
        } else {
            let entity = cx.create_detached_component(doc, stack)?;
            self.memory_layout.insert(key.to_owned(), entity);
            entity
        };
        cx.reconcile_children(entity.stable_id(), children)?;
        Ok(entity.stable_id())
    }
    fn memory_text(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        key: &str,
        value: String,
        style: NodeStyle,
    ) -> Result<StableNodeId, FrameworkError> {
        let entity = if let Some(entity) = self.memory_group_titles.get(key).copied() {
            cx.update_component(entity, |view, _| *view = Text::new(value).style(style))?;
            entity
        } else {
            let entity = cx.create_detached_component(doc, Text::new(value).style(style))?;
            self.memory_group_titles.insert(key.to_owned(), entity);
            entity
        };
        Ok(entity.stable_id())
    }
    fn memory_button(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        key: &str,
        label: &str,
        intent: ShellIntent,
        disabled: bool,
        primary: bool,
    ) -> Result<StableNodeId, FrameworkError> {
        let button = self.upsert_tagged_button(
            cx,
            doc,
            key,
            label,
            if primary {
                ButtonKind::Primary
            } else {
                ButtonKind::Subtle
            },
            intent,
            disabled,
        )?;
        cx.update_component(button, |view, _| {
            view.icon = match key {
                "memory-add-user" | "memory-add-project" | "memory-new" => {
                    Some(memory_icon_data("plus"))
                }
                "memory-save" => Some(memory_icon_data("save")),
                _ => None,
            };
            view.icon_size = Some(if key == "memory-save" { 15.0 } else { 14.0 });
            view.icon_gap = 6.0;
            let layout = Arc::make_mut(&mut view.style.layout);
            layout.width = None;
            layout.flex_grow = Some(0.0);
            layout.flex_shrink = Some(0.0);
            layout.height = Some(LengthSpec::Px(30.0));
            layout.min_height = Some(LengthSpec::Px(30.0));
            layout.font_size = Some(12.0);
            layout.border_radius = Some(6.0);
            layout.padding_left = Some(LengthSpec::Px(10.0));
            layout.padding_right = Some(LengthSpec::Px(10.0));
            *view = view
                .clone()
                .style(memory_button_style(view.style.clone(), primary, disabled));
        })?;
        Ok(button.stable_id())
    }
    fn memory_checkbox(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        keep: &mut HashSet<String>,
        order: &mut Vec<StableNodeId>,
        id: &str,
        label: &str,
        checked: bool,
        intent: ShellIntent,
    ) -> Result<(), FrameworkError> {
        keep.insert(id.into());
        let mut view = Checkbox::new(label, checked);
        let layout = Arc::make_mut(&mut view.style.layout);
        layout.width = Some(LengthSpec::Shrink);
        layout.flex_grow = Some(0.0);
        layout.font_size = Some(12.0);
        let entity = if let Some(entity) = self.memory_checkboxes.get(id).copied() {
            cx.update_component(entity, |current, _| *current = view)?;
            entity
        } else {
            let entity = cx.create_detached_component(doc, view)?;
            let sink = Arc::clone(&self.sink);
            cx.on(entity, move |_, _: &ToggleChanged, _| {
                emit(&sink, intent.clone())
            })?;
            self.memory_checkboxes.insert(id.into(), entity);
            entity
        };
        order.push(entity.stable_id());
        Ok(())
    }
    pub(super) fn sync_memory_page(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        if self.memory_editor_generation != Some(snapshot.memory_editor_generation) {
            for id in ["memory-title", "memory-body", "memory-tags"] {
                if let Some(editor) = self.form_fields.get(id) {
                    cx.clear_text_history(editor.stable_id())?;
                }
            }
            self.memory_editor_generation = Some(snapshot.memory_editor_generation);
        }
        self.memory_layout
            .retain(|_, entity| cx.world().contains(entity.stable_id()));
        self.memory_group_titles
            .retain(|_, entity| cx.world().contains(entity.stable_id()));
        self.memory_icons
            .retain(|_, entity| cx.world().contains(entity.stable_id()));
        self.memory_checkboxes
            .retain(|_, entity| cx.world().contains(entity.stable_id()));
        let stale = self
            .project_cards
            .keys()
            .filter(|key| {
                key.starts_with("memory-")
                    && !snapshot
                        .memory_cards
                        .iter()
                        .any(|card| **key == format!("memory-{}", card.id))
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(card) = self.project_cards.remove(&key) {
                cx.remove_view(card)?;
            }
        }
        let wide = snapshot.workspace.viewport_geometry().logical_size.0 > 920.0;
        let mut keep = HashSet::new();
        let mut toolbar_items = Vec::new();
        for (id, label, enabled, command) in [
            (
                "memory-global",
                "Memory",
                snapshot.memory_global_enabled,
                MemoryMessage::ToggleGlobal,
            ),
            (
                "memory-baseline",
                "基线注入",
                snapshot.memory_baseline_enabled,
                MemoryMessage::ToggleBaseline,
            ),
        ] {
            self.memory_checkbox(
                cx,
                doc,
                &mut keep,
                &mut toolbar_items,
                id,
                label,
                enabled,
                message(command),
            )?;
            let entity = self.memory_checkboxes[id];
            cx.update_component(entity, |view, _| {
                view.disabled = id == "memory-baseline" && !snapshot.memory_global_enabled;
                let layout = Arc::make_mut(&mut view.style.layout);
                layout.width = Some(LengthSpec::Shrink);
                layout.flex_grow = Some(0.0);
                layout.font_size = Some(12.0);
            })?;
        }
        let cooldown_label = self.memory_text(
            cx,
            doc,
            "cooldown-label",
            "冷却 turn".into(),
            text_style(12.0, 400, true),
        )?;
        let value = snapshot.memory_cooldown.parse::<f64>().unwrap_or_else(|_| {
            crate::application::MemorySettings::default().cooldown_turns as f64
        });
        let mut number_style = text_style(12.0, 400, false);
        let layout = Arc::make_mut(&mut number_style.layout);
        layout.width = Some(LengthSpec::Px(64.0));
        layout.height = Some(LengthSpec::Px(26.0));
        layout.min_height = Some(LengthSpec::Px(26.0));
        layout.border_radius = Some(6.0);
        let number = if let Some(number) = self.memory_cooldown_input {
            cx.update_component(number, |view, _| {
                if view.value() != value {
                    *view = NumberInput::new(value)
                        .range(1.0, 100.0)
                        .precision(0)
                        .style(number_style);
                }
            })?;
            number
        } else {
            let number = cx.create_detached_component(
                doc,
                NumberInput::new(value)
                    .range(1.0, 100.0)
                    .precision(0)
                    .style(number_style),
            )?;
            let sink = self.sink.clone();
            cx.on(number, move |_, event: &NumberChanged, _| {
                emit(
                    &sink,
                    message(MemoryMessage::CommitCooldown(event.value as u64)),
                )
            })?;
            self.memory_cooldown_input = Some(number);
            number
        };
        let cooldown = self.memory_stack(
            cx,
            doc,
            "cooldown",
            Stack::row(6.0),
            &[cooldown_label, number.stable_id()],
        )?;
        toolbar_items.push(cooldown);
        let switches = self.memory_stack(
            cx,
            doc,
            "switches",
            Stack::row(8.0).with_layout(|layout| {
                if !wide {
                    layout.flex_wrap = nana_ui_core::FlexWrap::Wrap;
                }
            }),
            &toolbar_items,
        )?;
        let refresh = self.memory_icon(
            cx,
            doc,
            "memory-refresh",
            "refresh",
            "刷新",
            message(MemoryMessage::Refresh),
        )?;
        let toolbar = self.memory_stack(
            cx,
            doc,
            "toolbar",
            content_row(12.0)
                .outline(SemanticColorRole::Border, 0.0)
                .with_layout(|layout| {
                    layout.justify_content = JustifySpec::SpaceBetween;
                    layout.padding_bottom = Some(LengthSpec::Px(10.0));
                    layout.border_bottom_width = Some(1.0);
                }),
            &[switches, refresh],
        )?;

        let mut lists = Vec::new();
        for (key, label, scope) in [
            ("user", "用户", MemoryScope::User),
            ("project", "项目", MemoryScope::Project),
        ] {
            let display = lilia_contracts::memory_scope_display(key)
                .expect("known memory scope display contract");
            let title = self.memory_text(
                cx,
                doc,
                &format!("group-{key}"),
                display.title.into(),
                text_style(13.0, 600, false),
            )?;
            let add = self.memory_button(
                cx,
                doc,
                &format!("memory-add-{key}"),
                "新增",
                message(MemoryMessage::NewInScope(scope)),
                false,
                false,
            )?;
            let heading = self.memory_stack(
                cx,
                doc,
                &format!("head-{key}"),
                content_row(12.0).with_layout(|layout| {
                    layout.justify_content = JustifySpec::SpaceBetween;
                    layout.margin_top = Some(LengthSpec::Px(4.0));
                    layout.margin_bottom = Some(LengthSpec::Px(8.0));
                }),
                &[title, add],
            )?;
            lists.push(heading);
            let mut cards = Vec::new();
            for memory in snapshot
                .memory_cards
                .iter()
                .filter(|memory| memory.scope_label == label)
            {
                let id = format!("memory-{}", memory.id);
                let row = if let Some(row) = self.project_cards.get(&id).copied() {
                    row
                } else {
                    let row = cx.create_detached_component(
                        doc,
                        ListItem::new(memory.title.clone()).auto_height(true),
                    )?;
                    bind_activate(
                        cx,
                        row,
                        self.sink.clone(),
                        ShellIntent::SelectMemory(memory.id.clone()),
                    )?;
                    self.project_cards.insert(id.clone(), row);
                    row
                };
                cx.update_component(row, |view, _| {
                    view.label = memory.title.clone();
                    view.style = content_row(8.0)
                        .padding(10.0)
                        .surface(SemanticColorRole::Surface)
                        .outline(
                            if memory.selected {
                                SemanticColorRole::Accent
                            } else {
                                SemanticColorRole::Border
                            },
                            1.0,
                        )
                        .radius(8.0)
                        .node_style();
                    if memory.selected {
                        view.style = view.style.clone().outline_mix(
                            nana_ui_core::SemanticColorMix::new(
                                SemanticColorRole::Accent,
                                SemanticColorRole::Border,
                                0.50,
                            ),
                            1.0,
                        );
                    }
                    Arc::make_mut(&mut view.style.layout).opacity =
                        Some(if memory.enabled { 1.0 } else { 0.62 });
                })?;
                let mut title_style = text_style(13.0, 600, false);
                let layout = Arc::make_mut(&mut title_style.layout);
                layout.white_space_nowrap = true;
                layout.text_overflow_ellipsis = true;
                let title = self.memory_text(
                    cx,
                    doc,
                    &format!("title-{}", memory.id),
                    memory.title.clone(),
                    title_style,
                )?;
                let mut body_style = text_style(12.0, 400, true);
                let layout = Arc::make_mut(&mut body_style.layout);
                layout.max_height = Some(LengthSpec::Px(36.0));
                layout.overflow_y = nana_ui_core::OverflowSpec::Hidden;
                let body = self.memory_text(
                    cx,
                    doc,
                    &format!("body-{}", memory.id),
                    memory.body.clone(),
                    body_style,
                )?;
                let date = updated_label(memory.updated_at);
                let meta = self.memory_text(
                    cx,
                    doc,
                    &format!("meta-{}", memory.id),
                    format!(
                        "{} · {}",
                        if memory.enabled { "启用" } else { "停用" },
                        date
                    ),
                    text_style(11.0, 400, true),
                )?;
                let content = self.memory_stack(
                    cx,
                    doc,
                    &format!("content-{}", memory.id),
                    Stack::column(4.0).grow(1.0).shrink(1.0),
                    &[title, body, meta],
                )?;
                let mut actions = Vec::new();
                for (suffix, label, command) in [
                    ("edit", "编辑", MemoryMessage::Select(memory.id.clone())),
                    (
                        "toggle",
                        if memory.enabled { "停用" } else { "启用" },
                        MemoryMessage::ToggleEntry(memory.id.clone()),
                    ),
                    (
                        "delete",
                        "删除",
                        MemoryMessage::DeleteEntry(memory.id.clone()),
                    ),
                ] {
                    actions.push(self.memory_icon(
                        cx,
                        doc,
                        &format!("memory-{}-{suffix}", memory.id),
                        suffix,
                        label,
                        message(command),
                    )?);
                }
                let actions = self.memory_stack(
                    cx,
                    doc,
                    &format!("actions-{}", memory.id),
                    Stack::row(8.0),
                    &actions,
                )?;
                cx.set_list_item_slots(
                    row,
                    nana_ui::runtime::ListItemSlots {
                        content: Some(content),
                        trailing: Some(actions),
                        ..Default::default()
                    },
                )?;
                cards.push(row.stable_id());
            }
            let empty = cards.is_empty();
            if empty {
                let mut style = text_style(11.0, 400, true);
                let layout = Arc::make_mut(&mut style.layout);
                layout.padding_top = Some(LengthSpec::Px(14.0));
                layout.padding_bottom = Some(LengthSpec::Px(20.0));
                let empty = self.memory_text(
                    cx,
                    doc,
                    &format!("empty-{key}"),
                    display.empty.into(),
                    style,
                )?;
                cards.push(empty);
            }
            lists.push(self.memory_stack(
                cx,
                doc,
                &format!("cards-{key}"),
                Stack::column(6.0).with_layout(|layout| {
                    layout.padding_bottom = Some(LengthSpec::Px(if empty { 0.0 } else { 18.0 }))
                }),
                &cards,
            )?);
        }
        let lists = self.memory_stack(
            cx,
            doc,
            "lists",
            Stack::column(0.0)
                .grow(if wide { 1.0 } else { 0.0 })
                .shrink(if wide { 1.0 } else { 0.0 }),
            &lists,
        )?;
        let editor_title = self.memory_text(
            cx,
            doc,
            "editor-title",
            if snapshot.memory_selected.is_some() {
                "编辑记忆"
            } else {
                "新增记忆"
            }
            .into(),
            text_style(13.0, 600, false),
        )?;
        let (scopes, project_scope, user_scope) = if let Some(radio) = self.memory_scope_radio {
            radio
        } else {
            use nana_ui::runtime::{
                SegmentedControl, SegmentedOption, SegmentedSelectionRequested,
                SelectionOrientation,
            };
            let mut control =
                SegmentedControl::radio_group().size(nana_ui_core::ControlSize::Small);
            control.orientation = SelectionOrientation::Horizontal;
            control.style = Stack::row(8.0).node_style();
            let group = cx.create_detached_component(doc, control)?;
            let project = cx.create_detached_component(doc, SegmentedOption::new("项目"))?;
            let user = cx.create_detached_component(doc, SegmentedOption::new("用户"))?;
            let sink = self.sink.clone();
            cx.on(group, move |_, event: &SegmentedSelectionRequested, _| {
                let scope = if event.option == project.stable_id() {
                    MemoryScope::Project
                } else if event.option == user.stable_id() {
                    MemoryScope::User
                } else {
                    return;
                };
                emit(&sink, message(MemoryMessage::SetScope(scope)));
            })?;
            self.memory_scope_radio = Some((group, project, user));
            (group, project, user)
        };
        cx.set_segmented_options(
            scopes,
            vec![project_scope, user_scope],
            Some(if snapshot.memory_scope_label == "用户" {
                user_scope
            } else {
                project_scope
            }),
        )?;
        let scopes = scopes.stable_id();
        let head = self.memory_stack(
            cx,
            doc,
            "editor-head",
            content_row(12.0)
                .with_layout(|layout| layout.justify_content = JustifySpec::SpaceBetween),
            &[editor_title, scopes],
        )?;
        let mut editor = vec![head];
        for (key, value, intent) in [
            (
                "memory-title",
                snapshot.memory_title.as_str(),
                ShellIntent::MemoryTitleChanged as fn(String) -> ShellIntent,
            ),
            (
                "memory-body",
                snapshot.memory_body.as_str(),
                ShellIntent::MemoryBodyChanged,
            ),
            (
                "memory-tags",
                snapshot.memory_tags.as_str(),
                ShellIntent::MemoryTagsChanged,
            ),
        ] {
            keep.insert(key.into());
            let field = self.form_editor(cx, doc, key, value, intent)?;
            match field {
                ShellFormEditor::Line(field) => cx.update_component(field, |view, _| {
                    memory_input_layout(&mut view.style, false);
                    view.label = Some(Arc::from(settings_field_label(key)));
                    view.max_length = (key == "memory-title").then_some(120);
                    view.placeholder = Arc::from(if key == "memory-tags" {
                        "逗号分隔"
                    } else {
                        ""
                    });
                })?,
                ShellFormEditor::Multiline(field) => cx.update_component(field, |view, _| {
                    memory_input_layout(&mut view.style, true);
                    view.label = Some(Arc::from(settings_field_label(key)));
                    view.resize_vertical = true;
                })?,
            }
            let label = self.memory_text(
                cx,
                doc,
                &format!("field-label-{key}"),
                settings_field_label(key).into(),
                text_style(12.0, 400, true),
            )?;
            editor.push(self.memory_stack(
                cx,
                doc,
                &format!("field-{key}"),
                Stack::column(6.0),
                &[label, field.stable_id()],
            )?);
        }
        self.memory_checkbox(
            cx,
            doc,
            &mut keep,
            &mut editor,
            "memory-enabled",
            "启用这条记忆",
            snapshot.memory_draft_enabled,
            message(MemoryMessage::ToggleDraftEnabled),
        )?;
        let clear = self.memory_button(
            cx,
            doc,
            "memory-new",
            "清空",
            ShellIntent::NewMemory,
            false,
            false,
        )?;
        let save = self.memory_button(
            cx,
            doc,
            "memory-save",
            "保存",
            ShellIntent::SaveMemory,
            snapshot.memory_title.trim().is_empty() || snapshot.memory_body.trim().is_empty(),
            true,
        )?;
        editor.push(
            self.memory_stack(
                cx,
                doc,
                "editor-actions",
                content_row(12.0)
                    .with_layout(|layout| layout.justify_content = JustifySpec::SpaceBetween),
                &[clear, save],
            )?,
        );
        let editor = self.memory_stack(
            cx,
            doc,
            "editor",
            Stack::column(12.0)
                .padding(12.0)
                .surface(SemanticColorRole::Surface)
                .outline(SemanticColorRole::Border, 1.0)
                .radius(8.0)
                .with_layout(|layout| {
                    if wide {
                        layout.width = Some(LengthSpec::Px(380.0));
                        layout.min_width = Some(LengthSpec::Px(320.0));
                        layout.max_width = Some(LengthSpec::Px(380.0));
                        layout.flex_shrink = Some(1.0);
                    }
                }),
            &editor,
        )?;
        let grid = self.memory_stack(
            cx,
            doc,
            "grid",
            if wide {
                content_row(16.0)
                    .with_layout(|layout| layout.align_items = nana_ui_core::AlignSpec::Start)
            } else {
                Stack::column(16.0)
            },
            &[lists, editor],
        )?;
        let mut page = vec![toolbar];
        if !snapshot.project_page_body.is_empty() {
            let mut style = text_style(12.0, 400, false);
            style.foreground = Some(SemanticColorRole::Danger);
            style = style
                .outline_mix(
                    nana_ui_core::SemanticColorMix::new(
                        SemanticColorRole::Danger,
                        SemanticColorRole::Border,
                        0.35,
                    ),
                    1.0,
                )
                .surface_mix(nana_ui_core::SemanticColorMix::alpha(
                    SemanticColorRole::Danger,
                    if snapshot.theme == ThemeMode::Dark {
                        0.14
                    } else {
                        0.10
                    },
                ))
                .radius(6.0);
            let error_layout = Arc::make_mut(&mut style.layout);
            error_layout.padding_top = Some(LengthSpec::Px(8.0));
            error_layout.padding_bottom = Some(LengthSpec::Px(8.0));
            error_layout.padding_left = Some(LengthSpec::Px(10.0));
            error_layout.padding_right = Some(LengthSpec::Px(10.0));
            page.push(self.memory_text(
                cx,
                doc,
                "error",
                snapshot.project_page_body.clone(),
                style,
            )?);
        }
        page.push(grid);
        let injection_title = self.memory_text(
            cx,
            doc,
            "injection-title",
            "会话注入".into(),
            text_style(13.0, 600, false),
        )?;
        let mut injection = vec![injection_title];
        cx.update_component(self.memory_task_menu, |menu, _| {
            *menu = composer_menu(&snapshot.memory_task_label, snapshot.memory_task_menu_open)
        })?;
        sync_action_menu_items(
            cx,
            doc,
            self.memory_task_menu,
            snapshot.memory_task_menu_open,
            &snapshot.memory_task_options,
            &mut self.memory_task_items,
            &self.sink,
            |id| message(MemoryMessage::SelectInjectionTask(id.to_owned())),
        )?;
        injection.push(self.memory_task_menu.stable_id());
        if let Some(enabled) = snapshot.memory_task_enabled {
            self.upsert_switch(
                cx,
                doc,
                &mut keep,
                &mut injection,
                "memory-task-enabled",
                "为此会话注入记忆",
                enabled,
                message(MemoryMessage::ToggleTaskInjection),
            )?;
            injection.push(self.memory_button(
                cx,
                doc,
                "memory-task-reset",
                "重置会话冷却",
                message(MemoryMessage::ResetTaskCooldown),
                false,
                false,
            )?);
        }
        page.push(
            self.memory_stack(
                cx,
                doc,
                "injection",
                Stack::column(8.0)
                    .padding(12.0)
                    .outline(SemanticColorRole::Border, 1.0)
                    .radius(8.0),
                &injection,
            )?,
        );
        cx.update_component(self.project_page_content, |view, _| {
            *view = Stack::column(12.0).with_layout(|layout| {
                layout.padding_top = Some(LengthSpec::Px(16.0));
                layout.padding_left = Some(LengthSpec::Px(20.0));
                layout.padding_right = Some(LengthSpec::Px(20.0));
                layout.padding_bottom = Some(LengthSpec::Px(20.0));
            })
        })?;
        cx.reconcile_children(self.project_page_content.stable_id(), &page)
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_page_uses_window_breakpoint_and_keeps_empty_scope_sections() {
        for width in [880.0, 960.0, 1440.0] {
            let mut snapshot = empty_snapshot();
            snapshot.project_page = Some(ShellProjectPage::Memory);
            snapshot.workspace.update(
                nana_ui::WorkspaceMutation::SetViewport {
                    width,
                    height: 900.0,
                },
                std::time::Duration::ZERO,
            );
            let (mut document, mut handles) =
                mount_primary_shell(&snapshot, Arc::new(|_| {})).unwrap();
            handles.sync(&mut document, &snapshot).unwrap();
            let doc = document.document();
            let ids = document.context().world().document_order(doc);
            document.context_mut().resolve_styles(&ids).unwrap();
            document
                .context_mut()
                .layout_document(doc, nana_ui::runtime::LayoutViewport::new(width, 900.0))
                .unwrap();
            let world = document.context().world();
            let list = world
                .layout_box(handles.memory_layout["lists"].stable_id())
                .unwrap();
            let editor = world
                .layout_box(handles.memory_layout["editor"].stable_id())
                .unwrap();
            if width > 920.0 {
                assert!(
                    editor.x >= list.x + list.width + 15.0,
                    "{width}: {list:?} {editor:?}"
                );
                assert!((list.y - editor.y).abs() < 1.0);
                assert!(editor.width >= 319.0 && editor.width <= 381.0);
            } else {
                assert!(editor.y >= list.y + list.height + 15.0);
                assert!((list.x - editor.x).abs() < 1.0);
            }
            for key in ["group-user", "group-project", "empty-user", "empty-project"] {
                assert!(world.is_overlay_reachable(handles.memory_group_titles[key].stable_id()));
            }
            let field = world
                .layout_box(handles.memory_cooldown_input.unwrap().stable_id())
                .unwrap();
            assert_eq!(field.width, 64.0);
            assert_eq!(field.height, 26.0);
        }
    }

    #[test]
    fn memory_page_grid_height_contains_both_columns_before_injection() {
        for width in [880.0, 960.0, 1440.0] {
            for count in [0, 12] {
                let mut snapshot = empty_snapshot();
                snapshot.project_page = Some(ShellProjectPage::Memory);
                snapshot.memory_title = "正常入口多行记忆".into();
                snapshot.memory_body="项目约定：使用正常用户入口。\n验证要求：保存、禁用、重新启用。\n重启之后保留全部三行。".into();
                snapshot.memory_tags = "验收,持久化".into();
                snapshot.memory_task_enabled = Some(true);
                snapshot.workspace.update(
                    nana_ui::WorkspaceMutation::SetViewport {
                        width,
                        height: 600.0,
                    },
                    std::time::Duration::ZERO,
                );
                snapshot.memory_cards = (0..count)
                    .map(|index| ShellMemoryCard {
                        id: index.to_string(),
                        title: format!("Memory {index}"),
                        subtitle: String::new(),
                        scope_label: "项目".into(),
                        enabled: true,
                        selected: false,
                        body: "First line\nSecond line\nThird line".into(),
                        updated_at: 1,
                    })
                    .collect();
                let (mut document, mut handles) =
                    mount_primary_shell(&snapshot, Arc::new(|_| {})).unwrap();
                handles.sync(&mut document, &snapshot).unwrap();
                let doc = document.document();
                let ids = document.context().world().document_order(doc);
                document.context_mut().resolve_styles(&ids).unwrap();
                document
                    .context_mut()
                    .layout_document(doc, nana_ui::runtime::LayoutViewport::new(width, 600.0))
                    .unwrap();
                let mut shaper = nana_ui::NanaTextShaper::default();
                document
                    .context_mut()
                    .shape_text(&ids, &mut shaper)
                    .unwrap();
                document.context_mut().resolve_styles(&ids).unwrap();
                document
                    .context_mut()
                    .layout_document(doc, nana_ui::runtime::LayoutViewport::new(width, 600.0))
                    .unwrap();
                let world = document.context().world();
                let list = world
                    .layout_box(handles.memory_layout["lists"].stable_id())
                    .unwrap();
                let editor = world
                    .layout_box(handles.memory_layout["editor"].stable_id())
                    .unwrap();
                let grid = world
                    .layout_box(handles.memory_layout["grid"].stable_id())
                    .unwrap();
                let injection = world
                    .layout_box(handles.memory_layout["injection"].stable_id())
                    .unwrap();
                let tags = world
                    .layout_box(handles.memory_layout["field-memory-tags"].stable_id())
                    .unwrap();
                let enabled = world
                    .layout_box(handles.memory_checkboxes["memory-enabled"].stable_id())
                    .unwrap();
                let actions = world
                    .layout_box(handles.memory_layout["editor-actions"].stable_id())
                    .unwrap();
                assert!(
                    grid.y + grid.height >= editor.y + editor.height - 0.1,
                    "{width}/{count}: grid {grid:?}, editor {editor:?}"
                );
                assert!(grid.y + grid.height >= list.y + list.height - 0.1);
                assert!(
                    injection.y >= editor.y + editor.height + 11.0,
                    "{width}/{count}: injection {injection:?}, editor {editor:?}"
                );
                assert!(injection.y >= list.y + list.height + 11.0);
                assert!(
                    enabled.y >= tags.y + tags.height + 11.0,
                    "{width}/{count}: tags {tags:?}, enabled {enabled:?}"
                );
                assert!(actions.y >= enabled.y + enabled.height + 11.0);
                assert!(actions.y + actions.height <= editor.y + editor.height - 11.0);
            }
        }
    }

    #[test]
    fn memory_scope_radio_uses_normal_activation_and_arrow_navigation() {
        let mut snapshot = empty_snapshot();
        snapshot.project_page = Some(ShellProjectPage::Memory);
        snapshot.memory_scope_label = "项目".into();
        let intents = Arc::new(Mutex::new(Vec::new()));
        let output = intents.clone();
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| output.lock().unwrap().push(intent)),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let (group, project, user) = handles.memory_scope_radio.unwrap();
        assert!(document
            .context_mut()
            .activate_node(user.stable_id())
            .unwrap());
        assert!(matches!(
            intents.lock().unwrap().last(),
            Some(ShellIntent::MemoryCommand(MemoryMessage::SetScope(
                MemoryScope::User
            )))
        ));
        snapshot.memory_scope_label = "用户".into();
        handles.sync(&mut document, &snapshot).unwrap();
        assert!(!document
            .context()
            .read(project, |option| option.selected())
            .unwrap());
        assert!(document
            .context()
            .read(user, |option| option.selected())
            .unwrap());
        let doc = document.document();
        let event = nana_ui_platform::InputEvent::Keyboard {
            pressed: true,
            key: "ArrowLeft".into(),
            text: None,
            code: "ArrowLeft".into(),
            repeat: false,
            modifiers: Default::default(),
        };
        assert!(
            nana_ui::RuntimeInputAdapter::default()
                .dispatch(document.context_mut(), doc, &event)
                .unwrap()
                .prevent_default
        );
        assert!(matches!(
            intents.lock().unwrap().last(),
            Some(ShellIntent::MemoryCommand(MemoryMessage::SetScope(
                MemoryScope::Project
            )))
        ));
        snapshot.memory_scope_label = "项目".into();
        handles.sync(&mut document, &snapshot).unwrap();
        assert!(document
            .context()
            .read(project, |option| option.selected())
            .unwrap());
        assert!(!document
            .context()
            .read(user, |option| option.selected())
            .unwrap());
        assert_eq!(
            document
                .context()
                .read(group, |control| control.orientation)
                .unwrap(),
            nana_ui::runtime::SelectionOrientation::Horizontal
        );
        assert_eq!(
            document.context().world().focused(doc),
            Some(project.stable_id())
        );
    }

    #[test]
    fn memory_body_resize_survives_normal_snapshot_refresh_without_editing_draft() {
        let mut snapshot = empty_snapshot();
        snapshot.project_page = Some(ShellProjectPage::Memory);
        snapshot.memory_body = "原始正文".into();
        snapshot.workspace.update(
            nana_ui::WorkspaceMutation::SetViewport {
                width: 1440.0,
                height: 900.0,
            },
            std::time::Duration::ZERO,
        );
        let intents = Arc::new(Mutex::new(Vec::new()));
        let output = intents.clone();
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| output.lock().unwrap().push(intent)),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let doc = document.document();
        let body = handles.form_fields["memory-body"].stable_id();
        let layout = |context: &mut AppContext| {
            let ids = context.world().document_order(doc);
            context.resolve_styles(&ids).unwrap();
            context
                .shape_text(&ids, &mut nana_ui::NanaTextShaper::default())
                .unwrap();
            context
                .layout_document(doc, nana_ui::runtime::LayoutViewport::new(1440.0, 900.0))
                .unwrap();
            context.rebuild_hit_test(doc);
        };
        layout(document.context_mut());
        let before = document.context().world().layout_box(body).unwrap();
        let region = handles.project_page.stable_id();
        let region_style = document
            .context()
            .world()
            .node_style(region)
            .unwrap()
            .clone();
        let nana_ui::runtime::ComponentGeometry::TextInput {
            resize_grip: Some(grip),
            ..
        } = document.context().world().component_geometry(body).unwrap()
        else {
            panic!("memory resize grip")
        };
        let (x, y) = document
            .context()
            .world()
            .layout_pointer_position(body, grip.x + grip.width / 2.0, grip.y + grip.height / 2.0)
            .unwrap();
        assert!(!handles
            .drag_retained_ui_at(
                &mut document,
                "lilia.ui.field.memory-body",
                (0.0, 0.0),
                (x, y + 64.0)
            )
            .unwrap());
        assert_eq!(document.context().world().layout_box(body).unwrap(), before);
        assert!(handles
            .drag_retained_ui_at(
                &mut document,
                "lilia.ui.field.memory-body",
                (x, y),
                (x, y + 64.0)
            )
            .unwrap());
        assert!(!intents
            .lock()
            .unwrap()
            .iter()
            .any(|intent| matches!(intent, ShellIntent::MemoryBodyChanged(_))));
        assert_eq!(
            document.context().world().node_style(region),
            Some(&region_style)
        );
        snapshot.memory_body = "刷新后的正文".into();
        handles.sync(&mut document, &snapshot).unwrap();
        layout(document.context_mut());
        let after = document.context().world().layout_box(body).unwrap();
        assert!((after.height - before.height - 64.0).abs() < 0.5);
        assert!((after.width - before.width).abs() < 0.5);
        assert_eq!(document.context().world().text(body), Some("刷新后的正文"));
        assert_eq!(document.context().world().pointer_capture(doc, 1), None);
    }

    fn history_key(cx: &mut AppContext, doc: DocumentId, key: &str, control: bool) {
        nana_ui::RuntimeInputAdapter::default()
            .dispatch(
                cx,
                doc,
                &if control {
                    crate::agent_debug::retained_key_event(&format!("Meta+{key}"))
                } else {
                    nana_ui_platform::InputEvent::Keyboard {
                        pressed: true,
                        key: key.into(),
                        code: key.into(),
                        text: Some(key.into()),
                        repeat: false,
                        modifiers: Default::default(),
                    }
                },
            )
            .unwrap();
    }

    #[test]
    fn memory_history_survives_draft_echo_but_not_same_value_record_switch() {
        let mut snapshot = empty_snapshot();
        snapshot.project_page = Some(ShellProjectPage::Memory);
        let (mut document, mut handles) = mount_primary_shell(&snapshot, Arc::new(|_| {})).unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let doc = document.document();
        let editor = handles.form_fields["memory-body"].stable_id();
        document.context_mut().focus_node(doc, editor).unwrap();
        history_key(document.context_mut(), doc, "a", false);
        snapshot.memory_body = "a".into();
        handles.sync(&mut document, &snapshot).unwrap();
        history_key(document.context_mut(), doc, "z", true);
        assert_eq!(document.context().world().text(editor), Some(""));
        history_key(document.context_mut(), doc, "b", false);
        snapshot.memory_body = "b".into();
        snapshot.memory_editor_generation += 1;
        handles.sync(&mut document, &snapshot).unwrap();
        history_key(document.context_mut(), doc, "z", true);
        assert_eq!(document.context().world().text(editor), Some("b"));
    }

    #[test]
    fn composer_history_survives_revision_echo_but_not_same_value_task_switch() {
        let mut snapshot = empty_snapshot();
        snapshot.composer_task_id = Some("task-a".into());
        snapshot.composer_disabled = false;
        let (mut document, mut handles) = mount_primary_shell(&snapshot, Arc::new(|_| {})).unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let doc = document.document();
        let editor = handles.composer.stable_id();
        document.context_mut().focus_node(doc, editor).unwrap();
        history_key(document.context_mut(), doc, "a", false);
        snapshot.composer = "a".into();
        snapshot.composer_revision += 1;
        handles.sync(&mut document, &snapshot).unwrap();
        history_key(document.context_mut(), doc, "z", true);
        assert_eq!(document.context().world().text(editor), Some(""));
        history_key(document.context_mut(), doc, "b", false);
        snapshot.composer = "b".into();
        snapshot.composer_task_id = Some("task-b".into());
        handles.sync(&mut document, &snapshot).unwrap();
        history_key(document.context_mut(), doc, "z", true);
        assert_eq!(document.context().world().text(editor), Some("b"));
    }

    #[test]
    fn roadmap_and_file_history_do_not_cross_same_value_documents() {
        let mut snapshot = empty_snapshot();
        snapshot.project_page = Some(ShellProjectPage::Roadmap);
        snapshot.milestone_editor_identity = Some(("project".into(), "milestone-a".into()));
        let (mut document, mut handles) = mount_primary_shell(&snapshot, Arc::new(|_| {})).unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let doc = document.document();
        let editor = handles.form_fields["project-milestone-title"].stable_id();
        document.context_mut().focus_node(doc, editor).unwrap();
        history_key(document.context_mut(), doc, "a", false);
        snapshot.milestone_title = "a".into();
        snapshot.milestone_editor_identity = Some(("project".into(), "milestone-b".into()));
        handles.sync(&mut document, &snapshot).unwrap();
        history_key(document.context_mut(), doc, "z", true);
        assert_eq!(document.context().world().text(editor), Some("a"));

        snapshot.project_page = None;
        snapshot.document = Some(ShellDocumentSnapshot {
            item_id: "file-a".into(),
            title: "file-a".into(),
            text: String::new(),
            language: "text".into(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        handles.sync(&mut document, &snapshot).unwrap();
        snapshot.panes = vec![ShellPaneRow {
            id: "primary".into(),
            active: true,
            items: vec![ShellPaneItem {
                id: "file-a".into(),
                title: "file-a".into(),
                kind: "document-editor".into(),
                selected: true,
                closable: true,
            }],
            document: snapshot.document.clone(),
            terminal: None,
        }];
        snapshot.pane_layout = ShellPaneLayout::Leaf("primary".into());
        handles.sync(&mut document, &snapshot).unwrap();
        let editor = handles.workspace_editor.stable_id();
        document.context_mut().focus_node(doc, editor).unwrap();
        history_key(document.context_mut(), doc, "b", false);
        snapshot.document.as_mut().unwrap().text = "b".into();
        handles.sync(&mut document, &snapshot).unwrap();
        history_key(document.context_mut(), doc, "z", true);
        assert_eq!(document.context().world().text(editor), Some(""));
        history_key(document.context_mut(), doc, "c", false);
        snapshot.document.as_mut().unwrap().text = "c".into();
        snapshot.document.as_mut().unwrap().item_id = "file-b".into();
        handles.sync(&mut document, &snapshot).unwrap();
        history_key(document.context_mut(), doc, "z", true);
        assert_eq!(document.context().world().text(editor), Some("c"));
    }

    #[test]
    fn memory_history_chords_use_visible_retained_targets_and_emit_draft_changes() {
        let mut snapshot = empty_snapshot();
        snapshot.project_page = Some(ShellProjectPage::Memory);
        snapshot.memory_body = "正文".into();
        snapshot.workspace.update(
            nana_ui::WorkspaceMutation::SetViewport {
                width: 1440.0,
                height: 900.0,
            },
            std::time::Duration::ZERO,
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| captured.lock().unwrap().push(intent)),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let doc = document.document();
        let context = document.context_mut();
        let ids = context.world().document_order(doc);
        context.resolve_styles(&ids).unwrap();
        context
            .shape_text(&ids, &mut nana_ui::NanaTextShaper::default())
            .unwrap();
        context
            .layout_document(doc, nana_ui::runtime::LayoutViewport::new(1440.0, 900.0))
            .unwrap();
        context.rebuild_hit_test(doc);
        let target = "lilia.ui.field.memory-title";
        assert!(handles
            .act_retained_ui(&mut document, target, Some("原始标题"))
            .unwrap());
        snapshot.memory_title = "原始标题".into();
        handles.sync(&mut document, &snapshot).unwrap();
        assert!(handles
            .act_retained_ui(&mut document, "lilia.ui.field.memory-body", Some("正文"))
            .unwrap());
        assert!(handles
            .act_retained_ui(&mut document, "lilia.ui.composer.memory-save", None)
            .unwrap());
        assert!(handles
            .act_retained_ui(&mut document, target, Some("临时标题"))
            .unwrap());
        assert!(handles
            .key_retained_ui(&mut document, target, "Meta+z")
            .unwrap());
        assert!(
            matches!(events.lock().unwrap().last(), Some(ShellIntent::MemoryTitleChanged(value)) if value == "原始标题")
        );
        assert!(handles
            .key_retained_ui(&mut document, target, "Meta+Shift+z")
            .unwrap());
        assert!(
            matches!(events.lock().unwrap().last(), Some(ShellIntent::MemoryTitleChanged(value)) if value == "临时标题")
        );
        assert!(!handles
            .key_retained_ui(&mut document, "lilia.ui.field.missing", "Meta+z")
            .unwrap());
    }

    #[test]
    fn memory_title_normal_paste_respects_limit_and_emits_the_accepted_draft() {
        let mut snapshot = empty_snapshot();
        snapshot.project_page = Some(ShellProjectPage::Memory);
        let intents = Arc::new(Mutex::new(Vec::new()));
        let output = intents.clone();
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| output.lock().unwrap().push(intent)),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let doc = document.document();
        let title = handles.form_fields["memory-title"].stable_id();
        document.context_mut().focus_node(doc, title).unwrap();
        let clipboard =
            nana_ui_platform::shared_clipboard(nana_ui_platform::MemoryClipboard::new());
        clipboard
            .lock()
            .unwrap()
            .write_text(&format!("{}😀", "a".repeat(119)));
        let event = nana_ui_platform::InputEvent::Keyboard {
            pressed: true,
            key: "v".into(),
            code: "KeyV".into(),
            text: None,
            repeat: false,
            modifiers: nana_ui_platform::InputModifiers {
                control: true,
                ..Default::default()
            },
        };
        nana_ui::RuntimeInputAdapter::default()
            .with_clipboard(clipboard)
            .dispatch(document.context_mut(), doc, &event)
            .unwrap();
        let expected = "a".repeat(119);
        assert_eq!(
            document.context().world().text(title),
            Some(expected.as_str())
        );
        assert!(
            matches!(intents.lock().unwrap().last(), Some(ShellIntent::MemoryTitleChanged(value)) if value == &expected)
        );
    }

    #[test]
    fn memory_page_card_state_and_refresh_use_normal_bound_controls() {
        let mut snapshot = empty_snapshot();
        snapshot.project_page = Some(ShellProjectPage::Memory);
        snapshot.memory_global_enabled = false;
        snapshot.memory_selected = Some("one".into());
        snapshot.memory_cards = vec![ShellMemoryCard {
            id: "one".into(),
            title: "Memory".into(),
            subtitle: String::new(),
            scope_label: "项目".into(),
            enabled: false,
            selected: true,
            body: "Two lines of text".into(),
            updated_at: 1,
        }];
        let intents = Arc::new(Mutex::new(Vec::new()));
        let output = intents.clone();
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| output.lock().unwrap().push(intent)),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let cx = document.context_mut();
        let row = handles.project_cards["memory-one"];
        assert_eq!(
            cx.world()
                .node_style(row.stable_id())
                .unwrap()
                .layout
                .opacity,
            Some(0.62)
        );
        assert_eq!(
            cx.world().node_style(row.stable_id()).unwrap().border,
            Some(SemanticColorRole::Accent)
        );
        assert!(cx
            .read(handles.memory_checkboxes["memory-baseline"], |view| view
                .disabled)
            .unwrap());
        assert!(cx
            .activate_icon_button(handles.memory_icons["memory-refresh"])
            .unwrap());
        assert!(cx
            .activate_icon_button(handles.memory_icons["memory-one-toggle"])
            .unwrap());
        let emitted = intents.lock().unwrap();
        assert!(emitted
            .iter()
            .any(|intent| matches!(intent, ShellIntent::MemoryCommand(MemoryMessage::Refresh))));
        assert!(emitted.iter().any(|intent|matches!(intent,ShellIntent::MemoryCommand(MemoryMessage::ToggleEntry(id)) if id=="one")));
    }
}
