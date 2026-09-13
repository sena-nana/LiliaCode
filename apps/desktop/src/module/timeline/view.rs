use crate::runtime_layout::{pill_button, reconcile_children, token_chip};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineImage {
    pub source: String,
    pub data_url: std::sync::Arc<str>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineRow {
    pub id: String,
    pub markdown: String,
    pub images: Vec<TimelineImage>,
    pub expanded: bool,
    pub can_expand: bool,
    pub can_retry: bool,
    pub can_copy: bool,
    pub can_branch: bool,
}

fn timeline_markdown_view(item: &TimelineRow) -> NativeMarkdown {
    let mut markdown = NativeMarkdown::parse(&item.markdown);
    apply_timeline_markdown(&mut markdown, item);
    markdown
}

fn apply_timeline_markdown(markdown: &mut NativeMarkdown, item: &TimelineRow) {
    *markdown = NativeMarkdown::parse(&item.markdown);
    for image in &item.images {
        markdown.resolve_image(
            &image.source,
            image.data_url.clone(),
            image.width,
            image.height,
        );
    }
}

fn content_hash(item: &TimelineRow) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    item.markdown.hash(&mut hasher);
    for image in &item.images {
        image.source.hash(&mut hasher);
        image.width.hash(&mut hasher);
        image.height.hash(&mut hasher);
        image.data_url.hash(&mut hasher);
    }
    hasher.finish()
}
use nana_ui::ButtonKind;
use nana_ui::runtime::{
    Activate, AppContext, Button, Chip, DocumentId, Entity, FlexDirection, FrameworkError,
    LengthSpec, List, NativeMarkdown, NodeStyle, RichTextEvent, ScrollAxes, ScrollChanged,
    ScrollView, Stack, VirtualListItems, VirtualListLayout,
};
use nana_ui_platform::WindowId;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
const TIMELINE_OVERSCAN_EXTENT: f32 = 480.0;
const TIMELINE_DEFAULT_VIEWPORT_EXTENT: f32 = 720.0;
const TIMELINE_ROW_FALLBACK_EXTENT: f32 = 72.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineTarget {
    pub window_id: WindowId,
    pub task_id: Option<String>,
}
impl TimelineTarget {
    pub(crate) fn matches(&self, current: &Self) -> bool {
        self.task_id.is_some() && self == current
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TimelineAction {
    Expand(String),
    Copy(String),
    Retry(String),
    Continue(String),
    Fork(String),
    OpenImage { source: String, alt: String },
    LoadEarlier,
    Scrolled { offset: f32, viewport_extent: f32 },
}
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineViewSnapshot {
    pub target: TimelineTarget,
    pub rows: Vec<TimelineRow>,
    pub layout: VirtualListLayout,
    pub scroll_offset: f32,
    pub viewport_extent: f32,
    pub can_load_earlier: bool,
}
type Sink = Arc<dyn Fn(TimelineTarget, TimelineAction) + Send + Sync>;
pub(crate) struct TimelineView {
    pub(crate) root: Entity<Stack>,
    pub(crate) timeline_scroll: Entity<ScrollView>,
    pub(crate) timeline_list: Entity<List>,
    timeline_virtual: VirtualListItems<String, Stack>,
    pub(crate) timeline_markdown: HashMap<String, Entity<NativeMarkdown>>,
    timeline_markdown_source: HashMap<String, u64>,
    pub(crate) timeline_actions: HashMap<String, Entity<Chip>>,
    timeline_toolbars: HashMap<String, Entity<Stack>>,
    pub(crate) load_earlier: Option<Entity<Button>>,
    sink: Sink,
    target: TimelineTarget,
    first_item: Option<String>,
    scroll_target: Arc<Mutex<(TimelineTarget, f32)>>,
}
impl TimelineView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        target: TimelineTarget,
        sink: Sink,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, Stack::fill_column(6.0))?;
        let timeline_scroll = context.create_detached_component(
            document,
            ScrollView::new(ScrollAxes::Vertical).style(timeline_scroll_style()),
        )?;
        let timeline_list = context.create_detached_component(
            document,
            List::new()
                .label("时间线")
                .style(timeline_list_style(0.0, 0.0, 0.0)),
        )?;
        context.append_child(timeline_scroll, timeline_list)?;
        context.append_child(root, timeline_scroll)?;
        let scroll_target = Arc::new(Mutex::new((
            target.clone(),
            TIMELINE_DEFAULT_VIEWPORT_EXTENT,
        )));
        let scroll_binding = Arc::clone(&scroll_target);
        let scroll_sink = Arc::clone(&sink);
        context.on(timeline_scroll, move |_, event: &ScrollChanged, _| {
            let (target, viewport_extent) = scroll_binding.lock().unwrap().clone();
            scroll_sink(
                target,
                TimelineAction::Scrolled {
                    offset: event.offset.y,
                    viewport_extent,
                },
            );
        })?;
        Ok(Self {
            root,
            timeline_scroll,
            timeline_list,
            timeline_virtual: VirtualListItems::default(),
            timeline_markdown: HashMap::new(),
            timeline_markdown_source: HashMap::new(),
            timeline_actions: HashMap::new(),
            timeline_toolbars: HashMap::new(),
            load_earlier: None,
            sink,
            target,
            first_item: None,
            scroll_target,
        })
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TimelineViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let task_changed = self.target != snapshot.target;
        let prepended = !task_changed
            && self.first_item.as_ref().is_some_and(|first| {
                snapshot
                    .rows
                    .iter()
                    .position(|row| &row.id == first)
                    .is_some_and(|index| index > 0)
            });
        let reading_anchor = if prepended {
            self.timeline_virtual.mounted_keys().iter().find_map(|key| {
                let row = self.timeline_virtual.entity(key)?.stable_id();
                let anchor = context
                    .capture_scroll_anchor(self.timeline_scroll, row)
                    .ok()??;
                let bounds = context.world().layout_box(row)?;
                (anchor.viewport_y + bounds.height > 0.0).then_some(anchor)
            })
        } else {
            None
        };
        self.first_item = snapshot.rows.first().map(|row| row.id.clone());
        if task_changed {
            context.materialize_virtual_list(
                self.timeline_list,
                &mut self.timeline_virtual,
                &VirtualListLayout::default(),
                0.0,
                1.0,
                0.0,
                |_| String::new(),
                |_, _| Stack::column(6.0),
            )?;
            self.timeline_markdown.clear();
            self.timeline_markdown_source.clear();
            self.timeline_actions.clear();
            self.timeline_toolbars.clear();
            if let Some(button) = self.load_earlier.take() {
                context.remove_view(button)?;
            }
        }
        self.target = snapshot.target.clone();
        let layout = timeline_virtual_layout(snapshot);
        let viewport_extent = viewport_extent(context, self.timeline_scroll, snapshot);
        *self.scroll_target.lock().unwrap() = (snapshot.target.clone(), viewport_extent);
        let window = context.materialize_virtual_list(
            self.timeline_list,
            &mut self.timeline_virtual,
            &layout,
            snapshot.scroll_offset.max(0.0),
            viewport_extent,
            TIMELINE_OVERSCAN_EXTENT,
            |index| {
                snapshot
                    .rows
                    .get(index)
                    .map(|row| row.id.clone())
                    .unwrap_or_else(|| format!("missing-{index}"))
            },
            |_, _| Stack::column(6.0),
        )?;
        context.update_component(self.timeline_list, |list, _| {
            list.style = timeline_list_style(
                window.total_extent,
                window.leading_extent,
                window.trailing_extent,
            );
        })?;
        let mounted = self
            .timeline_virtual
            .mounted_keys()
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        self.timeline_markdown
            .retain(|key, _| mounted.contains(key));
        self.timeline_markdown_source
            .retain(|key, _| mounted.contains(key));
        self.timeline_actions
            .retain(|key, _| timeline_action_is_mounted(key, &mounted));
        self.timeline_toolbars
            .retain(|key, _| mounted.contains(key));
        let mut action_keep = HashSet::new();
        for item in snapshot
            .rows
            .iter()
            .filter(|item| mounted.contains(&item.id))
        {
            let Some(root) = self.timeline_virtual.entity(&item.id) else {
                continue;
            };
            let source = content_hash(item);
            let markdown = if let Some(entity) = self.timeline_markdown.get(&item.id).copied() {
                if self.timeline_markdown_source.get(&item.id) != Some(&source) {
                    context.update_component(entity, |markdown, _| {
                        apply_timeline_markdown(markdown, item);
                    })?;
                    context.assemble_markdown(entity)?;
                    self.timeline_markdown_source
                        .insert(item.id.clone(), source);
                }
                entity
            } else {
                let entity =
                    context.create_detached_component(document_id, timeline_markdown_view(item))?;
                let target = snapshot.target.clone();
                let sink = Arc::clone(&self.sink);
                context.on(entity, move |_, event: &RichTextEvent, _| {
                    if let RichTextEvent::ImageActivated(image) = event {
                        sink(
                            target.clone(),
                            TimelineAction::OpenImage {
                                source: image.source.clone(),
                                alt: image.alt.clone(),
                            },
                        );
                    }
                })?;
                context.assemble_markdown(entity)?;
                self.timeline_markdown.insert(item.id.clone(), entity);
                self.timeline_markdown_source
                    .insert(item.id.clone(), source);
                entity
            };
            let mut children = vec![markdown.stable_id()];
            let mut actions = Vec::new();
            if item.can_expand {
                let id = format!("expand-{}", item.id);
                action_keep.insert(id.clone());
                let button = self.upsert_timeline_action(
                    context,
                    document_id,
                    &id,
                    if item.expanded { "收起" } else { "展开" },
                    TimelineAction::Expand(item.id.clone()),
                )?;
                actions.push(button.stable_id());
            }
            if item.can_copy {
                let id = format!("copy-{}", item.id);
                action_keep.insert(id.clone());
                let button = self.upsert_timeline_action(
                    context,
                    document_id,
                    &id,
                    "复制",
                    TimelineAction::Copy(item.id.clone()),
                )?;
                actions.push(button.stable_id());
            }
            if item.can_retry {
                let id = format!("retry-{}", item.id);
                action_keep.insert(id.clone());
                let button = self.upsert_timeline_action(
                    context,
                    document_id,
                    &id,
                    "重试",
                    TimelineAction::Retry(item.id.clone()),
                )?;
                actions.push(button.stable_id());
            }
            if item.can_branch {
                let continue_id = format!("continue-{}", item.id);
                action_keep.insert(continue_id.clone());
                let button = self.upsert_timeline_action(
                    context,
                    document_id,
                    &continue_id,
                    "从这里继续",
                    TimelineAction::Continue(item.id.clone()),
                )?;
                actions.push(button.stable_id());
                let fork_id = format!("fork-{}", item.id);
                action_keep.insert(fork_id.clone());
                let button = self.upsert_timeline_action(
                    context,
                    document_id,
                    &fork_id,
                    "从这里分叉",
                    TimelineAction::Fork(item.id.clone()),
                )?;
                actions.push(button.stable_id());
            }
            if !actions.is_empty() {
                let toolbar = if let Some(toolbar) = self.timeline_toolbars.get(&item.id) {
                    *toolbar
                } else {
                    let toolbar =
                        context.create_detached_component(document_id, Stack::row(6.0))?;
                    self.timeline_toolbars.insert(item.id.clone(), toolbar);
                    toolbar
                };
                reconcile_children(context, toolbar.stable_id(), &actions)?;
                children.push(toolbar.stable_id());
            } else if let Some(toolbar) = self.timeline_toolbars.remove(&item.id) {
                context.remove_view(toolbar)?;
            }
            reconcile_children(context, root.stable_id(), &children)?;
        }
        let stale_actions: Vec<_> = self
            .timeline_actions
            .keys()
            .filter(|key| !action_keep.contains(*key))
            .cloned()
            .collect();
        for key in stale_actions {
            if let Some(entity) = self.timeline_actions.remove(&key) {
                let _ = context.remove_view(entity);
            }
        }
        let mut children = vec![self.timeline_scroll.stable_id()];
        if snapshot.can_load_earlier {
            if self.load_earlier.is_none() {
                let button = context.create_detached_component(
                    document_id,
                    pill_button("加载更早", ButtonKind::Subtle),
                )?;
                let target = snapshot.target.clone();
                let sink = Arc::clone(&self.sink);
                context.on(button, move |_, _: &Activate, _| {
                    sink(target.clone(), TimelineAction::LoadEarlier)
                })?;
                self.load_earlier = Some(button);
            }
            children.push(self.load_earlier.unwrap().stable_id());
        } else if let Some(button) = self.load_earlier.take() {
            context.remove_view(button)?;
        }
        reconcile_children(context, self.root.stable_id(), &children)?;
        let requested = snapshot.scroll_offset.max(0.0);
        let current = context
            .world()
            .scroll_offset(self.timeline_scroll.stable_id())
            .unwrap_or_default()
            .y;
        if let Some(anchor) = reading_anchor {
            context.restore_scroll_anchor(self.timeline_scroll, anchor)?;
        } else if task_changed || (requested - current).abs() > 0.5 {
            context.restore_scroll_anchor(
                self.timeline_scroll,
                nana_ui::runtime::ScrollAnchor {
                    row: self.timeline_list.stable_id(),
                    viewport_y: -requested,
                },
            )?;
        }
        Ok(())
    }

    fn upsert_timeline_action(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        id: &str,
        label: &str,
        intent: TimelineAction,
    ) -> Result<Entity<Chip>, FrameworkError> {
        if let Some(button) = self.timeline_actions.get(id).copied() {
            context.update_component(button, |button, _| {
                *button = token_chip(label, false);
            })?;
            Ok(button)
        } else {
            let button =
                context.create_detached_component(document_id, token_chip(label, false))?;
            let sink = Arc::clone(&self.sink);
            let target = self.target.clone();
            context.on(button, move |_, _: &Activate, _| {
                sink(target.clone(), intent.clone())
            })?;
            self.timeline_actions.insert(id.to_owned(), button);
            Ok(button)
        }
    }
}
fn timeline_virtual_layout(snapshot: &TimelineViewSnapshot) -> VirtualListLayout {
    if snapshot.layout.len() == snapshot.rows.len() {
        snapshot.layout.clone()
    } else {
        VirtualListLayout::new(snapshot.rows.iter().map(|_| TIMELINE_ROW_FALLBACK_EXTENT))
    }
}

fn viewport_extent(
    context: &AppContext,
    scroll: Entity<ScrollView>,
    snapshot: &TimelineViewSnapshot,
) -> f32 {
    context
        .world()
        .layout_box(scroll.stable_id())
        .map(|bounds| bounds.height)
        .filter(|height| height.is_finite() && *height > 0.0)
        .or_else(|| {
            (snapshot.viewport_extent.is_finite() && snapshot.viewport_extent > 0.0)
                .then_some(snapshot.viewport_extent)
        })
        .unwrap_or(TIMELINE_DEFAULT_VIEWPORT_EXTENT)
}

fn timeline_scroll_style() -> NodeStyle {
    let mut style = NodeStyle::default();
    let layout = Arc::make_mut(&mut style.layout);
    layout.flex_grow = Some(1.0);
    layout.flex_shrink = Some(1.0);
    layout.width = Some(LengthSpec::Fill);
    layout.height = Some(LengthSpec::Fill);
    layout.min_width = Some(LengthSpec::Px(0.0));
    layout.min_height = Some(LengthSpec::Px(0.0));
    style
}

fn timeline_list_style(total: f32, leading: f32, trailing: f32) -> NodeStyle {
    let mut style = NodeStyle::default();
    let layout = Arc::make_mut(&mut style.layout);
    layout.direction = Some(FlexDirection::Column);
    layout.width = Some(LengthSpec::Fill);
    layout.min_height = Some(LengthSpec::Px(total.max(0.0)));
    layout.padding_top = Some(LengthSpec::Px(leading.max(0.0)));
    layout.padding_bottom = Some(LengthSpec::Px(trailing.max(0.0)));
    style
}

fn timeline_action_is_mounted(action_id: &str, mounted: &HashSet<String>) -> bool {
    ["expand-", "copy-", "retry-", "continue-", "fork-"]
        .into_iter()
        .find_map(|prefix| action_id.strip_prefix(prefix))
        .is_some_and(|id| mounted.contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::{LayoutViewport, RuntimeDocument, ScrollOffset};

    #[test]
    fn resolved_markdown_images_become_inline_resources() {
        let markdown = concat!(
            "![Native 图片](data:image/png;base64,",
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=)"
        );
        let loaded = crate::markdown_images::load_markdown_image(
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
        )
        .unwrap();
        let row = TimelineRow {
            id: "image".into(),
            markdown: markdown.into(),
            images: vec![TimelineImage {
                source: NativeMarkdown::parse(markdown)
                    .images()
                    .into_iter()
                    .next()
                    .unwrap()
                    .source,
                data_url: loaded.data_url(),
                width: loaded.pixels.width,
                height: loaded.pixels.height,
            }],
            expanded: true,
            can_expand: false,
            can_retry: false,
            can_copy: true,
            can_branch: false,
        };
        let view = timeline_markdown_view(&row);
        assert!(view.blocks().iter().any(|block| {
            matches!(
                block,
                nana_ui::MarkdownBlock::Text { spans, .. }
                    if spans.iter().any(|span| {
                        span.image_resource.as_ref().is_some_and(|image| image.width == 1 && image.height == 1)
                    })
            )
        }));
    }

    fn snapshot(window_id: WindowId, task: &str) -> TimelineViewSnapshot {
        TimelineViewSnapshot {
            target: TimelineTarget {
                window_id,
                task_id: Some(task.into()),
            },
            rows: (0..100)
                .map(|index| TimelineRow {
                    id: format!("event-{index}"),
                    markdown: format!("Timeline row {index}"),
                    images: Vec::new(),
                    expanded: false,
                    can_expand: true,
                    can_copy: true,
                    can_retry: true,
                    can_branch: false,
                })
                .collect(),
            layout: VirtualListLayout::new(std::iter::repeat_n(72.0, 100)),
            scroll_offset: 0.0,
            viewport_extent: 240.0,
            can_load_earlier: true,
        }
    }

    #[test]
    fn both_window_views_share_actions_and_reject_queued_foreign_or_previous_task_events() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let mut views = Vec::new();
        for (document_id, window) in [(631, WindowId::PRIMARY), (632, WindowId(19))] {
            let document = DocumentId::new(document_id).unwrap();
            let host = context
                .create_component(document, Stack::fill_column(0.0))
                .unwrap();
            let snapshot = snapshot(window, "task-a");
            let sink_events = Arc::clone(&events);
            let mut view = TimelineView::mount(
                &mut context,
                document,
                snapshot.target.clone(),
                Arc::new(move |target, action| sink_events.lock().unwrap().push((target, action))),
            )
            .unwrap();
            context.append_child(host, view.root).unwrap();
            view.sync(&mut context, document, &snapshot).unwrap();
            assert!(view.timeline_virtual.mounted_keys().len() < snapshot.rows.len());
            for key in ["expand-event-0", "copy-event-0", "retry-event-0"] {
                context
                    .update_component(view.timeline_actions[key], |_, cx| cx.emit(Activate))
                    .unwrap();
            }
            context
                .update_component(view.load_earlier.unwrap(), |_, cx| cx.emit(Activate))
                .unwrap();
            views.push((view, document, snapshot));
        }
        let received = events.lock().unwrap().clone();
        assert_eq!(received.len(), 8);
        for (index, (_, _, snapshot)) in views.iter().enumerate() {
            let actions = &received[index * 4..index * 4 + 4];
            assert!(
                actions
                    .iter()
                    .all(|(target, _)| target.matches(&snapshot.target))
            );
            assert_eq!(
                actions
                    .iter()
                    .map(|(_, action)| action.clone())
                    .collect::<Vec<_>>(),
                vec![
                    TimelineAction::Expand("event-0".into()),
                    TimelineAction::Copy("event-0".into()),
                    TimelineAction::Retry("event-0".into()),
                    TimelineAction::LoadEarlier
                ]
            );
        }
        assert!(!received[0].0.matches(&views[1].2.target));
        let (view, document, current) = &mut views[0];
        let old_row = view
            .timeline_virtual
            .entity(&"event-0".to_owned())
            .unwrap()
            .stable_id();
        current.target.task_id = Some("task-b".into());
        view.sync(&mut context, *document, current).unwrap();
        assert!(!context.world().contains(old_row));
        assert!(!received[0].0.matches(&current.target));
        context
            .update_component(view.timeline_actions["copy-event-0"], |_, cx| {
                cx.emit(Activate)
            })
            .unwrap();
        assert!(
            events
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .0
                .matches(&current.target)
        );
        let closed = TimelineTarget {
            window_id: current.target.window_id,
            task_id: None,
        };
        assert!(!current.target.matches(&closed));
    }

    #[test]
    fn different_window_viewports_restore_independent_positions_and_prepend_without_jumping() {
        let mut instances = Vec::new();
        for (index, width, height, offset) in [(0, 640.0, 240.0, 400.0), (1, 420.0, 400.0, 800.0)] {
            let document_id = DocumentId::new(641 + index).unwrap();
            let mut document = RuntimeDocument::new(document_id);
            let mut snapshot = snapshot(WindowId(index), "shared-task");
            snapshot.can_load_earlier = false;
            snapshot.viewport_extent = height;
            snapshot.scroll_offset = offset;
            let events = Arc::new(Mutex::new(Vec::new()));
            let sink_events = Arc::clone(&events);
            let context = document.context_mut();
            let host = context
                .create_component(document_id, Stack::fill_column(0.0))
                .unwrap();
            let mut view = TimelineView::mount(
                context,
                document_id,
                snapshot.target.clone(),
                Arc::new(move |target, action| sink_events.lock().unwrap().push((target, action))),
            )
            .unwrap();
            context.append_child(host, view.root).unwrap();
            view.sync(context, document_id, &snapshot).unwrap();
            document
                .flush(
                    LayoutViewport::new(width, height),
                    &mut nana_ui::NanaTextShaper::default(),
                )
                .unwrap();
            assert!(
                (document
                    .context()
                    .world()
                    .scroll_offset(view.timeline_scroll.stable_id())
                    .unwrap()
                    .y
                    - offset)
                    .abs()
                    < 1.0,
                "requested={offset}, offset={:?}, scroll={:?}, metrics={:?}, list={:?}",
                document
                    .context()
                    .world()
                    .scroll_offset(view.timeline_scroll.stable_id()),
                document
                    .context()
                    .world()
                    .layout_box(view.timeline_scroll.stable_id()),
                document
                    .context()
                    .world()
                    .scroll_metrics(view.timeline_scroll.stable_id()),
                document
                    .context()
                    .world()
                    .layout_box(view.timeline_list.stable_id())
            );
            // A real user scroll becomes the next authoritative projection; syncing it must not restore again.
            document
                .context_mut()
                .scroll_at(
                    document_id,
                    width * 0.5,
                    height * 0.5,
                    ScrollOffset { x: 0.0, y: 35.0 },
                )
                .unwrap();
            snapshot.scroll_offset = document
                .context()
                .world()
                .scroll_offset(view.timeline_scroll.stable_id())
                .unwrap()
                .y;
            let scroll_event_count = events.lock().unwrap().len();
            view.sync(document.context_mut(), document_id, &snapshot)
                .unwrap();
            document
                .flush(
                    LayoutViewport::new(width, height),
                    &mut nana_ui::NanaTextShaper::default(),
                )
                .unwrap();
            assert_eq!(events.lock().unwrap().len(), scroll_event_count);
            assert!(
                (document
                    .context()
                    .world()
                    .scroll_offset(view.timeline_scroll.stable_id())
                    .unwrap()
                    .y
                    - snapshot.scroll_offset)
                    .abs()
                    < 1.0
            );
            instances.push((document, view, snapshot, width, height, events));
        }
        let other_position = instances[1]
            .0
            .context()
            .world()
            .scroll_offset(instances[1].1.timeline_scroll.stable_id())
            .unwrap();
        let (document, view, snapshot, width, height, events) = &mut instances[0];
        let anchor = view
            .timeline_virtual
            .mounted_keys()
            .iter()
            .find_map(|key| {
                let row = view.timeline_virtual.entity(key)?.stable_id();
                let anchor = document
                    .context()
                    .capture_scroll_anchor(view.timeline_scroll, row)
                    .ok()??;
                (anchor.viewport_y >= 0.0).then_some(anchor)
            })
            .unwrap();
        snapshot.rows.insert(
            0,
            TimelineRow {
                id: "earlier".into(),
                markdown: "Earlier event".into(),
                images: Vec::new(),
                expanded: false,
                can_expand: false,
                can_copy: false,
                can_retry: false,
                can_branch: false,
            },
        );
        snapshot.layout = VirtualListLayout::new(std::iter::repeat_n(72.0, snapshot.rows.len()));
        snapshot.scroll_offset += 72.0;
        let document_id = document.document();
        view.sync(document.context_mut(), document_id, snapshot)
            .unwrap();
        document
            .flush(
                LayoutViewport::new(*width, *height),
                &mut nana_ui::NanaTextShaper::default(),
            )
            .unwrap();
        let after = document
            .context()
            .capture_scroll_anchor(view.timeline_scroll, anchor.row)
            .unwrap()
            .unwrap();
        assert!((anchor.viewport_y - after.viewport_y).abs() < 1.0);
        let count = events.lock().unwrap().len();
        snapshot.scroll_offset = document
            .context()
            .world()
            .scroll_offset(view.timeline_scroll.stable_id())
            .unwrap()
            .y;
        let document_id = document.document();
        view.sync(document.context_mut(), document_id, snapshot)
            .unwrap();
        document
            .flush(
                LayoutViewport::new(*width, *height),
                &mut nana_ui::NanaTextShaper::default(),
            )
            .unwrap();
        assert_eq!(events.lock().unwrap().len(), count);
        assert_eq!(
            instances[1]
                .0
                .context()
                .world()
                .scroll_offset(instances[1].1.timeline_scroll.stable_id())
                .unwrap(),
            other_position
        );
    }

    #[test]
    fn branchable_rows_dispatch_fork_and_continue() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let document = DocumentId::new(633).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(WindowId::PRIMARY, "task");
        snapshot.rows.truncate(1);
        snapshot.rows[0].can_expand = false;
        snapshot.rows[0].can_copy = false;
        snapshot.rows[0].can_retry = false;
        snapshot.rows[0].can_branch = true;
        snapshot.can_load_earlier = false;
        snapshot.layout = VirtualListLayout::new([TIMELINE_ROW_FALLBACK_EXTENT]);
        let sink_events = Arc::clone(&events);
        let mut view = TimelineView::mount(
            &mut context,
            document,
            snapshot.target.clone(),
            Arc::new(move |target, action| sink_events.lock().unwrap().push((target, action))),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        view.sync(&mut context, document, &snapshot).unwrap();
        for key in ["continue-event-0", "fork-event-0"] {
            context
                .update_component(view.timeline_actions[key], |_, cx| cx.emit(Activate))
                .unwrap();
        }
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .map(|(_, action)| action.clone())
                .collect::<Vec<_>>(),
            vec![
                TimelineAction::Continue("event-0".into()),
                TimelineAction::Fork("event-0".into()),
            ]
        );
    }
}
