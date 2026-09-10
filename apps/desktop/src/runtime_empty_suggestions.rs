use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use nana_ui::runtime::{
    AlignSpec, AppContext, DocumentId, Entity, FrameworkError, LengthSpec, ListItem, ListItemSlots,
    SemanticColorRole, Stack, Text,
};
use nana_ui_platform::WindowId;

use crate::runtime_shell::{bind_activate, ShellIntent, ShellSuggestionRow};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmptySuggestionsSnapshot {
    pub items: Vec<ShellSuggestionRow>,
    pub can_refresh: bool,
    pub status: Option<String>,
}

#[derive(Default)]
pub(crate) struct EmptySuggestions {
    buttons: HashMap<String, SuggestionEntry>,
    status: Option<Entity<Text>>,
}

struct SuggestionEntry {
    root: Entity<ListItem>,
    content: Entity<Stack>,
    summary: Entity<Text>,
    source: Option<Entity<Text>>,
}

fn suggestion_button(label: &str, source: Option<&str>) -> ListItem {
    let height = if source.is_some() { 36.0 } else { 24.0 };
    let mut button = ListItem::new(
        source
            .map(|source| format!("{label} · {source}"))
            .unwrap_or_else(|| label.to_owned()),
    );
    let layout = Arc::make_mut(&mut button.style.layout);
    layout.width = Some(LengthSpec::Auto);
    layout.height = Some(LengthSpec::Px(height));
    layout.min_height = Some(LengthSpec::Px(height));
    layout.max_height = Some(LengthSpec::Px(height));
    layout.min_width = Some(LengthSpec::Px(if source.is_some() { 150.0 } else { 0.0 }));
    layout.max_width = Some(if source.is_some() {
        LengthSpec::Px(280.0)
    } else {
        LengthSpec::Percent(100.0)
    });
    layout.padding_left = Some(LengthSpec::Px(8.0));
    layout.padding_right = Some(LengthSpec::Px(8.0));
    layout.padding_top = Some(LengthSpec::Px(if source.is_some() { 4.0 } else { 0.0 }));
    layout.padding_bottom = layout.padding_top;
    layout.flex_shrink = Some(1.0);
    layout.align_items = if source.is_some() {
        AlignSpec::Start
    } else {
        AlignSpec::Center
    };
    layout.border_radius = Some(nana_ui::UI_METRICS.radius_sm);
    button
}

fn suggestion_text(label: &str, source: bool) -> Text {
    let mut text = Text::new(label);
    text.style.foreground = Some(if source {
        SemanticColorRole::Faint
    } else {
        SemanticColorRole::Muted
    });
    let layout = Arc::make_mut(&mut text.style.layout);
    layout.font_size = Some(if source { 11.0 } else { 12.0 });
    layout.font_weight = Some(if source { 500 } else { 600 });
    layout.line_height = Some(nana_ui_core::LineHeightSpec::Relative(1.0));
    layout.white_space_nowrap = true;
    layout.text_overflow_ellipsis = true;
    layout.max_width = Some(LengthSpec::Percent(100.0));
    text
}

impl EmptySuggestions {
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        parent: Entity<Stack>,
        snapshot: &EmptySuggestionsSnapshot,
        visible: bool,
        window_id: WindowId,
        sink: Arc<dyn Fn(ShellIntent) + Send + Sync>,
    ) -> Result<(), FrameworkError> {
        let mut desired = Vec::new();
        if visible {
            for item in &snapshot.items {
                desired.push((
                    format!("item:{}", item.id),
                    item.label.clone(),
                    item.source
                        .clone()
                        .filter(|source| !source.trim().is_empty()),
                    ShellIntent::ApplySuggestion {
                        window_id,
                        item_id: item.id.clone(),
                    },
                ));
            }
            if snapshot.can_refresh {
                desired.push((
                    "refresh".to_owned(),
                    "刷新建议".to_owned(),
                    None,
                    ShellIntent::RefreshSuggestions(window_id),
                ));
            }
        }
        let mut order = Vec::new();
        if let Some(message) = snapshot.status.as_ref().filter(|_| visible) {
            let text = if let Some(text) = self.status {
                context.update_component(text, |text, _| *text = Text::new(message.clone()))?;
                text
            } else {
                let text =
                    context.create_detached_component(document, Text::new(message.clone()))?;
                self.status = Some(text);
                text
            };
            order.push(text.stable_id());
        } else if let Some(text) = self.status.take() {
            context.remove_view(text)?;
        }
        let mut keep = HashSet::new();
        for (id, label, source, intent) in desired {
            keep.insert(id.clone());
            if !self.buttons.contains_key(&id) {
                let root = context.create_detached_component(
                    document,
                    suggestion_button(&label, source.as_deref()),
                )?;
                let content = context.create_detached_component(
                    document,
                    Stack::column(3.0)
                        .align(AlignSpec::Start)
                        .width(LengthSpec::Auto)
                        .with_layout(|layout| layout.max_width = Some(LengthSpec::Percent(100.0))),
                )?;
                let summary =
                    context.create_detached_component(document, suggestion_text(&label, false))?;
                context.append_child(content, summary)?;
                bind_activate(context, root, sink.clone(), intent)?;
                self.buttons.insert(
                    id.clone(),
                    SuggestionEntry {
                        root,
                        content,
                        summary,
                        source: None,
                    },
                );
            }
            let entry = self.buttons.get_mut(&id).expect("suggestion entry exists");
            context.update_component(entry.root, |button, _| {
                *button = suggestion_button(&label, source.as_deref())
            })?;
            context.update_component(entry.summary, |text, _| {
                *text = suggestion_text(&label, false)
            })?;
            let mut children = vec![entry.summary.stable_id()];
            if let Some(source) = source {
                let text = if let Some(text) = entry.source {
                    context
                        .update_component(text, |text, _| *text = suggestion_text(&source, true))?;
                    text
                } else {
                    let text = context
                        .create_detached_component(document, suggestion_text(&source, true))?;
                    entry.source = Some(text);
                    text
                };
                children.push(text.stable_id());
            } else if let Some(text) = entry.source.take() {
                context.remove_view(text)?;
            }
            context.reconcile_children(entry.content.stable_id(), &children)?;
            context.set_list_item_slots(
                entry.root,
                ListItemSlots {
                    content: Some(entry.content.stable_id()),
                    ..Default::default()
                },
            )?;
            order.push(entry.root.stable_id());
        }
        let stale = self
            .buttons
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale {
            if let Some(entry) = self.buttons.remove(&id) {
                context.remove_view(entry.root)?;
            }
        }
        context
            .reconcile_children(parent.stable_id(), &order)
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::{Activate, RuntimeDocument};
    use std::sync::Mutex;

    #[test]
    fn source_caption_uses_two_real_text_rows_and_returns_to_the_single_line_height() {
        let id = DocumentId::new(43).unwrap();
        let mut document = RuntimeDocument::new(id);
        let context = document.context_mut();
        let parent = context.create_component(id, Stack::row(6.0)).unwrap();
        let mut controls = EmptySuggestions::default();
        let mut snapshot = EmptySuggestionsSnapshot {
            items: vec![ShellSuggestionRow {
                id: "source".into(),
                label: "Review changes".into(),
                source: Some("org/repo · PR #42".into()),
                prompt: "Review now".into(),
            }],
            ..Default::default()
        };
        controls
            .sync(
                context,
                id,
                parent,
                &snapshot,
                true,
                WindowId(43),
                Arc::new(|_| {}),
            )
            .unwrap();
        let ids = context.world().document_order(id);
        context.resolve_styles(&ids).unwrap();
        context
            .shape_text(&ids, &mut nana_ui::NanaTextShaper::default())
            .unwrap();
        context
            .layout_document(id, nana_ui::runtime::LayoutViewport::new(430.0, 200.0))
            .unwrap();
        let entry = &controls.buttons["item:source"];
        let root = entry.root;
        let summary = entry.summary;
        let source = entry.source.unwrap();
        let bounds = context.world().layout_box(root.stable_id()).unwrap();
        assert!((bounds.height - 36.0).abs() < 0.5);
        assert!(bounds.width >= 150.0 && bounds.width <= 280.5);
        let summary_bounds = context.world().layout_box(summary.stable_id()).unwrap();
        let source_bounds = context.world().layout_box(source.stable_id()).unwrap();
        assert!(source_bounds.y >= summary_bounds.y + summary_bounds.height + 2.5);
        assert!(source_bounds.y + source_bounds.height <= bounds.y + bounds.height);
        assert_eq!(
            context
                .world()
                .node_style(summary.stable_id())
                .unwrap()
                .layout
                .font_size,
            Some(12.0)
        );
        assert_eq!(
            context
                .world()
                .node_style(summary.stable_id())
                .unwrap()
                .layout
                .font_weight,
            Some(600)
        );
        assert_eq!(
            context
                .world()
                .node_style(source.stable_id())
                .unwrap()
                .layout
                .font_size,
            Some(11.0)
        );
        assert_eq!(
            context
                .world()
                .node_style(source.stable_id())
                .unwrap()
                .layout
                .font_weight,
            Some(500)
        );

        snapshot.items[0].source = None;
        controls
            .sync(
                context,
                id,
                parent,
                &snapshot,
                true,
                WindowId(43),
                Arc::new(|_| {}),
            )
            .unwrap();
        context
            .layout_document(id, nana_ui::runtime::LayoutViewport::new(430.0, 200.0))
            .unwrap();
        assert_eq!(
            controls.buttons["item:source"].root.stable_id(),
            root.stable_id()
        );
        assert!(!context.world().is_mounted(source.stable_id()));
        assert!((context.world().layout_box(root.stable_id()).unwrap().height - 24.0).abs() < 0.5);
    }

    #[test]
    fn reused_suggestion_buttons_emit_current_window_identity_instead_of_captured_prompts() {
        for window in [WindowId::PRIMARY, WindowId(42)] {
            let document_id = DocumentId::new(42).unwrap();
            let mut document = RuntimeDocument::new(document_id);
            let context = document.context_mut();
            let parent = context
                .create_component(document_id, Stack::row(6.0))
                .unwrap();
            let received = Arc::new(Mutex::new(Vec::new()));
            let events = received.clone();
            let sink: Arc<dyn Fn(ShellIntent) + Send + Sync> =
                Arc::new(move |intent| events.lock().unwrap().push(intent));
            let mut controls = EmptySuggestions::default();
            let mut snapshot = EmptySuggestionsSnapshot {
                items: vec![ShellSuggestionRow {
                    id: "same".into(),
                    label: "旧建议".into(),
                    source: None,
                    prompt: "旧提示".into(),
                }],
                can_refresh: true,
                status: None,
            };
            controls
                .sync(
                    context,
                    document_id,
                    parent,
                    &snapshot,
                    true,
                    window,
                    sink.clone(),
                )
                .unwrap();
            let button = controls.buttons["item:same"].root;
            snapshot.items[0].label = "新建议".into();
            snapshot.items[0].prompt = "新提示".into();
            controls
                .sync(
                    context,
                    document_id,
                    parent,
                    &snapshot,
                    true,
                    window,
                    sink.clone(),
                )
                .unwrap();
            assert_eq!(
                controls.buttons["item:same"].root.stable_id(),
                button.stable_id()
            );
            context
                .update_component(button, |_, cx| cx.emit(Activate))
                .unwrap();
            context
                .update_component(controls.buttons["refresh"].root, |_, cx| cx.emit(Activate))
                .unwrap();
            let events = received.lock().unwrap();
            assert!(matches!(events.as_slice(), [
                ShellIntent::ApplySuggestion { window_id, item_id },
                ShellIntent::RefreshSuggestions(refresh_window),
            ] if *window_id == window && item_id == "same" && *refresh_window == window));
            drop(events);
            controls
                .sync(context, document_id, parent, &snapshot, false, window, sink)
                .unwrap();
            assert!(!context.world().is_mounted(button.stable_id()));
            assert!(controls.buttons.is_empty());
        }
    }
}
