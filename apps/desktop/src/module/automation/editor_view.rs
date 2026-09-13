use super::editor::{NodeEditorAction, NodeEditorSnapshot, field_choices, field_label};
use super::view::{AutomationAction, AutomationTarget};
use crate::runtime_layout::reconcile_children;
use crate::runtime_shell::{IntentSink, ShellIntent, emit};
use nana_ui::ButtonKind;
use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, Entity, FormField, FrameworkError, ScrollAxes,
    ScrollView, SearchDropdown, SearchDropdownEvent, SearchDropdownOption, StableNodeId, Stack,
    Switch, TextArea, TextChanged, TextInput, ToggleChanged,
};
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
        let root = context.create_detached_component(document, Stack::fill_column(8.0))?;
        let actions = context.create_detached_component(document, Stack::bar(8.0).wrap(true))?;
        let save = context.create_detached_component(
            document,
            Button::new("保存节点").kind(ButtonKind::Primary),
        )?;
        let close = context.create_detached_component(
            document,
            Button::new("返回画布").kind(ButtonKind::Subtle),
        )?;
        let binding = Arc::new(Mutex::new(None));
        for (button, action) in [
            (save, NodeEditorAction::Save),
            (close, NodeEditorAction::Close),
        ] {
            let target = Arc::clone(&binding);
            let callback = Arc::clone(&sink);
            context.on(button, move |_, _: &Activate, _| {
                dispatch(&callback, &target, action.clone())
            })?;
            context.append_child(actions, button)?;
        }
        let scroll =
            context.create_detached_component(document, ScrollView::new(ScrollAxes::Vertical))?;
        let fields = context.create_detached_component(document, Stack::column(12.0))?;
        let title = context.create_detached_component(document, TextInput::new(""))?;
        let title_field = context.create_detached_component(
            document,
            FormField::new("节点名称").control_child(title.stable_id()),
        )?;
        context.append_child(title_field, title)?;
        context.append_child(root, actions)?;
        context.append_child(root, title_field)?;
        context.append_child(root, scroll)?;
        context.append_child(scroll, fields)?;
        let target = Arc::clone(&binding);
        let callback = Arc::clone(&sink);
        context.on(title, move |_, event: &TextChanged, _| {
            dispatch(
                &callback,
                &target,
                NodeEditorAction::Title(event.value.clone()),
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
                    let input = context.create_detached_component(
                        document,
                        Switch::new(field_label(&key), false),
                    )?;
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
                    let input = context.create_detached_component(
                        document,
                        SearchDropdown::new(None::<String>).placeholder("请选择"),
                    )?;
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
                    let input = context.create_detached_component(
                        document,
                        TextArea::new("")
                            .height(120.0)
                            .placeholder(if key == "cases" {
                                "每行一个匹配值"
                            } else {
                                ""
                            }),
                    )?;
                    context.on(input, move |_, event: &TextChanged, _| {
                        let value = Value::String(event.value.clone());
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
                    let input = context.create_detached_component(document, TextInput::new(""))?;
                    context.on(input, move |_, event: &TextChanged, _| {
                        dispatch(
                            &sink,
                            &binding,
                            NodeEditorAction::Field {
                                key: key.clone(),
                                value: Value::String(event.value.clone()),
                            },
                        )
                    })?;
                    Input::Single(input)
                };
                let row = context.create_detached_component(
                    document,
                    FormField::new(if field.key == "createTask" {
                        "任务目标"
                    } else {
                        field_label(&field.key)
                    })
                    .control_child(input.id()),
                )?;
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
