use crate::runtime_layout::{pending_actions_row, pending_interaction_card, pill_button};
use crate::runtime_shell::{
    bind_activate, emit, pending_action_specs, ShellIntent, ShellPending, ShellPendingKind,
};
use nana_ui::runtime::{
    AppContext, Button, Card, DocumentId, Entity, FormField, FrameworkError, LengthSpec,
    ScrollAxes, ScrollView, StableNodeId, Stack, Text, TextArea, TextChanged,
};
use nana_ui::ButtonKind;
use nana_ui_platform::WindowId;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
type Sink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

fn pending_textarea(value: String, height: f32) -> TextArea {
    let mut editor = TextArea::new(value).height(height);
    let layout = Arc::make_mut(&mut editor.style.layout);
    layout.min_height = Some(LengthSpec::Px(height));
    layout.max_height = Some(LengthSpec::Px(92.0));
    editor
}

pub struct PendingPanel {
    pub pending_panel: Entity<Card>,
    pending_scroll: Entity<ScrollView>,
    pending_body: Entity<Stack>,
    pending_actions: Entity<Stack>,
    pending_title: Entity<Text>,
    pending_prompt: Entity<Text>,
    pending_draft: Entity<TextArea>,
    pending_tool_command: Entity<TextArea>,
    pending_tool_message: Entity<TextArea>,
    pending_request: Arc<Mutex<String>>,
    pending_is_question: Arc<AtomicBool>,
    pending_tool_command_value: Arc<Mutex<String>>,
    pending_tool_message_value: Arc<Mutex<String>>,
    extra_buttons: HashMap<String, Entity<Button>>,
    form_fields: HashMap<String, Entity<TextArea>>,
    form_wrappers: HashMap<String, Entity<FormField>>,
    sink: Sink,
}

impl PendingPanel {
    pub fn mount(
        context: &mut AppContext,
        document_id: DocumentId,
        window_id: WindowId,
        sink: Sink,
    ) -> Result<Self, FrameworkError> {
        let sink: Sink = if window_id == WindowId::PRIMARY {
            sink
        } else {
            Arc::new(move |intent| {
                if matches!(intent, ShellIntent::OpenMarkdownLink(_)) {
                    emit(&sink, intent);
                } else {
                    emit(
                        &sink,
                        ShellIntent::TaskPopupPending {
                            window_id,
                            intent: Box::new(intent),
                        },
                    );
                }
            })
        };
        let pending_panel =
            context.create_detached_component(document_id, pending_interaction_card())?;
        let pending_body = context.create_detached_component(document_id, Stack::column(8.0))?;
        let pending_scroll = context.create_detached_component(
            document_id,
            ScrollView::new(ScrollAxes::Vertical).style(
                Stack::column(0.0)
                    .with_layout(|layout| layout.max_height = Some(LengthSpec::Px(260.0)))
                    .shrink(1.0)
                    .node_style(),
            ),
        )?;
        context.append_child(pending_scroll, pending_body)?;
        let pending_title =
            context.create_detached_component(document_id, Text::new(String::new()))?;
        let pending_prompt =
            context.create_detached_component(document_id, Text::new(String::new()))?;
        let pending_draft = context
            .create_detached_component(document_id, pending_textarea(String::new(), 74.0))?;
        let pending_request = Arc::new(Mutex::new(String::new()));
        let pending_is_question = Arc::new(AtomicBool::new(false));
        let pending_tool_command_value = Arc::new(Mutex::new(String::new()));
        let pending_tool_message_value = Arc::new(Mutex::new(String::new()));
        context.on(pending_draft, {
            let sink = Arc::clone(&sink);
            let pending_request = Arc::clone(&pending_request);
            let pending_is_question = Arc::clone(&pending_is_question);
            move |_, event: &TextChanged, _| {
                let request_id = pending_request
                    .lock()
                    .map(|guard| guard.clone())
                    .unwrap_or_default();
                emit(
                    &sink,
                    if pending_is_question.load(Ordering::Relaxed) {
                        ShellIntent::AskUserPending {
                            request_id,
                            action: "freeform".into(),
                            value: event.value.clone(),
                        }
                    } else {
                        ShellIntent::PendingDraftChanged {
                            request_id,
                            value: event.value.clone(),
                        }
                    },
                );
            }
        })?;
        let pending_tool_command = context
            .create_detached_component(document_id, pending_textarea(String::new(), 74.0))?;
        context.on(pending_tool_command, {
            let sink = Arc::clone(&sink);
            let pending_request = Arc::clone(&pending_request);
            let pending_tool_command_value = Arc::clone(&pending_tool_command_value);
            let pending_tool_message_value = Arc::clone(&pending_tool_message_value);
            move |_, event: &TextChanged, _| {
                let request_id = pending_request
                    .lock()
                    .map(|guard| guard.clone())
                    .unwrap_or_default();
                if let Ok(mut guard) = pending_tool_command_value.lock() {
                    *guard = event.value.clone();
                }
                let message = pending_tool_message_value
                    .lock()
                    .map(|guard| guard.clone())
                    .unwrap_or_default();
                emit(
                    &sink,
                    ShellIntent::ToolConsentDraftChanged {
                        request_id,
                        command: event.value.clone(),
                        message,
                    },
                );
            }
        })?;
        let pending_tool_message = context
            .create_detached_component(document_id, pending_textarea(String::new(), 74.0))?;
        context.on(pending_tool_message, {
            let sink = Arc::clone(&sink);
            let pending_request = Arc::clone(&pending_request);
            let pending_tool_command_value = Arc::clone(&pending_tool_command_value);
            let pending_tool_message_value = Arc::clone(&pending_tool_message_value);
            move |_, event: &TextChanged, _| {
                let request_id = pending_request
                    .lock()
                    .map(|guard| guard.clone())
                    .unwrap_or_default();
                if let Ok(mut guard) = pending_tool_message_value.lock() {
                    *guard = event.value.clone();
                }
                let command = pending_tool_command_value
                    .lock()
                    .map(|guard| guard.clone())
                    .unwrap_or_default();
                emit(
                    &sink,
                    ShellIntent::ToolConsentDraftChanged {
                        request_id,
                        command,
                        message: event.value.clone(),
                    },
                );
            }
        })?;
        let pending_actions =
            context.create_detached_component(document_id, pending_actions_row())?;
        Ok(Self {
            pending_panel,
            pending_scroll,
            pending_body,
            pending_actions,
            pending_title,
            pending_prompt,
            pending_draft,
            pending_tool_command,
            pending_tool_message,
            pending_request,
            pending_is_question,
            pending_tool_command_value,
            pending_tool_message_value,
            extra_buttons: HashMap::new(),
            form_fields: HashMap::new(),
            form_wrappers: HashMap::new(),
            sink,
        })
    }

    #[cfg(test)]
    pub(crate) fn actions_node(&self) -> StableNodeId {
        self.pending_actions.stable_id()
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_targets(&self) -> Vec<(String, StableNodeId)> {
        let mut nodes = vec![
            ("scroll".into(), self.pending_scroll.stable_id()),
            ("draft".into(), self.pending_draft.stable_id()),
            ("tool-command".into(), self.pending_tool_command.stable_id()),
            ("tool-message".into(), self.pending_tool_message.stable_id()),
        ];
        nodes.extend(
            self.extra_buttons
                .iter()
                .map(|(id, node)| (id.clone(), node.stable_id())),
        );
        nodes.extend(
            self.form_fields
                .iter()
                .map(|(id, node)| (id.clone(), node.stable_id())),
        );
        nodes
    }
    pub fn sync(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        pending: Option<&ShellPending>,
    ) -> Result<(), FrameworkError> {
        self.pending_is_question.store(
            pending.is_some_and(|pending| pending.kind == ShellPendingKind::AskUser),
            Ordering::Relaxed,
        );
        let Some(pending) = pending else {
            if let Ok(mut guard) = self.pending_request.lock() {
                guard.clear();
            }
            context.reconcile_children(self.pending_actions.stable_id(), &[])?;
            context.reconcile_children(self.pending_body.stable_id(), &[])?;
            context.reconcile_children(self.pending_panel.stable_id(), &[])?;
            return Ok(());
        };
        if let Ok(mut guard) = self.pending_request.lock() {
            *guard = pending.request_id.clone();
        }
        context.update_component(self.pending_title, |text, _| {
            *text = Text::new(pending.title.clone());
        })?;
        context.update_component(self.pending_prompt, |text, _| {
            *text = Text::new(pending.prompt.clone());
        })?;
        context.update_component(self.pending_draft, |editor, _| {
            editor.placeholder = Arc::from("补充说明");
            let value = pending
                .ask
                .as_ref()
                .map(|ask| ask.freeform.clone())
                .unwrap_or_else(|| pending.draft.clone());
            if editor.state.value != value {
                editor.state.replace_value(value);
            }
        })?;
        let mut keep = HashSet::new();
        let mut field_keep = HashSet::new();
        let mut order = vec![
            self.pending_title.stable_id(),
            self.pending_prompt.stable_id(),
        ];
        let request_id = pending.request_id.clone();
        match pending.kind {
            ShellPendingKind::PlanApproval => {
                order.push(self.pending_draft.stable_id());
            }
            ShellPendingKind::ToolConsent => {
                if let Some(tool) = &pending.tool {
                    if let Ok(mut guard) = self.pending_tool_command_value.lock() {
                        *guard = tool.command.clone();
                    }
                    if let Ok(mut guard) = self.pending_tool_message_value.lock() {
                        *guard = tool.message.clone();
                    }
                    if tool.command_editable {
                        context.update_component(self.pending_tool_command, |editor, _| {
                            editor.placeholder = Arc::from("确认执行的命令");
                            if editor.state.value != tool.command {
                                editor.state.replace_value(tool.command.clone());
                            }
                        })?;
                        order.push(self.pending_tool_command.stable_id());
                    }
                    context.update_component(self.pending_tool_message, |editor, _| {
                        editor.placeholder = Arc::from("拒绝理由");
                        if editor.state.value != tool.message {
                            editor.state.replace_value(tool.message.clone());
                        }
                    })?;
                    order.push(self.pending_tool_message.stable_id());
                }
            }
            ShellPendingKind::AskUser => {
                if pending.ask.as_ref().is_some_and(|ask| ask.show_freeform) {
                    order.push(self.pending_draft.stable_id());
                }
            }
            ShellPendingKind::McpElicitation => {
                if let Some(mcp) = &pending.mcp {
                    if let Some(url) = &mcp.url {
                        let open = self.upsert_tagged_button(
                            context,
                            document_id,
                            &format!("pending-mcp-url-{request_id}"),
                            "打开链接",
                            ButtonKind::Subtle,
                            ShellIntent::OpenMarkdownLink(url.clone()),
                            false,
                        )?;
                        keep.insert(format!("pending-mcp-url-{request_id}"));
                        order.push(open.stable_id());
                    }
                    if let Some(raw) = &mcp.raw_json {
                        self.upsert_field(
                            context,
                            document_id,
                            &mut field_keep,
                            &mut order,
                            &format!("pending-mcp-raw-{request_id}"),
                            "内容",
                            raw,
                            {
                                let request_id = request_id.clone();
                                move |value| ShellIntent::McpRawJsonChanged {
                                    request_id: request_id.clone(),
                                    value,
                                }
                            },
                        )?;
                    }
                    for field in &mcp.fields {
                        if field.options.is_empty() && field.kind != "boolean" {
                            self.upsert_field(
                                context,
                                document_id,
                                &mut field_keep,
                                &mut order,
                                &format!("pending-mcp-field-{request_id}-{}", field.key),
                                &field.label,
                                &field.value,
                                {
                                    let request_id = request_id.clone();
                                    let field_key = field.key.clone();
                                    move |value| ShellIntent::McpFieldChanged {
                                        request_id: request_id.clone(),
                                        field_key: field_key.clone(),
                                        value,
                                    }
                                },
                            )?;
                        } else if field.kind == "boolean" {
                            let id = format!("pending-mcp-bool-{request_id}-{}", field.key);
                            let button = self.upsert_tagged_button(
                                context,
                                document_id,
                                &id,
                                if field.enabled {
                                    "已开启"
                                } else {
                                    "已关闭"
                                },
                                if field.enabled {
                                    ButtonKind::Primary
                                } else {
                                    ButtonKind::Subtle
                                },
                                ShellIntent::McpToggleBoolean {
                                    request_id: request_id.clone(),
                                    field_key: field.key.clone(),
                                },
                                false,
                            )?;
                            keep.insert(id);
                            order.push(button.stable_id());
                        } else {
                            for option in &field.options {
                                let id = format!(
                                    "pending-mcp-opt-{request_id}-{}-{}",
                                    field.key, option.value
                                );
                                let button = self.upsert_tagged_button(
                                    context,
                                    document_id,
                                    &id,
                                    &option.label,
                                    if option.selected {
                                        ButtonKind::Primary
                                    } else {
                                        ButtonKind::Subtle
                                    },
                                    ShellIntent::McpToggleOption {
                                        request_id: request_id.clone(),
                                        field_key: field.key.clone(),
                                        value: option.value.clone(),
                                        multi: option.multi,
                                    },
                                    false,
                                )?;
                                keep.insert(id);
                                order.push(button.stable_id());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        for option in &pending.options {
            let id = format!("pending-opt-{}-{}", request_id, option.id);
            let button = self.upsert_tagged_button(
                context,
                document_id,
                &id,
                &option.label,
                if option.selected {
                    ButtonKind::Primary
                } else if option.danger {
                    ButtonKind::Danger
                } else {
                    ButtonKind::Subtle
                },
                ShellIntent::SelectPendingOption {
                    request_id: request_id.clone(),
                    option_id: option.id.clone(),
                },
                false,
            )?;
            keep.insert(id);
            order.push(button.stable_id());
        }
        if let Some(ask) = &pending.ask {
            if ask.show_other {
                let id = format!("pending-ask-other-{request_id}");
                let button = self.upsert_tagged_button(
                    context,
                    document_id,
                    &id,
                    if ask.other_selected {
                        "✓ 其他"
                    } else {
                        "其他"
                    },
                    if ask.other_selected {
                        ButtonKind::Primary
                    } else {
                        ButtonKind::Subtle
                    },
                    ShellIntent::AskUserPending {
                        request_id: request_id.clone(),
                        action: "select".to_owned(),
                        value: "other".to_owned(),
                    },
                    false,
                )?;
                keep.insert(id);
                order.push(button.stable_id());
            }
        }
        let actions = pending_action_specs(pending);
        let mut action_order = Vec::new();
        for (id, label, kind, intent, disabled) in actions {
            keep.insert(id.clone());
            let button = self.upsert_tagged_button(
                context,
                document_id,
                &id,
                &label,
                kind,
                intent,
                disabled,
            )?;
            action_order.push(button.stable_id());
        }
        context.reconcile_children(self.pending_actions.stable_id(), &action_order)?;
        let stale: Vec<_> = self
            .extra_buttons
            .keys()
            .filter(|key| key.starts_with("pending-") && !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(button) = self.extra_buttons.remove(&key) {
                let _ = context.remove_view(button);
            }
        }
        let stale_fields: Vec<_> = self
            .form_fields
            .keys()
            .filter(|key| key.starts_with("pending-") && !field_keep.contains(*key))
            .cloned()
            .collect();
        for key in stale_fields {
            if let Some(field) = self.form_fields.remove(&key) {
                let _ = context.remove_view(field);
            }
            if let Some(wrapper) = self.form_wrappers.remove(&key) {
                let _ = context.remove_view(wrapper);
            }
        }
        context.reconcile_children(self.pending_body.stable_id(), &order)?;
        let mut panel_order = vec![self.pending_scroll.stable_id()];
        if !action_order.is_empty() {
            panel_order.push(self.pending_actions.stable_id());
        }
        context
            .reconcile_children(self.pending_panel.stable_id(), &panel_order)
            .map(|_| ())
    }

    fn upsert_tagged_button(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        id: &str,
        label: &str,
        kind: ButtonKind,
        intent: ShellIntent,
        disabled: bool,
    ) -> Result<Entity<Button>, FrameworkError> {
        if let Some(button) = self.extra_buttons.get(id).copied() {
            context.update_component(button, |button, _| {
                *button = pill_button(label, kind);
                button.disabled = disabled;
            })?;
            Ok(button)
        } else {
            let mut view = pill_button(label, kind);
            view.disabled = disabled;
            let button = context.create_detached_component(document_id, view)?;
            bind_activate(context, button, Arc::clone(&self.sink), intent)?;
            self.extra_buttons.insert(id.to_owned(), button);
            Ok(button)
        }
    }

    fn upsert_field(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        keep: &mut HashSet<String>,
        order: &mut Vec<StableNodeId>,
        id: &str,
        label: &str,
        value: &str,
        intent: impl Fn(String) -> ShellIntent + Send + Sync + 'static,
    ) -> Result<(), FrameworkError> {
        keep.insert(id.to_owned());
        let editor = if let Some(field) = self.form_fields.get(id).copied() {
            context.update_component(field, |editor, _| {
                if editor.state.value != value {
                    editor.state.replace_value(value.to_owned());
                }
            })?;
            field
        } else {
            let field = context
                .create_detached_component(document_id, pending_textarea(value.to_owned(), 74.0))?;
            let sink = Arc::clone(&self.sink);
            context.on(field, move |_, event: &TextChanged, _| {
                emit(&sink, intent(event.value.clone()));
            })?;
            self.form_fields.insert(id.to_owned(), field);
            field
        };
        let wrapper = if let Some(wrapper) = self.form_wrappers.get(id).copied() {
            context.update_component(wrapper, |field, _| {
                *field = FormField::new(label).control_child(editor.stable_id());
            })?;
            wrapper
        } else {
            let wrapper = context.create_detached_component(
                document_id,
                FormField::new(label).control_child(editor.stable_id()),
            )?;
            context.set_form_field_control(wrapper, Some(editor.stable_id()))?;
            self.form_wrappers.insert(id.to_owned(), wrapper);
            wrapper
        };
        order.push(wrapper.stable_id());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_shell::{ShellAskUserPending, ShellToolConsentPending};

    fn request(kind: ShellPendingKind) -> ShellPending {
        ShellPending {
            request_id: "request-1".into(),
            kind,
            title: "请确认".into(),
            prompt: "选择下一步".into(),
            draft: "旧草稿".into(),
            options: Vec::new(),
            tool: None,
            ask: None,
            mcp: None,
        }
    }

    #[test]
    fn long_pending_content_scrolls_without_hiding_approval_actions() {
        let document_id = DocumentId::new(905).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, Stack::column(0.0))
            .unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let events = observed.clone();
        let mut panel = PendingPanel::mount(
            context,
            document_id,
            WindowId::PRIMARY,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(root, panel.pending_panel).unwrap();
        let mut pending = request(ShellPendingKind::PlanApproval);
        pending.prompt = "请核对这一行计划。\n".repeat(100);
        panel.sync(context, document_id, Some(&pending)).unwrap();
        let viewport = nana_ui::runtime::LayoutViewport::new(330.0, 430.0);
        context.layout_document(document_id, viewport).unwrap();
        let mut shaper = nana_ui::NanaTextShaper::default();
        context
            .shape_text(&[panel.pending_prompt.stable_id()], &mut shaper)
            .unwrap();
        context.layout_document(document_id, viewport).unwrap();
        context.rebuild_hit_test(document_id);
        let scroll = context
            .world()
            .layout_box(panel.pending_scroll.stable_id())
            .unwrap();
        let content = context
            .world()
            .layout_box(panel.pending_body.stable_id())
            .unwrap();
        let actions = context
            .world()
            .layout_box(panel.pending_actions.stable_id())
            .unwrap();
        assert!(content.height > scroll.height * 2.0);
        assert!(scroll.height <= 260.5);
        assert!(actions.y >= scroll.y + scroll.height);
        assert!(actions.y + actions.height <= 430.0);
        let approve = panel.extra_buttons["pending-plan-approve-request-1"];
        let bounds = context.world().layout_box(approve.stable_id()).unwrap();
        let (x, y) = (
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        assert_eq!(
            context.world().hit_test(document_id, x, y),
            Some(approve.stable_id())
        );
        assert!(context.activate_node(approve.stable_id()).unwrap());
        assert!(
            matches!(observed.lock().unwrap().last(), Some(ShellIntent::RespondPlan { request_id, action }) if request_id == "request-1" && action == "approve")
        );
    }

    #[test]
    fn shared_question_panel_preserves_freeform_and_routes_window_actions() {
        for window_id in [WindowId::PRIMARY, WindowId(42)] {
            let document_id = DocumentId::new(903).unwrap();
            let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
            let context = document.context_mut();
            let root = context
                .create_component(document_id, Stack::column(4.0))
                .unwrap();
            let observed = Arc::new(Mutex::new(Vec::new()));
            let events = observed.clone();
            let mut panel = PendingPanel::mount(
                context,
                document_id,
                window_id,
                Arc::new(move |event| events.lock().unwrap().push(event)),
            )
            .unwrap();
            context.append_child(root, panel.pending_panel).unwrap();
            let mut pending = request(ShellPendingKind::AskUser);
            pending.ask = Some(ShellAskUserPending {
                show_other: true,
                other_selected: true,
                freeform: "回答草稿".into(),
                show_freeform: true,
                show_skip: false,
                show_back: false,
                show_cancel: true,
                show_reject: false,
                can_submit: true,
                submit_label: "提交".into(),
                reject_label: "拒绝".into(),
            });
            panel.sync(context, document_id, Some(&pending)).unwrap();
            assert_eq!(
                context
                    .read(panel.pending_draft, |editor| editor.state.value.clone())
                    .unwrap(),
                "回答草稿"
            );
            assert!(context
                .focus_node(document_id, panel.pending_draft.stable_id())
                .unwrap());
            context.select_all_focused_text(document_id).unwrap();
            assert!(context
                .replace_focused_text(document_id, "补充回答")
                .unwrap());
            assert!(context
                .activate_node(panel.extra_buttons["pending-ask-other-request-1"].stable_id())
                .unwrap());
            let events = observed.lock().unwrap();
            let unwrap = |event: &ShellIntent| match event {
                ShellIntent::TaskPopupPending {
                    window_id: target,
                    intent,
                } => {
                    assert_eq!(*target, window_id);
                    assert_ne!(window_id, WindowId::PRIMARY);
                    intent.as_ref().clone()
                }
                _ => {
                    assert_eq!(window_id, WindowId::PRIMARY);
                    event.clone()
                }
            };
            assert!(events.iter().any(|event| matches!(unwrap(event), ShellIntent::AskUserPending { request_id, action, value } if request_id == "request-1" && action == "freeform" && value == "补充回答")));
            assert!(events.iter().any(|event| matches!(unwrap(event), ShellIntent::AskUserPending { request_id, action, value } if request_id == "request-1" && action == "select" && value == "other")));
        }
    }

    #[test]
    fn tool_consent_edits_keep_the_other_field_and_current_request() {
        let document_id = DocumentId::new(904).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, Stack::column(4.0))
            .unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let events = observed.clone();
        let mut panel = PendingPanel::mount(
            context,
            document_id,
            WindowId::PRIMARY,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(root, panel.pending_panel).unwrap();
        let mut pending = request(ShellPendingKind::ToolConsent);
        pending.tool = Some(ShellToolConsentPending {
            command: "ls".into(),
            message: "保留说明".into(),
            command_editable: true,
            can_allow: true,
            can_deny: true,
        });
        panel.sync(context, document_id, Some(&pending)).unwrap();
        pending.request_id = "request-2".into();
        panel.sync(context, document_id, Some(&pending)).unwrap();
        assert!(context
            .focus_node(document_id, panel.pending_tool_command.stable_id())
            .unwrap());
        context.select_all_focused_text(document_id).unwrap();
        assert!(context.replace_focused_text(document_id, "pwd").unwrap());
        assert!(observed.lock().unwrap().iter().any(|event| matches!(event,
            ShellIntent::ToolConsentDraftChanged { request_id, command, message }
            if request_id == "request-2" && command == "pwd" && message == "保留说明")));
    }
}
