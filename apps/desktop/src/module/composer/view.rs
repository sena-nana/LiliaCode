use super::presentation::{
    ComposerAttachment, ComposerMentionItem, ComposerSlashItem, ComposerSuggestion,
    COMPOSER_PERMISSION_OPTIONS, COMPOSER_REASONING_OPTIONS, COMPOSER_REVIEW_OPTIONS,
    COMPOSER_WORKTREE_OPTIONS,
};
use crate::runtime_compat::HostedWindowId;
use crate::runtime_layout::{
    composer_card, composer_interrupt_button, composer_send_button, flatten_composer_textarea,
    reconcile_children, trigger_slot, Bound,
};
use crate::runtime_shell::{emit, IntentSink, ShellIntent};
use nana_ui::runtime::view::{
    entity_ref, signal, widget, with_refs, AnyView, EachExt, EntityRef, IntoView, Signal, WhenExt,
};
use nana_ui::runtime::{
    ActionMenu, ActionMenuItem, Activate, AppContext, Button, Card, ComponentView, DocumentId,
    Dropdown, DropdownEvent, DropdownOption, DropdownSelection, Entity, FrameworkError, IconButton,
    IconGlyph, JustifySpec, KeyInput, LengthSpec, PopoverToggled, StableNodeId, Stack, TextArea,
    TextAtomSpan, TextChanged, TextInput,
};
use nana_ui::{ButtonKind, ControlSize, Icon, PopoverPlacement, UI_METRICS};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
const PLUS_SLOT_SIZE: f32 = UI_METRICS.icon_button_size;
const COMPOSER_MIN_HEIGHT: f32 = UI_METRICS.control_height;
const COMPOSER_MAX_HEIGHT: f32 = 72.0;
fn extra_button(label: &str, kind: ButtonKind) -> Button {
    let mut button = crate::runtime_layout::pill_button(label, kind);
    if kind == ButtonKind::Text {
        // The toolbar lives directly on the conversation surface. Disabled
        // text actions keep their muted label, but must not reintroduce a
        // second filled control surface through Button's default disabled
        // paint.
        button.style.interaction.disabled.background = None;
        button.style.interaction.disabled.border = None;
    }
    button
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    // Keep the attachment/action affordance in the same lightweight chrome as
    // the text controls below the composer.  `bare_trigger` removes the
    // resting fill and border while preserving the real menu hit target and
    // hover/pressed feedback.
    ActionMenu::new()
        .trigger_icon(Icon::Add, "添加")
        .bare_trigger(true)
        .open(open)
}

/// 输入条贴着窗口底部，菜单必须向上展开。
fn composer_menu(label: &str, open: bool) -> ActionMenu {
    ActionMenu::new()
        .trigger(label.to_owned())
        .placement(PopoverPlacement::Top)
        .bare_trigger(true)
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
        160.0
    } else {
        132.0
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

fn clamped_composer_height(height: f32) -> f32 {
    height.clamp(COMPOSER_MIN_HEIGHT, COMPOSER_MAX_HEIGHT)
}

fn dispatch(sink: &IntentSink, binding: &Arc<Mutex<ComposerBinding>>, action: ComposerInputAction) {
    let target = binding.lock().unwrap().target.clone();
    emit(sink, ShellIntent::AddressedComposer { target, action });
}

fn edit_composer(binding: &Arc<Mutex<ComposerBinding>>, sink: &IntentSink, value: String) {
    if let Some(intent) = binding.lock().unwrap().edit(value) {
        emit(sink, intent);
    }
}

fn button_kind_key(kind: ButtonKind) -> u8 {
    match kind {
        ButtonKind::Ghost => 0,
        ButtonKind::Subtle => 1,
        ButtonKind::Selected => 2,
        ButtonKind::Primary => 3,
        ButtonKind::Warning => 4,
        ButtonKind::Danger => 5,
        ButtonKind::Text => 6,
        ButtonKind::Menu => 7,
    }
}

fn task_key(task: &Option<String>, id: &str) -> String {
    format!("{}::{id}", task.as_deref().unwrap_or(""))
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ExtraKey {
    Plus,
    Attach,
    Permission,
    Worktree,
    Dynamic {
        task: Option<String>,
        id: String,
        label: String,
        kind: u8,
        action: ComposerInputAction,
    },
}

#[derive(Clone)]
enum ExtraRow {
    Plus,
    Attach,
    Permission,
    Worktree,
    Dynamic {
        id: String,
        label: String,
        kind: ButtonKind,
        action: ComposerInputAction,
    },
}

fn extra_key(row: &ExtraRow, task: &Option<String>) -> ExtraKey {
    match row {
        ExtraRow::Plus => ExtraKey::Plus,
        ExtraRow::Attach => ExtraKey::Attach,
        ExtraRow::Permission => ExtraKey::Permission,
        ExtraRow::Worktree => ExtraKey::Worktree,
        ExtraRow::Dynamic {
            id,
            label,
            kind,
            action,
        } => ExtraKey::Dynamic {
            task: task.clone(),
            id: id.clone(),
            label: label.clone(),
            kind: button_kind_key(*kind),
            action: action.clone(),
        },
    }
}

fn extra_rows(snapshot: &ComposerViewSnapshot) -> Vec<ExtraRow> {
    let mut rows = vec![ExtraRow::Plus, ExtraRow::Attach, ExtraRow::Permission];
    if snapshot.worktree_label.is_some() {
        rows.push(ExtraRow::Worktree);
    }
    if let Some(label) = &snapshot.branch_label {
        rows.push(ExtraRow::Dynamic {
            id: "branch-clear".into(),
            label: format!("{label} · 取消"),
            kind: ButtonKind::Text,
            action: ComposerInputAction::ClearBranch,
        });
    }
    if snapshot.suggestions_can_refresh {
        rows.push(ExtraRow::Dynamic {
            id: "refresh-suggestions".into(),
            label: "刷新建议".into(),
            kind: ButtonKind::Text,
            action: ComposerInputAction::RefreshSuggestions,
        });
    }
    for suggestion in &snapshot.suggestions {
        rows.push(ExtraRow::Dynamic {
            id: suggestion.id.clone(),
            label: suggestion.label.clone(),
            kind: ButtonKind::Text,
            action: ComposerInputAction::ApplySuggestion(suggestion.prompt.clone()),
        });
    }
    for attachment in &snapshot.attachments {
        rows.push(ExtraRow::Dynamic {
            id: attachment.id.clone(),
            label: attachment.label.clone(),
            kind: ButtonKind::Text,
            action: ComposerInputAction::RemoveAttachment(attachment.id.clone()),
        });
    }
    rows
}

#[derive(Clone)]
struct MenuEntry {
    key: String,
    id: String,
    label: String,
    active: bool,
    action: ComposerInputAction,
}

#[derive(Clone)]
struct CompletionEntry {
    key: String,
    id: String,
    label: String,
    action: ComposerInputAction,
}

fn menu_entry(
    task: &Option<String>,
    id: &str,
    label: &str,
    active: bool,
    action: ComposerInputAction,
) -> MenuEntry {
    MenuEntry {
        key: task_key(task, id),
        id: id.to_owned(),
        label: label.to_owned(),
        active,
        action,
    }
}

fn plus_entries(snapshot: &ComposerViewSnapshot) -> Vec<MenuEntry> {
    plus_menu_items(snapshot)
        .into_iter()
        .map(|(id, label)| {
            let action = ComposerInputAction::Plus(id.clone());
            menu_entry(&snapshot.composer_task_id, &id, &label, false, action)
        })
        .collect()
}

fn permission_entries(snapshot: &ComposerViewSnapshot) -> Vec<MenuEntry> {
    COMPOSER_PERMISSION_OPTIONS
        .iter()
        .map(|(id, label)| {
            menu_entry(
                &snapshot.composer_task_id,
                id,
                label,
                *id == snapshot.permission_selection,
                ComposerInputAction::Permission((*id).to_owned()),
            )
        })
        .collect()
}

fn worktree_entries(snapshot: &ComposerViewSnapshot) -> Vec<MenuEntry> {
    COMPOSER_WORKTREE_OPTIONS
        .iter()
        .map(|(id, label)| {
            menu_entry(
                &snapshot.composer_task_id,
                id,
                label,
                *id == snapshot.worktree_selection,
                ComposerInputAction::Worktree((*id).to_owned()),
            )
        })
        .collect()
}

fn completion_entries(snapshot: &ComposerViewSnapshot) -> Vec<CompletionEntry> {
    composer_completion_entries(snapshot)
        .into_iter()
        .map(|(id, label, action)| CompletionEntry {
            key: task_key(&snapshot.composer_task_id, &id),
            id,
            label,
            action,
        })
        .collect()
}

fn model_option_list(snapshot: &ComposerViewSnapshot) -> Vec<DropdownOption> {
    snapshot
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
        .collect()
}

fn reasoning_option_list() -> Vec<DropdownOption> {
    COMPOSER_REASONING_OPTIONS
        .iter()
        .map(|(id, label)| DropdownOption::new(*id, *label))
        .collect()
}

fn review_option_list() -> Vec<DropdownOption> {
    COMPOSER_REVIEW_OPTIONS
        .iter()
        .map(|(id, label)| DropdownOption::new(*id, *label))
        .collect()
}

fn menu_row(
    item: MenuEntry,
    entries: Signal<Vec<MenuEntry>>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
) -> impl IntoView {
    let id = item.id.clone();
    let action = item.action.clone();
    let mut built = ActionMenuItem::new(item.label);
    if item.active {
        built = built.active(true);
    }
    widget(built)
        .bind(move |view| {
            if let Some(found) = entries.with(|rows| {
                rows.iter()
                    .find(|row| row.id == id)
                    .map(|row| (row.label.clone(), row.active))
            }) {
                view.label = Arc::from(found.0);
                view.active = found.1;
            }
        })
        .on(move |_event: &Activate| dispatch(&sink, &binding, action.clone()))
}

fn menu_surface(
    open: Signal<bool>,
    entries: Signal<Vec<MenuEntry>>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
) -> impl IntoView {
    open.then_show(move || {
        let sink = Arc::clone(&sink);
        let binding = Arc::clone(&binding);
        entries.each(
            |item| item.key.clone(),
            move |item| menu_row(item, entries, Arc::clone(&sink), Arc::clone(&binding)),
        )
    })
}

fn completion_row(
    item: CompletionEntry,
    entries: Signal<Vec<CompletionEntry>>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
) -> impl IntoView {
    let id = item.id.clone();
    let action = item.action;
    widget(ActionMenuItem::new(item.label))
        .bind(move |view| {
            if let Some(label) = entries.with(|rows| {
                rows.iter()
                    .find(|row| row.id == id)
                    .map(|row| row.label.clone())
            }) {
                view.label = Arc::from(label);
            }
        })
        .on(move |_event: &Activate| dispatch(&sink, &binding, action.clone()))
}

struct LiveSlots {
    plus_slot: EntityRef<Stack>,
    plus_menu: EntityRef<ActionMenu>,
    attach: EntityRef<IconButton>,
    permission_slot: EntityRef<Stack>,
    permission_icon: EntityRef<IconGlyph>,
    permission_menu: EntityRef<ActionMenu>,
    worktree_slot: EntityRef<Stack>,
    worktree_menu: EntityRef<ActionMenu>,
}

#[derive(Clone)]
struct Chrome {
    plus_slot: EntityRef<Stack>,
    plus_menu: EntityRef<ActionMenu>,
    attach: EntityRef<IconButton>,
    permission_slot: EntityRef<Stack>,
    permission_icon: EntityRef<IconGlyph>,
    permission_menu: EntityRef<ActionMenu>,
    worktree_slot: EntityRef<Stack>,
    worktree_menu: EntityRef<ActionMenu>,
    plus_open: Signal<bool>,
    plus_entries: Signal<Vec<MenuEntry>>,
    permission_open: Signal<bool>,
    permission_label: Signal<Arc<str>>,
    permission_entries: Signal<Vec<MenuEntry>>,
    worktree_open: Signal<bool>,
    worktree_label: Signal<Arc<str>>,
    worktree_entries: Signal<Vec<MenuEntry>>,
    sink: IntentSink,
    binding: Arc<Mutex<ComposerBinding>>,
}

fn extra_row(row: ExtraRow, chrome: Chrome) -> AnyView {
    match row {
        ExtraRow::Plus => plus_slot_view(&chrome).into_any(),
        ExtraRow::Attach => attach_view(&chrome).into_any(),
        ExtraRow::Permission => permission_slot_view(&chrome).into_any(),
        ExtraRow::Worktree => worktree_slot_view(&chrome).into_any(),
        ExtraRow::Dynamic {
            label,
            kind,
            action,
            ..
        } => {
            let sink = Arc::clone(&chrome.sink);
            let binding = Arc::clone(&chrome.binding);
            widget(extra_button(&label, kind))
                .on_activate(move || dispatch(&sink, &binding, action.clone()))
                .into_any()
        }
    }
}

fn plus_slot_view(chrome: &Chrome) -> impl IntoView {
    let open = chrome.plus_open;
    let entries = chrome.plus_entries;
    let sink = Arc::clone(&chrome.sink);
    let binding = Arc::clone(&chrome.binding);
    let toggle_sink = Arc::clone(&chrome.sink);
    let toggle_binding = Arc::clone(&chrome.binding);
    widget(trigger_slot(PLUS_SLOT_SIZE, PLUS_SLOT_SIZE))
        .entity_ref(chrome.plus_slot)
        .children(
            widget(composer_plus_menu(false))
                .entity_ref(chrome.plus_menu)
                .bind(move |menu| menu.popover.open = open.get())
                .on(move |_event: &PopoverToggled| {
                    dispatch(
                        &toggle_sink,
                        &toggle_binding,
                        ComposerInputAction::ToggleActions,
                    );
                })
                .children(menu_surface(open, entries, sink, binding)),
        )
}

fn attach_view(chrome: &Chrome) -> impl IntoView {
    let sink = Arc::clone(&chrome.sink);
    let binding = Arc::clone(&chrome.binding);
    widget(composer_attach_button())
        .entity_ref(chrome.attach)
        .on_activate(move || {
            dispatch(
                &sink,
                &binding,
                ComposerInputAction::Plus("add-file".to_owned()),
            )
        })
}

fn permission_slot_view(chrome: &Chrome) -> impl IntoView {
    let open = chrome.permission_open;
    let label = chrome.permission_label;
    let entries = chrome.permission_entries;
    let sink = Arc::clone(&chrome.sink);
    let binding = Arc::clone(&chrome.binding);
    let toggle_sink = Arc::clone(&chrome.sink);
    let toggle_binding = Arc::clone(&chrome.binding);
    widget(Stack::row(4.0))
        .entity_ref(chrome.permission_slot)
        .children((
            widget(IconGlyph::new(Icon::ShieldCheck)).entity_ref(chrome.permission_icon),
            widget(composer_menu("", false))
                .entity_ref(chrome.permission_menu)
                .bind(move |menu| {
                    menu.popover.open = open.get();
                    menu.popover.trigger = label.get();
                })
                .on(move |_event: &PopoverToggled| {
                    dispatch(
                        &toggle_sink,
                        &toggle_binding,
                        ComposerInputAction::TogglePermission,
                    );
                })
                .children(menu_surface(open, entries, sink, binding)),
        ))
}

fn worktree_slot_view(chrome: &Chrome) -> impl IntoView {
    let open = chrome.worktree_open;
    let label = chrome.worktree_label;
    let entries = chrome.worktree_entries;
    let sink = Arc::clone(&chrome.sink);
    let binding = Arc::clone(&chrome.binding);
    let toggle_sink = Arc::clone(&chrome.sink);
    let toggle_binding = Arc::clone(&chrome.binding);
    widget(Stack::row(4.0))
        .entity_ref(chrome.worktree_slot)
        .children((
            widget(IconGlyph::new(Icon::GitBranch)),
            widget(composer_menu("", false))
                .entity_ref(chrome.worktree_menu)
                .bind(move |menu| {
                    menu.popover.open = open.get();
                    menu.popover.trigger = label.get();
                })
                .on(move |_event: &PopoverToggled| {
                    dispatch(
                        &toggle_sink,
                        &toggle_binding,
                        ComposerInputAction::ToggleWorktree,
                    );
                })
                .children(menu_surface(open, entries, sink, binding)),
        ))
}

fn node_children(context: &AppContext, id: StableNodeId) -> Vec<StableNodeId> {
    context
        .world()
        .node(id)
        .map(|node| node.children.clone())
        .unwrap_or_default()
}

fn zip_menu(
    context: &AppContext,
    menu: Entity<ActionMenu>,
    entries: &[MenuEntry],
) -> HashMap<String, Entity<ActionMenuItem>> {
    let Some(branch) = node_children(context, menu.stable_id()).into_iter().next() else {
        return HashMap::new();
    };
    let Some(list) = node_children(context, branch).into_iter().next() else {
        return HashMap::new();
    };
    let mut items = HashMap::new();
    let mut seen = HashSet::new();
    let mut cursor = node_children(context, list).into_iter();
    for entry in entries {
        if !seen.insert(entry.id.clone()) {
            continue;
        }
        let Some(child) = cursor.next() else {
            break;
        };
        items.insert(entry.id.clone(), Entity::from_stable_id(child));
    }
    items
}

fn zip_completion(
    context: &AppContext,
    slot: Entity<Stack>,
    entries: &[CompletionEntry],
) -> HashMap<String, Entity<ActionMenuItem>> {
    let mut items = HashMap::new();
    let mut seen = HashSet::new();
    let mut cursor = node_children(context, slot.stable_id()).into_iter();
    for entry in entries {
        if !seen.insert(entry.id.clone()) {
            continue;
        }
        let Some(child) = cursor.next() else {
            break;
        };
        items.insert(entry.id.clone(), Entity::from_stable_id(child));
    }
    items
}

fn zip_extra_buttons(
    context: &AppContext,
    extras: Entity<Stack>,
    rows: &[ExtraRow],
) -> HashMap<String, Entity<Button>> {
    let mut buttons = HashMap::new();
    let mut cursor = node_children(context, extras.stable_id()).into_iter();
    for row in rows {
        let Some(child) = cursor.next() else {
            break;
        };
        if let ExtraRow::Dynamic { id, .. } = row {
            buttons.insert(id.clone(), Entity::from_stable_id(child));
        }
    }
    buttons
}

fn widen_extras(context: &mut AppContext, extras: Entity<Stack>) -> Result<(), FrameworkError> {
    context.update_component(extras, |stack, _| {
        let mut hidden = false;
        stack.share_layouts(&mut |layout| hidden = layout.hidden);
        *stack = Stack::fill_row(6.0);
        stack.share_layouts(&mut |layout| Arc::make_mut(layout).hidden = hidden);
    })
}

pub struct ComposerView {
    sink: IntentSink,
    slots: LiveSlots,
    worktree_fallback_slot: Entity<Stack>,
    worktree_fallback_menu: Entity<ActionMenu>,
    composer_text: Bound<String>,
    composer_placeholder: Bound<Arc<str>>,
    composer_disabled: Bound<bool>,
    composer_height: Bound<f32>,
    composer_atoms: Bound<Arc<[TextAtomSpan]>>,
    composer_task: Bound<Option<String>>,
    browser_disabled: Bound<bool>,
    send_disabled: Bound<bool>,
    plus_open: Bound<bool>,
    plus_entries: Bound<Vec<MenuEntry>>,
    permission_open: Bound<bool>,
    permission_label: Bound<Arc<str>>,
    permission_entries: Bound<Vec<MenuEntry>>,
    worktree_open: Bound<bool>,
    worktree_label: Bound<Arc<str>>,
    worktree_entries: Bound<Vec<MenuEntry>>,
    model_id: Bound<String>,
    model_options: Bound<Vec<DropdownOption>>,
    reasoning_id: Bound<String>,
    extra_rows: Bound<Vec<ExtraRow>>,
    completion_rows: Bound<Vec<CompletionEntry>>,
    review_selection: Bound<String>,
    review_placeholder: Bound<Arc<str>>,
    review_text: Bound<String>,
    review_submit_disabled: Bound<bool>,
    pub(crate) composer_generation: ComposerGeneration,
    pub(crate) composer_binding: Arc<Mutex<ComposerBinding>>,
    pub(crate) browser_open: Entity<Button>,
    /// Retained composer surface: suggestions/review, input card, and the
    /// small controls row all live under one stage, while the card itself
    /// remains limited to the editor and send/stop action.
    pub(crate) stage: Entity<Stack>,
    pub(crate) composer_dock: Entity<Card>,
    pub(crate) composer: Entity<TextArea>,
    pub(crate) composer_toolbar: Entity<Stack>,
    pub(crate) toolbar_actions: Entity<Stack>,
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
        let composer_binding = Arc::new(Mutex::new(ComposerBinding::new(
            snapshot.window_id,
            snapshot.composer_task_id.clone(),
            snapshot.composer_revision,
            snapshot.composer.clone(),
            snapshot.composer_turn_id.clone(),
        )));
        let composer_text = Bound::new();
        let composer_placeholder = Bound::new();
        let composer_disabled = Bound::new();
        let composer_height = Bound::new();
        let composer_atoms = Bound::new();
        let composer_task = Bound::new();
        let browser_disabled = Bound::new();
        let send_disabled = Bound::new();
        let plus_open = Bound::new();
        let plus_entries_bound = Bound::new();
        let permission_open = Bound::new();
        let permission_label = Bound::new();
        let permission_entries_bound = Bound::new();
        let worktree_open = Bound::new();
        let worktree_label = Bound::new();
        let worktree_entries_bound = Bound::new();
        let model_id = Bound::new();
        let model_options = Bound::new();
        let reasoning_id = Bound::new();
        let extra_rows_bound = Bound::new();
        let completion_rows = Bound::new();
        let review_selection = Bound::new();
        let review_placeholder = Bound::new();
        let review_text = Bound::new();
        let review_submit_disabled = Bound::new();
        let stored_slots = Arc::new(Mutex::new(None));
        let text_slot = composer_text.clone();
        let placeholder_slot = composer_placeholder.clone();
        let disabled_slot = composer_disabled.clone();
        let height_slot = composer_height.clone();
        let atoms_slot = composer_atoms.clone();
        let task_slot = composer_task.clone();
        let browser_slot = browser_disabled.clone();
        let send_slot = send_disabled.clone();
        let plus_open_slot = plus_open.clone();
        let plus_entries_slot = plus_entries_bound.clone();
        let permission_open_slot = permission_open.clone();
        let permission_label_slot = permission_label.clone();
        let permission_entries_slot = permission_entries_bound.clone();
        let worktree_open_slot = worktree_open.clone();
        let worktree_label_slot = worktree_label.clone();
        let worktree_entries_slot = worktree_entries_bound.clone();
        let model_id_slot = model_id.clone();
        let model_options_slot = model_options.clone();
        let reasoning_id_slot = reasoning_id.clone();
        let extra_rows_slot = extra_rows_bound.clone();
        let completion_slot_signal = completion_rows.clone();
        let review_selection_slot = review_selection.clone();
        let review_placeholder_slot = review_placeholder.clone();
        let review_text_slot = review_text.clone();
        let review_submit_slot = review_submit_disabled.clone();
        let view_sink = Arc::clone(&sink);
        let view_binding = Arc::clone(&composer_binding);
        let slots_for_view = Arc::clone(&stored_slots);
        let (
            mounted,
            (
                (
                    _stage,
                    composer_dock,
                    composer,
                    composer_toolbar,
                    composer_actions,
                    browser_open,
                    toolbar_actions,
                ),
                (send, interrupt, model, reasoning),
                (review_slot, review_target, review_value, review_submit, review_cancel),
            ),
        ) = context.mount_view_detached(document_id, move || {
            let dock_ref = entity_ref::<Card>();
            let stage_ref = entity_ref::<Stack>();
            let composer_ref = entity_ref::<TextArea>();
            let toolbar_ref = entity_ref::<Stack>();
            let actions_ref = entity_ref::<Stack>();
            let toolbar_actions_ref = entity_ref::<Stack>();
            let browser_ref = entity_ref::<Button>();
            let send_ref = entity_ref::<IconButton>();
            let interrupt_ref = entity_ref::<IconButton>();
            let model_ref = entity_ref::<Dropdown>();
            let reasoning_ref = entity_ref::<Dropdown>();
            let review_slot_ref = entity_ref::<Stack>();
            let review_target_ref = entity_ref::<Dropdown>();
            let review_value_ref = entity_ref::<TextInput>();
            let review_submit_ref = entity_ref::<Button>();
            let review_cancel_ref = entity_ref::<Button>();
            let plus_slot_ref = entity_ref::<Stack>();
            let plus_menu_ref = entity_ref::<ActionMenu>();
            let attach_ref = entity_ref::<IconButton>();
            let permission_slot_ref = entity_ref::<Stack>();
            let permission_icon_ref = entity_ref::<IconGlyph>();
            let permission_menu_ref = entity_ref::<ActionMenu>();
            let worktree_slot_ref = entity_ref::<Stack>();
            let worktree_menu_ref = entity_ref::<ActionMenu>();
            let text = text_slot.install(signal(snapshot.composer.clone()));
            let placeholder =
                placeholder_slot.install(signal(Arc::from(snapshot.composer_placeholder.as_str())));
            let disabled = disabled_slot.install(signal(snapshot.composer_disabled));
            let height =
                height_slot.install(signal(clamped_composer_height(snapshot.composer_height)));
            let atoms =
                atoms_slot.install(signal(composer_atom_chips(&snapshot.composer_atom_spans)));
            let task_id = task_slot.install(signal(snapshot.composer_task_id.clone()));
            let browser_off = browser_slot.install(signal(!snapshot.can_open_browser));
            let send_off =
                send_slot.install(signal(!snapshot.can_send || snapshot.pending_blocks_send));
            let plus_open = plus_open_slot.install(signal(snapshot.composer_plus_open));
            let plus_entries = plus_entries_slot.install(signal(plus_entries(snapshot)));
            let permission_open =
                permission_open_slot.install(signal(snapshot.composer_permission_menu_open));
            let permission_label = permission_label_slot
                .install(signal(Arc::from(snapshot.permission_label.as_str())));
            let permission_entries =
                permission_entries_slot.install(signal(permission_entries(snapshot)));
            let worktree_open =
                worktree_open_slot.install(signal(snapshot.composer_worktree_menu_open));
            let worktree_label = worktree_label_slot.install(signal(Arc::from(
                snapshot.worktree_label.as_deref().unwrap_or(""),
            )));
            let worktree_entries =
                worktree_entries_slot.install(signal(worktree_entries(snapshot)));
            let model_id = model_id_slot.install(signal(snapshot.model.clone()));
            let model_options = model_options_slot.install(signal(model_option_list(snapshot)));
            let reasoning_id = reasoning_id_slot.install(signal(snapshot.reasoning.clone()));
            let rows = extra_rows_slot.install(signal(extra_rows(snapshot)));
            let completion = completion_slot_signal.install(signal(completion_entries(snapshot)));
            let review_selected = review_selection_slot.install(signal(
                snapshot
                    .review_target
                    .clone()
                    .unwrap_or_else(|| "changes".to_owned()),
            ));
            let review_hint = review_placeholder_slot.install(signal(Arc::from(
                review_value_placeholder(snapshot.review_target.as_deref().unwrap_or("changes")),
            )));
            let review_value_text = review_text_slot.install(signal(snapshot.review_value.clone()));
            let review_target_name = snapshot.review_target.as_deref().unwrap_or("changes");
            let submit_off = review_submit_slot.install(signal(
                snapshot.composer_disabled
                    || (review_target_name != "changes" && snapshot.review_value.trim().is_empty()),
            ));
            *slots_for_view.lock().unwrap() = Some(LiveSlots {
                plus_slot: plus_slot_ref,
                plus_menu: plus_menu_ref,
                attach: attach_ref,
                permission_slot: permission_slot_ref,
                permission_icon: permission_icon_ref,
                permission_menu: permission_menu_ref,
                worktree_slot: worktree_slot_ref,
                worktree_menu: worktree_menu_ref,
            });
            let chrome = Chrome {
                plus_slot: plus_slot_ref,
                plus_menu: plus_menu_ref,
                attach: attach_ref,
                permission_slot: permission_slot_ref,
                permission_icon: permission_icon_ref,
                permission_menu: permission_menu_ref,
                worktree_slot: worktree_slot_ref,
                worktree_menu: worktree_menu_ref,
                plus_open,
                plus_entries,
                permission_open,
                permission_label,
                permission_entries,
                worktree_open,
                worktree_label,
                worktree_entries,
                sink: Arc::clone(&view_sink),
                binding: Arc::clone(&view_binding),
            };
            let editor_sink = Arc::clone(&view_sink);
            let editor_binding = Arc::clone(&view_binding);
            let browser_sink = Arc::clone(&view_sink);
            let browser_binding = Arc::clone(&view_binding);
            let send_sink = Arc::clone(&view_sink);
            let send_binding = Arc::clone(&view_binding);
            let interrupt_sink = Arc::clone(&view_sink);
            let interrupt_binding = Arc::clone(&view_binding);
            let model_sink = Arc::clone(&view_sink);
            let model_binding = Arc::clone(&view_binding);
            let reasoning_sink = Arc::clone(&view_sink);
            let reasoning_binding = Arc::clone(&view_binding);
            let review_target_sink = Arc::clone(&view_sink);
            let review_target_binding = Arc::clone(&view_binding);
            let review_value_sink = Arc::clone(&view_sink);
            let review_value_binding = Arc::clone(&view_binding);
            let review_submit_sink = Arc::clone(&view_sink);
            let review_submit_binding = Arc::clone(&view_binding);
            let review_cancel_sink = Arc::clone(&view_sink);
            let review_cancel_binding = Arc::clone(&view_binding);
            let completion_sink = Arc::clone(&view_sink);
            let completion_binding = Arc::clone(&view_binding);
            let interrupt = widget(composer_interrupt_button(true))
                .entity_ref(interrupt_ref)
                .on_activate(move || {
                    dispatch(
                        &interrupt_sink,
                        &interrupt_binding,
                        ComposerInputAction::Interrupt,
                    )
                });
            let row_chrome = chrome.clone();
            let extras_each = rows
                .each(
                    move |row| {
                        let task = task_id.get();
                        extra_key(row, &task)
                    },
                    move |row| extra_row(row, row_chrome.clone()),
                )
                .horizontal(6.0);
            let completion_each = completion
                .each(
                    |item| item.key.clone(),
                    move |item| {
                        completion_row(
                            item,
                            completion,
                            Arc::clone(&completion_sink),
                            Arc::clone(&completion_binding),
                        )
                    },
                )
                .gap(1.0);
            let dock = widget(composer_card()).entity_ref(dock_ref).children((
                widget(flatten_composer_textarea(TextArea::new(
                    snapshot.composer.clone(),
                )))
                .entity_ref(composer_ref)
                .placeholder(placeholder)
                .disabled(disabled)
                .bind(move |area| {
                    area.atom_spans = atoms.get();
                    let layout = Arc::make_mut(&mut area.style.layout);
                    layout.height = Some(LengthSpec::Px(height.get()));
                })
                .on_input(move |event: &TextChanged| {
                    text.set(event.value.to_string());
                    edit_composer(&editor_binding, &editor_sink, event.value.to_string());
                }),
                widget(
                    Stack::row(6.0)
                        .width(LengthSpec::Percent(100.0))
                        .justify(JustifySpec::End),
                )
                .entity_ref(actions_ref)
                .children((
                    widget(composer_send_button(true))
                        .entity_ref(send_ref)
                        .disabled(send_off)
                        .on_activate(move || {
                            dispatch(&send_sink, &send_binding, ComposerInputAction::Submit)
                        }),
                    interrupt,
                )),
            ));
            let review = widget(Stack::row(6.0))
                .entity_ref(review_slot_ref)
                .children((
                    widget(composer_review_dropdown(review_target_name))
                        .entity_ref(review_target_ref)
                        .bind(move |field| {
                            field.selection =
                                DropdownSelection::Single(Some(Arc::from(review_selected.get())));
                            field.options = review_option_list();
                        })
                        .on(move |event: &DropdownEvent<Arc<str>>| {
                            if let DropdownEvent::Select(value) = event {
                                dispatch(
                                    &review_target_sink,
                                    &review_target_binding,
                                    ComposerInputAction::ReviewTarget(value.to_string()),
                                );
                            }
                        }),
                    widget(composer_review_value(
                        &snapshot.review_value,
                        review_value_placeholder(review_target_name),
                    ))
                    .entity_ref(review_value_ref)
                    .value(review_value_text)
                    .placeholder(review_hint)
                    .on_input(move |event: &TextChanged| {
                        dispatch(
                            &review_value_sink,
                            &review_value_binding,
                            ComposerInputAction::ReviewValue(event.value.to_string()),
                        );
                    }),
                    widget(extra_button("开始审查", ButtonKind::Subtle))
                        .entity_ref(review_submit_ref)
                        .disabled(submit_off)
                        .on_activate(move || {
                            dispatch(
                                &review_submit_sink,
                                &review_submit_binding,
                                ComposerInputAction::SubmitReview,
                            )
                        }),
                    widget(extra_button("取消", ButtonKind::Ghost))
                        .entity_ref(review_cancel_ref)
                        .on_activate(move || {
                            dispatch(
                                &review_cancel_sink,
                                &review_cancel_binding,
                                ComposerInputAction::CancelReview,
                            )
                        }),
                ));
            let model = widget(composer_model_dropdown(snapshot))
                .entity_ref(model_ref)
                .bind(move |field| {
                    field.selection = DropdownSelection::Single(Some(Arc::from(model_id.get())));
                    field.options = model_options.get();
                    field.disabled = disabled.get();
                })
                .on(move |event: &DropdownEvent<Arc<str>>| {
                    if let DropdownEvent::Select(value) = event {
                        dispatch(
                            &model_sink,
                            &model_binding,
                            ComposerInputAction::Model(value.to_string()),
                        );
                    }
                });
            let reasoning = widget(composer_reasoning_dropdown(
                &snapshot.reasoning,
                snapshot.composer_disabled,
            ))
            .entity_ref(reasoning_ref)
            .bind(move |field| {
                field.selection = DropdownSelection::Single(Some(Arc::from(reasoning_id.get())));
                field.options = reasoning_option_list();
                field.disabled = disabled.get();
            })
            .on(move |event: &DropdownEvent<Arc<str>>| {
                if let DropdownEvent::Select(value) = event {
                    dispatch(
                        &reasoning_sink,
                        &reasoning_binding,
                        ComposerInputAction::Reasoning(value.to_string()),
                    );
                }
            });
            let toolbar = widget(Stack::bar(8.0).justify(JustifySpec::SpaceBetween))
                .entity_ref(toolbar_ref)
                .children((
                    extras_each,
                    widget(Stack::row(6.0))
                        .entity_ref(toolbar_actions_ref)
                        .children((
                            widget(extra_button("浏览器", ButtonKind::Text))
                                .entity_ref(browser_ref)
                                .disabled(browser_off)
                                .on_activate(move || {
                                    let target = browser_binding.lock().unwrap().target.clone();
                                    if let Some(task_id) = target.task_id {
                                        emit(
                                            &browser_sink,
                                            ShellIntent::OpenBrowser {
                                                window_id: target.window_id,
                                                task_id,
                                            },
                                        );
                                    }
                                }),
                            model,
                            reasoning,
                        )),
                ));
            with_refs(
                widget(
                    Stack::column(8.0)
                        .align(nana_ui::runtime::AlignSpec::Stretch)
                        .width(LengthSpec::Percent(100.0)),
                )
                .entity_ref(stage_ref)
                .children((completion_each, review, dock, toolbar)),
                (
                    (
                        stage_ref,
                        dock_ref,
                        composer_ref,
                        toolbar_ref,
                        actions_ref,
                        browser_ref,
                        toolbar_actions_ref,
                    ),
                    (send_ref, interrupt_ref, model_ref, reasoning_ref),
                    (
                        review_slot_ref,
                        review_target_ref,
                        review_value_ref,
                        review_submit_ref,
                        review_cancel_ref,
                    ),
                ),
            )
        })?;
        let roots = mounted.roots().to_vec();
        if roots.len() != 1 {
            mounted.unmount(context)?;
            return Err(FrameworkError::InvalidInput);
        }
        let stage = Entity::<Stack>::from_stable_id(roots[0]);
        let completion_slot = node_children(context, stage.stable_id())
            .into_iter()
            .next()
            .map(Entity::<Stack>::from_stable_id)
            .ok_or(FrameworkError::InvalidInput)?;
        drop(mounted);
        let extras_id = node_children(context, composer_toolbar.stable_id())
            .into_iter()
            .next()
            .ok_or(FrameworkError::InvalidInput)?;
        let slots = stored_slots
            .lock()
            .unwrap()
            .take()
            .ok_or(FrameworkError::InvalidInput)?;
        let plus_slot = slots.plus_slot.get().ok_or(FrameworkError::InvalidInput)?;
        let plus_menu = slots.plus_menu.get().ok_or(FrameworkError::InvalidInput)?;
        let attach = slots.attach.get().ok_or(FrameworkError::InvalidInput)?;
        let permission_slot = slots
            .permission_slot
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        let permission_menu = slots
            .permission_menu
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        #[cfg(test)]
        let permission_icon = slots
            .permission_icon
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        let (_, worktree_fallback_slot) = context.mount_view_detached(document_id, || {
            let slot = entity_ref::<Stack>();
            with_refs(widget(Stack::row(4.0)).entity_ref(slot), slot)
        })?;
        let (_, worktree_fallback_menu) = context.mount_view_detached(document_id, || {
            let menu = entity_ref::<ActionMenu>();
            with_refs(widget(composer_menu("", false)).entity_ref(menu), menu)
        })?;
        let (worktree_slot, worktree_menu) = live_worktree(
            context,
            &slots,
            worktree_fallback_slot,
            worktree_fallback_menu,
        );
        let mut view = Self {
            sink,
            slots,
            worktree_fallback_slot,
            worktree_fallback_menu,
            composer_text,
            composer_placeholder,
            composer_disabled,
            composer_height,
            composer_atoms,
            composer_task,
            browser_disabled,
            send_disabled,
            plus_open,
            plus_entries: plus_entries_bound,
            permission_open,
            permission_label,
            permission_entries: permission_entries_bound,
            worktree_open,
            worktree_label,
            worktree_entries: worktree_entries_bound,
            model_id,
            model_options,
            reasoning_id,
            extra_rows: extra_rows_bound,
            completion_rows,
            review_selection,
            review_placeholder,
            review_text,
            review_submit_disabled,
            composer_generation: ComposerGeneration::default(),
            composer_binding,
            browser_open,
            stage,
            composer_dock,
            composer,
            composer_toolbar,
            toolbar_actions,
            extras: Entity::from_stable_id(extras_id),
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
            composer_actions,
            send,
            interrupt: Some(interrupt),
            last_failed_revision: None,
        };
        bind_composer_keys(
            context,
            view.composer,
            Arc::clone(&view.sink),
            Arc::clone(&view.composer_binding),
        )?;
        view.sync(context, document_id, snapshot)?;
        Ok(view)
    }

    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let composer_generation = ComposerGeneration::new(
            snapshot.composer_task_id.clone(),
            snapshot.composer_revision,
        );
        if snapshot.composer_task_id != self.composer_generation.task_id {
            self.last_failed_revision = None;
        }
        let failed_resync =
            snapshot.apply_failed && self.last_failed_revision != Some(snapshot.composer_revision);
        if failed_resync {
            self.last_failed_revision = Some(snapshot.composer_revision);
        }
        let write_composer = !composer_is_focused(context, self.composer)
            || self.composer_generation != composer_generation
            || failed_resync;
        self.composer_generation = composer_generation;
        if write_composer {
            *self.composer_binding.lock().unwrap() = ComposerBinding::new(
                snapshot.window_id,
                snapshot.composer_task_id.clone(),
                snapshot.composer_revision,
                snapshot.composer.clone(),
                snapshot.composer_turn_id.clone(),
            );
            self.composer_text.set(snapshot.composer.clone());
            context.update_component(self.composer, |area, _| {
                if area.state.value != snapshot.composer {
                    area.state.replace_value(snapshot.composer.clone());
                }
            })?;
        } else {
            self.composer_binding.lock().unwrap().target.turn_id =
                snapshot.composer_turn_id.clone();
        }
        let rows = extra_rows(snapshot);
        let completion = completion_entries(snapshot);
        let plus = plus_entries(snapshot);
        let permission = permission_entries(snapshot);
        let worktree = worktree_entries(snapshot);
        {
            let mut binding = self.composer_binding.lock().unwrap();
            binding.sync_keys(
                &snapshot.composer,
                snapshot.can_send,
                snapshot.composer_disabled,
                snapshot.pending_blocks_send,
                completion
                    .iter()
                    .map(|entry| entry.action.clone())
                    .collect(),
            );
        }
        self.composer_placeholder
            .set(Arc::from(snapshot.composer_placeholder.as_str()));
        self.composer_disabled.set(snapshot.composer_disabled);
        self.composer_height
            .set(clamped_composer_height(snapshot.composer_height));
        self.composer_atoms
            .set(composer_atom_chips(&snapshot.composer_atom_spans));
        self.composer_task.set(snapshot.composer_task_id.clone());
        self.browser_disabled.set(!snapshot.can_open_browser);
        self.send_disabled
            .set(!snapshot.can_send || snapshot.pending_blocks_send);
        self.plus_open.set(snapshot.composer_plus_open);
        self.plus_entries.set(plus.clone());
        self.permission_open
            .set(snapshot.composer_permission_menu_open);
        self.permission_label
            .set(Arc::from(snapshot.permission_label.as_str()));
        self.permission_entries.set(permission.clone());
        self.worktree_open.set(snapshot.composer_worktree_menu_open);
        self.worktree_label
            .set(Arc::from(snapshot.worktree_label.as_deref().unwrap_or("")));
        self.worktree_entries.set(worktree.clone());
        self.model_id.set(snapshot.model.clone());
        self.model_options.set(model_option_list(snapshot));
        self.reasoning_id.set(snapshot.reasoning.clone());
        self.extra_rows.set(rows.clone());
        self.completion_rows.set(completion.clone());
        let review_target = snapshot
            .review_target
            .clone()
            .unwrap_or_else(|| "changes".to_owned());
        self.review_selection.set(review_target.clone());
        self.review_placeholder
            .set(Arc::from(review_value_placeholder(&review_target)));
        if context.world().focused(document_id) != Some(self.review_value.stable_id()) {
            self.review_text.set(snapshot.review_value.clone());
        }
        self.review_submit_disabled.set(
            snapshot.composer_disabled
                || (review_target != "changes" && snapshot.review_value.trim().is_empty()),
        );
        context.flush_reactive()?;
        self.adopt_slots(context)?;
        self.place_stage(context, snapshot)?;
        self.plus_items = zip_menu(context, self.plus_menu, &plus);
        self.permission_items = zip_menu(context, self.permission_menu, &permission);
        self.worktree_items = zip_menu(context, self.worktree_menu, &worktree);
        self.completion_items = zip_completion(context, self.completion_slot, &completion);
        self.extra_buttons = zip_extra_buttons(context, self.extras, &rows);
        widen_extras(context, self.extras)
    }

    fn adopt_slots(&mut self, context: &AppContext) -> Result<(), FrameworkError> {
        self.plus_slot = self
            .slots
            .plus_slot
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        self.plus_menu = self
            .slots
            .plus_menu
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        self.attach = self
            .slots
            .attach
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        self.permission_slot = self
            .slots
            .permission_slot
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        self.permission_menu = self
            .slots
            .permission_menu
            .get()
            .ok_or(FrameworkError::InvalidInput)?;
        #[cfg(test)]
        {
            self.permission_icon = self
                .slots
                .permission_icon
                .get()
                .ok_or(FrameworkError::InvalidInput)?;
        }
        let (slot, menu) = live_worktree(
            context,
            &self.slots,
            self.worktree_fallback_slot,
            self.worktree_fallback_menu,
        );
        self.worktree_slot = slot;
        self.worktree_menu = menu;
        Ok(())
    }

    fn place_stage(
        &self,
        context: &mut AppContext,
        snapshot: &ComposerViewSnapshot,
    ) -> Result<(), FrameworkError> {
        // The outlined card intentionally contains only the editor and its
        // submit/stop action. Suggestions, review controls, and the small
        // action toolbar are siblings in `stage`, outside the card.
        let review_children = match snapshot.review_target.as_deref() {
            None => Vec::new(),
            Some("changes") => vec![
                self.review_target.stable_id(),
                self.review_submit.stable_id(),
                self.review_cancel.stable_id(),
            ],
            Some(_) => vec![
                self.review_target.stable_id(),
                self.review_value.stable_id(),
                self.review_submit.stable_id(),
                self.review_cancel.stable_id(),
            ],
        };
        reconcile_children(context, self.review_slot.stable_id(), &review_children)?;
        let interrupt = self.interrupt.ok_or(FrameworkError::InvalidInput)?;
        let show_interrupt = snapshot.can_interrupt
            && snapshot.composer_turn_id.is_some()
            && (!snapshot.can_send || snapshot.pending_blocks_send);
        let second = if show_interrupt {
            interrupt.stable_id()
        } else {
            self.send.stable_id()
        };
        reconcile_children(
            context,
            self.composer_toolbar.stable_id(),
            &[self.extras.stable_id(), self.toolbar_actions.stable_id()],
        )?;
        reconcile_children(
            context,
            self.toolbar_actions.stable_id(),
            &[
                self.browser_open.stable_id(),
                self.model.stable_id(),
                self.reasoning.stable_id(),
            ],
        )?;
        reconcile_children(
            context,
            self.composer_dock.stable_id(),
            &[self.composer.stable_id(), self.composer_actions.stable_id()],
        )?;
        reconcile_children(context, self.composer_actions.stable_id(), &[second])?;
        let mut stage = Vec::with_capacity(4);
        if !self.completion_rows.signal().get_untracked().is_empty() {
            stage.push(self.completion_slot.stable_id());
        }
        if snapshot.review_target.is_some() {
            stage.push(self.review_slot.stable_id());
        }
        stage.push(self.composer_dock.stable_id());
        stage.push(self.composer_toolbar.stable_id());
        reconcile_children(context, self.stage.stable_id(), &stage)
    }
}

fn live_worktree(
    context: &AppContext,
    slots: &LiveSlots,
    fallback_slot: Entity<Stack>,
    fallback_menu: Entity<ActionMenu>,
) -> (Entity<Stack>, Entity<ActionMenu>) {
    let slot = slots
        .worktree_slot
        .get()
        .filter(|slot| context.world().contains(slot.stable_id()));
    let menu = slots
        .worktree_menu
        .get()
        .filter(|menu| context.world().contains(menu.stable_id()));
    match (slot, menu) {
        (Some(slot), Some(menu)) => (slot, menu),
        _ => (fallback_slot, fallback_menu),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_input::ScriptedInput;
    use nana_ui::runtime::InputRouteOutcome;
    use nana_ui::runtime::TextSelection;
    use nana_ui_platform::InputModifiers;

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
    fn text_toolbar_buttons_stay_flat_when_disabled() {
        let button = extra_button("浏览器", ButtonKind::Text);
        assert_eq!(button.style.interaction.disabled.background, None);
        assert_eq!(button.style.interaction.disabled.border, None);
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
        context.append_child(host, view.stage).unwrap();
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
        context.append_child(host, view.stage).unwrap();
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

    fn tap(
        input: &mut ScriptedInput,
        context: &mut AppContext,
        name: &str,
        shift: bool,
        repeat: bool,
    ) -> InputRouteOutcome {
        input
            .press(
                context,
                name,
                InputModifiers {
                    shift,
                    ..Default::default()
                },
                repeat,
            )
            .unwrap()
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
        context.append_child(host, view.stage).unwrap();
        assert!(context
            .focus_node(document, view.composer.stable_id())
            .unwrap());
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
        let mut input = ScriptedInput::bind(&mut context, document);
        for repeat in [false, true, false] {
            assert!(tap(&mut input, &mut context, "Enter", false, repeat).prevent_default);
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
        assert!(tap(&mut input, &mut context, "Enter", true, false).prevent_default);
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
        let mut input = ScriptedInput::bind(&mut context, document);
        for name in ["ArrowDown", "ArrowUp", "ArrowUp"] {
            assert!(tap(&mut input, &mut context, name, false, false).prevent_default);
        }
        assert!(tap(&mut input, &mut context, "Enter", false, false).prevent_default);
        assert!(tap(&mut input, &mut context, "Enter", false, true).prevent_default);
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
        tap(&mut input, &mut context, "Tab", false, false);
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
        let mut input = ScriptedInput::bind(&mut context, document);
        tap(&mut input, &mut context, "Enter", false, false);
        assert!(events.lock().unwrap().is_empty());

        snapshot.can_send = true;
        snapshot.pending_blocks_send = true;
        view.sync(&mut context, document, &snapshot).unwrap();
        tap(&mut input, &mut context, "Enter", false, false);
        assert!(events.lock().unwrap().is_empty());

        snapshot.pending_blocks_send = false;
        snapshot.composer_disabled = true;
        view.sync(&mut context, document, &snapshot).unwrap();
        tap(&mut input, &mut context, "Enter", false, false);
        assert!(events.lock().unwrap().is_empty());

        snapshot.composer_disabled = false;
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(context
            .focus_node(document, view.composer.stable_id())
            .unwrap());
        tap(&mut input, &mut context, "Enter", false, false);
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(ShellIntent::AddressedComposer {
                action: ComposerInputAction::Submit,
                ..
            })
        ));
    }
}
