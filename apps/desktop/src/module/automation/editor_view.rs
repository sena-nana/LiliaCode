use super::editor::{field_choices, field_label, NodeEditorAction, NodeEditorSnapshot};
use super::view::{AutomationAction, AutomationTarget};
use crate::runtime_layout::reconcile_children;
use crate::runtime_shell::{emit, IntentSink, ShellIntent};
use nana_ui::runtime::view::{entity_ref, widget, with_refs};
#[cfg(test)]
use nana_ui::runtime::Activate;
use nana_ui::runtime::{
    AppContext, Button, DocumentId, Entity, FormField, FrameworkError, ScrollAxes, ScrollView,
    SearchDropdown, SearchDropdownEvent, SearchDropdownOption, StableNodeId, Stack, Switch,
    TextArea, TextChanged, TextInput, ToggleChanged,
};
use nana_ui::ButtonKind;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Binding = Arc<Mutex<Option<(AutomationTarget, String)>>>;

fn dispatch(sink: &IntentSink, binding: &Binding, action: NodeEditorAction) {
    if let Some((target, node_id)) = binding.lock().unwrap().clone() {
        emit(
            sink,
            ShellIntent::Automation {
                target,
                action: AutomationAction::Node { node_id, action },
            },
        );
    }
}

enum Input {
    Single(Entity<TextInput>),
    Multi(Entity<TextArea>),
    Choice(Entity<SearchDropdown>),
    Toggle(Entity<Switch>),
}
impl Input {
    fn id(&self) -> StableNodeId {
        match self {
            Self::Single(v) => v.stable_id(),
            Self::Multi(v) => v.stable_id(),
            Self::Choice(v) => v.stable_id(),
            Self::Toggle(v) => v.stable_id(),
        }
    }
}
struct Field {
    row: Entity<FormField>,
    input: Input,
}

#[cfg(test)]
mod tests {
    use super::super::editor::NodeEditorField;
    use super::*;
    use crate::runtime_compat::HostedWindowId;

    #[test]
    fn edits_are_addressed_and_switching_node_replaces_fields() {
        let mut context = AppContext::new();
        let document = DocumentId::new(336).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut view = NodeEditorView::mount(
            &mut context,
            document,
            Arc::new(move |event| received.lock().unwrap().push(event)),
        )
        .unwrap();
        let target = AutomationTarget {
            window_id: HostedWindowId::PRIMARY,
            workflow_id: "workflow".into(),
            modified_at: 2,
        };
        let first = NodeEditorSnapshot {
            node_id: "first".into(),
            title: "Agent".into(),
            fields: vec![NodeEditorField {
                key: "permission".into(),
                value: Value::String("ask".into()),
            }],
        };
        view.sync(&mut context, document, Some(&target), Some(&first))
            .unwrap();
        let Input::Choice(choice) = view.inputs["permission"].input else {
            panic!("permission selector missing")
        };
        context
            .update_component(choice, |_, cx| {
                cx.emit(SearchDropdownEvent::Select("readonly".into()))
            })
            .unwrap();
        let old = view.inputs["permission"].row;
        let second = NodeEditorSnapshot {
            node_id: "second".into(),
            title: "Confirm".into(),
            fields: vec![NodeEditorField {
                key: "prompt".into(),
                value: Value::String("Continue?".into()),
            }],
        };
        view.sync(&mut context, document, Some(&target), Some(&second))
            .unwrap();
        assert!(context.world().node(old.stable_id()).is_none());
        context
            .update_component(view.save, |_, cx| cx.emit(Activate))
            .unwrap();
        view.sync(&mut context, document, None, None).unwrap();
        context
            .update_component(view.save, |_, cx| cx.emit(Activate))
            .unwrap();
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], ShellIntent::Automation { target, action: AutomationAction::Node { node_id, action: NodeEditorAction::Field { key, value } } } if target.workflow_id == "workflow" && node_id == "first" && key == "permission" && value == "readonly")
        );
        assert!(
            matches!(&events[1], ShellIntent::Automation { action: AutomationAction::Node { node_id, action: NodeEditorAction::Save }, .. } if node_id == "second")
        );
    }
}

pub(crate) struct NodeEditorView {
    pub(crate) root: Entity<Stack>,
    fields: Entity<Stack>,
    title: Entity<TextInput>,
    save: Entity<Button>,
    inputs: HashMap<String, Field>,
    binding: Binding,
    sink: IntentSink,
}
impl NodeEditorView {
    pub(crate) fn debug_nodes(&self) -> Vec<(String, StableNodeId)> {
        self.inputs
            .iter()
            .map(|(key, field)| (format!("auto-node-{key}"), field.input.id()))
            .collect()
    }

    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        let binding = Arc::new(Mutex::new(None));
        let save_target = Arc::clone(&binding);
        let save_sink = Arc::clone(&sink);
        let close_target = Arc::clone(&binding);
        let close_sink = Arc::clone(&sink);
        let title_target = Arc::clone(&binding);
        let title_sink = Arc::clone(&sink);
        let (_, (root, fields, title, save, _close, _title_field)) =
            context.mount_view_detached(document, move || {
                let root = entity_ref::<Stack>();
                let fields = entity_ref::<Stack>();
                let title = entity_ref::<TextInput>();
                let save = entity_ref::<Button>();
                let close = entity_ref::<Button>();
                let title_field = entity_ref::<FormField>();
                let title_control =
                    widget(TextInput::new(""))
                        .entity_ref(title)
                        .on(move |event: &TextChanged| {
                            dispatch(
                                &title_sink,
                                &title_target,
                                NodeEditorAction::Title(event.value.to_string()),
                            )
                        });
                with_refs(
                    widget(Stack::fill_column(8.0)).entity_ref(root).children((
                        widget(Stack::bar(8.0).wrap(true)).children((
                            widget(Button::new("保存节点").kind(ButtonKind::Primary))
                                .entity_ref(save)
                                .on_activate(move || {
                                    dispatch(&save_sink, &save_target, NodeEditorAction::Save)
                                }),
                            widget(Button::new("返回画布").kind(ButtonKind::Subtle))
                                .entity_ref(close)
                                .on_activate(move || {
                                    dispatch(&close_sink, &close_target, NodeEditorAction::Close)
                                }),
                        )),
                        widget(FormField::new("节点名称"))
                            .entity_ref(title_field)
                            .child_slot(title_control, |field, id| field.control_child(id)),
                        widget(ScrollView::new(ScrollAxes::Vertical))
                            .children(widget(Stack::column(12.0)).entity_ref(fields)),
                    )),
                    (root, fields, title, save, close, title_field),
                )
            })?;
        Ok(Self {
            root,
            fields,
            title,
            save,
            inputs: HashMap::new(),
            binding,
            sink,
        })
    }

    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        target: Option<&AutomationTarget>,
        snapshot: Option<&NodeEditorSnapshot>,
    ) -> Result<(), FrameworkError> {
        *self.binding.lock().unwrap() = target
            .zip(snapshot)
            .map(|(target, editor)| (target.clone(), editor.node_id.clone()));
        let Some(snapshot) = snapshot.filter(|_| target.is_some()) else {
            for field in self.inputs.values() {
                if let Input::Choice(input) = field.input {
                    context.update_component(input, |input, _| input.close())?;
                }
            }
            return Ok(());
        };
        context.update_component(self.title, |input, _| {
            if input.state.value != snapshot.title {
                input.state.replace_value(snapshot.title.clone());
            }
        })?;
        context.update_component(self.save, |button, _| {
            button.disabled = snapshot.title.trim().is_empty()
        })?;
        let mut order = Vec::new();
        for field in &snapshot.fields {
            if !self.inputs.contains_key(&field.key) {
                let key = field.key.clone();
                let binding = Arc::clone(&self.binding);
                let sink = Arc::clone(&self.sink);
                let choices = field_choices(&key);
                let input = if key == "createTask" {
                    let label = field_label(&key);
                    let (_, input) = context.mount_view_detached(document, move || {
                        let input = entity_ref::<Switch>();
                        with_refs(widget(Switch::new(label, false)).entity_ref(input), input)
                    })?;
                    context.on(input, move |_, event: &ToggleChanged, _| {
                        dispatch(
                            &sink,
                            &binding,
                            NodeEditorAction::Field {
                                key: key.clone(),
                                value: Value::Bool(event.checked),
                            },
                        )
                    })?;
                    Input::Toggle(input)
                } else if !choices.is_empty() {
                    let (_, input) = context.mount_view_detached(document, || {
                        let input = entity_ref::<SearchDropdown>();
                        with_refs(
                            widget(SearchDropdown::new(None::<String>).placeholder("请选择"))
                                .entity_ref(input),
                            input,
                        )
                    })?;
                    context.on(input, move |_, event: &SearchDropdownEvent, _| {
                        if let SearchDropdownEvent::Select(value) = event {
                            dispatch(
                                &sink,
                                &binding,
                                NodeEditorAction::Field {
                                    key: key.clone(),
                                    value: Value::String(value.to_string()),
                                },
                            );
                        }
                    })?;
                    Input::Choice(input)
                } else if matches!(key.as_str(), "prompt" | "text" | "summary" | "cases") {
                    let placeholder = if key == "cases" {
                        "每行一个匹配值"
                    } else {
                        ""
                    };
                    let (_, input) = context.mount_view_detached(document, move || {
                        let input = entity_ref::<TextArea>();
                        with_refs(
                            widget(TextArea::new("").height(120.0).placeholder(placeholder))
                                .entity_ref(input),
                            input,
                        )
                    })?;
                    context.on(input, move |_, event: &TextChanged, _| {
                        let value = Value::String(event.value.to_string());
                        dispatch(
                            &sink,
                            &binding,
                            NodeEditorAction::Field {
                                key: key.clone(),
                                value,
                            },
                        );
                    })?;
                    Input::Multi(input)
                } else {
                    let (_, input) = context.mount_view_detached(document, || {
                        let input = entity_ref::<TextInput>();
                        with_refs(widget(TextInput::new("")).entity_ref(input), input)
                    })?;
                    context.on(input, move |_, event: &TextChanged, _| {
                        dispatch(
                            &sink,
                            &binding,
                            NodeEditorAction::Field {
                                key: key.clone(),
                                value: Value::String(event.value.to_string()),
                            },
                        )
                    })?;
                    Input::Single(input)
                };
                let row_label = if field.key == "createTask" {
                    "任务目标".to_owned()
                } else {
                    field_label(&field.key).to_owned()
                };
                let input_id = input.id();
                let (_, row) = context.mount_view_detached(document, move || {
                    let row = entity_ref::<FormField>();
                    with_refs(
                        widget(FormField::new(row_label).control_child(input_id)).entity_ref(row),
                        row,
                    )
                })?;
                reconcile_children(context, row.stable_id(), &[input.id()])?;
                self.inputs.insert(field.key.clone(), Field { row, input });
            }
            let control = &self.inputs[&field.key];
            let text = match &field.value {
                Value::Null => String::new(),
                Value::String(value) => value.clone(),
                Value::Array(values) if field.key == "cases" => values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n"),
                value => value.to_string(),
            };
            match control.input {
                Input::Single(input) => context.update_component(input, |input, _| {
                    if input.state.value != text {
                        input.state.replace_value(text);
                    }
                })?,
                Input::Multi(input) => context.update_component(input, |input, _| {
                    if input.state.value != text {
                        input.state.replace_value(text);
                    }
                })?,
                Input::Toggle(input) => context.update_component(input, |input, _| {
                    input.checked = field.value.as_bool().unwrap_or(false)
                })?,
                Input::Choice(input) => context.update_component(input, |input, _| {
                    input.options = field_choices(&field.key)
                        .iter()
                        .map(|(key, label)| SearchDropdownOption::new(*key, *label))
                        .collect();
                    if !text.is_empty()
                        && !field_choices(&field.key)
                            .iter()
                            .any(|(key, _)| *key == text)
                    {
                        input
                            .options
                            .push(SearchDropdownOption::new(text.clone(), text.clone()));
                    }
                    input.value = (!text.is_empty()).then(|| Arc::from(text));
                })?,
            }
            order.push(control.row.stable_id());
        }
        let stale: Vec<_> = self
            .inputs
            .keys()
            .filter(|key| !snapshot.fields.iter().any(|field| &field.key == *key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(field) = self.inputs.remove(&key) {
                context.remove_view(field.row)?;
            }
        }
        reconcile_children(context, self.fields.stable_id(), &order)
    }
}
