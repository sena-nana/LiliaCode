use std::sync::Arc;

use nana_ui::runtime::{
    AlignSpec, AppContext, Button, Card, Chip, DocumentId, Entity, FrameworkError, IconButton,
    JustifySpec, LengthSpec, SemanticColorRole, StableNodeId, Stack, Text, TextArea,
};
use nana_ui::{ButtonKind, CardKind, ControlSize, Icon, UI_METRICS};

const COMPOSER_SEND_SIZE: f32 = 30.0;
const PILL_RADIUS: f32 = 999.0;
pub(crate) const COMPOSER_CARD_RADIUS: f32 = 8.0;

pub(crate) fn form_text_input(value: impl Into<String>) -> nana_ui::runtime::TextInput {
    let input = nana_ui::runtime::TextInput::new(value);
    let layout = Stack::from_layout(input.style.layout.clone())
        .height(LengthSpec::Px(40.0))
        .node_style()
        .layout;
    input.layout(layout)
}

pub(crate) fn composer_card() -> Card {
    let mut card = Card::new()
        .kind(CardKind::Outlined)
        .padding(UI_METRICS.control_padding_x);
    card.style.background = Some(SemanticColorRole::Surface);
    let layout = Arc::make_mut(&mut card.style.layout);
    layout.direction = Some(nana_ui_core::FlexDirection::Column);
    layout.flex_grow = Some(0.0);
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

pub(crate) fn headline_offset_space() -> Stack {
    Stack::column(0.0)
        .height(LengthSpec::Viewport {
            axis: nana_ui_core::ViewportAxis::Height,
            value: 16.0,
        })
        .shrink(0.0)
}

pub(crate) fn mount_empty_headline(
    context: &mut AppContext,
    document: DocumentId,
    title: String,
) -> Result<(Entity<Stack>, Entity<Text>, Entity<Stack>), FrameworkError> {
    let slot =
        context.create_detached_component(document, headline_slot(!title.trim().is_empty()))?;
    let group = context.create_detached_component(
        document,
        Stack::column(14.0)
            .align(AlignSpec::Center)
            .max_width(680.0)
            .width(LengthSpec::CalcPercentOffset {
                percent: 100.0,
                offset_px: -48.0,
            }),
    )?;
    let heading = context.create_detached_component(document, conversation_headline(title))?;
    let actions = context.create_detached_component(
        document,
        Stack::bar(6.0)
            .justify(JustifySpec::Center)
            .max_width(560.0)
            .min_height(LengthSpec::Px(24.0))
            .wrap(true),
    )?;
    let offset = context.create_detached_component(document, headline_offset_space())?;
    context.append_child(group, heading)?;
    context.append_child(group, actions)?;
    context.append_child(slot, group)?;
    context.append_child(slot, offset)?;
    Ok((slot, heading, actions))
}

pub(crate) fn sync_conversation_body(
    context: &mut AppContext,
    body: StableNodeId,
    heading: Option<StableNodeId>,
    error: Option<StableNodeId>,
    timeline: StableNodeId,
    earlier: Option<StableNodeId>,
) -> Result<(), FrameworkError> {
    let mut order = Vec::with_capacity(3);
    order.extend(error);
    order.push(heading.unwrap_or(timeline));
    if heading.is_none() {
        order.extend(earlier);
    }
    context.reconcile_children(body, &order).map(|_| ())
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
    layout.border_radius = Some(COMPOSER_CARD_RADIUS);
    card
}

pub(crate) fn pending_actions_row() -> Stack {
    Stack::bar(6.0).wrap(true)
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

pub(crate) fn token_chip(label: impl Into<String>, selected: bool) -> Chip {
    Chip::new(label.into()).selected(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_chip_projects_selected_state() {
        let idle = token_chip("Plan", false);
        assert!(!idle.selected);
        assert_eq!(idle.label.as_ref(), "Plan");
        let on = token_chip("Goal", true);
        assert!(on.selected);
        assert_eq!(on.label.as_ref(), "Goal");
    }

    #[test]
    fn composer_card_is_outlined() {
        let card = composer_card();
        assert_eq!(card.kind, CardKind::Outlined);
        let pending = pending_interaction_card();
        assert_eq!(pending.kind, CardKind::Outlined);
    }
}

pub(crate) fn flatten_composer_textarea(area: TextArea) -> TextArea {
    let mut style = area.style.clone();
    style.background = None;
    style.border = None;
    style.interaction.hovered.border = None;
    style.interaction.focused.border = None;
    let layout = Arc::make_mut(&mut style.layout);
    layout.border_width = Some(0.0);
    layout.border_radius = Some(0.0);
    layout.min_height = Some(LengthSpec::Px(ControlSize::Medium.height()));
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
