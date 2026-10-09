use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use nana_ui::runtime::view::{entity_ref, widget, with_refs};
use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, Dropdown, DropdownEvent, DropdownOption,
    DropdownSelection, Entity, FrameworkError, LengthSpec, NodeStyle, SettingsCard, SettingsRow,
    StableNodeId, Stack, Switch, Text, TextArea, TextChanged, TextInput, ToggleChanged,
    ValidationMessage,
};
use nana_ui::{ButtonKind, ValidationIntent};

use crate::runtime_shell::ShellIntent;

type Edit = Arc<dyn Fn(String) -> ShellIntent + Send + Sync>;
type Sink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

#[derive(Clone)]
pub(crate) enum SurfaceControl {
    Section {
        id: String,
        label: String,
    },
    Toggle {
        id: String,
        label: String,
        checked: bool,
        intent: ShellIntent,
    },
    Choice {
        id: String,
        label: String,
        selected: String,
        options: Vec<(String, String)>,
        edit: Edit,
    },
    Text {
        id: String,
        value: String,
    },
    Error {
        id: String,
        value: String,
    },
    Action {
        id: String,
        label: String,
        intent: ShellIntent,
        enabled: bool,
        danger: bool,
    },
    Field {
        id: String,
        label: String,
        value: String,
        multiline: bool,
        edit: Edit,
    },
    Secret {
        id: String,
        label: String,
        value: String,
        edit: Edit,
    },
}

impl std::fmt::Debug for SurfaceControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceControl")
            .field("id", &self.id())
            .finish()
    }
}

impl SurfaceControl {
    pub fn section(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Section {
            id: id.into(),
            label: label.into(),
        }
    }
    pub fn toggle(
        id: impl Into<String>,
        label: impl Into<String>,
        checked: bool,
        intent: ShellIntent,
    ) -> Self {
        Self::Toggle {
            id: id.into(),
            label: label.into(),
            checked,
            intent,
        }
    }
    pub fn choice(
        id: impl Into<String>,
        label: impl Into<String>,
        selected: impl Into<String>,
        options: Vec<(String, String)>,
        edit: impl Fn(String) -> ShellIntent + Send + Sync + 'static,
    ) -> Self {
        let selected = selected.into();
        Self::Choice {
            id: id.into(),
            label: label.into(),
            selected: selected.clone(),
            options: {
                let mut options = options;
                if !options.iter().any(|(id, _)| id == &selected) {
                    options.insert(
                        0,
                        (
                            selected.clone(),
                            if selected.is_empty() {
                                "自动选择".into()
                            } else {
                                selected.clone()
                            },
                        ),
                    );
                }
                options
            },
            edit: Arc::new(edit),
        }
    }
    pub fn text(id: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Text {
            id: id.into(),
            value: value.into(),
        }
    }
    pub fn error(id: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Error {
            id: id.into(),
            value: value.into(),
        }
    }
    pub fn action(id: impl Into<String>, label: impl Into<String>, intent: ShellIntent) -> Self {
        Self::Action {
            id: id.into(),
            label: label.into(),
            intent,
            enabled: true,
            danger: false,
        }
    }
    pub fn field(
        id: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
        multiline: bool,
        edit: impl Fn(String) -> ShellIntent + Send + Sync + 'static,
    ) -> Self {
        Self::Field {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            multiline,
            edit: Arc::new(edit),
        }
    }
    pub fn secret(
        id: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
        edit: impl Fn(String) -> ShellIntent + Send + Sync + 'static,
    ) -> Self {
        Self::Secret {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            edit: Arc::new(edit),
        }
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        if let Self::Action { enabled: state, .. } = &mut self {
            *state = enabled;
        }
        self
    }
    pub fn danger(mut self) -> Self {
        if let Self::Action { danger, .. } = &mut self {
            *danger = true;
        }
        self
    }
    fn id(&self) -> &str {
        match self {
            Self::Section { id, .. }
            | Self::Toggle { id, .. }
            | Self::Choice { id, .. }
            | Self::Text { id, .. }
            | Self::Error { id, .. }
            | Self::Action { id, .. }
            | Self::Field { id, .. }
            | Self::Secret { id, .. } => id,
        }
    }
}

enum SurfaceNode {
    Section(Entity<SettingsCard>, Entity<Stack>),
    Toggle(Entity<Switch>, Arc<Mutex<ShellIntent>>),
    Choice(Entity<SettingsRow>, Entity<Dropdown>, Arc<Mutex<Edit>>),
    Text(Entity<Text>),
    Error(Entity<ValidationMessage>),
    Action(Entity<Button>, Arc<Mutex<ShellIntent>>),
    Field(Entity<SettingsRow>, Entity<TextArea>, Arc<Mutex<Edit>>),
    Secret(Entity<SettingsRow>, Entity<TextInput>, Arc<Mutex<Edit>>),
}

impl SurfaceNode {
    fn id(&self) -> StableNodeId {
        match self {
            Self::Section(view, _) => view.stable_id(),
            Self::Toggle(view, _) => view.stable_id(),
            Self::Choice(view, ..) => view.stable_id(),
            Self::Text(view) => view.stable_id(),
            Self::Error(view) => view.stable_id(),
            Self::Action(view, _) => view.stable_id(),
            Self::Field(view, ..) => view.stable_id(),
            Self::Secret(view, ..) => view.stable_id(),
        }
    }
}

#[derive(Default)]
pub(crate) struct SurfaceHandles {
    nodes: HashMap<String, SurfaceNode>,
    settings_width: Option<f32>,
}

impl SurfaceHandles {
    pub fn settings_width(&mut self, width: f32) {
        self.settings_width = Some(width);
    }
    #[cfg(debug_assertions)]
    pub(crate) fn debug_nodes(&self) -> Vec<(String, StableNodeId, Option<Entity<TextArea>>)> {
        self.nodes
            .iter()
            .map(|(id, node)| {
                let editor = match node {
                    SurfaceNode::Field(_, editor, _) => Some(*editor),
                    _ => None,
                };
                (
                    id.clone(),
                    match node {
                        SurfaceNode::Secret(_, view, _) => view.stable_id(),
                        SurfaceNode::Choice(_, view, _) => view.stable_id(),
                        _ => editor
                            .map(|view| view.stable_id())
                            .unwrap_or_else(|| node.id()),
                    },
                    editor,
                )
            })
            .collect()
    }
    pub fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        controls: &[SurfaceControl],
        sink: Sink,
    ) -> Result<Vec<StableNodeId>, FrameworkError> {
        macro_rules! mount_view {
            ($kind:ty, $view:expr) => {{
                let (_, entity) = context.mount_view_detached(document, || {
                    let entity = entity_ref::<$kind>();
                    with_refs(widget($view).entity_ref(entity), entity)
                })?;
                entity
            }};
        }
        let stacked = self.settings_width.is_none_or(|width| width <= 900.0);
        let mut keep = HashSet::new();
        for control in controls {
            let id = control.id().to_owned();
            keep.insert(id.clone());
            if !self.nodes.contains_key(&id) {
                let node = match control {
                    SurfaceControl::Section { label, .. } => {
                        let card_label = label.clone();
                        let card = mount_view!(SettingsCard, SettingsCard::new(card_label));
                        let body = mount_view!(Stack, Stack::column(12.0));
                        context.append_child(card, body)?;
                        SurfaceNode::Section(card, body)
                    }
                    SurfaceControl::Toggle {
                        label,
                        checked,
                        intent,
                        ..
                    } => {
                        let switch_label = label.clone();
                        let view = mount_view!(Switch, Switch::new(switch_label, *checked));
                        let binding = Arc::new(Mutex::new(intent.clone()));
                        let current = binding.clone();
                        let dispatch = sink.clone();
                        context.on(view, move |_, _: &ToggleChanged, _| {
                            if let Ok(intent) = current.lock() {
                                dispatch(intent.clone());
                            }
                        })?;
                        SurfaceNode::Toggle(view, binding)
                    }
                    SurfaceControl::Choice {
                        label,
                        selected,
                        options,
                        edit,
                        ..
                    } => {
                        let selected = selected.clone();
                        let options = options.clone();
                        let editor = mount_view!(
                            Dropdown,
                            Dropdown::single(Some(selected))
                                .placeholder("自动选择")
                                .options(options.iter().map(|(id, label)| {
                                    DropdownOption::new(id.clone(), label.clone())
                                }))
                        );
                        let wrapper =
                            field_wrapper(context, document, label, editor.stable_id(), stacked)?;
                        let binding = Arc::new(Mutex::new(edit.clone()));
                        let current = binding.clone();
                        let dispatch = sink.clone();
                        context.on(editor, move |_, event: &DropdownEvent<Arc<str>>, _| {
                            if let DropdownEvent::Select(value) = event {
                                if let Ok(edit) = current.lock() {
                                    dispatch(edit(value.to_string()));
                                }
                            }
                        })?;
                        SurfaceNode::Choice(wrapper, editor, binding)
                    }
                    SurfaceControl::Text { value, .. } => {
                        let value = value.clone();
                        SurfaceNode::Text(mount_view!(Text, surface_text(value)))
                    }
                    SurfaceControl::Error { value, .. } => {
                        let value = value.clone();
                        SurfaceNode::Error(mount_view!(
                            ValidationMessage,
                            ValidationMessage::new(value, ValidationIntent::Danger)
                        ))
                    }
                    SurfaceControl::Action { label, intent, .. } => {
                        let label = label.clone();
                        let view = mount_view!(Button, Button::new(label));
                        let binding = Arc::new(Mutex::new(intent.clone()));
                        let current = binding.clone();
                        let dispatch = sink.clone();
                        context.on(view, move |_, _: &Activate, _| {
                            if let Ok(intent) = current.lock() {
                                dispatch(intent.clone());
                            }
                        })?;
                        SurfaceNode::Action(view, binding)
                    }
                    SurfaceControl::Field {
                        label,
                        value,
                        multiline: false,
                        edit,
                        ..
                    } => {
                        let value = value.clone();
                        let editor = mount_view!(TextInput, TextInput::new(value));
                        let wrapper =
                            field_wrapper(context, document, label, editor.stable_id(), stacked)?;
                        let binding = Arc::new(Mutex::new(edit.clone()));
                        let current = binding.clone();
                        let dispatch = sink.clone();
                        context.on(editor, move |_, event: &TextChanged, _| {
                            if let Ok(edit) = current.lock() {
                                dispatch(edit(event.value.to_string()));
                            }
                        })?;
                        SurfaceNode::Secret(wrapper, editor, binding)
                    }
                    SurfaceControl::Field {
                        label,
                        value,
                        multiline,
                        edit,
                        ..
                    } => {
                        let value = value.clone();
                        let height = if *multiline { 128.0 } else { 40.0 };
                        let editor = mount_view!(TextArea, TextArea::new(value).height(height));
                        let wrapper =
                            field_wrapper(context, document, label, editor.stable_id(), stacked)?;
                        let binding = Arc::new(Mutex::new(edit.clone()));
                        let current = binding.clone();
                        let dispatch = sink.clone();
                        context.on(editor, move |_, event: &TextChanged, _| {
                            if let Ok(edit) = current.lock() {
                                dispatch(edit(event.value.to_string()));
                            }
                        })?;
                        SurfaceNode::Field(wrapper, editor, binding)
                    }
                    SurfaceControl::Secret {
                        label, value, edit, ..
                    } => {
                        let value = value.clone();
                        let editor = mount_view!(TextInput, TextInput::new(value).secure(true));
                        let wrapper =
                            field_wrapper(context, document, label, editor.stable_id(), stacked)?;
                        let binding = Arc::new(Mutex::new(edit.clone()));
                        let current = binding.clone();
                        let dispatch = sink.clone();
                        context.on(editor, move |_, event: &TextChanged, _| {
                            if let Ok(edit) = current.lock() {
                                dispatch(edit(event.value.to_string()));
                            }
                        })?;
                        SurfaceNode::Secret(wrapper, editor, binding)
                    }
                };
                self.nodes.insert(id.clone(), node);
            }
            let node = &self.nodes[&id];
            match node {
                SurfaceNode::Choice(wrapper, editor, _) => {
                    context.update_component(*wrapper, |row, _| row.stacked = stacked)?;
                    context.update_component(*editor, |view, _| {
                        field_width(&mut view.style, self.settings_width, stacked)
                    })?;
                }
                SurfaceNode::Field(wrapper, editor, _) => {
                    context.update_component(*wrapper, |row, _| row.stacked = stacked)?;
                    context.update_component(*editor, |view, _| {
                        field_width(&mut view.style, self.settings_width, stacked)
                    })?;
                }
                SurfaceNode::Secret(wrapper, editor, _) => {
                    context.update_component(*wrapper, |row, _| row.stacked = stacked)?;
                    context.update_component(*editor, |view, _| {
                        field_width(&mut view.style, self.settings_width, stacked)
                    })?;
                }
                _ => {}
            }
            match (node, control) {
                (SurfaceNode::Section(view, _), SurfaceControl::Section { label, .. }) => {
                    context.update_component(*view, |card, _| {
                        *card = SettingsCard::new(label.clone())
                    })?
                }
                (
                    SurfaceNode::Toggle(view, binding),
                    SurfaceControl::Toggle {
                        label,
                        checked,
                        intent,
                        ..
                    },
                ) => {
                    if let Ok(mut current) = binding.lock() {
                        *current = intent.clone();
                    }
                    context.update_component(*view, |toggle, _| {
                        *toggle = Switch::new(label.clone(), *checked)
                    })?;
                }
                (
                    SurfaceNode::Choice(_, view, binding),
                    SurfaceControl::Choice {
                        selected,
                        options,
                        edit,
                        ..
                    },
                ) => {
                    if let Ok(mut current) = binding.lock() {
                        *current = edit.clone();
                    }
                    context.update_component(*view, |dropdown, _| {
                        dropdown.selection =
                            DropdownSelection::Single(Some(Arc::from(selected.as_str())));
                        dropdown.options = options
                            .iter()
                            .map(|(id, label)| DropdownOption::new(id.clone(), label.clone()))
                            .collect();
                    })?;
                }
                (SurfaceNode::Text(view), SurfaceControl::Text { value, .. }) => context
                    .update_component(*view, |text, _| *text = surface_text(value.clone()))?,
                (SurfaceNode::Error(view), SurfaceControl::Error { value, .. }) => context
                    .update_component(*view, |message, _| {
                        *message = ValidationMessage::new(value.clone(), ValidationIntent::Danger)
                    })?,
                (
                    SurfaceNode::Action(view, binding),
                    SurfaceControl::Action {
                        label,
                        intent,
                        enabled,
                        danger,
                        ..
                    },
                ) => {
                    if let Ok(mut current) = binding.lock() {
                        *current = intent.clone();
                    }
                    context.update_component(*view, |button, _| {
                        *button = Button::new(label.clone())
                            .kind(if *danger {
                                ButtonKind::Danger
                            } else {
                                ButtonKind::Subtle
                            })
                            .disabled(!enabled)
                    })?;
                }
                (
                    SurfaceNode::Field(_, view, binding),
                    SurfaceControl::Field { value, edit, .. },
                ) => {
                    if let Ok(mut current) = binding.lock() {
                        *current = edit.clone();
                    }
                    context.update_component(*view, |editor, _| {
                        if editor.state.value != *value {
                            editor.state.replace_value(value.clone());
                        }
                    })?;
                }
                (
                    SurfaceNode::Secret(_, view, binding),
                    SurfaceControl::Secret { value, edit, .. }
                    | SurfaceControl::Field {
                        value,
                        edit,
                        multiline: false,
                        ..
                    },
                ) => {
                    if let Ok(mut current) = binding.lock() {
                        *current = edit.clone();
                    }
                    context.update_component(*view, |editor, _| {
                        if editor.state.value != *value {
                            editor.state.replace_value(value.clone());
                        }
                    })?;
                }
                _ => {}
            }
        }
        let mut grouped = Vec::new();
        let mut section = None;
        let mut section_children = Vec::new();
        for control in controls {
            let Some(node) = self.nodes.get(control.id()) else {
                continue;
            };
            if matches!(control, SurfaceControl::Section { .. }) {
                if let Some(parent) = section {
                    context.reconcile_children(parent, &section_children)?;
                }
                section_children.clear();
                section = match node {
                    SurfaceNode::Section(_, body) => Some(body.stable_id()),
                    _ => None,
                };
                grouped.push(node.id());
            } else if section.is_some() {
                section_children.push(node.id());
            } else {
                grouped.push(node.id());
            }
        }
        if let Some(parent) = section {
            context.reconcile_children(parent, &section_children)?;
        }
        let section_bodies: HashSet<_> = self
            .nodes
            .values()
            .filter_map(|node| match node {
                SurfaceNode::Section(_, body) => Some(body.stable_id()),
                _ => None,
            })
            .collect();
        let mut mutations = nana_ui::runtime::MutationQueue::new();
        for id in &grouped {
            if context
                .world()
                .node(*id)
                .and_then(|node| node.parent)
                .is_some_and(|parent| section_bodies.contains(&parent))
            {
                // The caller mounts these roots only after sync returns. Preserve them
                // before deleting a former section and its remaining descendants.
                mutations.detach(*id);
            }
        }
        if !mutations.is_empty() {
            context.commit_mutations(mutations)?;
        }
        let stale: Vec<_> = self
            .nodes
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect();
        for id in stale {
            if let Some(node) = self.nodes.remove(&id) {
                if !context.world().contains(node.id()) {
                    continue;
                }
                match node {
                    SurfaceNode::Section(view, _) => {
                        context.remove_view(view)?;
                    }
                    SurfaceNode::Toggle(view, _) => {
                        context.remove_view(view)?;
                    }
                    SurfaceNode::Choice(view, ..) => {
                        context.remove_view(view)?;
                    }
                    SurfaceNode::Text(view) => {
                        context.remove_view(view)?;
                    }
                    SurfaceNode::Error(view) => {
                        context.remove_view(view)?;
                    }
                    SurfaceNode::Action(view, _) => {
                        context.remove_view(view)?;
                    }
                    SurfaceNode::Field(view, ..) => {
                        context.remove_view(view)?;
                    }
                    SurfaceNode::Secret(view, ..) => {
                        context.remove_view(view)?;
                    }
                }
            }
        }
        Ok(grouped)
    }
}

fn field_wrapper(
    context: &mut AppContext,
    document: DocumentId,
    label: &str,
    editor: StableNodeId,
    stacked: bool,
) -> Result<Entity<SettingsRow>, FrameworkError> {
    let row = context.mount_settings_leaf_row(document, label, None, editor)?;
    context.update_component(row, |row, _| row.stacked = stacked)?;
    // `mount_settings_leaf_row` assembles the initial horizontal row.  The
    // breakpoint flag is part of the row's layout contract, so re-run its
    // assembler after changing `stacked`; otherwise the cached label/control
    // slots keep their pre-breakpoint geometry.
    context.assemble_settings_row(row)?;
    // Re-project once more so the SettingsRowCopy slot receives the forced
    // breakpoint state as well; the assembler only changes child ordering.
    context.update_component(row, |row, _| row.stacked = stacked)?;
    Ok(row)
}
fn field_width(style: &mut NodeStyle, settings_width: Option<f32>, stacked: bool) {
    let layout = Arc::make_mut(&mut style.layout);
    layout.width = Some(if settings_width.is_some() {
        LengthSpec::Px(360.0)
    } else {
        LengthSpec::Fill
    });
    layout.max_width = Some(LengthSpec::Percent(100.0));
    layout.flex_shrink = Some(if stacked { 1.0 } else { 0.0 });
    layout.flex_grow = Some(0.0);
    // Keep the inline control visually to the right of the row copy even
    // when the copy has no measured text height (for example during the
    // first headless layout pass).
    layout.margin_left = (!stacked).then_some(LengthSpec::Px(1.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_waiting_and_completed_runs_preserve_details_and_live_actions() {
        let mut context = AppContext::new();
        let document = DocumentId::new(1).unwrap();
        let root = context
            .create_component(document, Stack::column(8.0).width(LengthSpec::Px(600.0)))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = events.clone();
        let sink: Sink = Arc::new(move |intent| received.lock().unwrap().push(intent));
        let mut surface = SurfaceHandles::default();
        let mut retained = None;
        let mut old_section = None;

        for (run, waiting) in [(1, true), (1, false), (2, true), (2, false), (3, true)] {
            let mut controls = vec![SurfaceControl::choice(
                "auto-run-selection",
                "运行记录",
                run.to_string(),
                vec![],
                |value| ShellIntent::SelectAutomation(value),
            )];
            if waiting {
                controls.extend([
                    SurfaceControl::section("auto-human-waiting", "等待输入"),
                    SurfaceControl::field("auto-response", "回复", "", true, |_| {
                        ShellIntent::RefreshAutomations
                    }),
                    SurfaceControl::action(
                        "auto-resume",
                        "继续运行",
                        ShellIntent::RefreshAutomations,
                    ),
                    SurfaceControl::action(
                        "auto-cancel",
                        "取消运行",
                        ShellIntent::CreateAutomation,
                    ),
                ]);
            }
            let output = format!(
                "run {run}: {}",
                if waiting { "waiting" } else { "succeeded" }
            );
            controls.extend([
                SurfaceControl::choice(
                    "auto-node-output-selection",
                    "节点",
                    "human",
                    vec![("human".into(), "人工输入".into())],
                    |_| ShellIntent::RefreshAutomations,
                ),
                SurfaceControl::field("auto-node-output", "输出", &output, true, |_| {
                    ShellIntent::RefreshAutomations
                }),
            ]);
            let children = surface
                .sync(&mut context, document, &controls, sink.clone())
                .unwrap();
            // sync must return live roots before the caller can attach them.
            assert!(children.iter().all(|id| context.world().contains(*id)));
            context
                .reconcile_children(root.stable_id(), &children)
                .unwrap();
            context
                .layout_document(
                    document,
                    nana_ui::runtime::LayoutViewport::new(600.0, 900.0),
                )
                .unwrap();

            let SurfaceNode::Choice(choice, editor, _) =
                surface.nodes["auto-node-output-selection"]
            else {
                panic!("node selector");
            };
            let SurfaceNode::Field(wrapper, output_editor, _) = surface.nodes["auto-node-output"]
            else {
                panic!("node output");
            };
            let ids = (
                choice.stable_id(),
                editor.stable_id(),
                wrapper.stable_id(),
                output_editor.stable_id(),
            );
            assert_eq!(*retained.get_or_insert(ids), ids);
            assert_eq!(
                context
                    .read(output_editor, |field| field.state.value.clone())
                    .unwrap(),
                output
            );
            assert!(
                context
                    .world()
                    .layout_box(output_editor.stable_id())
                    .unwrap()
                    .height
                    > 0.0
            );

            let expected_parent = if waiting {
                let SurfaceNode::Section(card, body) = surface.nodes["auto-human-waiting"] else {
                    panic!("waiting section");
                };
                old_section = Some(card.stable_id());
                for id in ["auto-resume", "auto-cancel"] {
                    let SurfaceNode::Action(button, _) = surface.nodes[id] else {
                        panic!("run action");
                    };
                    assert!(context.activate_button(button).unwrap());
                }
                let mut events = events.lock().unwrap();
                assert!(matches!(events.remove(0), ShellIntent::RefreshAutomations));
                assert!(matches!(events.remove(0), ShellIntent::CreateAutomation));
                assert!(events.is_empty());
                body.stable_id()
            } else {
                assert!(!context.world().contains(old_section.take().unwrap()));
                assert!(!surface.nodes.contains_key("auto-cancel"));
                root.stable_id()
            };
            assert_eq!(
                context.world().node(choice.stable_id()).unwrap().parent,
                Some(expected_parent)
            );
            assert_eq!(
                context.world().node(wrapper.stable_id()).unwrap().parent,
                Some(expected_parent)
            );
        }
    }

    #[test]
    fn settings_breakpoint_reflows_real_controls_and_keeps_automation_stacked() {
        let mut context = AppContext::new();
        let document = DocumentId::new(1).unwrap();
        let root = context
            .create_component(document, Stack::column(0.0).width(LengthSpec::Px(600.0)))
            .unwrap();
        let controls = [SurfaceControl::field(
            "name",
            "名称",
            "值",
            false,
            |_| ShellIntent::RefreshAutomations,
        )];
        let sink: Sink = Arc::new(|_| {});
        let mut surface = SurfaceHandles::default();
        for (window_width, expected_stacked) in [(1200.0, false), (900.0, true), (901.0, false)] {
            surface.settings_width(window_width);
            let children = surface
                .sync(&mut context, document, &controls, sink.clone())
                .unwrap();
            context
                .reconcile_children(root.stable_id(), &children)
                .unwrap();
            context
                .layout_document(
                    document,
                    nana_ui::runtime::LayoutViewport::new(window_width, 600.0),
                )
                .unwrap();
            let SurfaceNode::Secret(wrapper, editor, _) = &surface.nodes["name"] else {
                panic!("single-line editor");
            };
            let row = context.read(*wrapper, Clone::clone).unwrap();
            let label = context.world().layout_box(row.label_slot.unwrap()).unwrap();
            let field = context.world().layout_box(editor.stable_id()).unwrap();
            assert_eq!(row.stacked, expected_stacked);
            assert!((field.width - 360.0).abs() < 0.5, "field: {field:?}");
            if expected_stacked {
                assert!(field.y >= label.y + label.height, "{label:?} {field:?}");
            } else {
                assert!(field.x > label.x, "{label:?} {field:?}");
            }
        }
        let mut automation = SurfaceHandles::default();
        automation
            .sync(&mut context, DocumentId::new(2).unwrap(), &controls, sink)
            .unwrap();
        let SurfaceNode::Secret(row, _, _) = &automation.nodes["name"] else {
            panic!("single-line editor");
        };
        assert!(context.read(*row, |row| row.stacked).unwrap());
    }
}

/// Detail text keeps its own line breaks and wraps long paths and commands.
fn surface_text(value: String) -> Text {
    let mut text = Text::new(value);
    let layout = Arc::make_mut(&mut text.style.layout);
    layout.white_space = nana_ui_core::WhiteSpaceSpec::PreWrap;
    layout.min_width = Some(LengthSpec::Px(0.0));
    text
}
