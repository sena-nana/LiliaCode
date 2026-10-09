//! LiliaCode layout constants and shared text recipes.
//!
//! Colours always come from `SemanticColorRole`; sizes here are the product's
//! own composition (column widths, rail geometry), not control metrics, which
//! stay in NanaUI's `UI_METRICS`.

use std::sync::Arc;

use nana_ui::runtime::{Button, JustifySpec, LengthSpec, SemanticColorRole, Stack, Text};
use nana_ui::{ButtonKind, ControlSize, Icon};
use nana_ui_core::type_scale;

/// Column holding the step icons on the left of the timeline.
pub(crate) const TIMELINE_RAIL: f32 = 28.0;
/// Step icon disc on the rail.
pub(crate) const TIMELINE_NODE: f32 = 22.0;
/// Vertical breathing room above and below one timeline entry.
pub(crate) const TIMELINE_ENTRY_PADDING: f32 = 7.0;
/// Widest a user bubble grows before it wraps.
pub(crate) const BUBBLE_MAX_WIDTH: f32 = 620.0;

/// Secondary line under or beside a label.
pub(crate) fn meta(value: impl Into<String>) -> Text {
    Text::new(value)
        .font_size(type_scale::META)
        .color(SemanticColorRole::Muted)
        .truncating()
}

/// Short uppercase-style section caption (sidebar groups, panel sections).
pub(crate) fn caption(value: impl Into<String>) -> Text {
    Text::new(value)
        .font_size(type_scale::HINT)
        .font_weight(700)
        .color(SemanticColorRole::Faint)
        .truncating()
}

/// Readable width of document-like pages (overview, sessions, forms).
pub(crate) const PAGE_WIDTH: f32 = 760.0;

/// Centred column holding a page's title and content.
pub(crate) fn page_column() -> Stack {
    Stack::column(12.0)
        .max_width(PAGE_WIDTH)
        .with_layout(|layout| {
            layout.margin_left = Some(LengthSpec::Auto);
            layout.margin_right = Some(LengthSpec::Auto);
        })
}

pub(crate) fn page_title(value: impl Into<String>) -> Text {
    Text::new(value)
        .font_size(type_scale::TITLE)
        .font_weight(600)
        .color(SemanticColorRole::Text)
}

pub(crate) fn page_subtitle(value: impl Into<String>) -> Text {
    Text::new(value)
        .font_size(type_scale::BODY)
        .color(SemanticColorRole::Muted)
}

/// A full-width, left-aligned row that opens something: a session in a list,
/// a project on the overview.
pub(crate) fn list_row_button(label: impl Into<String>, icon: Icon) -> Button {
    let button = Button::new(label)
        .kind(ButtonKind::Text)
        .size(ControlSize::Medium)
        .icon(icon)
        .icon_size(15.0)
        .icon_gap(10.0)
        .content_align(nana_ui::runtime::TextHorizontalAlignment::Start);
    let mut style = button.style.clone();
    style.background = None;
    style.border = None;
    style.foreground = Some(SemanticColorRole::Text);
    style.radius = Some(nana_ui_core::RadiusTier::Sm);
    style.interaction = Default::default();
    style.interaction.hovered.background = Some(SemanticColorRole::Hover);
    style.interaction.pressed.background = Some(SemanticColorRole::Active);
    style.interaction.focused = nana_ui::runtime::SemanticPaint::FOCUS_SURFACE;
    let layout = Arc::make_mut(&mut style.layout);
    layout.width = Some(LengthSpec::Fill);
    layout.height = Some(LengthSpec::Px(40.0));
    layout.justify_content = JustifySpec::Start;
    layout.padding_left = Some(LengthSpec::Px(12.0));
    layout.padding_right = Some(LengthSpec::Px(12.0));
    layout.border_width = Some(0.0);
    layout.font_weight = Some(500);
    button.style(style)
}
