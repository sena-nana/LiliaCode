use crate::runtime_layout::{pending_actions_row, pending_interaction_card, Bound};
use lilia_contracts::TaskId;
use nana_ui::runtime::view::{entity_ref, signal, widget, with_refs, EachExt, IntoView, Signal};
#[cfg(test)]
use nana_ui::runtime::Activate;
use nana_ui::runtime::{
    AppContext, Button, Card, ComponentView, DocumentId, Entity, FormField, FrameworkError,
    MountedView, StableNodeId, Stack, Text, TextArea, TextChanged,
};
use nana_ui::{ButtonKind, ControlSize};
use nana_ui_platform::WindowId;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnStopTarget {
    pub task_id: TaskId,
    pub turn_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKind {
    PermissionApproval,
    PlanApproval,
    AskUser,
    ToolConsent,
    McpElicitation,
    ArchitectureChange,
    TitleUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingOption {
    pub id: String,
    pub label: String,
    pub selected: bool,
    pub danger: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolConsentPending {
    pub command: String,
    pub message: String,
    pub command_editable: bool,
    pub can_allow: bool,
    pub can_deny: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AskUserPending {
    pub show_other: bool,
    pub other_selected: bool,
    pub freeform: String,
    pub show_freeform: bool,
    pub show_skip: bool,
    pub show_back: bool,
    pub show_cancel: bool,
    pub show_reject: bool,
    pub can_submit: bool,
    pub submit_label: String,
    pub reject_label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpFieldOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
    pub multi: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpField {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub value: String,
    pub enabled: bool,
    pub options: Vec<McpFieldOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpPending {
    pub url: Option<String>,
    pub raw_json: Option<String>,
    pub fields: Vec<McpField>,
    pub can_accept: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingSnapshot {
    pub stop_target: Option<TurnStopTarget>,
    pub request_id: String,
    pub kind: PendingKind,
    pub title: String,
    pub prompt: String,
    pub draft: String,
    pub options: Vec<PendingOption>,
    pub tool: Option<ToolConsentPending>,
    pub ask: Option<AskUserPending>,
    pub mcp: Option<McpPending>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PendingAction {
    RespondApproval {
        approved: bool,
    },
    RespondTitle {
        accepted: bool,
    },
    RespondArchitecture {
        approved: bool,
    },
    RespondPlan {
        action: String,
    },
    RespondToolConsent {
        approved: bool,
    },
    ToolConsentDraftChanged {
        command: String,
        message: String,
    },
    AskUserPending {
        action: String,
        value: String,
    },
    PendingDraftChanged {
        value: String,
    },
    SelectPendingOption {
        option_id: String,
    },
    RespondMcp {
        action: String,
    },
    McpFieldChanged {
        field_key: String,
        value: String,
    },
    McpRawJsonChanged {
        value: String,
    },
    McpToggleOption {
        field_key: String,
        value: String,
        multi: bool,
    },
    McpToggleBoolean {
        field_key: String,
    },
    OpenMarkdownLink(String),
    InterruptTurn(Option<TurnStopTarget>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingTarget {
    pub window_id: WindowId,
    pub task_id: Option<String>,
    pub request_id: String,
}
impl PendingTarget {
    pub(crate) fn matches(&self, current: &Self) -> bool {
        self.task_id.is_some() && !self.request_id.is_empty() && self == current
    }
}
type Sink = Arc<dyn Fn(PendingTarget, PendingAction) + Send + Sync>;

pub(crate) struct PendingView {
    pub(crate) root: Entity<Card>,
    mounted: Option<MountedView>,
    request: Option<RequestView>,
    sink: Sink,
}
struct PendingField {
    editor: Entity<TextArea>,
    wrapper: Entity<FormField>,
}
struct PendingButton {
    node: Entity<Button>,
}

#[derive(Clone)]
enum FieldRole {
    Draft,
    Ask,
    Command,
    Message,
    Raw,
    Mcp(String),
}

#[derive(Clone)]
enum BodyContent {
    Field {
        label: String,
        value: String,
        role: FieldRole,
    },
    Button {
        label: String,
        kind: ButtonKind,
        disabled: bool,
        action: PendingAction,
    },
}

#[derive(Clone)]
struct BodyModel {
    key: String,
    visible: bool,
    content: BodyContent,
}

#[derive(Clone)]
struct ActionModel {
    key: String,
    label: String,
    kind: ButtonKind,
    disabled: bool,
    visible: bool,
    action: PendingAction,
}

struct RequestView {
    target: PendingTarget,
    kind: PendingKind,
    title: Entity<Text>,
    prompt: Entity<Text>,
    actions: Entity<Stack>,
    fields: HashMap<String, PendingField>,
    buttons: HashMap<String, PendingButton>,
    tool_draft: Arc<Mutex<(String, String)>>,
    body_list: Entity<Stack>,
    title_text: Bound<String>,
    prompt_text: Bound<String>,
    body: Bound<Vec<BodyModel>>,
    action_rows: Bound<Vec<ActionModel>>,
    show_body: Bound<bool>,
    show_actions: Bound<bool>,
    field_values: Arc<Mutex<HashMap<String, Signal<String>>>>,
}
impl PendingView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Sink,
    ) -> Result<Self, FrameworkError> {
        let (_, root) = context.mount_view_detached(document, || {
            let root = entity_ref::<Card>();
            with_refs(widget(pending_interaction_card()).entity_ref(root), root)
        })?;
        context
            .compat_world_mut()
            .register_focus_scope(root.stable_id())?;
        Ok(Self {
            root,
            mounted: None,
            request: None,
            sink,
        })
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        _document: DocumentId,
        window_id: WindowId,
        task_id: Option<String>,
        pending: Option<&PendingSnapshot>,
    ) -> Result<(), FrameworkError> {
        let target = pending.map(|pending| PendingTarget {
            window_id,
            task_id,
            request_id: pending.request_id.clone(),
        });
        let changed = self
            .request
            .as_ref()
            .map(|request| (&request.target, request.kind))
            != target.as_ref().zip(pending.map(|pending| pending.kind));
        if changed {
            if let Some(mounted) = self.mounted.take() {
                mounted.unmount(context)?;
            }
            self.request = None;
        }
        let (Some(pending), Some(target)) = (pending, target) else {
            return Ok(());
        };
        if self.request.is_none() {
            let sink = Arc::clone(&self.sink);
            let (mounted, request) =
                RequestView::mount(context, self.root.stable_id(), target, sink, pending)?;
            self.mounted = Some(mounted);
            self.request = Some(request);
            return Ok(());
        }
        self.request.as_mut().unwrap().apply(context, pending)
    }
    pub(crate) fn restore_focus(
        &self,
        context: &mut AppContext,
        document: DocumentId,
    ) -> Result<(), FrameworkError> {
        if context.world().focused(document).is_none() && self.request.is_some() {
            let mut changes = nana_ui::runtime::MutationQueue::new();
            changes.restore_focus_within(self.root.stable_id());
            context.commit_mutations(changes)?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn actions(&self) -> Option<StableNodeId> {
        self.request.as_ref().map(|r| r.actions.stable_id())
    }

    pub(crate) fn debug_nodes(&self) -> Vec<(String, StableNodeId)> {
        let Some(request) = &self.request else {
            return Vec::new();
        };
        request
            .fields
            .iter()
            .map(|(id, field)| (id.clone(), field.editor.stable_id()))
            .chain(
                request
                    .buttons
                    .iter()
                    .map(|(id, button)| (id.clone(), button.node.stable_id())),
            )
            .collect()
    }
}
impl RequestView {
    fn mount(
        context: &mut AppContext,
        parent: StableNodeId,
        target: PendingTarget,
        sink: Sink,
        pending: &PendingSnapshot,
    ) -> Result<(MountedView, Self), FrameworkError> {
        let title_text = Bound::new();
        let prompt_text = Bound::new();
        let body: Bound<Vec<BodyModel>> = Bound::new();
        let action_rows: Bound<Vec<ActionModel>> = Bound::new();
        let show_body = Bound::new();
        let show_actions = Bound::new();
        let field_values = Arc::new(Mutex::new(HashMap::<String, Signal<String>>::new()));
        let button_actions = Arc::new(Mutex::new(
            HashMap::<String, Arc<Mutex<PendingAction>>>::new(),
        ));
        let tool_draft = Arc::new(Mutex::new(tool_draft_of(pending)));
        let initial_body = body_models(pending);
        let initial_actions = action_models(pending);
        let title_slot = title_text.clone();
        let prompt_slot = prompt_text.clone();
        let body_slot = body.clone();
        let action_slot = action_rows.clone();
        let show_body_slot = show_body.clone();
        let show_actions_slot = show_actions.clone();
        let values = Arc::clone(&field_values);
        let slots = button_actions;
        let draft = Arc::clone(&tool_draft);
        let row_sink = Arc::clone(&sink);
        let action_sink = Arc::clone(&sink);
        let row_target = target.clone();
        let action_target = target.clone();
        let opening_title = pending.title.clone();
        let opening_prompt = pending.prompt.clone();
        let (mounted, (title, prompt)) = context.mount_view(parent, move || {
            let title_text = title_slot.install(signal(opening_title));
            let prompt_text = prompt_slot.install(signal(opening_prompt));
            let body = body_slot.install(signal(initial_body));
            let action_rows = action_slot.install(signal(initial_actions));
            let show_body = show_body_slot
                .install(signal(body.with(|rows| rows.iter().any(|row| row.visible))));
            let show_actions = show_actions_slot.install(signal(
                action_rows.with(|rows| rows.iter().any(|row| row.visible)),
            ));
            let title = entity_ref::<Text>();
            let prompt = entity_ref::<Text>();
            let values = Arc::clone(&values);
            let slots = Arc::clone(&slots);
            let draft = Arc::clone(&draft);
            let row_sink = Arc::clone(&row_sink);
            let action_sink = Arc::clone(&action_sink);
            with_refs(
                (
                    widget(
                        Text::new("")
                            .font_weight(600)
                            .color(nana_ui::runtime::SemanticColorRole::Text),
                    )
                    .entity_ref(title)
                    .value(title_text),
                    widget(Text::new("").color(nana_ui::runtime::SemanticColorRole::Text))
                        .entity_ref(prompt)
                        .value(prompt_text),
                    body.each(|row| row.key.clone(), {
                        let body = body;
                        let values = Arc::clone(&values);
                        let slots = Arc::clone(&slots);
                        let draft = Arc::clone(&draft);
                        let sink = Arc::clone(&row_sink);
                        let target = row_target.clone();
                        move |row| body_row(row, body, &values, &slots, &draft, &sink, &target)
                    })
                    .gap(4.0)
                    .visible(show_body),
                    action_rows
                        .each(|row| row.key.clone(), {
                            let action_rows = action_rows;
                            let slots = Arc::clone(&slots);
                            let sink = action_sink;
                            let target = action_target;
                            move |row| action_button(row, action_rows, &slots, &sink, &target)
                        })
                        .horizontal(6.0)
                        .visible(show_actions),
                ),
                (title, prompt),
            )
        })?;
        let root_ids = mounted.roots().to_vec();
        if root_ids.len() != 4 {
            mounted.unmount(context)?;
            return Err(FrameworkError::InvalidInput);
        }
        let body_list = Entity::<Stack>::from_stable_id(root_ids[2]);
        let actions = Entity::<Stack>::from_stable_id(root_ids[3]);
        context.update_component(actions, |stack, _| {
            let mut hidden = false;
            stack.share_layouts(&mut |layout| hidden = layout.hidden);
            *stack = pending_actions_row();
            stack.share_layouts(&mut |layout| {
                std::sync::Arc::make_mut(layout).hidden = hidden;
            });
        })?;
        let mut request = Self {
            target,
            kind: pending.kind,
            title,
            prompt,
            actions,
            fields: HashMap::new(),
            buttons: HashMap::new(),
            tool_draft,
            body_list,
            title_text,
            prompt_text,
            body,
            action_rows,
            show_body,
            show_actions,
            field_values,
        };
        request.rebuild(context);
        Ok((mounted, request))
    }

    fn apply(
        &mut self,
        context: &mut AppContext,
        pending: &PendingSnapshot,
    ) -> Result<(), FrameworkError> {
        if pending.kind == PendingKind::ToolConsent {
            if let Ok(mut draft) = self.tool_draft.lock() {
                *draft = tool_draft_of(pending);
            }
        }
        self.title_text.set(pending.title.clone());
        self.prompt_text.set(pending.prompt.clone());
        let body = keep_hidden(&self.body.signal().get(), body_models(pending));
        let actions = keep_hidden_actions(&self.action_rows.signal().get(), action_models(pending));
        let updates = self
            .field_values
            .lock()
            .map(|values| {
                body.iter()
                    .filter_map(|row| {
                        let BodyContent::Field { value, .. } = &row.content else {
                            return None;
                        };
                        values
                            .get(&row.key)
                            .copied()
                            .map(|slot| (slot, value.clone()))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for (slot, value) in updates {
            slot.set(value);
        }
        self.show_body.set(body.iter().any(|row| row.visible));
        self.show_actions.set(actions.iter().any(|row| row.visible));
        self.body.set(body);
        self.action_rows.set(actions);
        context.flush_reactive()?;
        self.rebuild(context);
        Ok(())
    }

    fn rebuild(&mut self, context: &AppContext) {
        let body = self.body.signal().get();
        let actions = self.action_rows.signal().get();
        let body_children = context
            .world()
            .node(self.body_list.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        let action_children = context
            .world()
            .node(self.actions.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        let mut fields = HashMap::new();
        let mut buttons = HashMap::new();
        for (row, child) in body.iter().zip(body_children) {
            match &row.content {
                BodyContent::Field { .. } => {
                    let wrapper = Entity::<FormField>::from_stable_id(child);
                    let Some(editor) = context.read(wrapper, |field| field.control).ok().flatten()
                    else {
                        continue;
                    };
                    fields.insert(
                        row.key.clone(),
                        PendingField {
                            editor: Entity::from_stable_id(editor),
                            wrapper,
                        },
                    );
                }
                BodyContent::Button { .. } => {
                    buttons.insert(
                        row.key.clone(),
                        PendingButton {
                            node: Entity::from_stable_id(child),
                        },
                    );
                }
            }
        }
        for (row, child) in actions.iter().zip(action_children) {
            buttons.insert(
                row.key.clone(),
                PendingButton {
                    node: Entity::from_stable_id(child),
                },
            );
        }
        self.fields = fields;
        self.buttons = buttons;
    }
}

fn body_row(
    row: BodyModel,
    body: Signal<Vec<BodyModel>>,
    values: &Mutex<HashMap<String, Signal<String>>>,
    slots: &Mutex<HashMap<String, Arc<Mutex<PendingAction>>>>,
    draft: &Arc<Mutex<(String, String)>>,
    sink: &Sink,
    target: &PendingTarget,
) -> nana_ui::runtime::view::AnyView {
    let key = row.key;
    match row.content {
        BodyContent::Field { label, value, role } => {
            let value_signal = field_signal(values, &key, value);
            let sink = Arc::clone(sink);
            let target = target.clone();
            let draft = Arc::clone(draft);
            let show = body;
            let label_rows = body;
            let show_key = key.clone();
            let label_key = key;
            widget(FormField::new(label))
                .bind(move |field| {
                    if let Some(label) = label_rows.with(|rows| field_label(rows, &label_key)) {
                        field.label = std::sync::Arc::from(label);
                    }
                })
                .visible(move || show.with(|rows| row_shown(rows, &show_key)))
                .control(
                    widget(TextArea::new("").height(48.0))
                        .value(value_signal)
                        .on_input(move |event: &TextChanged| {
                            sink(
                                target.clone(),
                                field_change(&role, &draft, event.value.to_string()),
                            )
                        }),
                )
                .into_any()
        }
        BodyContent::Button {
            label,
            kind,
            disabled,
            action,
        } => {
            let face = ButtonFace {
                label,
                kind,
                disabled,
                visible: true,
                action,
            };
            let rows = body;
            let lookup_key = key.clone();
            wire_button(
                key,
                face,
                move || rows.with(|rows| button_face(rows, &lookup_key)),
                slots,
                sink,
                target,
                ButtonShape::Choice,
            )
            .into_any()
        }
    }
}

/// Body buttons are the answers to pick from; footer buttons commit.
#[derive(Clone, Copy)]
enum ButtonShape {
    Choice,
    Action,
}

/// One answer row: a radio glyph that fills when chosen, the label on the
/// start edge. `Primary` is how the body model marks the chosen answer.
fn choice_button(face: &ButtonFace) -> Button {
    let selected = face.kind == ButtonKind::Primary;
    let icon = if matches!(face.action, PendingAction::OpenMarkdownLink(_)) {
        nana_ui::icons_tabler::EXTERNAL_LINK
    } else if selected {
        nana_ui::icons_tabler::CIRCLE_CHECK
    } else {
        nana_ui::icons_tabler::CIRCLE
    };
    let mut button = crate::ui::theme::list_row_button(face.label.clone(), icon);
    button.disabled = face.disabled;
    let layout = std::sync::Arc::make_mut(&mut button.style.layout);
    layout.height = Some(nana_ui::runtime::LengthSpec::Px(34.0));
    if selected {
        button.style.background = Some(nana_ui::runtime::SemanticColorRole::AccentSoft);
        button.style.foreground = Some(nana_ui::runtime::SemanticColorRole::AccentOnSoft);
        button.style.interaction.hovered.background =
            Some(nana_ui::runtime::SemanticColorRole::AccentSoft);
    } else if face.kind == ButtonKind::Danger {
        button.style.foreground = Some(nana_ui::runtime::SemanticColorRole::Danger);
    }
    button
}

fn action_button(
    row: ActionModel,
    rows: Signal<Vec<ActionModel>>,
    slots: &Mutex<HashMap<String, Arc<Mutex<PendingAction>>>>,
    sink: &Sink,
    target: &PendingTarget,
) -> nana_ui::runtime::view::El<Button> {
    let key = row.key.clone();
    let face = ButtonFace {
        label: row.label,
        kind: row.kind,
        disabled: row.disabled,
        visible: row.visible,
        action: row.action,
    };
    let lookup_key = key.clone();
    wire_button(
        key,
        face,
        move || rows.with(|rows| action_face(rows, &lookup_key)),
        slots,
        sink,
        target,
        ButtonShape::Action,
    )
}

struct ButtonFace {
    label: String,
    kind: ButtonKind,
    disabled: bool,
    visible: bool,
    action: PendingAction,
}

fn wire_button(
    key: String,
    face: ButtonFace,
    refresh: impl Fn() -> Option<ButtonFace> + Send + Sync + 'static,
    slots: &Mutex<HashMap<String, Arc<Mutex<PendingAction>>>>,
    sink: &Sink,
    target: &PendingTarget,
    shape: ButtonShape,
) -> nana_ui::runtime::view::El<Button> {
    let initial = match shape {
        ButtonShape::Choice => choice_button(&face),
        ButtonShape::Action => Button::new(face.label.clone())
            .kind(face.kind)
            .size(ControlSize::Small),
    };
    let slot = Arc::new(Mutex::new(face.action));
    if let Ok(mut slots) = slots.lock() {
        slots.insert(key, Arc::clone(&slot));
    }
    let refresh = Arc::new(refresh);
    let show = Arc::clone(&refresh);
    let press = Arc::clone(&slot);
    let sink = Arc::clone(sink);
    let target = target.clone();
    widget(initial)
        .disabled(face.disabled)
        .visible(move || (show.as_ref())().is_some_and(|face| face.visible))
        .bind(move |button| {
            let Some(face) = (refresh.as_ref())() else {
                return;
            };
            match shape {
                ButtonShape::Choice => *button = choice_button(&face),
                ButtonShape::Action => {
                    button.label = face.label.clone();
                    button.kind = face.kind;
                    button.disabled = face.disabled;
                    button.size = ControlSize::Small;
                }
            }
            if let Ok(mut action) = slot.lock() {
                *action = face.action;
            }
        })
        .on_activate(move || {
            if let Ok(action) = press.lock() {
                sink(target.clone(), action.clone());
            }
        })
}

fn field_signal(
    values: &Mutex<HashMap<String, Signal<String>>>,
    key: &str,
    value: String,
) -> Signal<String> {
    let mut values = values.lock().unwrap();
    if let Some(existing) = values.get(key).copied() {
        return existing;
    }
    let created = signal(value);
    values.insert(key.to_owned(), created);
    created
}

fn field_change(role: &FieldRole, draft: &Mutex<(String, String)>, value: String) -> PendingAction {
    match role {
        FieldRole::Draft => PendingAction::PendingDraftChanged { value },
        FieldRole::Ask => PendingAction::AskUserPending {
            action: "freeform".into(),
            value,
        },
        FieldRole::Command => {
            let mut draft = draft.lock().unwrap();
            draft.0 = value;
            PendingAction::ToolConsentDraftChanged {
                command: draft.0.clone(),
                message: draft.1.clone(),
            }
        }
        FieldRole::Message => {
            let mut draft = draft.lock().unwrap();
            draft.1 = value;
            PendingAction::ToolConsentDraftChanged {
                command: draft.0.clone(),
                message: draft.1.clone(),
            }
        }
        FieldRole::Raw => PendingAction::McpRawJsonChanged { value },
        FieldRole::Mcp(field_key) => PendingAction::McpFieldChanged {
            field_key: field_key.clone(),
            value,
        },
    }
}

fn field_label(rows: &[BodyModel], key: &str) -> Option<String> {
    rows.iter().find_map(|row| match &row.content {
        BodyContent::Field { label, .. } if row.key == key => Some(label.clone()),
        _ => None,
    })
}

fn row_shown(rows: &[BodyModel], key: &str) -> bool {
    rows.iter()
        .find(|row| row.key == key)
        .is_some_and(|row| row.visible)
}

fn button_face(rows: &[BodyModel], key: &str) -> Option<ButtonFace> {
    rows.iter().find(|row| row.key == key).and_then(|row| {
        let BodyContent::Button {
            label,
            kind,
            disabled,
            action,
        } = &row.content
        else {
            return None;
        };
        Some(ButtonFace {
            label: label.clone(),
            kind: *kind,
            disabled: *disabled,
            visible: row.visible,
            action: action.clone(),
        })
    })
}

fn action_face(rows: &[ActionModel], key: &str) -> Option<ButtonFace> {
    rows.iter()
        .find(|row| row.key == key)
        .map(|row| ButtonFace {
            label: row.label.clone(),
            kind: row.kind,
            disabled: row.disabled,
            visible: row.visible,
            action: row.action.clone(),
        })
}

fn tool_draft_of(pending: &PendingSnapshot) -> (String, String) {
    pending
        .tool
        .as_ref()
        .map(|tool| (tool.command.clone(), tool.message.clone()))
        .unwrap_or_default()
}

fn body_models(pending: &PendingSnapshot) -> Vec<BodyModel> {
    let mut rows = Vec::new();
    let request = &pending.request_id;
    match pending.kind {
        PendingKind::PlanApproval => rows.push(field_model(
            "draft",
            "补充说明",
            &pending.draft,
            true,
            FieldRole::Draft,
        )),
        PendingKind::AskUser => {
            if let Some(ask) = &pending.ask {
                rows.push(field_model(
                    "ask",
                    "补充说明",
                    &ask.freeform,
                    ask.show_freeform,
                    FieldRole::Ask,
                ));
            }
        }
        PendingKind::ToolConsent => {
            if let Some(tool) = &pending.tool {
                if tool.command_editable {
                    rows.push(field_model(
                        "command",
                        "确认执行的命令",
                        &tool.command,
                        true,
                        FieldRole::Command,
                    ));
                }
                rows.push(field_model(
                    "message",
                    "拒绝理由",
                    &tool.message,
                    true,
                    FieldRole::Message,
                ));
            }
        }
        PendingKind::McpElicitation => {
            if let Some(mcp) = &pending.mcp {
                if let Some(url) = &mcp.url {
                    rows.push(button_model(
                        format!("pending-mcp-url-{request}"),
                        "打开链接",
                        ButtonKind::Subtle,
                        PendingAction::OpenMarkdownLink(url.clone()),
                    ));
                }
                if let Some(raw) = &mcp.raw_json {
                    rows.push(field_model("raw", "原始 JSON", raw, true, FieldRole::Raw));
                }
                for field in &mcp.fields {
                    if field.options.is_empty() && field.kind != "boolean" {
                        rows.push(field_model(
                            format!("mcp-{}", field.key),
                            &field.label,
                            &field.value,
                            true,
                            FieldRole::Mcp(field.key.clone()),
                        ));
                    } else if field.kind == "boolean" {
                        rows.push(button_model(
                            format!("pending-mcp-bool-{request}-{}", field.key),
                            format!(
                                "{} · {}",
                                field.label,
                                if field.enabled {
                                    "已开启"
                                } else {
                                    "已关闭"
                                }
                            ),
                            if field.enabled {
                                ButtonKind::Primary
                            } else {
                                ButtonKind::Subtle
                            },
                            PendingAction::McpToggleBoolean {
                                field_key: field.key.clone(),
                            },
                        ));
                    } else {
                        for option in &field.options {
                            rows.push(button_model(
                                format!("pending-mcp-opt-{request}-{}-{}", field.key, option.value),
                                format!("{} · {}", field.label, option.label),
                                if option.selected {
                                    ButtonKind::Primary
                                } else {
                                    ButtonKind::Subtle
                                },
                                PendingAction::McpToggleOption {
                                    field_key: field.key.clone(),
                                    value: option.value.clone(),
                                    multi: option.multi,
                                },
                            ));
                        }
                    }
                }
            }
        }
        _ => {}
    }
    for option in &pending.options {
        rows.push(button_model(
            format!("pending-opt-{request}-{}", option.id),
            option.label.clone(),
            if option.selected {
                ButtonKind::Primary
            } else if option.danger {
                ButtonKind::Danger
            } else {
                ButtonKind::Subtle
            },
            PendingAction::SelectPendingOption {
                option_id: option.id.clone(),
            },
        ));
    }
    if let Some(ask) = &pending.ask {
        if ask.show_other {
            rows.push(button_model(
                format!("pending-ask-other-{request}"),
                if ask.other_selected {
                    "✓ 其他".to_owned()
                } else {
                    "其他".to_owned()
                },
                if ask.other_selected {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Subtle
                },
                PendingAction::AskUserPending {
                    action: "select".into(),
                    value: "other".into(),
                },
            ));
        }
    }
    rows
}

fn action_models(pending: &PendingSnapshot) -> Vec<ActionModel> {
    action_specs(pending)
        .into_iter()
        .map(|(key, label, kind, action, disabled)| ActionModel {
            key,
            label,
            kind,
            disabled,
            visible: true,
            action,
        })
        .collect()
}

fn field_model(
    key: impl Into<String>,
    label: impl Into<String>,
    value: &str,
    visible: bool,
    role: FieldRole,
) -> BodyModel {
    BodyModel {
        key: key.into(),
        visible,
        content: BodyContent::Field {
            label: label.into(),
            value: value.to_owned(),
            role,
        },
    }
}

fn button_model(
    key: impl Into<String>,
    label: impl Into<String>,
    kind: ButtonKind,
    action: PendingAction,
) -> BodyModel {
    BodyModel {
        key: key.into(),
        visible: true,
        content: BodyContent::Button {
            label: label.into(),
            kind,
            disabled: false,
            action,
        },
    }
}

fn keep_hidden(previous: &[BodyModel], mut current: Vec<BodyModel>) -> Vec<BodyModel> {
    let seen = current
        .iter()
        .map(|row| row.key.clone())
        .collect::<HashSet<_>>();
    for old in previous {
        if !seen.contains(&old.key) {
            let mut hidden = old.clone();
            hidden.visible = false;
            current.push(hidden);
        }
    }
    current
}

fn keep_hidden_actions(
    previous: &[ActionModel],
    mut current: Vec<ActionModel>,
) -> Vec<ActionModel> {
    let seen = current
        .iter()
        .map(|row| row.key.clone())
        .collect::<HashSet<_>>();
    for old in previous {
        if !seen.contains(&old.key) {
            let mut hidden = old.clone();
            hidden.visible = false;
            current.push(hidden);
        }
    }
    current
}

fn action_specs(
    pending: &PendingSnapshot,
) -> Vec<(String, String, ButtonKind, PendingAction, bool)> {
    let request_id = pending.request_id.clone();
    match pending.kind {
        PendingKind::PermissionApproval => vec![
            (
                format!("pending-approve-{request_id}"),
                "允许".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondApproval { approved: true },
                false,
            ),
            (
                format!("pending-reject-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondApproval { approved: false },
                false,
            ),
        ],
        PendingKind::PlanApproval => vec![
            (
                format!("pending-plan-approve-{request_id}"),
                "执行计划".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondPlan {
                    action: "approve".to_owned(),
                },
                false,
            ),
            (
                format!("pending-plan-revise-{request_id}"),
                "要求修改".to_owned(),
                ButtonKind::Subtle,
                PendingAction::RespondPlan {
                    action: "revise".to_owned(),
                },
                pending.draft.trim().is_empty(),
            ),
            (
                format!("pending-plan-decline-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondPlan {
                    action: "decline".to_owned(),
                },
                false,
            ),
            (
                format!("pending-plan-interrupt-{request_id}"),
                "取消任务".to_owned(),
                ButtonKind::Subtle,
                PendingAction::InterruptTurn(pending.stop_target.clone()),
                pending.stop_target.is_none(),
            ),
        ],
        PendingKind::ToolConsent => {
            let tool = pending.tool.as_ref();
            vec![
                (
                    format!("pending-consent-allow-{request_id}"),
                    "允许".to_owned(),
                    ButtonKind::Primary,
                    PendingAction::RespondToolConsent { approved: true },
                    tool.is_some_and(|tool| !tool.can_allow),
                ),
                (
                    format!("pending-consent-deny-{request_id}"),
                    "拒绝".to_owned(),
                    ButtonKind::Danger,
                    PendingAction::RespondToolConsent { approved: false },
                    tool.is_some_and(|tool| !tool.can_deny),
                ),
            ]
        }
        PendingKind::AskUser => {
            let ask = pending.ask.as_ref();
            let mut actions = Vec::new();
            if ask.is_some_and(|ask| ask.show_skip) {
                actions.push((
                    format!("pending-ask-skip-{request_id}"),
                    "跳过".to_owned(),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "skip".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            if ask.is_some_and(|ask| ask.show_back) {
                actions.push((
                    format!("pending-ask-back-{request_id}"),
                    "上一题".to_owned(),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "back".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            if ask.is_some_and(|ask| ask.show_cancel) {
                actions.push((
                    format!("pending-ask-cancel-{request_id}"),
                    "关闭".to_owned(),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "cancel".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            if ask.is_some_and(|ask| ask.show_reject) {
                actions.push((
                    format!("pending-ask-reject-{request_id}"),
                    ask.map(|ask| ask.reject_label.clone())
                        .unwrap_or_else(|| "不要".to_owned()),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "reject".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            actions.push((
                format!("pending-ask-submit-{request_id}"),
                ask.map(|ask| ask.submit_label.clone())
                    .unwrap_or_else(|| "提交".to_owned()),
                ButtonKind::Primary,
                PendingAction::AskUserPending {
                    action: "submit".to_owned(),
                    value: String::new(),
                },
                ask.is_some_and(|ask| !ask.can_submit),
            ));
            actions
        }
        PendingKind::ArchitectureChange => vec![
            (
                format!("pending-arch-allow-{request_id}"),
                "允许".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondArchitecture { approved: true },
                false,
            ),
            (
                format!("pending-arch-deny-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondArchitecture { approved: false },
                false,
            ),
        ],
        PendingKind::TitleUpdate => vec![
            (
                format!("pending-title-accept-{request_id}"),
                "采用".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondTitle { accepted: true },
                false,
            ),
            (
                format!("pending-title-reject-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondTitle { accepted: false },
                false,
            ),
        ],
        PendingKind::McpElicitation => vec![
            (
                format!("pending-mcp-accept-{request_id}"),
                "接受".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondMcp {
                    action: "accept".to_owned(),
                },
                pending.mcp.as_ref().is_some_and(|mcp| !mcp.can_accept),
            ),
            (
                format!("pending-mcp-decline-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Subtle,
                PendingAction::RespondMcp {
                    action: "decline".to_owned(),
                },
                false,
            ),
            (
                format!("pending-mcp-cancel-{request_id}"),
                "取消".to_owned(),
                ButtonKind::Subtle,
                PendingAction::RespondMcp {
                    action: "cancel".to_owned(),
                },
                false,
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::{MutationQueue, TextSelection};

    fn pending(kind: PendingKind) -> PendingSnapshot {
        PendingSnapshot {
            stop_target: Some(TurnStopTarget {
                task_id: TaskId::new("task-a").unwrap(),
                turn_id: "turn-a".into(),
            }),
            request_id: "request-a".into(),
            kind,
            title: "处理请求".into(),
            prompt: "请选择".into(),
            draft: "draft".into(),
            options: vec![PendingOption {
                id: "one".into(),
                label: "One".into(),
                selected: false,
                danger: false,
            }],
            tool: Some(ToolConsentPending {
                command: "echo one".into(),
                message: String::new(),
                command_editable: true,
                can_allow: true,
                can_deny: true,
            }),
            ask: Some(AskUserPending {
                show_other: true,
                other_selected: true,
                freeform: "my answer".into(),
                show_freeform: true,
                show_skip: true,
                show_back: true,
                show_cancel: true,
                show_reject: true,
                can_submit: true,
                submit_label: "回答".into(),
                reject_label: "拒绝".into(),
            }),
            mcp: Some(McpPending {
                url: Some("https://example.test".into()),
                raw_json: Some("{}".into()),
                can_accept: true,
                fields: vec![
                    McpField {
                        key: "name".into(),
                        label: "名称".into(),
                        kind: "string".into(),
                        value: "value".into(),
                        enabled: true,
                        options: vec![],
                    },
                    McpField {
                        key: "enabled".into(),
                        label: "开启".into(),
                        kind: "boolean".into(),
                        value: "true".into(),
                        enabled: true,
                        options: vec![],
                    },
                    McpField {
                        key: "choice".into(),
                        label: "选项".into(),
                        kind: "string".into(),
                        value: String::new(),
                        enabled: true,
                        options: vec![McpFieldOption {
                            value: "v".into(),
                            label: "V".into(),
                            selected: false,
                            multi: true,
                        }],
                    },
                ],
            }),
        }
    }

    #[test]
    fn long_approval_text_wraps_above_actions_in_a_narrow_card() {
        use nana_ui::runtime::{LayoutViewport, RuntimeDocument};
        let document_id = DocumentId::new(643).unwrap();
        let mut document = RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, nana_ui::runtime::Stack::column(0.0))
            .unwrap();
        let mut view = PendingView::mount(context, document_id, Arc::new(|_, _| {})).unwrap();
        context.append_child(root, view.root).unwrap();
        let mut snapshot = pending(PendingKind::PermissionApproval);
        snapshot.options.clear();
        snapshot.ask = None;
        snapshot.prompt =
            "请确认是否允许在当前任务的浏览器中执行操作，完成后会返回页面的最新状态。".repeat(3);
        view.sync(
            context,
            document_id,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        let request = view.request.as_ref().unwrap();
        let prompt = request.prompt.stable_id();
        let actions = request.actions.stable_id();
        document
            .flush(
                LayoutViewport::new(360.0, 800.0),
                &mut nana_ui::NanaTextShaper::default(),
            )
            .unwrap();
        let text = document.scene().node_bounds(prompt).unwrap();
        let buttons = document.scene().node_bounds(actions).unwrap();
        let card = document.scene().node_bounds(view.root.stable_id()).unwrap();
        assert!(
            text.height >= 60.0,
            "long prompt must occupy multiple lines: {text:?}"
        );
        assert!(
            text.x > card.x && text.x + text.width < card.x + card.width,
            "text stays within the padded card: {text:?}, {card:?}"
        );
        assert!(
            buttons.y >= text.y + text.height + 7.0,
            "actions must clear wrapped text: {text:?}, {buttons:?}"
        );
    }

    #[test]
    fn all_seven_forms_share_window_addressed_actions_and_dispose_previous_requests() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let forms = [
            PendingKind::PermissionApproval,
            PendingKind::PlanApproval,
            PendingKind::AskUser,
            PendingKind::ToolConsent,
            PendingKind::McpElicitation,
            PendingKind::ArchitectureChange,
            PendingKind::TitleUpdate,
        ];
        for (number, window) in [(641, WindowId::PRIMARY), (642, WindowId(19))] {
            let document = DocumentId::new(number).unwrap();
            let host = context
                .create_component(document, Stack::fill_column(0.0))
                .unwrap();
            let received = Arc::clone(&events);
            let mut view = PendingView::mount(
                &mut context,
                document,
                Arc::new(move |target, action| received.lock().unwrap().push((target, action))),
            )
            .unwrap();
            context.append_child(host, view.root).unwrap();
            for kind in forms {
                let snapshot = pending(kind);
                view.sync(
                    &mut context,
                    document,
                    window,
                    Some("task-a".into()),
                    Some(&snapshot),
                )
                .unwrap();
                let request = view.request.as_ref().unwrap();
                let key = action_specs(&snapshot)[0].0.clone();
                let button = request.buttons[&key].node;
                context
                    .update_component(button, |_, cx| cx.emit(Activate))
                    .unwrap();
                let (target, action) = events.lock().unwrap().last().unwrap().clone();
                assert_eq!(target, request.target);
                assert_eq!(action, action_specs(&snapshot)[0].3);
                let mut another = target.clone();
                another.window_id = if window == WindowId::PRIMARY {
                    WindowId(19)
                } else {
                    WindowId::PRIMARY
                };
                assert!(!target.matches(&another));
                another = target.clone();
                another.request_id = "request-b".into();
                assert!(!target.matches(&another));
                another = target.clone();
                another.task_id = Some("task-b".into());
                assert!(!target.matches(&another));
                another.task_id = None;
                assert!(!target.matches(&another));
                let nodes: Vec<_> = request
                    .fields
                    .values()
                    .flat_map(|f| [f.editor.stable_id(), f.wrapper.stable_id()])
                    .chain(request.buttons.values().map(|b| b.node.stable_id()))
                    .chain([
                        request.title.stable_id(),
                        request.prompt.stable_id(),
                        request.actions.stable_id(),
                    ])
                    .collect();
                view.sync(&mut context, document, window, Some("task-a".into()), None)
                    .unwrap();
                for node in nodes {
                    assert!(
                        !context.world().contains(node),
                        "leaked pending node {node:?}"
                    );
                }
            }
        }
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 14);
        for i in 0..7 {
            assert_eq!(events[i].1, events[i + 7].1);
        }
    }

    #[test]
    fn same_request_preserves_draft_selection_and_focus_through_parking() {
        let mut context = AppContext::new();
        let document = DocumentId::new(643).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut view = PendingView::mount(
            &mut context,
            document,
            Arc::new(move |target, action| received.lock().unwrap().push((target, action))),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = pending(PendingKind::AskUser);
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        let editor = view.request.as_ref().unwrap().fields["ask"].editor;
        let selection = TextSelection {
            anchor: 2,
            focus: 5,
            affinity: Default::default(),
        };
        context
            .update_component(editor, |input, cx| {
                input.state.selection = selection;
                cx.emit(TextChanged {
                    value: "my answer".into(),
                    selection,
                });
            })
            .unwrap();
        let old_event = events.lock().unwrap()[0].clone();
        context.focus_node(document, editor.stable_id()).unwrap();
        let mut changes = MutationQueue::new();
        changes.park_subtree(view.root.stable_id());
        context.commit_mutations(changes).unwrap();
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        view.restore_focus(&mut context, document).unwrap();
        assert_eq!(context.world().focused(document), Some(editor.stable_id()));
        assert_eq!(
            context
                .read(editor, |input| (
                    input.state.value.clone(),
                    input.state.selection
                ))
                .unwrap(),
            ("my answer".into(), selection)
        );
        snapshot.ask.as_mut().unwrap().show_freeform = false;
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        assert!(context.world().contains(editor.stable_id()));
        snapshot.ask.as_mut().unwrap().show_freeform = true;
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        view.restore_focus(&mut context, document).unwrap();
        assert_eq!(context.world().focused(document), Some(editor.stable_id()));
        assert_eq!(view.request.as_ref().unwrap().fields["ask"].editor, editor);
        snapshot.request_id = "request-b".into();
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        assert!(!context.world().contains(editor.stable_id()));
        assert!(!old_event.0.matches(&view.request.as_ref().unwrap().target));
    }

    #[test]
    fn tool_fields_merge_current_values_and_stop_keeps_the_displayed_turn() {
        let mut context = AppContext::new();
        let document = DocumentId::new(644).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut view = PendingView::mount(
            &mut context,
            document,
            Arc::new(move |target, action| received.lock().unwrap().push((target, action))),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let snapshot = pending(PendingKind::ToolConsent);
        view.sync(
            &mut context,
            document,
            WindowId(19),
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        for (key, value) in [("command", "echo changed"), ("message", "reason")] {
            let editor = view.request.as_ref().unwrap().fields[key].editor;
            context
                .update_component(editor, |_, cx| {
                    cx.emit(TextChanged {
                        value: value.into(),
                        selection: TextSelection::default(),
                    })
                })
                .unwrap();
        }
        assert_eq!(
            events.lock().unwrap().last().unwrap().1,
            PendingAction::ToolConsentDraftChanged {
                command: "echo changed".into(),
                message: "reason".into()
            }
        );
        let mut snapshot = pending(PendingKind::PlanApproval);
        view.sync(
            &mut context,
            document,
            WindowId(19),
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        let button =
            view.request.as_ref().unwrap().buttons["pending-plan-interrupt-request-a"].node;
        context
            .update_component(button, |_, cx| cx.emit(Activate))
            .unwrap();
        let old = events.lock().unwrap().last().unwrap().clone();
        snapshot.stop_target.as_mut().unwrap().turn_id = "turn-b".into();
        view.sync(
            &mut context,
            document,
            WindowId(19),
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        context
            .update_component(button, |_, cx| cx.emit(Activate))
            .unwrap();
        assert_eq!(
            old.1,
            PendingAction::InterruptTurn(Some(TurnStopTarget {
                task_id: TaskId::new("task-a").unwrap(),
                turn_id: "turn-a".into()
            }))
        );
        assert_eq!(
            events.lock().unwrap().last().unwrap().1,
            PendingAction::InterruptTurn(snapshot.stop_target)
        );
    }
}
