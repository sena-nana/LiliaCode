use std::sync::{Arc, Mutex};

use nana_ui::runtime::view::{widget, El, Signal};
use nana_ui::runtime::{
    AlignSpec, AppContext, Button, Card, FrameworkError, IconButton, JustifySpec, LengthSpec,
    SemanticColorRole, StableNodeId, Stack, Text, TextArea,
};
use nana_ui::{ButtonKind, CardKind, ControlSize, Icon, UI_METRICS};

/// A signal created inside `mount_view`, written later by sync.
///
/// `signal()` belongs to the mount scope, so the view closure calls
/// [`Bound::install`] and sync calls [`Bound::set`].
#[derive(Clone)]
pub(crate) struct Bound<T: 'static> {
    slot: Arc<Mutex<Option<Signal<T>>>>,
}

impl<T: 'static> Default for Bound<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> Bound<T> {
    pub(crate) fn new() -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn install(&self, signal: Signal<T>) -> Signal<T> {
        *self.slot.lock().expect("signal slot") = Some(signal);
        signal
    }

    pub(crate) fn set(&self, value: T) {
        if let Some(signal) = *self.slot.lock().expect("signal slot") {
            signal.set(value);
        }
    }

    pub(crate) fn signal(&self) -> Signal<T> {
        self.slot
            .lock()
            .expect("signal slot")
            .expect("signal installed")
    }
}

pub(crate) fn view_column(gap: f32) -> El<Stack> {
    widget(Stack::column(gap))
}

pub(crate) fn view_fill_column(gap: f32) -> El<Stack> {
    widget(Stack::fill_column(gap))
}

pub(crate) fn view_row(gap: f32) -> El<Stack> {
    widget(Stack::row(gap))
}

pub(crate) fn view_bar(gap: f32) -> El<Stack> {
    widget(Stack::bar(gap))
}

const COMPOSER_SEND_SIZE: f32 = 30.0;
const PILL_RADIUS: f32 = 999.0;
pub(crate) const COMPOSER_CARD_RADIUS: f32 = UI_METRICS.radius_lg;

pub(crate) fn wrapping_controls_row(gap: f32) -> Stack {
    Stack::row(gap)
        .min_width(LengthSpec::Px(0.0))
        .shrink(1.0)
        .with_layout(|layout| {
            layout.max_width = Some(LengthSpec::Percent(100.0));
            layout.flex_wrap = nana_ui_core::FlexWrap::Wrap;
        })
}

pub(crate) fn reconcile_children(
    context: &mut AppContext,
    parent: StableNodeId,
    ordered: &[StableNodeId],
) -> Result<(), FrameworkError> {
    let ordered = ordered
        .iter()
        .copied()
        .filter(|id| *id != parent && context.world().contains(*id))
        .collect::<Vec<_>>();
    context.reconcile_children(parent, &ordered).map(|_| ())
}

pub(crate) fn composer_card() -> Card {
    let mut card = Card::new()
        .kind(CardKind::Outlined)
        .padding(UI_METRICS.control_padding_x);
    card.style.background = Some(SemanticColorRole::Surface);
    let layout = Arc::make_mut(&mut card.style.layout);
    layout.direction = Some(nana_ui_core::FlexDirection::Column);
    layout.flex_grow = Some(0.0);
    layout.flex_shrink = Some(0.0);
    layout.height = Some(LengthSpec::Shrink);
    layout.gap = Some(LengthSpec::Px(7.0));
    layout.border_radius = Some(COMPOSER_CARD_RADIUS);
    card
}

pub(crate) fn conversation_headline(value: String) -> Text {
    let mut text = Text::new(value);
    let layout = Arc::make_mut(&mut text.style.layout);
    layout.font_size = Some(24.0);
    layout.font_weight = Some(500);
    layout.line_height = Some(nana_ui_core::LineHeightSpec::Relative(1.25));
    layout.letter_spacing = Some(0.2);
    layout.width = Some(LengthSpec::Percent(100.0));
    layout.max_width = Some(LengthSpec::Px(680.0));
    text.style.text_horizontal_alignment = nana_ui::runtime::TextHorizontalAlignment::Center;
    text
}

pub(crate) fn headline_slot(active: bool) -> Stack {
    if active {
        Stack::fill_column(0.0)
            .align(AlignSpec::Center)
            .justify(JustifySpec::Center)
    } else {
        Stack::column(0.0).height(LengthSpec::Px(0.0))
    }
}

pub(crate) fn trigger_slot(width: f32, height: f32) -> Stack {
    Stack::row(0.0)
        .align(AlignSpec::Center)
        .justify(JustifySpec::Center)
        .width(LengthSpec::Px(width))
        .height(LengthSpec::Px(height))
        .min_width(LengthSpec::Px(width))
        .min_height(LengthSpec::Px(height))
        .grow(0.0)
        .shrink(0.0)
}

pub(crate) fn pending_interaction_card() -> Card {
    let mut card = Card::new()
        .kind(CardKind::Outlined)
        .padding(UI_METRICS.control_padding_x);
    card.style.background = Some(SemanticColorRole::Surface);
    let layout = Arc::make_mut(&mut card.style.layout);
    layout.direction = Some(nana_ui_core::FlexDirection::Column);
    layout.gap = Some(LengthSpec::Px(8.0));
    layout.align_items = AlignSpec::Stretch;
    layout.border_radius = Some(COMPOSER_CARD_RADIUS);
    card
}

pub(crate) fn pending_actions_row() -> Stack {
    Stack::bar(6.0).wrap(true).justify(JustifySpec::End)
}

pub(crate) fn inspector_header_bar() -> Stack {
    Stack::bar(6.0).justify(JustifySpec::SpaceBetween)
}

fn round_icon_button(icon: Icon, label: &'static str, kind: ButtonKind) -> IconButton {
    let mut button = IconButton::new(icon, label).kind(kind).with_tooltip(label);
    let layout = Arc::make_mut(&mut button.style.layout);
    let edge = LengthSpec::Px(COMPOSER_SEND_SIZE);
    layout.min_width = Some(edge);
    layout.min_height = Some(edge);
    layout.width = Some(edge);
    layout.height = Some(edge);
    layout.padding_left = Some(LengthSpec::Px(0.0));
    layout.padding_right = Some(LengthSpec::Px(0.0));
    layout.border_radius = Some(COMPOSER_SEND_SIZE * 0.5);
    button
}

pub(crate) fn composer_send_button(enabled: bool) -> IconButton {
    round_icon_button(Icon::ArrowUp, "发送", ButtonKind::Primary).disabled(!enabled)
}

pub(crate) fn composer_interrupt_button(enabled: bool) -> IconButton {
    round_icon_button(Icon::Close, "停止", ButtonKind::Danger).disabled(!enabled)
}

pub(crate) fn sidebar_icon_button(icon: Icon, label: &'static str) -> IconButton {
    sized_icon_button(icon, label, ButtonKind::Text, UI_METRICS.icon_button_size)
}

pub(crate) fn window_control(icon: Icon, label: &'static str, kind: ButtonKind) -> IconButton {
    sized_icon_button(icon, label, kind, UI_METRICS.icon_button_size)
}

fn sized_icon_button(icon: Icon, label: &'static str, kind: ButtonKind, edge: f32) -> IconButton {
    let mut button = IconButton::new(icon, label)
        .kind(kind)
        .size(ControlSize::Small)
        .with_tooltip(label);
    let layout = Arc::make_mut(&mut button.style.layout);
    let edge = LengthSpec::Px(edge);
    layout.min_width = Some(edge);
    layout.min_height = Some(edge);
    layout.width = Some(edge);
    layout.height = Some(edge);
    layout.padding_left = Some(LengthSpec::Px(0.0));
    layout.padding_right = Some(LengthSpec::Px(0.0));
    layout.border_radius = Some(UI_METRICS.radius_sm);
    button
}

pub(crate) fn pill_button(label: &str, kind: ButtonKind) -> Button {
    let mut button = Button::new(label).kind(kind).size(ControlSize::Small);
    let layout = Arc::make_mut(&mut button.style.layout);
    layout.min_height = Some(LengthSpec::Px(UI_METRICS.compact_control_height));
    layout.padding_left = Some(LengthSpec::Px(UI_METRICS.compact_control_padding_x));
    layout.padding_right = Some(LengthSpec::Px(UI_METRICS.compact_control_padding_x));
    layout.border_radius = Some(PILL_RADIUS);
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_card_is_outlined() {
        let card = composer_card();
        assert_eq!(card.kind, CardKind::Outlined);
        let pending = pending_interaction_card();
        assert_eq!(pending.kind, CardKind::Outlined);
    }

    #[test]
    fn composer_textarea_has_no_inner_surface_when_disabled() {
        let area = flatten_composer_textarea(TextArea::new(""));
        assert_eq!(area.style.background, None);
        assert_eq!(area.style.border, None);
        assert_eq!(area.style.interaction.disabled.background, None);
        assert_eq!(area.style.interaction.disabled.border, None);
        assert_eq!(
            area.style.layout.line_height,
            Some(nana_ui_core::LineHeightSpec::Absolute(
                ControlSize::Medium.line_height(),
            ))
        );
        assert_eq!(area.style.control_padding_y, None);
        assert_eq!(
            area.style.layout.padding_top,
            Some(LengthSpec::Px(
                ControlSize::Medium.vertical_padding(UI_METRICS)
            ))
        );
    }
}

pub(crate) fn flatten_composer_textarea(area: TextArea) -> TextArea {
    let mut style = area.style.clone();
    style.background = None;
    style.border = None;
    style.interaction.hovered.border = None;
    style.interaction.focused.border = None;
    // `TextArea::disabled` applies the field's default Subtle fill. The
    // composer card already owns the only surface, so preserve the muted
    // disabled text while removing that second inner panel as well.
    style.interaction.disabled.background = None;
    style.interaction.disabled.border = None;
    // TextArea::new marks multiline fields with the theme's `Field` vertical
    // padding (10px in the desktop theme). The composer owns its compact
    // 32px line box, whose 8px insets are already authored below; clear the
    // intent token so style resolution does not overwrite those insets and
    // leave only enough room for the full placeholder glyph line.
    style.control_padding_y = None;
    let layout = Arc::make_mut(&mut style.layout);
    layout.border_width = Some(0.0);
    layout.border_radius = Some(0.0);
    // A multiline TextArea defaults to a relative 1.45 line height. The
    // composer starts at the compact one-line height, so that relative value
    // leaves less content space than the line needs and clips the placeholder
    // to its upper half. Use the shared medium control line instead; the
    // height signal still grows by one visible line step for each explicit
    // newline.
    layout.line_height = Some(nana_ui_core::LineHeightSpec::Absolute(
        ControlSize::Medium.line_height(),
    ));
    layout.min_height = Some(LengthSpec::Px(ControlSize::Medium.height_in(UI_METRICS)));
    layout.padding_left = Some(LengthSpec::Px(UI_METRICS.field_padding_x));
    layout.padding_right = Some(LengthSpec::Px(UI_METRICS.field_padding_x));
    layout.padding_top = Some(LengthSpec::Px(
        ControlSize::Medium.vertical_padding(UI_METRICS),
    ));
    layout.padding_bottom = Some(LengthSpec::Px(
        ControlSize::Medium.vertical_padding(UI_METRICS),
    ));
    area.style(style)
}
