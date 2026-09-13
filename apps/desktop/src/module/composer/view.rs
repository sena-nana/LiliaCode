use super::presentation::{
    COMPOSER_PERMISSION_OPTIONS, COMPOSER_REASONING_OPTIONS, COMPOSER_REVIEW_OPTIONS,
    COMPOSER_WORKTREE_OPTIONS, ComposerAttachment, ComposerMentionItem, ComposerSlashItem,
    ComposerSuggestion,
};
use crate::runtime_compat::HostedWindowId;
use crate::runtime_layout::{
    composer_card, composer_interrupt_button, composer_send_button, flatten_composer_textarea,
    reconcile_children, trigger_slot,
};
use crate::runtime_shell::{IntentSink, ShellIntent, emit};
use nana_ui::runtime::{
    ActionMenu, ActionMenuItem, Activate, AppContext, Button, Card, DocumentId, Dropdown,
    DropdownOption, Entity, FrameworkError, IconButton, IconGlyph, JustifySpec, KeyInput,
    LengthSpec, PopoverToggled, Stack, TextArea, TextAtomSpan, TextChanged, TextInput, View,
};
use nana_ui::{
    ButtonKind, ControlSize, DropdownEvent, DropdownSelection, Icon, PopoverPlacement, UI_METRICS,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
const PLUS_SLOT_SIZE: f32 = UI_METRICS.icon_button_size;
const COMPOSER_MIN_HEIGHT: f32 = UI_METRICS.control_height;
const COMPOSER_MAX_HEIGHT: f32 = 72.0;
fn extra_button(label: &str, kind: ButtonKind) -> Button {
    crate::runtime_layout::pill_button(label, kind)
}

#[derive(Clone, Debug)]
pub struct ComposerViewSnapshot {
    pub window_id: HostedWindowId,
    pub composer: String,
    pub composer_atom_spans: Vec<lilia_feature_composer::ContentAtomSpan>,
    pub composer_task_id: Option<String>,
    pub composer_revision: u64,
    pub composer_height: f32,
    pub composer_placeholder: String,
    pub composer_disabled: bool,
    pub can_send: bool,
    pub can_interrupt: bool,
    pub composer_turn_id: Option<String>,
    pub pending_blocks_send: bool,
    pub attachments: Vec<ComposerAttachment>,
    pub plan_mode: bool,
    pub goal_mode: bool,
    pub permission_label: String,
    pub permission_selection: String,
    pub reasoning: String,
    pub model: String,
    pub model_label: String,
    pub models: Vec<(String, String)>,
    pub worktree_label: Option<String>,
    pub worktree_selection: String,
    pub suggestions: Vec<ComposerSuggestion>,
    pub suggestions_can_refresh: bool,
    pub slash_items: Vec<ComposerSlashItem>,
    pub mention_items: Vec<ComposerMentionItem>,
    pub reference_items: Vec<ComposerMentionItem>,
    pub composer_plus_open: bool,
    pub composer_permission_menu_open: bool,
    pub composer_worktree_menu_open: bool,
    pub can_open_browser: bool,
    pub branch_label: Option<String>,
    pub review_target: Option<String>,
    pub review_value: String,
    pub can_manage_todos: bool,
    pub apply_failed: bool,
}
impl Default for ComposerViewSnapshot {
    fn default() -> Self {
        Self {
            window_id: HostedWindowId::PRIMARY,
            composer: Default::default(),
            composer_atom_spans: Default::default(),
            composer_task_id: Default::default(),
            composer_revision: Default::default(),
            composer_height: Default::default(),
            composer_placeholder: Default::default(),
            composer_disabled: Default::default(),
            can_send: Default::default(),
            can_interrupt: Default::default(),
            composer_turn_id: Default::default(),
            pending_blocks_send: Default::default(),
            attachments: Default::default(),
            plan_mode: Default::default(),
            goal_mode: Default::default(),
            permission_label: Default::default(),
            permission_selection: Default::default(),
            reasoning: "medium".into(),
            model: Default::default(),
            model_label: "自动选择".into(),
            models: vec![
                (String::new(), "自动选择".into()),
                ("native-debug".into(), "Native Debug".into()),
            ],
            worktree_label: Default::default(),
            worktree_selection: Default::default(),
            suggestions: Default::default(),
            suggestions_can_refresh: Default::default(),
            slash_items: Default::default(),
            mention_items: Default::default(),
            reference_items: Default::default(),
            composer_plus_open: Default::default(),
            composer_permission_menu_open: Default::default(),
            composer_worktree_menu_open: Default::default(),
            can_open_browser: Default::default(),
            branch_label: Default::default(),
            review_target: Default::default(),
            review_value: Default::default(),
            can_manage_todos: Default::default(),
            apply_failed: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ComposerGeneration {
    task_id: Option<String>,
    revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerTarget {
    pub turn_id: Option<String>,
    pub window_id: HostedWindowId,
    pub task_id: Option<String>,
    pub revision: u64,
    pub content: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerMenuKind {
    Actions,
    Permission,
    Worktree,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerInputAction {
    SetContent {
        value: String,
        expected_content: String,
    },
    Submit,
    Interrupt,
    ToggleActions,
    TogglePermission,
    ToggleWorktree,
    Plus(String),
    Permission(String),
    Worktree(String),
    RefreshSuggestions,
    ApplySuggestion(String),
    RemoveAttachment(String),
    ApplySlash(String),
    SelectMention(String),
    SelectConversationReference(String),
    Reasoning(String),
    Model(String),
    ClearBranch,
    ReviewTarget(String),
    ReviewValue(String),
    SubmitReview,
    CancelReview,
}

#[derive(Default)]
struct ComposerKeyState {
    content: String,
    candidates: Vec<ComposerInputAction>,
    active: usize,
    can_send: bool,
    locked: bool,
    pending_blocks_send: bool,
    submitted: bool,
}

pub(crate) struct ComposerBinding {
    pub(crate) target: ComposerTarget,
    key: ComposerKeyState,
}

pub(crate) fn mount_task_browser_button(
    context: &mut AppContext,
    document: DocumentId,
    binding: &Arc<Mutex<ComposerBinding>>,
    sink: &IntentSink,
    can_open_browser: bool,
) -> Result<Entity<Button>, FrameworkError> {
    let mut button = extra_button("浏览器", ButtonKind::Subtle);
    button.disabled = !can_open_browser;
    let button = context.create_detached_component(document, button)?;
    let binding = Arc::clone(binding);
    let sink = Arc::clone(sink);
    context.on(button, move |_, _: &Activate, _| {
        let target = binding.lock().unwrap().target.clone();
        if let Some(task_id) = target.task_id {
            emit(
                &sink,
                ShellIntent::OpenBrowser {
                    window_id: target.window_id,
                    task_id,
                },
            );
        }
    })?;
    Ok(button)
}

impl ComposerBinding {
    pub(crate) fn new(
        window_id: HostedWindowId,
        task_id: Option<String>,
        revision: u64,
        content: String,
        turn_id: Option<String>,
    ) -> Self {
        Self {
            target: ComposerTarget {
                turn_id,
                window_id,
                task_id,
                revision,
                content,
            },
            key: ComposerKeyState::default(),
        }
    }

    fn sync_keys(
        &mut self,
        content: &str,
        can_send: bool,
        locked: bool,
        pending_blocks_send: bool,
        candidates: Vec<ComposerInputAction>,
    ) {
        if self.key.content != content || self.key.candidates != candidates {
            self.key.active = 0;
            self.key.submitted = false;
        }
        self.key.content = content.to_owned();
        self.key.can_send = can_send;
        self.key.locked = locked;
        self.key.pending_blocks_send = pending_blocks_send;
        self.key.candidates = candidates;
    }

    pub(crate) fn edit(&mut self, value: String) -> Option<ShellIntent> {
        if self.target.content == value {
            return None;
        }
        let target = self.target.clone();
        let expected_content = self.target.content.clone();
        self.target.revision = self.target.revision.checked_add(1)?;
        self.target.content = value.clone();
        Some(ShellIntent::AddressedComposer {
            target,
            action: ComposerInputAction::SetContent {
                value,
                expected_content,
            },
        })
    }
}

fn bind_action<V: View>(
    context: &mut AppContext,
    entity: Entity<V>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
    action: ComposerInputAction,
) -> Result<(), FrameworkError> {
    context.on(entity, move |_, _: &Activate, _| {
        let target = binding.lock().unwrap().target.clone();
        emit(
            &sink,
            ShellIntent::AddressedComposer {
                target,
                action: action.clone(),
            },
        );
    })
}

pub(crate) fn bind_composer_input(
    context: &mut AppContext,
    editor: Entity<TextArea>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
) -> Result<(), FrameworkError> {
    context.on(editor, move |_, event: &TextChanged, _| {
        let intent = binding.lock().unwrap().edit(event.value.clone());
        if let Some(intent) = intent {
            emit(&sink, intent);
        }
    })
}

pub(crate) fn bind_composer_keys(
    context: &mut AppContext,
    editor: Entity<TextArea>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
) -> Result<(), FrameworkError> {
    context.on_view_key(editor, move |_, key: &KeyInput| {
        if !key.pressed
            || key.modifiers.shift
            || key.modifiers.control
            || key.modifiers.meta
            || key.modifiers.alt
        {
            return false;
        }
        let mut binding = binding.lock().unwrap();
        let action = match key.key.as_ref() {
            "ArrowDown" | "ArrowUp"
                if !binding.key.candidates.is_empty() && !binding.key.locked =>
            {
                let count = binding.key.candidates.len();
                binding.key.active = if key.key.as_ref() == "ArrowDown" {
                    (binding.key.active + 1) % count
                } else {
                    (binding.key.active + count - 1) % count
                };
                return true;
            }
            "Enter" | "Tab" if !binding.key.candidates.is_empty() => {
                if key.repeat || binding.key.locked || binding.key.submitted {
                    return true;
                }
                let Some(action) = binding.key.candidates.get(binding.key.active).cloned() else {
                    return false;
                };
                binding.key.submitted = true;
                Some(action)
            }
            "Enter" => {
                if key.repeat
                    || binding.key.submitted
                    || !binding.key.can_send
                    || binding.key.locked
                    || binding.key.pending_blocks_send
                {
                    return true;
                }
                binding.key.submitted = true;
                Some(ComposerInputAction::Submit)
            }
            _ => None,
        };
        let Some(action) = action else {
            return false;
        };
        let target = binding.target.clone();
        drop(binding);
        emit(&sink, ShellIntent::AddressedComposer { target, action });
        true
    })
}

pub(crate) fn bind_composer_button(
    context: &mut AppContext,
    button: Entity<IconButton>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
    action: ComposerInputAction,
) -> Result<(), FrameworkError> {
    context.on(button, move |_, _: &Activate, _| {
        let target = binding.lock().unwrap().target.clone();
        emit(
            &sink,
            ShellIntent::AddressedComposer {
                target,
                action: action.clone(),
            },
        );
    })
}

impl ComposerGeneration {
    pub fn new(task_id: Option<String>, revision: u64) -> Self {
        Self { task_id, revision }
    }
}

pub(crate) fn composer_is_focused(context: &AppContext, composer: Entity<TextArea>) -> bool {
    context
        .world()
        .node(composer.stable_id())
        .is_some_and(|node| context.world().focused(node.document) == Some(composer.stable_id()))
}

fn composer_completion_entries(
    snapshot: &ComposerViewSnapshot,
) -> Vec<(String, String, ComposerInputAction)> {
    snapshot
        .slash_items
        .iter()
        .map(|item| {
            (
                format!("slash-{}", item.name),
                item.label.clone(),
                ComposerInputAction::ApplySlash(item.name.clone()),
            )
        })
        .chain(snapshot.mention_items.iter().map(|item| {
            (
                format!("mention-{}", item.id),
                item.label.clone(),
                ComposerInputAction::SelectMention(item.id.clone()),
            )
        }))
        .chain(snapshot.reference_items.iter().map(|item| {
            (
                format!("reference-result-{}", item.id),
                item.label.clone(),
                ComposerInputAction::SelectConversationReference(item.id.clone()),
            )
        }))
        .collect()
}

fn composer_view(snapshot: &ComposerViewSnapshot) -> TextArea {
    flatten_composer_textarea(
        TextArea::new(snapshot.composer.clone())
            .atom_spans(composer_atom_chips(&snapshot.composer_atom_spans))
            .placeholder(snapshot.composer_placeholder.clone())
            .disabled(snapshot.composer_disabled)
            .height(
                snapshot
                    .composer_height
                    .clamp(COMPOSER_MIN_HEIGHT, COMPOSER_MAX_HEIGHT),
            ),
    )
}

pub(crate) fn composer_atom_chips(
    spans: &[lilia_feature_composer::ContentAtomSpan],
) -> Arc<[TextAtomSpan]> {
    spans
        .iter()
        .map(|span| {
            TextAtomSpan::new(span.start, span.end)
                .label(span.label.as_str())
                .token(span.token.as_str())
                .icon(match span.kind {
                    lilia_feature_composer::ContentAtomKind::Directory => Icon::Folder,
                    lilia_feature_composer::ContentAtomKind::Conversation => {
                        Icon::MessageSquarePlus
                    }
                    lilia_feature_composer::ContentAtomKind::File
                    | lilia_feature_composer::ContentAtomKind::Image => Icon::File,
                })
        })
        .collect()
}

fn composer_plus_menu(open: bool) -> ActionMenu {
    ActionMenu::new().trigger_icon(Icon::Add, "添加").open(open)
}

/// 输入条贴着窗口底部，菜单必须向上展开。
fn composer_menu(label: &str, open: bool) -> ActionMenu {
    ActionMenu::new()
        .trigger(label.to_owned())
        .placement(PopoverPlacement::Top)
        .open(open)
}

fn composer_model_dropdown(snapshot: &ComposerViewSnapshot) -> Dropdown {
    let mut field = Dropdown::single(Some(snapshot.model.clone()))
        .size(ControlSize::Small)
        .options(snapshot.models.iter().map(|(id, label)| {
            DropdownOption::new(
                id.clone(),
                if id.is_empty() {
                    snapshot.model_label.clone()
                } else {
                    label.clone()
                },
            )
        }));
    field.disabled = snapshot.composer_disabled;
    let width = if snapshot.window_id == HostedWindowId::PRIMARY {
        204.0
    } else {
        150.0
    };
    let layout = Arc::make_mut(&mut field.style.layout);
    layout.width = Some(LengthSpec::Px(width));
    layout.min_width = Some(LengthSpec::Px(width));
    layout.border_width = Some(0.0);
    field.style.border = None;
    field.style.background = None;
    field
}

fn composer_review_dropdown(selected: &str) -> Dropdown {
    let mut field = Dropdown::single(Some(selected.to_owned()))
        .size(ControlSize::Small)
        .options(
            COMPOSER_REVIEW_OPTIONS
                .iter()
                .map(|(id, label)| DropdownOption::new(*id, *label)),
        );
    let layout = Arc::make_mut(&mut field.style.layout);
    layout.width = Some(LengthSpec::Px(148.0));
    layout.min_width = Some(LengthSpec::Px(148.0));
    field
}

fn composer_review_value(value: &str, placeholder: &str) -> TextInput {
    let mut field = TextInput::new(value.to_owned()).placeholder(placeholder.to_owned());
    let layout = Arc::make_mut(&mut field.style.layout);
    layout.width = Some(LengthSpec::Px(160.0));
    layout.min_width = Some(LengthSpec::Px(120.0));
    layout.height = Some(LengthSpec::Px(UI_METRICS.compact_control_height));
    field
}

fn review_value_placeholder(target: &str) -> &'static str {
    if target == "branch" {
        "分支名称"
    } else {
        "提交 SHA"
    }
}

fn composer_reasoning_dropdown(selected: &str, disabled: bool) -> Dropdown {
    let mut field = Dropdown::single(Some(selected.to_owned()))
        .size(ControlSize::Small)
        .options(
            COMPOSER_REASONING_OPTIONS
                .iter()
                .map(|(id, label)| DropdownOption::new(*id, *label)),
        );
    field.disabled = disabled;
    let layout = Arc::make_mut(&mut field.style.layout);
    layout.width = Some(LengthSpec::Px(74.0));
    layout.min_width = Some(LengthSpec::Px(74.0));
    layout.border_width = Some(0.0);
    field.style.border = None;
    field.style.background = None;
    field
}

fn sync_action_menu_items(
    context: &mut AppContext,
    document_id: DocumentId,
    menu: Entity<ActionMenu>,
    open: bool,
    entries: &[(String, String, bool)],
    items: &mut HashMap<String, Entity<ActionMenuItem>>,
    sink: &IntentSink,
    binding: &Arc<Mutex<ComposerBinding>>,
    intent: impl Fn(&str) -> ComposerInputAction,
) -> Result<(), FrameworkError> {
    let mut order = Vec::new();
    if open {
        for (id, label, active) in entries {
            let item = if let Some(item) = items.get(id).copied() {
                context.update_component(item, |view, _| {
                    *view = action_menu_item(label, *active);
                })?;
                item
            } else {
                let item = context
                    .create_detached_component(document_id, action_menu_item(label, *active))?;
                bind_action(
                    context,
                    item,
                    Arc::clone(sink),
                    Arc::clone(binding),
                    intent(id),
                )?;
                items.insert(id.clone(), item);
                item
            };
            order.push(item.stable_id());
        }
        let keep: HashSet<&String> = entries.iter().map(|(id, _, _)| id).collect();
        items.retain(|key, item| {
            if keep.contains(key) {
                true
            } else {
                let _ = context.remove_view(*item);
                false
            }
        });
    } else {
        for (_, item) in items.drain() {
            let _ = context.remove_view(item);
        }
    }
    reconcile_children(context, menu.stable_id(), &order)
}

fn action_menu_item(label: &str, active: bool) -> ActionMenuItem {
    let mut item = ActionMenuItem::new(label.to_owned());
    if active {
        item = item.active(true);
    }
    item
}

fn composer_attach_button() -> IconButton {
    IconButton::new(Icon::Paperclip, "添加文件")
        .kind(ButtonKind::Text)
        .size(ControlSize::Small)
}

fn plus_menu_items(snapshot: &ComposerViewSnapshot) -> Vec<(String, String)> {
    let mut items = vec![
        ("add-file".into(), "添加文件".into()),
        ("add-directory".into(), "添加目录".into()),
        ("reference".into(), "引用其他对话".into()),
        ("paste-text".into(), "粘贴文字".into()),
        ("paste-image".into(), "粘贴图片".into()),
        ("paste-files".into(), "粘贴文件".into()),
        (
            "plan".into(),
            if snapshot.plan_mode {
                "关闭计划模式".into()
            } else {
                "开启计划模式".into()
            },
        ),
        (
            "goal".into(),
            if snapshot.goal_mode {
                "关闭目标模式".into()
            } else {
                "开启目标模式".into()
            },
        ),
    ];
    if snapshot.can_manage_todos {
        items.push(("new-guide".into(), "添加引导".into()));
        items.push(("edit-goal".into(), "设置目标".into()));
    }
    items
}

pub struct ComposerView {
    sink: IntentSink,
    extra_signatures: HashMap<String, (String, ComposerInputAction)>,
    pub(crate) composer_generation: ComposerGeneration,
    pub(crate) composer_binding: Arc<Mutex<ComposerBinding>>,
    pub(crate) browser_open: Entity<Button>,
    pub(crate) composer_dock: Entity<Card>,
    pub(crate) composer: Entity<TextArea>,
    pub(crate) composer_toolbar: Entity<Stack>,
    pub(crate) extras: Entity<Stack>,
    pub(crate) extra_buttons: HashMap<String, Entity<Button>>,
    pub(crate) completion_slot: Entity<Stack>,
    pub(crate) completion_items: HashMap<String, Entity<ActionMenuItem>>,
    pub(crate) plus_slot: Entity<Stack>,
    pub(crate) plus_menu: Entity<ActionMenu>,
    pub(crate) plus_items: HashMap<String, Entity<ActionMenuItem>>,
    pub(crate) attach: Entity<IconButton>,
    pub(crate) permission_slot: Entity<Stack>,
    #[cfg(test)]
    pub(crate) permission_icon: Entity<IconGlyph>,
    pub(crate) permission_menu: Entity<ActionMenu>,
    pub(crate) permission_items: HashMap<String, Entity<ActionMenuItem>>,
    pub(crate) model: Entity<Dropdown>,
    pub(crate) reasoning: Entity<Dropdown>,
    pub(crate) worktree_slot: Entity<Stack>,
    pub(crate) worktree_menu: Entity<ActionMenu>,
    pub(crate) worktree_items: HashMap<String, Entity<ActionMenuItem>>,
    pub(crate) review_slot: Entity<Stack>,
    pub(crate) review_target: Entity<Dropdown>,
    pub(crate) review_value: Entity<TextInput>,
    pub(crate) review_submit: Entity<Button>,
    pub(crate) review_cancel: Entity<Button>,
    pub(crate) composer_actions: Entity<Stack>,
    pub(crate) send: Entity<IconButton>,
    pub(crate) interrupt: Option<Entity<IconButton>>,
    last_failed_revision: Option<u64>,
}
impl ComposerView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        let composer_dock = context.create_detached_component(document_id, composer_card())?;
        let composer = context.create_detached_component(document_id, composer_view(snapshot))?;
        let composer_binding = Arc::new(Mutex::new(ComposerBinding::new(
            snapshot.window_id,
            snapshot.composer_task_id.clone(),
            snapshot.composer_revision,
            snapshot.composer.clone(),
            snapshot.composer_turn_id.clone(),
        )));
        bind_composer_input(
            context,
            composer,
            Arc::clone(&sink),
            Arc::clone(&composer_binding),
        )?;
        bind_composer_keys(
            context,
            composer,
            Arc::clone(&sink),
            Arc::clone(&composer_binding),
        )?;
        let extras = context.create_detached_component(document_id, Stack::fill_row(6.0))?;
        let plus_slot = context
            .create_detached_component(document_id, trigger_slot(PLUS_SLOT_SIZE, PLUS_SLOT_SIZE))?;
        let plus_menu = context.create_detached_component(
            document_id,
            composer_plus_menu(snapshot.composer_plus_open),
        )?;
        context.on(plus_menu, {
            let sink = Arc::clone(&sink);
            let binding = Arc::clone(&composer_binding);
            move |_, _: &PopoverToggled, _| {
                emit(
                    &sink,
                    ShellIntent::AddressedComposer {
                        target: binding.lock().unwrap().target.clone(),
                        action: ComposerInputAction::ToggleActions,
                    },
                )
            }
        })?;
        let attach = context.create_detached_component(document_id, composer_attach_button())?;
        bind_action(
            context,
            attach,
            Arc::clone(&sink),
            Arc::clone(&composer_binding),
            ComposerInputAction::Plus("add-file".to_owned()),
        )?;
        let permission_slot = context.create_detached_component(document_id, Stack::row(4.0))?;
        let permission_icon =
            context.create_detached_component(document_id, IconGlyph::new(Icon::ShieldCheck))?;
        let permission_menu = context.create_detached_component(
            document_id,
            composer_menu(
                &snapshot.permission_label,
                snapshot.composer_permission_menu_open,
            ),
        )?;
        context.on(permission_menu, {
            let sink = Arc::clone(&sink);
            let binding = Arc::clone(&composer_binding);
            move |_, _: &PopoverToggled, _| {
                emit(
                    &sink,
                    ShellIntent::AddressedComposer {
                        target: binding.lock().unwrap().target.clone(),
                        action: ComposerInputAction::TogglePermission,
                    },
                )
            }
        })?;
        let worktree_slot = context.create_detached_component(document_id, Stack::row(4.0))?;
        let worktree_icon =
            context.create_detached_component(document_id, IconGlyph::new(Icon::GitBranch))?;
        let worktree_menu = context.create_detached_component(
            document_id,
            composer_menu(
                snapshot.worktree_label.as_deref().unwrap_or_default(),
                snapshot.composer_worktree_menu_open,
            ),
        )?;
        context.on(worktree_menu, {
            let sink = Arc::clone(&sink);
            let binding = Arc::clone(&composer_binding);
            move |_, _: &PopoverToggled, _| {
                emit(
                    &sink,
                    ShellIntent::AddressedComposer {
                        target: binding.lock().unwrap().target.clone(),
                        action: ComposerInputAction::ToggleWorktree,
                    },
                )
            }
        })?;
        context.append_child(plus_slot, plus_menu)?;
        context.append_child(permission_slot, permission_icon)?;
        context.append_child(permission_slot, permission_menu)?;
        let model =
            context.create_detached_component(document_id, composer_model_dropdown(snapshot))?;
        let model_sink = Arc::clone(&sink);
        let model_binding = Arc::clone(&composer_binding);
        context.on(model, move |_, event: &DropdownEvent<Arc<str>>, _| {
            if let DropdownEvent::Select(value) = event {
                emit(
                    &model_sink,
                    ShellIntent::AddressedComposer {
                        target: model_binding.lock().unwrap().target.clone(),
                        action: ComposerInputAction::Model(value.to_string()),
                    },
                );
            }
        })?;
        let reasoning = context.create_detached_component(
            document_id,
            composer_reasoning_dropdown(&snapshot.reasoning, snapshot.composer_disabled),
        )?;
        let reasoning_sink = Arc::clone(&sink);
        let reasoning_binding = Arc::clone(&composer_binding);
        context.on(reasoning, move |_, event: &DropdownEvent<Arc<str>>, _| {
            if let DropdownEvent::Select(value) = event {
                emit(
                    &reasoning_sink,
                    ShellIntent::AddressedComposer {
                        target: reasoning_binding.lock().unwrap().target.clone(),
                        action: ComposerInputAction::Reasoning(value.to_string()),
                    },
                );
            }
        })?;
        context.append_child(worktree_slot, worktree_icon)?;
        context.append_child(worktree_slot, worktree_menu)?;
        let review_slot = context.create_detached_component(document_id, Stack::row(6.0))?;
        let review_target = context.create_detached_component(
            document_id,
            composer_review_dropdown(snapshot.review_target.as_deref().unwrap_or("changes")),
        )?;
        let review_target_sink = Arc::clone(&sink);
        let review_target_binding = Arc::clone(&composer_binding);
        context.on(
            review_target,
            move |_, event: &DropdownEvent<Arc<str>>, _| {
                if let DropdownEvent::Select(value) = event {
                    emit(
                        &review_target_sink,
                        ShellIntent::AddressedComposer {
                            target: review_target_binding.lock().unwrap().target.clone(),
                            action: ComposerInputAction::ReviewTarget(value.to_string()),
                        },
                    );
                }
            },
        )?;
        let review_value = context.create_detached_component(
            document_id,
            composer_review_value(
                &snapshot.review_value,
                review_value_placeholder(snapshot.review_target.as_deref().unwrap_or("changes")),
            ),
        )?;
        let review_value_sink = Arc::clone(&sink);
        let review_value_binding = Arc::clone(&composer_binding);
        context.on(review_value, move |_, event: &TextChanged, _| {
            emit(
                &review_value_sink,
                ShellIntent::AddressedComposer {
                    target: review_value_binding.lock().unwrap().target.clone(),
                    action: ComposerInputAction::ReviewValue(event.value.clone()),
                },
            );
        })?;
        let review_submit = context.create_detached_component(
            document_id,
            extra_button("开始审查", ButtonKind::Subtle),
        )?;
        bind_action(
            context,
            review_submit,
            Arc::clone(&sink),
            Arc::clone(&composer_binding),
            ComposerInputAction::SubmitReview,
        )?;
        let review_cancel = context.create_detached_component(
            document_id,
            extra_button("取消", ButtonKind::Ghost),
        )?;
        bind_action(
            context,
            review_cancel,
            Arc::clone(&sink),
            Arc::clone(&composer_binding),
            ComposerInputAction::CancelReview,
        )?;
        context.append_child(extras, plus_slot)?;
        context.append_child(extras, attach)?;
        context.append_child(extras, permission_slot)?;
        let actions = context.create_detached_component(document_id, Stack::row(6.0))?;
        let browser_open = mount_task_browser_button(
            context,
            document_id,
            &composer_binding,
            &sink,
            snapshot.can_open_browser,
        )?;
        let send = context
            .create_detached_component(document_id, composer_send_button(snapshot.can_send))?;
        bind_composer_button(
            context,
            send,
            Arc::clone(&sink),
            Arc::clone(&composer_binding),
            ComposerInputAction::Submit,
        )?;
        context.append_child(actions, browser_open)?;
        context.append_child(actions, send)?;
        let composer_toolbar = context.create_detached_component(
            document_id,
            Stack::bar(8.0).justify(JustifySpec::SpaceBetween),
        )?;
        context.append_child(composer_toolbar, extras)?;
        context.append_child(composer_toolbar, actions)?;
        let completion_slot = context.create_detached_component(document_id, Stack::column(1.0))?;
        context.append_child(composer_dock, composer)?;
        context.append_child(composer_dock, composer_toolbar)?;
        let mut view = Self {
            sink,
            extra_signatures: HashMap::new(),
            composer_generation: ComposerGeneration::default(),
            composer_binding,
            browser_open,
            composer_dock,
            composer,
            composer_toolbar,
            extras,
            extra_buttons: HashMap::new(),
            completion_slot,
            completion_items: HashMap::new(),
            plus_slot,
            plus_menu,
            plus_items: HashMap::new(),
            attach,
            permission_slot,
            #[cfg(test)]
            permission_icon,
            permission_menu,
            permission_items: HashMap::new(),
            model,
            reasoning,
            worktree_slot,
            worktree_menu,
            worktree_items: HashMap::new(),
            review_slot,
            review_target,
            review_value,
            review_submit,
            review_cancel,
            composer_actions: actions,
            send,
            interrupt: None,
            last_failed_revision: None,
        };
        view.sync(context, document_id, snapshot)?;
        Ok(view)
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        if self.composer_binding.lock().unwrap().target.task_id != snapshot.composer_task_id {
            for (_, item) in self.extra_buttons.drain() {
                context.remove_view(item)?;
            }
            for (_, item) in self.completion_items.drain() {
                context.remove_view(item)?;
            }
            for (_, item) in self.plus_items.drain() {
                context.remove_view(item)?;
            }
            for (_, item) in self.permission_items.drain() {
                context.remove_view(item)?;
            }
            for (_, item) in self.worktree_items.drain() {
                context.remove_view(item)?;
            }
            self.extra_signatures.clear();
        }
        let composer_generation = ComposerGeneration::new(
            snapshot.composer_task_id.clone(),
            snapshot.composer_revision,
        );
        if snapshot.composer_task_id != self.composer_generation.task_id {
            self.last_failed_revision = None;
        }
        let failed_resync = snapshot.apply_failed
            && self.last_failed_revision != Some(snapshot.composer_revision);
        if failed_resync {
            self.last_failed_revision = Some(snapshot.composer_revision);
        }
        let write_composer = !composer_is_focused(context, self.composer)
            || self.composer_generation != composer_generation
            || failed_resync;
        context.update_component(self.composer, |composer, _| {
            if write_composer && composer.state.value != snapshot.composer {
                composer.state.replace_value(snapshot.composer.clone());
            }
            composer.atom_spans = composer_atom_chips(&snapshot.composer_atom_spans);
            composer.placeholder = Arc::from(snapshot.composer_placeholder.as_str());
            composer.disabled = snapshot.composer_disabled;
            Arc::make_mut(&mut composer.style.layout).height = Some(LengthSpec::Px(
                snapshot
                    .composer_height
                    .clamp(COMPOSER_MIN_HEIGHT, COMPOSER_MAX_HEIGHT),
            ));
        })?;
        self.composer_generation = composer_generation;
        if write_composer {
            *self.composer_binding.lock().unwrap() = ComposerBinding::new(
                snapshot.window_id,
                snapshot.composer_task_id.clone(),
                snapshot.composer_revision,
                snapshot.composer.clone(),
                snapshot.composer_turn_id.clone(),
            );
        } else {
            self.composer_binding.lock().unwrap().target.turn_id =
                snapshot.composer_turn_id.clone();
        }
        context.update_component(self.browser_open, |button, _| {
            button.disabled = !snapshot.can_open_browser;
        })?;
        self.sync_composer_stage(context, document_id, snapshot)
    }
    fn sync_composer_actions(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let mut order = vec![self.browser_open.stable_id()];
        if snapshot.can_interrupt
            && snapshot.composer_turn_id.is_some()
            && (!snapshot.can_send || snapshot.pending_blocks_send)
        {
            let interrupt = if let Some(interrupt) = self.interrupt {
                context.update_component(interrupt, |button, _| {
                    *button = composer_interrupt_button(true);
                })?;
                interrupt
            } else {
                let interrupt = context
                    .create_detached_component(document_id, composer_interrupt_button(true))?;
                bind_composer_button(
                    context,
                    interrupt,
                    Arc::clone(&self.sink),
                    Arc::clone(&self.composer_binding),
                    ComposerInputAction::Interrupt,
                )?;
                self.interrupt = Some(interrupt);
                interrupt
            };
            order.push(interrupt.stable_id());
        } else if let Some(interrupt) = self.interrupt.take() {
            let _ = context.remove_view(interrupt);
            order.push(self.send.stable_id());
        } else {
            order.push(self.send.stable_id());
        }
        context.update_component(self.send, |button, _| {
            *button = composer_send_button(snapshot.can_send && !snapshot.pending_blocks_send);
        })?;
        reconcile_children(context, self.composer_actions.stable_id(), &order)
    }

    fn sync_composer_stage(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        context.update_component(self.plus_menu, |menu, _| {
            *menu = composer_plus_menu(snapshot.composer_plus_open);
        })?;
        context.update_component(self.permission_menu, |menu, _| {
            *menu = composer_menu(
                &snapshot.permission_label,
                snapshot.composer_permission_menu_open,
            );
        })?;
        if let Some(label) = snapshot.worktree_label.as_deref() {
            context.update_component(self.worktree_menu, |menu, _| {
                *menu = composer_menu(label, snapshot.composer_worktree_menu_open);
            })?;
        }
        context.update_component(self.model, |field, _| {
            field.selection = DropdownSelection::Single(Some(Arc::from(snapshot.model.as_str())));
            field.options = snapshot
                .models
                .iter()
                .map(|(id, label)| {
                    DropdownOption::new(
                        id.clone(),
                        if id.is_empty() {
                            snapshot.model_label.clone()
                        } else {
                            label.clone()
                        },
                    )
                })
                .collect();
            field.disabled = snapshot.composer_disabled;
        })?;
        context.update_component(self.reasoning, |field, _| {
            field.selection =
                DropdownSelection::Single(Some(Arc::from(snapshot.reasoning.as_str())));
            field.options = COMPOSER_REASONING_OPTIONS
                .iter()
                .map(|(id, label)| DropdownOption::new(*id, *label))
                .collect();
            field.disabled = snapshot.composer_disabled;
        })?;
        let plus_entries = plus_menu_items(snapshot)
            .into_iter()
            .map(|(id, label)| (id, label, false))
            .collect::<Vec<_>>();
        sync_action_menu_items(
            context,
            document_id,
            self.plus_menu,
            snapshot.composer_plus_open,
            &plus_entries,
            &mut self.plus_items,
            &self.sink,
            &self.composer_binding,
            |id| ComposerInputAction::Plus(id.to_owned()),
        )?;
        let permission_entries = COMPOSER_PERMISSION_OPTIONS
            .iter()
            .map(|(id, label)| {
                (
                    (*id).to_owned(),
                    (*label).to_owned(),
                    *id == snapshot.permission_selection,
                )
            })
            .collect::<Vec<_>>();
        sync_action_menu_items(
            context,
            document_id,
            self.permission_menu,
            snapshot.composer_permission_menu_open,
            &permission_entries,
            &mut self.permission_items,
            &self.sink,
            &self.composer_binding,
            |id| ComposerInputAction::Permission(id.to_owned()),
        )?;
        let worktree_entries = COMPOSER_WORKTREE_OPTIONS
            .iter()
            .map(|(id, label)| {
                (
                    (*id).to_owned(),
                    (*label).to_owned(),
                    *id == snapshot.worktree_selection,
                )
            })
            .collect::<Vec<_>>();
        sync_action_menu_items(
            context,
            document_id,
            self.worktree_menu,
            snapshot.composer_worktree_menu_open,
            &worktree_entries,
            &mut self.worktree_items,
            &self.sink,
            &self.composer_binding,
            |id| ComposerInputAction::Worktree(id.to_owned()),
        )?;
        self.reconcile_composer_extras(context, document_id, snapshot)?;
        self.reconcile_review_workflow(context, document_id, snapshot)?;
        self.reconcile_composer_completion(context, document_id, snapshot)?;
        self.sync_composer_actions(context, document_id, snapshot)?;

        let mut dock_stage = Vec::new();
        if !self.completion_items.is_empty() {
            dock_stage.push(self.completion_slot.stable_id());
        }
        dock_stage.push(self.composer.stable_id());
        if snapshot.review_target.is_some() {
            dock_stage.push(self.review_slot.stable_id());
        }
        dock_stage.push(self.composer_toolbar.stable_id());
        reconcile_children(context, self.composer_dock.stable_id(), &dock_stage)?;

        reconcile_children(
            context,
            self.composer_toolbar.stable_id(),
            &[self.extras.stable_id(), self.composer_actions.stable_id()],
        )
    }

    fn reconcile_composer_extras(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let mut keep = HashSet::new();
        let mut desired = Vec::new();
        if let Some(label) = &snapshot.branch_label {
            desired.push((
                "branch-clear".to_owned(),
                format!("{label} · 取消"),
                ButtonKind::Subtle,
                ComposerInputAction::ClearBranch,
            ));
        }
        if snapshot.suggestions_can_refresh {
            desired.push((
                "refresh-suggestions".to_owned(),
                "刷新建议".to_owned(),
                ButtonKind::Subtle,
                ComposerInputAction::RefreshSuggestions,
            ));
        }
        for suggestion in &snapshot.suggestions {
            desired.push((
                suggestion.id.clone(),
                suggestion.label.clone(),
                ButtonKind::Subtle,
                ComposerInputAction::ApplySuggestion(suggestion.prompt.clone()),
            ));
        }
        for attachment in &snapshot.attachments {
            desired.push((
                attachment.id.clone(),
                attachment.label.clone(),
                ButtonKind::Ghost,
                ComposerInputAction::RemoveAttachment(attachment.id.clone()),
            ));
        }
        let mut order = vec![
            self.plus_slot.stable_id(),
            self.attach.stable_id(),
            self.permission_slot.stable_id(),
        ];
        if snapshot.worktree_label.is_some() {
            order.push(self.worktree_slot.stable_id());
        }
        for (id, label, kind, intent) in desired {
            let signature = (label.clone(), intent.clone());
            if self.extra_signatures.get(&id) != Some(&signature) {
                if let Some(button) = self.extra_buttons.remove(&id) {
                    context.remove_view(button)?;
                }
            }
            self.extra_signatures.insert(id.clone(), signature);
            keep.insert(id.clone());
            let button = if let Some(button) = self.extra_buttons.get(&id).copied() {
                context.update_component(button, |button, _| {
                    *button = extra_button(&label, kind);
                })?;
                button
            } else {
                let button =
                    context.create_detached_component(document_id, extra_button(&label, kind))?;
                bind_action(
                    context,
                    button,
                    Arc::clone(&self.sink),
                    Arc::clone(&self.composer_binding),
                    intent,
                )?;
                self.extra_buttons.insert(id, button);
                button
            };
            order.push(button.stable_id());
        }
        let stale: Vec<_> = self
            .extra_buttons
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            self.extra_signatures.remove(&key);
            if let Some(button) = self.extra_buttons.remove(&key) {
                let _ = context.remove_view(button);
            }
        }
        reconcile_children(context, self.extras.stable_id(), &order)
    }

    fn reconcile_review_workflow(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let Some(target) = snapshot.review_target.as_deref() else {
            return reconcile_children(context, self.review_slot.stable_id(), &[]);
        };
        context.update_component(self.review_target, |field, _| {
            field.selection = DropdownSelection::Single(Some(Arc::from(target)));
            field.options = COMPOSER_REVIEW_OPTIONS
                .iter()
                .map(|(id, label)| DropdownOption::new(*id, *label))
                .collect();
        })?;
        let placeholder = review_value_placeholder(target);
        let write_value = context.world().focused(document_id) != Some(self.review_value.stable_id());
        context.update_component(self.review_value, |field, _| {
            field.placeholder = Arc::from(placeholder);
            if write_value && field.state.value != snapshot.review_value {
                field.state.replace_value(snapshot.review_value.clone());
            }
        })?;
        let submit_blocked = snapshot.composer_disabled
            || (target != "changes" && snapshot.review_value.trim().is_empty());
        context.update_component(self.review_submit, |button, _| {
            *button = extra_button("开始审查", ButtonKind::Subtle);
            button.disabled = submit_blocked;
        })?;
        context.update_component(self.review_cancel, |button, _| {
            *button = extra_button("取消", ButtonKind::Ghost);
        })?;
        let mut order = vec![self.review_target.stable_id()];
        if target != "changes" {
            order.push(self.review_value.stable_id());
        }
        order.push(self.review_submit.stable_id());
        order.push(self.review_cancel.stable_id());
        reconcile_children(context, self.review_slot.stable_id(), &order)
    }

    fn reconcile_composer_completion(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let mut keep = HashSet::new();
        let mut order = Vec::new();
        let desired = composer_completion_entries(snapshot);
        self.composer_binding.lock().unwrap().sync_keys(
            &snapshot.composer,
            snapshot.can_send,
            snapshot.composer_disabled,
            snapshot.pending_blocks_send,
            desired
                .iter()
                .map(|(_, _, action)| action.clone())
                .collect(),
        );
        for (id, label, intent) in desired {
            keep.insert(id.clone());
            let item = if let Some(item) = self.completion_items.get(&id).copied() {
                context.update_component(item, |item, _| {
                    *item = ActionMenuItem::new(label);
                })?;
                item
            } else {
                let item =
                    context.create_detached_component(document_id, ActionMenuItem::new(label))?;
                bind_action(
                    context,
                    item,
                    Arc::clone(&self.sink),
                    Arc::clone(&self.composer_binding),
                    intent,
                )?;
                self.completion_items.insert(id, item);
                item
            };
            order.push(item.stable_id());
        }
        self.completion_items.retain(|key, item| {
            if keep.contains(key) {
                true
            } else {
                let _ = context.remove_view(*item);
                false
            }
        });
        reconcile_children(context, self.completion_slot.stable_id(), &order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::RuntimeInputAdapter;
    use nana_ui::runtime::TextSelection;
    use nana_ui_platform::{InputEvent, InputModifiers};

    fn snapshot(window_id: HostedWindowId, task: &str) -> ComposerViewSnapshot {
        ComposerViewSnapshot {
            window_id,
            composer_task_id: Some(task.into()),
            composer_revision: 4,
            composer: "draft".into(),
            composer_height: 40.0,
            composer_placeholder: "输入消息".into(),
            can_send: true,
            composer_permission_menu_open: true,
            composer_plus_open: true,
            permission_label: "询问".into(),
            permission_selection: "ask".into(),
            attachments: vec![ComposerAttachment {
                id: "attachment".into(),
                label: "source.rs".into(),
            }],
            suggestions: vec![ComposerSuggestion {
                id: "suggestion".into(),
                label: "继续".into(),
                prompt: "first prompt".into(),
            }],
            reference_items: vec![ComposerMentionItem {
                id: "reference-task".into(),
                label: "历史对话".into(),
            }],
            slash_items: vec![ComposerSlashItem {
                name: "review".into(),
                label: "审阅".into(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn focused_pending_edits_survive_an_unchanged_projection_and_blocked_send_keeps_stop() {
        let mut context = AppContext::new();
        let document = DocumentId::new(311).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(nana_ui_platform::WindowId(42), "task");
        let mut view =
            ComposerView::mount(&mut context, document, &snapshot, Arc::new(|_| {})).unwrap();
        context.append_child(host, view.composer_dock).unwrap();
        context
            .focus_node(document, view.composer.stable_id())
            .unwrap();
        context
            .update_component(view.composer, |editor, cx| {
                editor.state.replace_value("new text".to_owned());
                cx.emit(TextChanged {
                    value: "new text".into(),
                    selection: TextSelection::default(),
                });
            })
            .unwrap();
        view.sync(&mut context, document, &snapshot).unwrap();
        let target = view.composer_binding.lock().unwrap().target.clone();
        assert_eq!(target.content, "new text");
        assert_eq!(target.revision, 5);
        snapshot.pending_blocks_send = true;
        snapshot.can_interrupt = true;
        snapshot.composer_turn_id = Some("running-turn".into());
        view.sync(&mut context, document, &snapshot).unwrap();
        let stop = view.interrupt.unwrap();
        let children = &context
            .world()
            .node(view.composer_actions.stable_id())
            .unwrap()
            .children;
        assert!(children.contains(&stop.stable_id()));
        assert!(!children.contains(&view.send.stable_id()));
        assert_eq!(
            view.composer_binding
                .lock()
                .unwrap()
                .target
                .turn_id
                .as_deref(),
            Some("running-turn")
        );
    }

    #[test]
    fn failed_apply_resyncs_focused_binding_to_the_store_revision() {
        let mut context = AppContext::new();
        let document = DocumentId::new(312).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(nana_ui_platform::WindowId(42), "task");
        let mut view =
            ComposerView::mount(&mut context, document, &snapshot, Arc::new(|_| {})).unwrap();
        context.append_child(host, view.composer_dock).unwrap();
        context
            .focus_node(document, view.composer.stable_id())
            .unwrap();
        context
            .update_component(view.composer, |editor, cx| {
                editor.state.replace_value("new text".to_owned());
                cx.emit(TextChanged {
                    value: "new text".into(),
                    selection: TextSelection::default(),
                });
            })
            .unwrap();
        assert_eq!(view.composer_binding.lock().unwrap().target.revision, 5);
        snapshot.apply_failed = true;
        view.sync(&mut context, document, &snapshot).unwrap();
        let target = view.composer_binding.lock().unwrap().target.clone();
        assert_eq!(target.revision, 4);
        assert_eq!(target.content, "draft");
        assert_eq!(
            context
                .read(view.composer, |editor| editor.state.value.clone())
                .unwrap(),
            "draft"
        );
        context
            .update_component(view.composer, |editor, cx| {
                editor.state.replace_value("retry".to_owned());
                cx.emit(TextChanged {
                    value: "retry".into(),
                    selection: TextSelection::default(),
                });
            })
            .unwrap();
        view.sync(&mut context, document, &snapshot).unwrap();
        let target = view.composer_binding.lock().unwrap().target.clone();
        assert_eq!(target.content, "retry");
        assert_eq!(target.revision, 5);
    }

    #[test]
    fn branch_clear_dispatches_clear_branch() {
        let mut snapshot = snapshot(HostedWindowId::PRIMARY, "task");
        snapshot.branch_label = Some("从这里分叉".into());
        snapshot.suggestions.clear();
        snapshot.attachments.clear();
        snapshot.composer_plus_open = false;
        snapshot.composer_permission_menu_open = false;
        let mut context = AppContext::new();
        let document = DocumentId::new(331).unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let mut view = ComposerView::mount(
            &mut context,
            document,
            &snapshot,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        view.sync(&mut context, document, &snapshot).unwrap();
        context
            .update_component(view.extra_buttons["branch-clear"], |_, cx| {
                cx.emit(Activate)
            })
            .unwrap();
        assert!(matches!(
            received.lock().unwrap().last(),
            Some(ShellIntent::AddressedComposer {
                action: ComposerInputAction::ClearBranch,
                ..
            })
        ));
    }

    #[test]
    fn plus_menu_exposes_new_guide_when_todos_can_be_managed() {
        let mut snapshot = snapshot(HostedWindowId::PRIMARY, "task");
        snapshot.can_manage_todos = true;
        snapshot.composer_plus_open = true;
        snapshot.composer_permission_menu_open = false;
        snapshot.suggestions.clear();
        snapshot.attachments.clear();
        snapshot.slash_items.clear();
        let mut context = AppContext::new();
        let document = DocumentId::new(333).unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let view = ComposerView::mount(
            &mut context,
            document,
            &snapshot,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        context
            .update_component(view.plus_items["new-guide"], |_, cx| cx.emit(Activate))
            .unwrap();
        assert!(matches!(
            received.lock().unwrap().last(),
            Some(ShellIntent::AddressedComposer {
                action: ComposerInputAction::Plus(id),
                ..
            }) if id == "new-guide"
        ));
    }

    #[test]
    fn review_workflow_exposes_target_value_and_submit() {
        let mut snapshot = snapshot(HostedWindowId::PRIMARY, "task");
        snapshot.review_target = Some("changes".into());
        snapshot.suggestions.clear();
        snapshot.attachments.clear();
        snapshot.composer_plus_open = false;
        snapshot.composer_permission_menu_open = false;
        snapshot.slash_items.clear();
        let mut context = AppContext::new();
        let document = DocumentId::new(332).unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let mut view = ComposerView::mount(
            &mut context,
            document,
            &snapshot,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        let children = context
            .world()
            .node(view.review_slot.stable_id())
            .unwrap()
            .children
            .clone();
        assert!(children.contains(&view.review_target.stable_id()));
        assert!(!children.contains(&view.review_value.stable_id()));
        context
            .update_component(view.review_submit, |_, cx| cx.emit(Activate))
            .unwrap();
        context
            .update_component(view.review_target, |_, cx| {
                cx.emit(DropdownEvent::<Arc<str>>::Select(Arc::from("branch")))
            })
            .unwrap();
        snapshot.review_target = Some("branch".into());
        view.sync(&mut context, document, &snapshot).unwrap();
        let children = context
            .world()
            .node(view.review_slot.stable_id())
            .unwrap()
            .children
            .clone();
        assert!(children.contains(&view.review_value.stable_id()));
        context
            .update_component(view.review_value, |_, cx| {
                cx.emit(TextChanged {
                    value: "parity-review-base".into(),
                    selection: TextSelection::default(),
                })
            })
            .unwrap();
        let observed = received.lock().unwrap();
        assert!(matches!(
            &observed[0],
            ShellIntent::AddressedComposer {
                action: ComposerInputAction::SubmitReview,
                ..
            }
        ));
        assert!(matches!(
            &observed[1],
            ShellIntent::AddressedComposer {
                action: ComposerInputAction::ReviewTarget(target),
                ..
            } if target == "branch"
        ));
        assert!(matches!(
            &observed[2],
            ShellIntent::AddressedComposer {
                action: ComposerInputAction::ReviewValue(value),
                ..
            } if value == "parity-review-base"
        ));
    }

    #[test]
    fn model_choice_dispatches_the_model() {
        let mut context = AppContext::new();
        let document = DocumentId::new(330).unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let view = ComposerView::mount(
            &mut context,
            document,
            &snapshot(HostedWindowId::PRIMARY, "task"),
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        context
            .update_component(view.model, |_, cx| {
                cx.emit(DropdownEvent::<Arc<str>>::Select(Arc::from("native-debug")))
            })
            .unwrap();
        assert!(matches!(
            received.lock().unwrap().last(),
            Some(ShellIntent::AddressedComposer {
                action: ComposerInputAction::Model(model),
                ..
            }) if model == "native-debug"
        ));
    }

    #[test]
    fn reasoning_choice_dispatches_the_effort() {
        let mut context = AppContext::new();
        let document = DocumentId::new(329).unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let view = ComposerView::mount(
            &mut context,
            document,
            &snapshot(HostedWindowId::PRIMARY, "task"),
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        context
            .update_component(view.reasoning, |_, cx| {
                cx.emit(DropdownEvent::<Arc<str>>::Select(Arc::from("high")))
            })
            .unwrap();
        assert!(matches!(
            received.lock().unwrap().last(),
            Some(ShellIntent::AddressedComposer {
                action: ComposerInputAction::Reasoning(effort),
                ..
            }) if effort == "high"
        ));
    }

    #[test]
    fn two_windows_route_every_composer_control_with_their_own_draft_identity() {
        let mut context = AppContext::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        for (window, task) in [
            (HostedWindowId::PRIMARY, "main"),
            (nana_ui_platform::WindowId(42), "popup"),
        ] {
            let document = DocumentId::new(window.0 + 100).unwrap();
            let received = Arc::clone(&events);
            let view = ComposerView::mount(
                &mut context,
                document,
                &snapshot(window, task),
                Arc::new(move |event| received.lock().unwrap().push(event)),
            )
            .unwrap();
            context
                .update_component(view.permission_items["readonly"], |_, cx| cx.emit(Activate))
                .unwrap();
            context
                .update_component(view.plus_items["plan"], |_, cx| cx.emit(Activate))
                .unwrap();
            context
                .update_component(view.extra_buttons["attachment"], |_, cx| cx.emit(Activate))
                .unwrap();
            context
                .update_component(view.completion_items["slash-review"], |_, cx| {
                    cx.emit(Activate)
                })
                .unwrap();
            context
                .update_component(
                    view.completion_items["reference-result-reference-task"],
                    |_, cx| cx.emit(Activate),
                )
                .unwrap();
            context
                .update_component(view.composer, |_, cx| {
                    cx.emit(TextChanged {
                        value: "edited".into(),
                        selection: TextSelection::default(),
                    })
                })
                .unwrap();
            context
                .update_component(view.send, |_, cx| cx.emit(Activate))
                .unwrap();
            let events = events.lock().unwrap();
            let recent = &events[events.len() - 7..];
            for event in recent {
                let ShellIntent::AddressedComposer { target, .. } = event else {
                    panic!("unaddressed composer control")
                };
                assert_eq!(target.window_id, window);
                assert_eq!(target.task_id.as_deref(), Some(task));
            }
            let ShellIntent::AddressedComposer { target, action } = &recent[6] else {
                unreachable!()
            };
            assert_eq!(target.revision, 5);
            assert_eq!(target.content, "edited");
            assert_eq!(action, &ComposerInputAction::Submit);
        }
    }

    #[test]
    fn changed_suggestion_rebinds_and_switching_task_disposes_old_actions() {
        let mut context = AppContext::new();
        let document = DocumentId::new(210).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut snapshot = snapshot(nana_ui_platform::WindowId(42), "first");
        let mut view = ComposerView::mount(
            &mut context,
            document,
            &snapshot,
            Arc::new(move |event| received.lock().unwrap().push(event)),
        )
        .unwrap();
        let old = view.extra_buttons["suggestion"];
        context
            .update_component(old, |_, cx| cx.emit(Activate))
            .unwrap();
        snapshot.suggestions[0].prompt = "replacement prompt".into();
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(!context.world().contains(old.stable_id()));
        let replacement = view.extra_buttons["suggestion"];
        context
            .update_component(replacement, |_, cx| cx.emit(Activate))
            .unwrap();
        let observed = events.lock().unwrap();
        assert!(
            matches!(&observed[0], ShellIntent::AddressedComposer {action: ComposerInputAction::ApplySuggestion(prompt), ..} if prompt == "first prompt")
        );
        assert!(
            matches!(&observed[1], ShellIntent::AddressedComposer {action: ComposerInputAction::ApplySuggestion(prompt), ..} if prompt == "replacement prompt")
        );
        drop(observed);
        let old_completion = view.completion_items["slash-review"];
        snapshot.composer_task_id = Some("second".into());
        snapshot.composer_revision = 0;
        snapshot.composer.clear();
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(!context.world().contains(replacement.stable_id()));
        assert!(!context.world().contains(old_completion.stable_id()));
        context
            .update_component(view.send, |_, cx| cx.emit(Activate))
            .unwrap();
        let events = events.lock().unwrap();
        assert!(
            matches!(events.last().unwrap(), ShellIntent::AddressedComposer {target, action: ComposerInputAction::Submit} if target.task_id.as_deref() == Some("second") && target.revision == 0 && target.content.is_empty())
        );
    }

    fn key(name: &str, shift: bool, repeat: bool) -> InputEvent {
        InputEvent::Keyboard {
            pressed: true,
            key: name.into(),
            code: name.into(),
            text: None,
            repeat,
            modifiers: InputModifiers {
                shift,
                ..Default::default()
            },
        }
    }

    fn mount_focused(
        snapshot: &ComposerViewSnapshot,
    ) -> (
        AppContext,
        DocumentId,
        ComposerView,
        Arc<Mutex<Vec<ShellIntent>>>,
    ) {
        let mut context = AppContext::new();
        let document = DocumentId::new(411).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let view = ComposerView::mount(
            &mut context,
            document,
            snapshot,
            Arc::new(move |event| received.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(host, view.composer_dock).unwrap();
        assert!(
            context
                .focus_node(document, view.composer.stable_id())
                .unwrap()
        );
        (context, document, view, events)
    }

    fn sendable_snapshot() -> ComposerViewSnapshot {
        let mut snapshot = snapshot(nana_ui_platform::WindowId(42), "task");
        snapshot.slash_items.clear();
        snapshot.mention_items.clear();
        snapshot.reference_items.clear();
        snapshot.composer_plus_open = false;
        snapshot.composer_permission_menu_open = false;
        snapshot
    }

    #[test]
    fn enter_sends_once_and_shift_enter_inserts_newline() {
        let (mut context, document, view, events) = mount_focused(&sendable_snapshot());
        let mut input = RuntimeInputAdapter::default();
        for repeat in [false, true, false] {
            assert!(
                input
                    .dispatch(&mut context, document, &key("Enter", false, repeat))
                    .unwrap()
                    .prevent_default
            );
        }
        {
            let events = events.lock().unwrap();
            assert_eq!(events.len(), 1);
            assert!(matches!(
                &events[0],
                ShellIntent::AddressedComposer {
                    action: ComposerInputAction::Submit,
                    ..
                }
            ));
        }
        assert_eq!(
            context
                .world()
                .text_input(view.composer.stable_id())
                .unwrap()
                .value,
            "draft"
        );
        assert!(
            input
                .dispatch(&mut context, document, &key("Enter", true, false))
                .unwrap()
                .prevent_default
        );
        assert_eq!(
            context
                .world()
                .text_input(view.composer.stable_id())
                .unwrap()
                .value,
            "draft\n"
        );
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| matches!(
                    event,
                    ShellIntent::AddressedComposer {
                        action: ComposerInputAction::Submit,
                        ..
                    }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn enter_and_tab_accept_visible_completion_instead_of_sending() {
        let mut snapshot = snapshot(nana_ui_platform::WindowId(42), "task");
        snapshot.composer_plus_open = false;
        snapshot.composer_permission_menu_open = false;
        let (mut context, document, mut view, events) = mount_focused(&snapshot);
        let mut input = RuntimeInputAdapter::default();
        for name in ["ArrowDown", "ArrowUp", "ArrowUp"] {
            assert!(
                input
                    .dispatch(&mut context, document, &key(name, false, false))
                    .unwrap()
                    .prevent_default
            );
        }
        assert!(
            input
                .dispatch(&mut context, document, &key("Enter", false, false))
                .unwrap()
                .prevent_default
        );
        assert!(
            input
                .dispatch(&mut context, document, &key("Enter", false, true))
                .unwrap()
                .prevent_default
        );
        {
            let actions = events.lock().unwrap();
            assert_eq!(actions.len(), 1);
            assert!(matches!(
                &actions[0],
                ShellIntent::AddressedComposer {
                    action: ComposerInputAction::SelectConversationReference(id),
                    ..
                } if id == "reference-task"
            ));
        }
        assert_eq!(
            context
                .world()
                .text_input(view.composer.stable_id())
                .unwrap()
                .value,
            "draft"
        );

        snapshot.slash_items.clear();
        snapshot.reference_items.clear();
        snapshot.mention_items = vec![ComposerMentionItem {
            id: "src/lib.rs".into(),
            label: "lib.rs".into(),
        }];
        view.sync(&mut context, document, &snapshot).unwrap();
        input
            .dispatch(&mut context, document, &key("Tab", false, false))
            .unwrap();
        let actions = events.lock().unwrap();
        assert_eq!(actions.len(), 2);
        assert!(matches!(
            &actions[1],
            ShellIntent::AddressedComposer {
                action: ComposerInputAction::SelectMention(id),
                ..
            } if id == "src/lib.rs"
        ));
    }

    #[test]
    fn enter_follows_synced_can_send_disabled_and_pending_blocks() {
        let mut snapshot = sendable_snapshot();
        snapshot.can_send = false;
        let (mut context, document, mut view, events) = mount_focused(&snapshot);
        let mut input = RuntimeInputAdapter::default();
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(events.lock().unwrap().is_empty());

        snapshot.can_send = true;
        snapshot.pending_blocks_send = true;
        view.sync(&mut context, document, &snapshot).unwrap();
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(events.lock().unwrap().is_empty());

        snapshot.pending_blocks_send = false;
        snapshot.composer_disabled = true;
        view.sync(&mut context, document, &snapshot).unwrap();
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(events.lock().unwrap().is_empty());

        snapshot.composer_disabled = false;
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(
            context
                .focus_node(document, view.composer.stable_id())
                .unwrap()
        );
        input
            .dispatch(&mut context, document, &key("Enter", false, false))
            .unwrap();
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(ShellIntent::AddressedComposer {
                action: ComposerInputAction::Submit,
                ..
            })
        ));
    }
}
