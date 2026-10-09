use crate::runtime_layout::{pill_button, reconcile_children};
pub(crate) use crate::ui::timeline::{StepKind, TimelineRole, TimelineTone};
use crate::ui::timeline_row::{RowChrome, RowView};
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
    pub role: TimelineRole,
    pub tone: TimelineTone,
    /// Step label, or the group summary.
    pub title: String,
    /// One-line object of a step (a path, a command) shown beside its title.
    pub detail: String,
    /// Body text: the message, or a step's expanded details.
    pub markdown: String,
    pub images: Vec<TimelineImage>,
    pub expanded: bool,
    pub can_expand: bool,
    pub can_retry: bool,
    pub can_copy: bool,
    pub can_branch: bool,
    /// User and assistant rows are the scrollbar's key nodes.
    pub key_node: bool,
}

fn timeline_markdown_view(item: &TimelineRow) -> NativeMarkdown {
    let mut markdown = NativeMarkdown::parse(&item.markdown);
    apply_timeline_markdown(&mut markdown, item);
    markdown
}

fn timeline_markdown_style(role: TimelineRole) -> NodeStyle {
    let mut style = NodeStyle::default();
    let layout = Arc::make_mut(&mut style.layout);
    layout.min_width = Some(LengthSpec::Px(0.0));
    layout.width = Some(if role == TimelineRole::User {
        LengthSpec::Shrink
    } else {
        LengthSpec::Fill
    });
    if matches!(role, TimelineRole::Step(_) | TimelineRole::Group) {
        style.foreground = Some(nana_ui::runtime::SemanticColorRole::Muted);
    }
    style
}

fn row_chrome(item: &TimelineRow) -> RowChrome {
    RowChrome {
        role: item.role,
        tone: item.tone,
        title: item.title.clone(),
        detail: item.detail.clone(),
        expanded: item.expanded,
        can_expand: item.can_expand,
        can_retry: item.can_retry,
        can_copy: item.can_copy,
        can_branch: item.can_branch,
        has_body: !item.markdown.trim().is_empty() || !item.images.is_empty(),
    }
}

fn apply_timeline_markdown(markdown: &mut NativeMarkdown, item: &TimelineRow) {
    *markdown = NativeMarkdown::parse(&item.markdown).style(timeline_markdown_style(item.role));
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
    std::mem::discriminant(&item.role).hash(&mut hasher);
    for image in &item.images {
        image.source.hash(&mut hasher);
        image.width.hash(&mut hasher);
        image.height.hash(&mut hasher);
        image.data_url.hash(&mut hasher);
    }
    hasher.finish()
}
use nana_ui::runtime::view::{entity_ref, widget, with_refs};
use nana_ui::runtime::{
    Activate, AlignSpec, AppContext, Button, DocumentId, Entity, FlexDirection, FrameworkError,
    LengthSpec, List, NativeMarkdown, NodeStyle, PositionSpec, RichTextEvent, ScrollAxes,
    ScrollChanged, ScrollView, Stack, Text, TextChanged, TextInput, VirtualListItems,
    VirtualListLayout,
};
use nana_ui::{ButtonKind, ControlSize, VirtualAlignment};
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
    Quote(String),
    Jump(String),
    OpenImage {
        source: String,
        alt: String,
    },
    LoadEarlier,
    SearchChanged(String),
    SearchStep(isize),
    SetSearchOpen(bool),
    TextSelected {
        event_id: String,
        text: Option<String>,
    },
    SelectionCopy,
    SelectionQuote,
    SelectionAsk,
    JumpToEnd,
    Scrolled {
        offset: f32,
        viewport_extent: f32,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineViewSnapshot {
    pub target: TimelineTarget,
    pub rows: Vec<TimelineRow>,
    pub layout: VirtualListLayout,
    pub scroll_offset: f32,
    pub viewport_extent: f32,
    pub can_load_earlier: bool,
    pub search_open: bool,
    pub search_query: String,
    pub search_status: String,
    pub search_can_step: bool,
    /// The message holding the current text selection.
    pub selection: Option<String>,
}
type Sink = Arc<dyn Fn(TimelineTarget, TimelineAction) + Send + Sync>;
pub(crate) struct TimelineView {
    pub(crate) root: Entity<Stack>,
    pub(crate) timeline_scroll: Entity<ScrollView>,
    pub(crate) timeline_list: Entity<List>,
    timeline_virtual: VirtualListItems<String, Stack>,
    pub(crate) timeline_markdown: HashMap<String, Entity<NativeMarkdown>>,
    timeline_markdown_source: HashMap<String, u64>,
    timeline_rows: HashMap<String, RowView>,
    pub(crate) key_markers: HashMap<String, Entity<Button>>,
    pub(crate) load_earlier: Option<Entity<Button>>,
    pub(crate) jump_to_end: Option<Entity<nana_ui::runtime::IconButton>>,
    pub(crate) selection_bar: Option<(Entity<Stack>, [Entity<Button>; 3])>,
    selected: Option<String>,
    search_bar: Option<Entity<Stack>>,
    search_input: Option<Entity<TextInput>>,
    pub(crate) search_previous: Option<Entity<Button>>,
    pub(crate) search_next: Option<Entity<Button>>,
    search_status: Option<Entity<Text>>,
    search_close: Option<Entity<nana_ui::runtime::IconButton>>,
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
        let scroll_target = Arc::new(Mutex::new((
            target.clone(),
            TIMELINE_DEFAULT_VIEWPORT_EXTENT,
        )));
        let scroll_binding = Arc::clone(&scroll_target);
        let scroll_sink = Arc::clone(&sink);
        let (_, (root, timeline_scroll, timeline_list)) =
            context.mount_view_detached(document, move || {
                let root = entity_ref::<Stack>();
                let timeline_scroll = entity_ref::<ScrollView>();
                let timeline_list = entity_ref::<List>();
                with_refs(
                    widget(Stack::fill_column(6.0).with_layout(|layout| {
                        layout.position = PositionSpec::Relative;
                    }))
                    .entity_ref(root)
                    .children(
                        widget(
                            ScrollView::new(ScrollAxes::Vertical).style(timeline_scroll_style()),
                        )
                        .entity_ref(timeline_scroll)
                        .on(move |event: &ScrollChanged| {
                            let (target, viewport_extent) = scroll_binding.lock().unwrap().clone();
                            scroll_sink(
                                target,
                                TimelineAction::Scrolled {
                                    offset: event.offset.y,
                                    viewport_extent,
                                },
                            );
                        })
                        .children(
                            widget(
                                List::new()
                                    .label("时间线")
                                    .style(timeline_list_style(0.0, 0.0, 0.0)),
                            )
                            .entity_ref(timeline_list),
                        ),
                    ),
                    (root, timeline_scroll, timeline_list),
                )
            })?;
        Ok(Self {
            root,
            timeline_scroll,
            timeline_list,
            timeline_virtual: VirtualListItems::default(),
            timeline_markdown: HashMap::new(),
            timeline_markdown_source: HashMap::new(),
            timeline_rows: HashMap::new(),
            key_markers: HashMap::new(),
            load_earlier: None,
            jump_to_end: None,
            selection_bar: None,
            selected: None,
            search_bar: None,
            search_input: None,
            search_previous: None,
            search_next: None,
            search_status: None,
            search_close: None,
            sink,
            target,
            first_item: None,
            scroll_target,
        })
    }
    pub(crate) fn search_input(&self) -> Option<nana_ui::runtime::StableNodeId> {
        self.search_input.map(|input| input.stable_id())
    }

    /// Mounted entries by event id.
    pub(crate) fn rows(&self) -> impl Iterator<Item = (&str, &RowView)> {
        self.timeline_rows
            .iter()
            .map(|(id, row)| (id.as_str(), row))
    }

    /// Emits the activation of the control named `"<action>-<event id>"`.
    #[cfg(test)]
    pub(crate) fn activate(&self, context: &mut AppContext, key: &str) {
        let (action, id) = key.split_once('-').expect("action key");
        let row = &self.timeline_rows[id];
        if action == "expand" {
            let button = row.actions.expand.expect("expand control");
            context
                .update_component(button, |_, cx| cx.emit(Activate))
                .unwrap();
            return;
        }
        let button = match action {
            "copy" => row.actions.copy,
            "quote" => row.actions.quote,
            "retry" => row.actions.retry,
            "continue" => row.actions.resume,
            "fork" => row.actions.fork,
            _ => None,
        }
        .expect("row action");
        context
            .update_component(button, |_, cx| cx.emit(Activate))
            .unwrap();
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
            self.timeline_rows.clear();
            for (_, marker) in self.key_markers.drain() {
                let _ = context.remove_view(marker);
            }
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
        self.timeline_rows.retain(|key, _| mounted.contains(key));
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
                let markdown_view = timeline_markdown_view(item);
                let (_, entity) = context.mount_view_detached(document_id, move || {
                    let entity = entity_ref::<NativeMarkdown>();
                    with_refs(widget(markdown_view).entity_ref(entity), entity)
                })?;
                let target = snapshot.target.clone();
                let sink = Arc::clone(&self.sink);
                let event_id = item.id.clone();
                context.on(entity, move |_, event: &RichTextEvent, _| match event {
                    RichTextEvent::ImageActivated(image) => sink(
                        target.clone(),
                        TimelineAction::OpenImage {
                            source: image.source.clone(),
                            alt: image.alt.clone(),
                        },
                    ),
                    RichTextEvent::SelectionChanged(selection) => sink(
                        target.clone(),
                        TimelineAction::TextSelected {
                            event_id: event_id.clone(),
                            text: selection.as_ref().map(|selection| selection.text.clone()),
                        },
                    ),
                    RichTextEvent::LinkActivated(_) => {}
                })?;
                context.assemble_markdown(entity)?;
                self.timeline_markdown.insert(item.id.clone(), entity);
                self.timeline_markdown_source
                    .insert(item.id.clone(), source);
                entity
            };
            let chrome = row_chrome(item);
            let reuse = self
                .timeline_rows
                .get(&item.id)
                .is_some_and(|row| row.role == item.role);
            if reuse {
                self.timeline_rows[&item.id].sync(chrome);
            } else {
                if let Some(stale) = self.timeline_rows.remove(&item.id) {
                    reconcile_children(context, stale.body, &[])?;
                    stale.unmount(context)?;
                }
                let sink = Arc::clone(&self.sink);
                let target = snapshot.target.clone();
                let row = RowView::mount(
                    context,
                    document_id,
                    &item.id,
                    chrome,
                    Arc::new(move |action| sink(target.clone(), action)),
                )?;
                self.timeline_rows.insert(item.id.clone(), row);
            }
            let row = &self.timeline_rows[&item.id];
            reconcile_children(context, row.body, &[markdown.stable_id()])?;
            reconcile_children(context, root.stable_id(), &[row.root])?;
        }
        let marker_ids = self.sync_key_markers(context, document_id, snapshot, &layout)?;
        let mut children = Vec::new();
        if let Some(search) = self.sync_message_search(context, document_id, snapshot)? {
            children.push(search);
        }
        children.push(self.timeline_scroll.stable_id());
        children.extend(marker_ids);
        if snapshot.can_load_earlier {
            if self.load_earlier.is_none() {
                let (_, button) = context.mount_view_detached(document_id, || {
                    let button = entity_ref::<Button>();
                    with_refs(
                        widget(pill_button("加载更早", ButtonKind::Subtle)).entity_ref(button),
                        button,
                    )
                })?;
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
        if let Some(button) = self.sync_jump_to_end(context, document_id, snapshot)? {
            children.push(button);
        }
        if let Some(bar) = self.sync_selection_bar(context, document_id, snapshot)? {
            children.push(bar);
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

    /// Copy, quote or ask about the selected text, in a bar over the
    /// selection. Scrolling syncs again, so the bar follows the text.
    fn sync_selection_bar(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TimelineViewSnapshot,
    ) -> Result<Option<nana_ui::runtime::StableNodeId>, FrameworkError> {
        if self.selected != snapshot.selection {
            let cleared = self
                .selected
                .take()
                .and_then(|id| self.timeline_markdown.get(&id).copied());
            if let Some(markdown) = cleared {
                context.update_component(markdown, |markdown, _| markdown.clear_selection())?;
            }
            self.selected = snapshot.selection.clone();
        }
        if snapshot.selection.is_none() {
            if let Some((bar, _)) = self.selection_bar.take() {
                context.remove_view(bar)?;
            }
            return Ok(None);
        }
        if let Some((bar, _)) = self.selection_bar {
            self.place_selection_bar(context, bar, snapshot)?;
            return Ok(Some(bar.stable_id()));
        }
        let (_, (bar, buttons)) = context.mount_view_detached(document_id, || {
            let bar = entity_ref::<Stack>();
            let buttons = [
                entity_ref::<Button>(),
                entity_ref::<Button>(),
                entity_ref::<Button>(),
            ];
            let button = |label: &'static str, icon| {
                Button::new(label)
                    .kind(ButtonKind::Text)
                    .size(ControlSize::Small)
                    .icon(icon)
            };
            with_refs(
                widget(selection_bar_style(None)).entity_ref(bar).children((
                    widget(button("复制", nana_ui::icons_tabler::COPY)).entity_ref(buttons[0]),
                    widget(button("引用", nana_ui::icons_tabler::QUOTE)).entity_ref(buttons[1]),
                    widget(button("在弹窗中提问", nana_ui::icons_tabler::MESSAGE))
                        .entity_ref(buttons[2]),
                )),
                (bar, buttons),
            )
        })?;
        for (button, action) in buttons.into_iter().zip([
            TimelineAction::SelectionCopy,
            TimelineAction::SelectionQuote,
            TimelineAction::SelectionAsk,
        ]) {
            let target = Arc::clone(&self.scroll_target);
            let sink = Arc::clone(&self.sink);
            context.on(button, move |_, _: &Activate, _| {
                sink(target.lock().unwrap().0.clone(), action.clone())
            })?;
        }
        self.selection_bar = Some((bar, buttons));
        self.place_selection_bar(context, bar, snapshot)?;
        Ok(Some(bar.stable_id()))
    }

    fn place_selection_bar(
        &self,
        context: &mut AppContext,
        bar: Entity<Stack>,
        snapshot: &TimelineViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let world = context.world();
        let origin = snapshot
            .selection
            .as_ref()
            .and_then(|id| self.timeline_markdown.get(id))
            .and_then(|markdown| world.text_selection_bounds(markdown.stable_id()))
            .zip(world.presentation_input_bounds(self.root.stable_id()))
            .map(|(selected, frame)| {
                let height = world
                    .canonical_layout_box(bar.stable_id())
                    .map(|bar| bar.height)
                    .filter(|height| *height > 0.0)
                    .unwrap_or(SELECTION_BAR_HEIGHT);
                selection_bar_origin(selected, frame, height)
            });
        context.update_component(bar, |stack, _| *stack = selection_bar_style(origin))
    }

    fn sync_jump_to_end(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TimelineViewSnapshot,
    ) -> Result<Option<nana_ui::runtime::StableNodeId>, FrameworkError> {
        let below = snapshot.layout.total_extent()
            - snapshot.scroll_offset
            - snapshot.viewport_extent.max(0.0);
        if below <= JUMP_TO_END_DISTANCE {
            if let Some(button) = self.jump_to_end.take() {
                context.remove_view(button)?;
            }
            return Ok(None);
        }
        if let Some(button) = self.jump_to_end {
            return Ok(Some(button.stable_id()));
        }
        let (_, button) = context.mount_view_detached(document_id, || {
            let button = entity_ref::<nana_ui::runtime::IconButton>();
            with_refs(widget(jump_to_end_button()).entity_ref(button), button)
        })?;
        let target = Arc::clone(&self.scroll_target);
        let sink = Arc::clone(&self.sink);
        context.on(button, move |_, _: &Activate, _| {
            let target = target.lock().unwrap().0.clone();
            sink(target, TimelineAction::JumpToEnd)
        })?;
        self.jump_to_end = Some(button);
        Ok(Some(button.stable_id()))
    }

    fn sync_key_markers(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TimelineViewSnapshot,
        layout: &VirtualListLayout,
    ) -> Result<Vec<nana_ui::runtime::StableNodeId>, FrameworkError> {
        let markers = timeline_key_markers(&snapshot.rows, layout);
        let mut keep = HashSet::new();
        let mut ids = Vec::new();
        for marker in &markers {
            keep.insert(marker.id.clone());
            let button = if let Some(button) = self.key_markers.get(&marker.id).copied() {
                let view = key_node_button(marker);
                context.update_component(button, |button, _| {
                    *button = view;
                })?;
                button
            } else {
                let view = key_node_button(marker);
                let (_, button) = context.mount_view_detached(document_id, move || {
                    let button = entity_ref::<Button>();
                    with_refs(widget(view).entity_ref(button), button)
                })?;
                let sink = Arc::clone(&self.sink);
                let target = self.target.clone();
                let row_id = marker.id.clone();
                context.on(button, move |_, _: &Activate, _| {
                    sink(target.clone(), TimelineAction::Jump(row_id.clone()));
                })?;
                self.key_markers.insert(marker.id.clone(), button);
                button
            };
            ids.push(button.stable_id());
        }
        let stale: Vec<_> = self
            .key_markers
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect();
        for id in stale {
            if let Some(button) = self.key_markers.remove(&id) {
                let _ = context.remove_view(button);
            }
        }
        Ok(ids)
    }

    fn sync_message_search(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TimelineViewSnapshot,
    ) -> Result<Option<nana_ui::runtime::StableNodeId>, FrameworkError> {
        if snapshot.target.task_id.is_none() || !snapshot.search_open {
            return Ok(None);
        }
        if self.search_bar.is_none() {
            let query = snapshot.search_query.clone();
            let (_, (bar, input, status, close)) =
                context.mount_view_detached(document_id, move || {
                    let bar = entity_ref::<Stack>();
                    let input = entity_ref::<TextInput>();
                    let status = entity_ref::<Text>();
                    let close = entity_ref::<nana_ui::runtime::IconButton>();
                    with_refs(
                        widget(Stack::bar(6.0).align(AlignSpec::Center))
                            .entity_ref(bar)
                            .children((
                                widget(message_search_input(query)).entity_ref(input),
                                widget(
                                    Text::new(String::new())
                                        .font_size(nana_ui_core::type_scale::META)
                                        .color(nana_ui::runtime::SemanticColorRole::Muted),
                                )
                                .entity_ref(status),
                                widget(crate::ui::timeline_row::row_action(
                                    nana_ui::icons_tabler::X,
                                    "关闭查找",
                                ))
                                .entity_ref(close),
                            )),
                        (bar, input, status, close),
                    )
                })?;
            let sink = Arc::clone(&self.sink);
            let target = self.target.clone();
            context.on(input, move |_, event: &TextChanged, _| {
                sink(
                    target.clone(),
                    TimelineAction::SearchChanged(event.value.to_string()),
                );
            })?;
            let sink = Arc::clone(&self.sink);
            let target = self.target.clone();
            context.on(close, move |_, _: &Activate, _| {
                sink(target.clone(), TimelineAction::SetSearchOpen(false));
            })?;
            self.search_close = Some(close);
            self.search_bar = Some(bar);
            self.search_input = Some(input);
            self.search_status = Some(status);
        }
        let input = self.search_input.unwrap();
        let status = self.search_status.unwrap();
        let bar = self.search_bar.unwrap();
        context.update_component(input, |input, _| {
            if input.state.value != snapshot.search_query {
                input.state.replace_value(snapshot.search_query.clone());
            }
        })?;
        let status_label = snapshot.search_status.clone();
        context.update_component(status, |status, _| {
            *status = Text::new(status_label);
        })?;
        let close = self.search_close.map(|close| close.stable_id());
        let mut order = vec![input.stable_id(), status.stable_id()];
        if snapshot.search_can_step {
            let previous = self.search_step_button(
                context,
                document_id,
                true,
                "上一条",
                TimelineAction::SearchStep(-1),
            )?;
            let next = self.search_step_button(
                context,
                document_id,
                false,
                "下一条",
                TimelineAction::SearchStep(1),
            )?;
            order.push(previous.stable_id());
            order.push(next.stable_id());
        } else {
            if let Some(button) = self.search_previous.take() {
                context.remove_view(button)?;
            }
            if let Some(button) = self.search_next.take() {
                context.remove_view(button)?;
            }
        }
        order.extend(close);
        reconcile_children(context, bar.stable_id(), &order)?;
        Ok(Some(bar.stable_id()))
    }

    fn search_step_button(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        previous: bool,
        label: &str,
        action: TimelineAction,
    ) -> Result<Entity<Button>, FrameworkError> {
        let slot = if previous {
            &mut self.search_previous
        } else {
            &mut self.search_next
        };
        if let Some(button) = *slot {
            let label = label.to_owned();
            context.update_component(button, |button, _| {
                *button = pill_button(&label, ButtonKind::Subtle);
            })?;
            return Ok(button);
        }
        let label = label.to_owned();
        let (_, button) = context.mount_view_detached(document_id, move || {
            let button = entity_ref::<Button>();
            with_refs(
                widget(pill_button(&label, ButtonKind::Subtle)).entity_ref(button),
                button,
            )
        })?;
        let sink = Arc::clone(&self.sink);
        let target = self.target.clone();
        context.on(button, move |_, _: &Activate, _| {
            sink(target.clone(), action.clone());
        })?;
        if previous {
            self.search_previous = Some(button);
        } else {
            self.search_next = Some(button);
        }
        Ok(button)
    }
}
fn timeline_virtual_layout(snapshot: &TimelineViewSnapshot) -> VirtualListLayout {
    timeline_layout_for(&snapshot.rows, &snapshot.layout)
}

fn timeline_layout_for(rows: &[TimelineRow], layout: &VirtualListLayout) -> VirtualListLayout {
    if layout.len() == rows.len() {
        layout.clone()
    } else {
        VirtualListLayout::new(rows.iter().map(|_| TIMELINE_ROW_FALLBACK_EXTENT))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TimelineKeyMarker {
    pub id: String,
    pub name: String,
    pub ratio: f32,
    /// A step that failed, so the reader can find what went wrong.
    pub failed: bool,
}

pub(crate) fn timeline_key_markers(
    rows: &[TimelineRow],
    layout: &VirtualListLayout,
) -> Vec<TimelineKeyMarker> {
    let resolved = timeline_layout_for(rows, layout);
    let total = resolved.total_extent();
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.key_node || row.tone == TimelineTone::Failed)
        .map(|(index, row)| {
            let start = resolved.extent(0..index);
            let ratio = if total > 0.0 {
                (start / total).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let failed = !row.key_node;
            TimelineKeyMarker {
                id: row.id.clone(),
                name: if failed {
                    key_node_name(&format!("{} {}", row.title, row.detail))
                } else {
                    key_node_name(&row.markdown)
                },
                ratio,
                failed,
            }
        })
        .collect()
}

pub(crate) fn timeline_event_scroll_offset(
    rows: &[TimelineRow],
    layout: &VirtualListLayout,
    row_id: &str,
    viewport_extent: f32,
) -> Option<f32> {
    let index = rows.iter().position(|row| row.id == row_id)?;
    timeline_row_scroll_offset(rows, layout, index, viewport_extent)
}

fn timeline_row_scroll_offset(
    rows: &[TimelineRow],
    layout: &VirtualListLayout,
    index: usize,
    viewport_extent: f32,
) -> Option<f32> {
    timeline_layout_for(rows, layout).offset_for_index(
        index,
        0.0,
        viewport_extent,
        VirtualAlignment::Start,
    )
}

/// Characters of the message that name its marker.
const KEY_NODE_LABEL_CHARS: usize = 24;

fn key_node_name(markdown: &str) -> String {
    let line = markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("关键");
    let snippet: String = line.chars().take(KEY_NODE_LABEL_CHARS).collect();
    if snippet.is_empty() {
        "关键".into()
    } else {
        snippet
    }
}

const KEY_MARKER_HEIGHT: f32 = 4.0;
const KEY_MARKER_WIDTH: f32 = 14.0;
/// Right-hand strip of the conversation holding one tick per message.
const TIMELINE_KEY_TRACK: f32 = 18.0;

/// One tick on the conversation's scroll map. The message snippet is its
/// accessible name; the tick itself carries no text.
fn key_node_button(marker: &TimelineKeyMarker) -> Button {
    let ratio = marker.ratio.clamp(0.0, 1.0);
    let mut button = Button::new("").size(nana_ui::ControlSize::Small);
    button.accessible_name = marker.name.clone();
    let mut style = button.style.clone();
    style.control_height = None;
    style.control_padding_x = None;
    style.control_padding_y = None;
    style.square = None;
    style.border = None;
    style.background = Some(if marker.failed {
        nana_ui::runtime::SemanticColorRole::Danger
    } else {
        nana_ui::runtime::SemanticColorRole::BorderStrong
    });
    style.interaction = Default::default();
    style.interaction.hovered.background = Some(nana_ui::runtime::SemanticColorRole::Accent);
    style.interaction.pressed.background = Some(nana_ui::runtime::SemanticColorRole::AccentStrong);
    let layout = Arc::make_mut(&mut style.layout);
    layout.position = PositionSpec::Absolute;
    layout.width = Some(LengthSpec::Px(KEY_MARKER_WIDTH));
    layout.height = Some(LengthSpec::Px(KEY_MARKER_HEIGHT));
    layout.min_width = Some(LengthSpec::Px(KEY_MARKER_WIDTH));
    layout.min_height = Some(LengthSpec::Px(KEY_MARKER_HEIGHT));
    layout.flex_shrink = Some(0.0);
    layout.border_width = Some(0.0);
    layout.border_radius = Some(KEY_MARKER_HEIGHT / 2.0);
    layout.padding_left = Some(LengthSpec::Px(0.0));
    layout.padding_right = Some(LengthSpec::Px(0.0));
    layout.offset_right = Some(LengthSpec::Px(
        (TIMELINE_KEY_TRACK - KEY_MARKER_WIDTH) / 2.0,
    ));
    layout.offset_top = Some(LengthSpec::CalcPercentOffset {
        percent: ratio * 100.0,
        offset_px: -KEY_MARKER_HEIGHT * ratio,
    });
    layout.z_index = Some(5);
    button.style(style)
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

fn message_search_input(query: String) -> TextInput {
    let mut input = TextInput::new(query)
        .placeholder("搜索消息")
        .size(ControlSize::Small);
    let layout = Arc::make_mut(&mut input.style.layout);
    layout.flex_grow = Some(1.0);
    layout.flex_shrink = Some(1.0);
    layout.min_width = Some(LengthSpec::Px(0.0));
    input
}

/// How far above the newest output the reader has to be before the
/// jump-to-end button shows.
const JUMP_TO_END_DISTANCE: f32 = 240.0;
const JUMP_TO_END_SIZE: f32 = 32.0;

/// Where the selection bar sits in the conversation: centred over the
/// selection, under it when the selection starts too near the top, and never
/// past the conversation's edges.
fn selection_bar_origin(
    selected: nana_ui::runtime::LayoutBox,
    frame: nana_ui::runtime::LayoutBox,
    height: f32,
) -> (f32, f32) {
    let inner = nana_ui::runtime::LayoutBox {
        x: frame.x + SELECTION_BAR_MARGIN,
        y: frame.y + SELECTION_BAR_MARGIN,
        width: (frame.width - 2.0 * SELECTION_BAR_MARGIN).max(SELECTION_BAR_WIDTH),
        height: (frame.height - 2.0 * SELECTION_BAR_MARGIN).max(height),
    };
    let (x, y) = nana_ui::runtime::resolve_popover_origin(
        selected,
        SELECTION_BAR_WIDTH,
        height,
        inner,
        nana_ui::PopoverPlacement::Top,
        nana_ui::PopoverAlignment::Center,
        6.0,
    );
    (x - frame.x, y - frame.y)
}

/// Before any selection geometry, the bar waits at the bottom centre.
fn selection_bar_style(origin: Option<(f32, f32)>) -> Stack {
    Stack::row(2.0)
        .align(AlignSpec::Center)
        .justify(nana_ui::runtime::JustifySpec::Center)
        .padding_xy(4.0, 2.0)
        .surface(nana_ui::runtime::SemanticColorRole::Surface)
        .outline(nana_ui::runtime::SemanticColorRole::Border, 1.0)
        .radius(nana_ui_core::RadiusTier::Md)
        .width(LengthSpec::Px(SELECTION_BAR_WIDTH))
        .with_layout(|layout| {
            layout.position = PositionSpec::Absolute;
            layout.z_index = Some(6);
            match origin {
                Some((left, top)) => {
                    layout.offset_left = Some(LengthSpec::Px(left));
                    layout.offset_top = Some(LengthSpec::Px(top));
                }
                None => {
                    layout.offset_bottom = Some(LengthSpec::Px(12.0));
                    layout.offset_left = Some(LengthSpec::CalcPercentOffset {
                        percent: 50.0,
                        offset_px: -SELECTION_BAR_WIDTH / 2.0,
                    });
                }
            }
        })
}

const SELECTION_BAR_WIDTH: f32 = 260.0;
const SELECTION_BAR_HEIGHT: f32 = 32.0;
const SELECTION_BAR_MARGIN: f32 = 8.0;

fn jump_to_end_button() -> nana_ui::runtime::IconButton {
    let mut button =
        nana_ui::runtime::IconButton::new(nana_ui::icons_tabler::ARROW_DOWN, "回到底部")
            .kind(ButtonKind::Menu)
            .with_tooltip("回到底部");
    let edge = LengthSpec::Px(JUMP_TO_END_SIZE);
    let layout = Arc::make_mut(&mut button.style.layout);
    layout.width = Some(edge);
    layout.height = Some(edge);
    layout.min_width = Some(edge);
    layout.min_height = Some(edge);
    layout.padding_left = Some(LengthSpec::Px(0.0));
    layout.padding_right = Some(LengthSpec::Px(0.0));
    layout.border_radius = Some(JUMP_TO_END_SIZE / 2.0);
    layout.position = PositionSpec::Absolute;
    layout.offset_bottom = Some(LengthSpec::Px(12.0));
    layout.offset_left = Some(LengthSpec::CalcPercentOffset {
        percent: 50.0,
        offset_px: -JUMP_TO_END_SIZE / 2.0,
    });
    button.style = button
        .style
        .surface(nana_ui::runtime::SemanticColorRole::Surface)
        .outline(nana_ui::runtime::SemanticColorRole::Border, 1.0);
    button
}

fn timeline_scroll_style() -> NodeStyle {
    let mut style = NodeStyle::default();
    let layout = Arc::make_mut(&mut style.layout);
    layout.flex_grow = Some(1.0);
    layout.flex_shrink = Some(1.0);
    layout.width = Some(LengthSpec::CalcPercentOffset {
        percent: 100.0,
        offset_px: -TIMELINE_KEY_TRACK,
    });
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
            role: crate::module::timeline::view::TimelineRole::Reply,
            tone: crate::module::timeline::view::TimelineTone::Settled,
            title: String::new(),
            detail: String::new(),
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
            key_node: false,
        };
        let view = timeline_markdown_view(&row);
        assert!(view.blocks().iter().any(|block| {
            matches!(
                block,
                nana_ui::runtime::MarkdownBlock::Text { spans, .. }
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
                    role: crate::module::timeline::view::TimelineRole::Step(
                        crate::module::timeline::view::StepKind::Tool,
                    ),
                    tone: crate::module::timeline::view::TimelineTone::Settled,
                    title: String::new(),
                    detail: String::new(),
                    markdown: format!("Timeline row {index}"),
                    images: Vec::new(),
                    expanded: false,
                    can_expand: true,
                    can_copy: true,
                    can_retry: true,
                    can_branch: false,
                    key_node: false,
                })
                .collect(),
            layout: VirtualListLayout::new(std::iter::repeat_n(72.0, 100)),
            scroll_offset: 0.0,
            viewport_extent: 240.0,
            can_load_earlier: true,
            search_query: String::new(),
            search_status: String::new(),
            search_open: false,
            search_can_step: false,
            selection: None,
        }
    }

    #[test]
    fn message_search_next_steps_to_the_following_hit() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let document = DocumentId::new(633).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(WindowId::PRIMARY, "task-a");
        snapshot.search_open = true;
        snapshot.search_query = "登录".to_owned();
        snapshot.search_status = "2/2".to_owned();
        snapshot.search_can_step = true;
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
        let next = view.search_next.expect("search step");
        context
            .update_component(next, |_, cx| cx.emit(Activate))
            .unwrap();
        assert_eq!(
            events
                .lock()
                .unwrap()
                .last()
                .map(|(_, action)| action.clone()),
            Some(TimelineAction::SearchStep(1))
        );
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
                view.activate(&mut context, key);
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
            assert!(actions
                .iter()
                .all(|(target, _)| target.matches(&snapshot.target)));
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
        view.activate(&mut context, "copy-event-0");
        assert!(events
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .0
            .matches(&current.target));
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
                role: crate::module::timeline::view::TimelineRole::Step(
                    crate::module::timeline::view::StepKind::Tool,
                ),
                tone: crate::module::timeline::view::TimelineTone::Settled,
                title: String::new(),
                detail: String::new(),
                markdown: "Earlier event".into(),
                images: Vec::new(),
                expanded: false,
                can_expand: false,
                can_copy: false,
                can_retry: false,
                can_branch: false,
                key_node: false,
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
    fn jump_to_end_shows_only_while_reading_older_output() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let document = DocumentId::new(635).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(WindowId::PRIMARY, "task");
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

        let button = view
            .jump_to_end
            .expect("jump control while reading the top");
        context
            .update_component(button, |_, cx| cx.emit(Activate))
            .unwrap();
        assert!(matches!(
            events.lock().unwrap().last(),
            Some((target, TimelineAction::JumpToEnd)) if *target == snapshot.target
        ));

        snapshot.scroll_offset = snapshot.layout.total_extent() - snapshot.viewport_extent;
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(view.jump_to_end.is_none());
        assert!(context.world().node(button.stable_id()).is_none());
    }

    #[test]
    fn message_actions_keep_their_space_and_show_on_hover_or_keyboard_focus() {
        let mut context = AppContext::new();
        let document = DocumentId::new(636).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(WindowId::PRIMARY, "task");
        snapshot.rows.truncate(1);
        snapshot.rows[0].role = TimelineRole::Reply;
        snapshot.rows[0].can_expand = false;
        snapshot.rows[0].can_branch = true;
        snapshot.can_load_earlier = false;
        snapshot.layout = VirtualListLayout::new([TIMELINE_ROW_FALLBACK_EXTENT]);
        let mut view = TimelineView::mount(
            &mut context,
            document,
            snapshot.target.clone(),
            Arc::new(|_, _| {}),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        view.sync(&mut context, document, &snapshot).unwrap();

        let row = &view.timeline_rows["event-0"];
        let fork = row.actions.fork.unwrap().stable_id();
        let bar = context.world().parent_id(fork).unwrap();
        let message = context.world().parent_id(row.body).unwrap();
        let painted = |context: &AppContext| {
            context.world().node_style(bar).unwrap().layout.opacity != Some(0.0)
        };
        let shown = |context: &AppContext| !context.world().node_style(bar).unwrap().layout.hidden;
        assert!(!painted(&context));
        assert!(shown(&context));

        context
            .set_pointer_hover(document, 1, Some(message))
            .unwrap();
        context.flush_reactive().unwrap();
        assert!(painted(&context));
        context.set_pointer_hover(document, 1, Some(fork)).unwrap();
        context.flush_reactive().unwrap();
        assert!(painted(&context));
        context.set_pointer_hover(document, 1, None).unwrap();
        context.flush_reactive().unwrap();
        assert!(!painted(&context));
        assert!(shown(&context));

        assert!(context.focus_node(document, fork).unwrap());
        context.flush_reactive().unwrap();
        assert!(painted(&context));
        context.clear_focus(document).unwrap();
        context.flush_reactive().unwrap();
        assert!(!painted(&context));
    }

    #[test]
    fn selecting_text_offers_copy_quote_and_ask() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let document = DocumentId::new(637).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(WindowId::PRIMARY, "task");
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
        assert!(view.jump_to_end.is_some());
        assert!(view.selection_bar.is_none());

        let markdown = view.timeline_markdown["event-0"];
        context
            .update_component(markdown, |_, cx| {
                cx.emit(RichTextEvent::SelectionChanged(Some(
                    nana_ui::runtime::TextSelectionSnapshot {
                        start: 0,
                        end: 8,
                        text: "Timeline".into(),
                    },
                )))
            })
            .unwrap();
        assert!(matches!(
            events.lock().unwrap().last(),
            Some((_, TimelineAction::TextSelected { event_id, text: Some(text) }))
                if event_id == "event-0" && text == "Timeline"
        ));

        snapshot.selection = Some("event-0".into());
        view.sync(&mut context, document, &snapshot).unwrap();
        let (bar, buttons) = view.selection_bar.expect("selection bar");
        let root = context.world().node(view.root.stable_id()).unwrap();
        assert!(root.children.contains(&bar.stable_id()));
        for button in buttons {
            context
                .update_component(button, |_, cx| cx.emit(Activate))
                .unwrap();
        }
        let tail = events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .take(3)
            .map(|(_, action)| action.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            tail,
            [
                TimelineAction::SelectionAsk,
                TimelineAction::SelectionQuote,
                TimelineAction::SelectionCopy,
            ]
        );

        snapshot.selection = None;
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(view.selection_bar.is_none());
        assert!(context.world().node(bar.stable_id()).is_none());
        assert!(view.jump_to_end.is_some());
    }

    #[test]
    fn the_selection_bar_sits_over_the_selected_text_without_covering_it() {
        let mut context = AppContext::new();
        let document = DocumentId::new(638).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut snapshot = snapshot(WindowId::PRIMARY, "task");
        snapshot.rows.truncate(3);
        snapshot.rows[0].role = TimelineRole::Reply;
        snapshot.rows[0].can_expand = false;
        snapshot.layout = VirtualListLayout::new([72.0; 3]);
        snapshot.scroll_offset = 0.0;
        snapshot.can_load_earlier = false;
        let mut view = TimelineView::mount(
            &mut context,
            document,
            snapshot.target.clone(),
            Arc::new(|_, _| {}),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        view.sync(&mut context, document, &snapshot).unwrap();
        let relayout = |context: &mut AppContext| {
            context
                .layout_document(
                    document,
                    nana_ui::runtime::LayoutViewport::new(900.0, 600.0),
                )
                .unwrap();
        };
        relayout(&mut context);
        view.sync(&mut context, document, &snapshot).unwrap();
        relayout(&mut context);

        let markdown = view.timeline_markdown["event-0"];
        let area = context
            .world()
            .canonical_layout_box(markdown.stable_id())
            .unwrap();
        context
            .update_component(markdown, |markdown, _| {
                markdown.pointer_down(area.x + 1.0, area.y + 4.0, area);
                markdown.pointer_move(area.x + 40.0, area.y + 4.0, area);
                markdown.pointer_up(area.x + 40.0, area.y + 4.0, area);
            })
            .unwrap();
        snapshot.selection = Some("event-0".into());
        view.sync(&mut context, document, &snapshot).unwrap();
        relayout(&mut context);
        view.sync(&mut context, document, &snapshot).unwrap();
        relayout(&mut context);

        let (bar, _) = view.selection_bar.expect("selection bar");
        let world = context.world();
        let selected = world
            .text_selection_bounds(markdown.stable_id())
            .expect("selected text");
        let shown = world.presentation_input_bounds(bar.stable_id()).unwrap();
        let frame = world
            .presentation_input_bounds(view.root.stable_id())
            .unwrap();
        assert!(
            shown.y + shown.height <= selected.y || shown.y >= selected.y + selected.height,
            "{shown:?} covers {selected:?}"
        );
        assert!(shown.y - (selected.y + selected.height) < 12.0 && selected.y - shown.y < 48.0);
        assert!(shown.x >= frame.x && shown.x + shown.width <= frame.x + frame.width);
    }

    #[test]
    fn failed_steps_join_the_scroll_map_under_their_own_name() {
        let mut snapshot = snapshot(WindowId::PRIMARY, "task");
        snapshot.rows.truncate(4);
        snapshot.layout = VirtualListLayout::new([100.0; 4]);
        snapshot.rows[1].key_node = true;
        snapshot.rows[1].markdown = "先看滚动条".into();
        snapshot.rows[3].tone = TimelineTone::Failed;
        snapshot.rows[3].title = "运行".into();
        snapshot.rows[3].detail = "cargo test".into();

        let markers = timeline_key_markers(&snapshot.rows, &snapshot.layout);
        assert_eq!(
            markers
                .iter()
                .map(|marker| (marker.id.as_str(), marker.name.as_str(), marker.failed))
                .collect::<Vec<_>>(),
            [
                ("event-1", "先看滚动条", false),
                ("event-3", "运行 cargo test", true),
            ]
        );
        assert!((markers[1].ratio - 0.75).abs() < 0.001);

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let document = DocumentId::new(638).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
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
        let failed = view.key_markers["event-3"];
        assert_eq!(
            context
                .world()
                .node_style(failed.stable_id())
                .unwrap()
                .background,
            Some(nana_ui::runtime::SemanticColorRole::Danger)
        );
        context
            .update_component(failed, |_, cx| cx.emit(Activate))
            .unwrap();
        assert!(matches!(
            events.lock().unwrap().last(),
            Some((_, TimelineAction::Jump(id))) if id == "event-3"
        ));
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
        snapshot.rows[0].role = TimelineRole::Reply;
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
            view.activate(&mut context, key);
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

    #[test]
    fn key_node_markers_jump_to_the_row_and_quote_uses_the_row_action() {
        let rows = vec![
            TimelineRow {
                id: "tool".into(),
                role: crate::module::timeline::view::TimelineRole::Step(
                    crate::module::timeline::view::StepKind::Tool,
                ),
                tone: crate::module::timeline::view::TimelineTone::Settled,
                title: String::new(),
                detail: String::new(),
                markdown: "tool output".into(),
                images: Vec::new(),
                expanded: false,
                can_expand: false,
                can_copy: true,
                can_retry: false,
                can_branch: false,
                key_node: false,
            },
            TimelineRow {
                id: "user-1".into(),
                role: crate::module::timeline::view::TimelineRole::User,
                tone: crate::module::timeline::view::TimelineTone::Settled,
                title: String::new(),
                detail: String::new(),
                markdown: "先看滚动条".into(),
                images: Vec::new(),
                expanded: true,
                can_expand: false,
                can_copy: true,
                can_retry: false,
                can_branch: false,
                key_node: true,
            },
        ];
        let layout = VirtualListLayout::new([100.0, 100.0]);
        let markers = timeline_key_markers(&rows, &layout);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].id, "user-1");
        assert_eq!(markers[0].name, "先看滚动条");
        assert_ne!(markers[0].name, "·");
        assert!(rows[1].markdown.starts_with(&markers[0].name));
        assert!((markers[0].ratio - 0.5).abs() < 0.001);
        assert_eq!(
            timeline_event_scroll_offset(&rows, &layout, "user-1", 80.0),
            Some(100.0)
        );
        assert_eq!(
            timeline_event_scroll_offset(&rows, &layout, "tool", 80.0),
            Some(0.0)
        );

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let document = DocumentId::new(634).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let snapshot = TimelineViewSnapshot {
            target: TimelineTarget {
                window_id: WindowId::PRIMARY,
                task_id: Some("task".into()),
            },
            rows,
            layout,
            scroll_offset: 0.0,
            viewport_extent: 80.0,
            can_load_earlier: false,
            search_query: String::new(),
            search_status: String::new(),
            search_open: false,
            search_can_step: false,
            selection: None,
        };
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
        assert!(view.key_markers.contains_key("user-1"));
        assert!(!view.key_markers.contains_key("tool"));
        let marker = view.key_markers["user-1"];
        let name = context
            .read(marker, |button| button.accessible_name.clone())
            .unwrap();
        assert_eq!(name, "先看滚动条");
        context
            .update_component(view.key_markers["user-1"], |_, cx| cx.emit(Activate))
            .unwrap();
        view.activate(&mut context, "quote-user-1");
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .map(|(_, action)| action.clone())
                .collect::<Vec<_>>(),
            vec![
                TimelineAction::Jump("user-1".into()),
                TimelineAction::Quote("user-1".into()),
            ]
        );
    }
}
