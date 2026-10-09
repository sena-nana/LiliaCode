//! One timeline entry: the rail icon, header, body slot and actions.
//!
//! The chrome is a reactive view bound to one `RowChrome` signal; the body
//! slot receives the entry's `NativeMarkdown`, which the timeline keeps across
//! updates so text selection and images survive a re-projection.

use std::sync::Arc;

use nana_ui::icons_tabler as tabler;
use nana_ui::runtime::view::{
    entity_ref, node_ref, signal, widget, with_refs, El, EntityRef, IntoView, Signal,
};
use nana_ui::runtime::{
    AlignSpec, AppContext, Button, DocumentId, Entity, FocusWithinChanged, FrameworkError,
    IconButton, IconGlyph, JustifySpec, LengthSpec, MountedView, PointerHoverChanged, PositionSpec,
    SemanticColorRole, StableNodeId, Stack, Text,
};
use nana_ui::{ButtonKind, ControlSize, Icon};
use nana_ui_core::{type_scale, RadiusTier};

use super::theme::{BUBBLE_MAX_WIDTH, TIMELINE_ENTRY_PADDING, TIMELINE_NODE, TIMELINE_RAIL};
use super::timeline::{TimelineRole, TimelineTone};
use crate::module::timeline::view::TimelineAction;
use crate::runtime_layout::Bound;

/// Everything about an entry except its body text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowChrome {
    pub role: TimelineRole,
    pub tone: TimelineTone,
    pub title: String,
    pub detail: String,
    pub expanded: bool,
    pub can_expand: bool,
    pub can_retry: bool,
    pub can_copy: bool,
    pub can_branch: bool,
    pub has_body: bool,
}

impl RowChrome {
    fn body_visible(&self) -> bool {
        self.has_body && (self.expanded || !self.can_expand)
    }

    fn has_actions(&self) -> bool {
        self.can_copy || self.can_branch || (self.can_retry && !self.is_step())
    }

    fn is_step(&self) -> bool {
        matches!(self.role, TimelineRole::Step(_) | TimelineRole::Group)
    }
}

pub(crate) type RowEmit = Arc<dyn Fn(TimelineAction) + Send + Sync>;

/// Handles of an entry's controls, by the action they emit.
#[derive(Clone, Copy, Default)]
pub(crate) struct RowActions {
    pub expand: Option<Entity<Button>>,
    pub copy: Option<Entity<IconButton>>,
    pub quote: Option<Entity<IconButton>>,
    pub retry: Option<Entity<IconButton>>,
    pub resume: Option<Entity<IconButton>>,
    pub fork: Option<Entity<IconButton>>,
}

struct ActionRefs {
    expand: EntityRef<Button>,
    copy: EntityRef<IconButton>,
    quote: EntityRef<IconButton>,
    retry: EntityRef<IconButton>,
    step_retry: EntityRef<IconButton>,
    resume: EntityRef<IconButton>,
    fork: EntityRef<IconButton>,
}

impl ActionRefs {
    fn new() -> Self {
        Self {
            expand: entity_ref(),
            copy: entity_ref(),
            quote: entity_ref(),
            retry: entity_ref(),
            step_retry: entity_ref(),
            resume: entity_ref(),
            fork: entity_ref(),
        }
    }

    fn resolve(&self, step: bool) -> RowActions {
        RowActions {
            expand: self.expand.get(),
            copy: self.copy.get(),
            quote: self.quote.get(),
            retry: if step {
                self.step_retry.get()
            } else {
                self.retry.get()
            },
            resume: self.resume.get(),
            fork: self.fork.get(),
        }
    }
}

/// The mounted chrome of one entry.
pub(crate) struct RowView {
    pub root: StableNodeId,
    /// Where the entry's markdown is attached.
    pub body: StableNodeId,
    pub role: TimelineRole,
    pub actions: RowActions,
    state: Bound<RowChrome>,
    mounted: MountedView,
}

impl RowView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        id: &str,
        chrome: RowChrome,
        emit: RowEmit,
    ) -> Result<Self, FrameworkError> {
        let role = chrome.role;
        let step = chrome.is_step();
        let state = Bound::new();
        let install = state.clone();
        let id = id.to_owned();
        let mut refs = None;
        let (mounted, body) = context.mount_view_detached(document, || {
            let state = install.install(signal(chrome));
            let reveal = Reveal {
                hovered: signal(false),
                focused: signal(false),
            };
            let body = node_ref();
            let actions = ActionRefs::new();
            let body_width = if role == TimelineRole::User {
                LengthSpec::Shrink
            } else {
                LengthSpec::Fill
            };
            let body_slot = widget(
                Stack::column(0.0)
                    .width(body_width)
                    .min_width(LengthSpec::Px(0.0)),
            )
            .node_ref(body)
            .visible(move || state.with(RowChrome::body_visible));
            let view = match role {
                TimelineRole::User => user_row(
                    body_slot,
                    reveal,
                    actions_row(&id, state, reveal, &emit, true, &actions),
                )
                .into_any(),
                TimelineRole::Reply => rail_row(
                    state,
                    false,
                    reply_body(
                        body_slot,
                        reveal,
                        actions_row(&id, state, reveal, &emit, false, &actions),
                    ),
                )
                .into_any(),
                TimelineRole::Step(_) | TimelineRole::Group => rail_row(
                    state,
                    true,
                    step_body(&id, state, &emit, body_slot, &actions),
                )
                .into_any(),
            };
            refs = Some(actions);
            with_refs(view, body)
        })?;
        let root = mounted
            .roots()
            .first()
            .copied()
            .ok_or(FrameworkError::InvalidInput)?;
        let actions = refs.map(|refs| refs.resolve(step)).unwrap_or_default();
        Ok(Self {
            root,
            body,
            role,
            actions,
            state,
            mounted,
        })
    }

    pub(crate) fn sync(&self, chrome: RowChrome) {
        self.state.signal().set_if_changed(chrome);
    }

    pub(crate) fn unmount(self, context: &mut AppContext) -> Result<(), FrameworkError> {
        self.mounted.unmount(context)
    }
}

fn entry_frame(gap: f32) -> Stack {
    Stack::column(gap)
        .padding_xy(0.0, TIMELINE_ENTRY_PADDING)
        .min_width(LengthSpec::Px(0.0))
        .with_layout(|layout| layout.position = PositionSpec::Relative)
}

/// What shows a message's actions: the pointer over the message, or keyboard
/// focus on one of them.
#[derive(Clone, Copy)]
struct Reveal {
    hovered: Signal<bool>,
    focused: Signal<bool>,
}

impl Reveal {
    fn shown(self) -> bool {
        self.hovered.get() || self.focused.get()
    }

    fn listen<C: nana_ui::runtime::ComponentView>(self, row: El<C>) -> El<C> {
        row.on(move |event: &PointerHoverChanged| {
            self.hovered.set_if_changed(event.hovered);
        })
        .on(move |event: &FocusWithinChanged| {
            self.focused.set_if_changed(event.focused);
        })
    }
}

fn user_row(body_slot: El<Stack>, reveal: Reveal, actions: impl IntoView) -> impl IntoView {
    let bubble = Stack::column(0.0)
        .padding_xy(14.0, 10.0)
        .surface(SemanticColorRole::AccentSoft)
        .width(LengthSpec::Shrink)
        .shrink(1.0)
        .min_width(LengthSpec::Px(0.0))
        .max_width(BUBBLE_MAX_WIDTH);
    let mut style = bubble.node_style();
    style.corner_radii = Some([
        Some(RadiusTier::Lg),
        Some(RadiusTier::Lg),
        Some(RadiusTier::Xs),
        Some(RadiusTier::Lg),
    ]);
    let bubble = bubble.style(style);
    reveal
        .listen(widget(entry_frame(4.0).align(AlignSpec::End).hittable()))
        .children((
            widget(Stack::bar(0.0).justify(JustifySpec::End))
                .children(widget(bubble).children(body_slot)),
            actions,
        ))
}

fn reply_body(body_slot: El<Stack>, reveal: Reveal, actions: impl IntoView) -> impl IntoView {
    reveal
        .listen(widget(body_column(6.0).hittable()))
        .children((body_slot, actions))
}

/// `[rail | body]`, with the 1px rail line behind process steps so
/// consecutive steps read as one thread.
fn rail_row<B: IntoView>(
    state: nana_ui::runtime::view::Signal<RowChrome>,
    line: bool,
    body: B,
) -> impl IntoView {
    let rail_line = widget(
        Stack::column(0.0)
            .surface(SemanticColorRole::Border)
            .with_layout(|layout| {
                layout.position = PositionSpec::Absolute;
                layout.offset_left = Some(LengthSpec::Px(TIMELINE_RAIL / 2.0 - 0.5));
                layout.offset_top = Some(LengthSpec::Px(0.0));
                layout.width = Some(LengthSpec::Px(1.0));
                // Percent heights resolve against the content box; the
                // entry's vertical padding is added so neighbouring steps
                // join into one line.
                layout.height = Some(LengthSpec::CalcPercentOffset {
                    percent: 100.0,
                    offset_px: TIMELINE_ENTRY_PADDING * 2.0,
                });
            }),
    )
    .visible(line);
    let node = widget(
        Stack::row(0.0)
            .align(AlignSpec::Center)
            .justify(JustifySpec::Center)
            .width(LengthSpec::Px(TIMELINE_NODE))
            .height(LengthSpec::Px(TIMELINE_NODE))
            .shrink(0.0)
            .surface(SemanticColorRole::Background)
            .radius_px(TIMELINE_NODE / 2.0),
    )
    .children(
        widget(IconGlyph::new(state.with_untracked(|row| row.role.icon())).size(14.0)).bind(
            move |glyph| {
                let (icon, role) = state.with(|row| (row.role.icon(), node_role(row)));
                glyph.icon = icon;
                *glyph = IconGlyph::new(icon).size(14.0).role(role);
            },
        ),
    );
    let rail = widget(
        Stack::column(0.0)
            .align(AlignSpec::Center)
            .width(LengthSpec::Px(TIMELINE_RAIL))
            .min_height(LengthSpec::Px(TIMELINE_NODE))
            .shrink(0.0),
    )
    .children(node);
    widget(
        Stack::row(0.0)
            .align(AlignSpec::Start)
            .width(LengthSpec::Fill)
            .padding_xy(0.0, TIMELINE_ENTRY_PADDING)
            .min_width(LengthSpec::Px(0.0))
            .with_layout(|layout| layout.position = PositionSpec::Relative),
    )
    .children((rail_line, rail, body))
}

/// The text column beside the rail: takes the remaining width, sized to its
/// content vertically.
fn body_column(gap: f32) -> Stack {
    Stack::column(gap)
        .grow(1.0)
        .shrink(1.0)
        .width(LengthSpec::Px(0.0))
        .min_width(LengthSpec::Px(0.0))
}

fn node_role(row: &RowChrome) -> SemanticColorRole {
    match row.role {
        TimelineRole::Reply => SemanticColorRole::Accent,
        _ => row.tone.icon_role(),
    }
}

fn step_body(
    id: &str,
    state: nana_ui::runtime::view::Signal<RowChrome>,
    emit: &RowEmit,
    body_slot: El<Stack>,
    refs: &ActionRefs,
) -> impl IntoView {
    let toggle_emit = Arc::clone(emit);
    let toggle_id = id.to_owned();
    let title = widget(title_button(&state.with_untracked(|row| row.clone())))
        .bind(move |button| {
            let row = state.get();
            *button = title_button(&row);
        })
        .entity_ref(refs.expand)
        .on_activate(move || {
            if state.with_untracked(|row| row.can_expand) {
                toggle_emit(TimelineAction::Expand(toggle_id.clone()));
            }
        });
    let preview = widget(
        Text::new(String::new())
            .font_size(type_scale::META)
            .color(SemanticColorRole::Muted)
            .truncating(),
    )
    .bind(move |text| {
        let detail = state.with(|row| row.detail.clone());
        if text.value != detail {
            text.value = detail;
        }
    })
    .visible(move || state.with(|row| !row.detail.is_empty() && !row.expanded));
    let retry_emit = Arc::clone(emit);
    let retry_id = id.to_owned();
    let retry = widget(row_action(tabler::REFRESH, "重试"))
        .entity_ref(refs.step_retry)
        .on_activate(move || retry_emit(TimelineAction::Retry(retry_id.clone())))
        .visible(move || state.with(|row| row.can_retry));
    let copy_emit = Arc::clone(emit);
    let copy_id = id.to_owned();
    let copy = widget(row_action(tabler::COPY, "复制"))
        .entity_ref(refs.copy)
        .on_activate(move || copy_emit(TimelineAction::Copy(copy_id.clone())))
        .visible(move || state.with(|row| row.can_copy));
    let head = widget(
        Stack::row(10.0)
            .align(AlignSpec::Center)
            .width(LengthSpec::Fill)
            .min_height(LengthSpec::Px(TIMELINE_NODE))
            .min_width(LengthSpec::Px(0.0)),
    )
    .children((
        title,
        widget(
            Stack::row(0.0)
                .grow(1.0)
                .shrink(1.0)
                .min_width(LengthSpec::Px(0.0)),
        )
        .children(preview),
        copy,
        retry,
    ));
    let detail = widget(Stack::column(0.0).min_width(LengthSpec::Px(0.0)))
        .visible(move || state.with(RowChrome::body_visible))
        .children(body_slot);
    widget(body_column(6.0)).children((head, detail))
}

fn title_button(row: &RowChrome) -> Button {
    let mut button = Button::new(row.title.clone())
        .kind(ButtonKind::Text)
        .size(ControlSize::Small)
        .icon_size(12.0)
        .icon_gap(4.0)
        .disabled(!row.can_expand);
    if row.can_expand {
        button = button.trailing_icon(if row.expanded {
            tabler::CHEVRON_DOWN
        } else {
            tabler::CHEVRON_RIGHT
        });
    }
    // An explicit style replaces the button recipe: the title reads as text
    // in the row's tone, with only the hover colour hinting that it toggles.
    let mut style = button.style.clone();
    let rest = row.tone.title_role();
    style.background = None;
    style.border = None;
    style.foreground = Some(rest);
    style.interaction = Default::default();
    style.interaction.hovered.foreground = Some(SemanticColorRole::Text);
    style.interaction.disabled.foreground = Some(rest);
    let layout = std::sync::Arc::make_mut(&mut style.layout);
    layout.padding_left = Some(LengthSpec::Px(0.0));
    layout.padding_right = Some(LengthSpec::Px(2.0));
    layout.min_height = Some(LengthSpec::Px(TIMELINE_NODE));
    layout.height = Some(LengthSpec::Px(TIMELINE_NODE));
    layout.border_width = Some(0.0);
    layout.font_size = Some(type_scale::BODY);
    layout.font_weight = Some(match row.role {
        TimelineRole::Group => 500,
        TimelineRole::Step(kind) if kind.is_process() => 500,
        _ => 600,
    });
    layout.flex_shrink = Some(0.0);
    button.style(style)
}

pub(crate) fn row_action(icon: Icon, label: &'static str) -> IconButton {
    let mut button = IconButton::new(icon, label)
        .kind(ButtonKind::Text)
        .size(ControlSize::Small)
        .with_tooltip(label);
    let edge = LengthSpec::Px(24.0);
    let layout = std::sync::Arc::make_mut(&mut button.style.layout);
    layout.width = Some(edge);
    layout.height = Some(edge);
    layout.min_width = Some(edge);
    layout.min_height = Some(edge);
    layout.padding_left = Some(LengthSpec::Px(0.0));
    layout.padding_right = Some(LengthSpec::Px(0.0));
    button.style.foreground = Some(SemanticColorRole::Muted);
    button
}

/// A message's actions keep their space but only paint while the pointer is
/// over the message, so hovering never moves the text around them.
fn actions_row(
    id: &str,
    state: nana_ui::runtime::view::Signal<RowChrome>,
    reveal: Reveal,
    emit: &RowEmit,
    trailing: bool,
    refs: &ActionRefs,
) -> impl IntoView {
    let action = |icon: Icon, label: &'static str, action: TimelineAction| {
        let emit = Arc::clone(emit);
        widget(row_action(icon, label)).on_activate(move || emit(action.clone()))
    };
    let copy = action(tabler::COPY, "复制", TimelineAction::Copy(id.to_owned()))
        .entity_ref(refs.copy)
        .visible(move || state.with(|row| row.can_copy));
    let quote = action(tabler::QUOTE, "引用", TimelineAction::Quote(id.to_owned()))
        .entity_ref(refs.quote)
        .visible(move || state.with(|row| row.can_copy));
    let retry = action(
        tabler::REFRESH,
        "重试",
        TimelineAction::Retry(id.to_owned()),
    )
    .entity_ref(refs.retry)
    .visible(move || state.with(|row| row.can_retry));
    let resume = action(
        tabler::ARROW_BACK_UP,
        "从这里继续",
        TimelineAction::Continue(id.to_owned()),
    )
    .entity_ref(refs.resume)
    .visible(move || state.with(|row| row.can_branch));
    let fork = action(
        tabler::GIT_FORK,
        "从这里分叉",
        TimelineAction::Fork(id.to_owned()),
    )
    .entity_ref(refs.fork)
    .visible(move || state.with(|row| row.can_branch));
    let mut bar = Stack::row(2.0).align(AlignSpec::Center);
    if trailing {
        bar = bar.justify(JustifySpec::End);
    }
    widget(bar)
        .visible(move || state.with(RowChrome::has_actions))
        .bind(move |bar| {
            let opacity = if reveal.shown() { 1.0 } else { 0.0 };
            *bar = bar
                .clone()
                .with_layout(|layout| layout.opacity = Some(opacity));
        })
        .children((copy, quote, retry, resume, fork))
}
