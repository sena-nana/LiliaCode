use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lilia_contracts::TaskId;
use nana_ui::runtime::{
    AppContext, Button, Card, DesktopShell, DocumentId, Entity, FrameworkError, IconButton,
    ImageViewer, ImageViewerEvent, List, OverlayHost, ScrollAxes, ScrollView, Stack, Text,
    TextArea, TextChanged,
};
use nana_ui::{ButtonKind, ControlSize, ThemeMode, WindowChrome};
use nana_ui_platform::WindowId;

use crate::runtime_layout::{composer_interrupt_button, composer_send_button, window_control};
use crate::runtime_shell::{
    bind_activate, composer_is_focused, emit, ComposerGeneration, ShellIntent, ShellTimelineRow,
};

const CONVERSATION_STATUS_DOCUMENT: u64 = 10_001;

#[cfg(debug_assertions)]
#[path = "runtime_windows_debug.rs"]
mod debug;

type IntentSink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct ConversationStatusRow {
    pub task_id: TaskId,
    pub title: String,
    pub project_name: String,
    pub status: String,
    pub phase: String,
    pub can_stop: bool,
}

#[derive(Debug, Clone)]
pub struct ConversationStatusSnapshot {
    pub theme: ThemeMode,
    pub pinned: bool,
    pub error: Option<String>,
    pub entries: Vec<ConversationStatusRow>,
}

#[derive(Debug, Clone)]
pub struct TaskPopupSnapshot {
    pub window_id: WindowId,
    pub theme: ThemeMode,
    pub title: String,
    pub heading: String,
    pub suggestions: crate::runtime_empty_suggestions::EmptySuggestionsSnapshot,
    pub error: Option<String>,
    pub timeline: Vec<ShellTimelineRow>,
    pub composer: String,
    pub composer_atom_spans: Vec<lilia_feature_composer::ContentAtomSpan>,
    pub composer_task_id: Option<String>,
    pub composer_revision: u64,
    pub composer_height: f32,
    pub conversation_controls: crate::runtime_conversation::ConversationControls,
    pub todo_panel: crate::todo_panel::TodoPanelSnapshot,
    pub markdown_preview: Option<crate::runtime_shell::ShellMarkdownPreview>,
    pub timeline_can_load_earlier: bool,
    pub composer_disabled: bool,
    pub can_send: bool,
    pub can_interrupt: bool,
    pub pending: Option<crate::runtime_shell::ShellPending>,
}

pub struct ConversationStatusHandles {
    sink: IntentSink,
    shell: Entity<DesktopShell>,
    title: Entity<Text>,
    error: Entity<Text>,
    list: Entity<List>,
    rows: HashMap<String, Entity<Stack>>,
    pin: Entity<Button>,
}

pub struct TaskPopupHandles {
    window_id: WindowId,
    sink: IntentSink,
    shell: Entity<DesktopShell>,
    image_viewer: Option<Entity<ImageViewer>>,
    image_viewer_source: Option<String>,
    heading: Entity<Text>,
    heading_slot: Entity<Stack>,
    heading_actions: Entity<Stack>,
    empty_suggestions: crate::runtime_empty_suggestions::EmptySuggestions,
    conversation_body: Entity<Stack>,
    error: Entity<Text>,
    timeline_scroll: Entity<ScrollView>,
    timeline_items: HashMap<String, (Entity<Stack>, crate::runtime_conversation::TimelineContent)>,
    load_earlier: Option<Entity<Button>>,
    conversation_controls: crate::runtime_conversation::ConversationControlsHandles,
    page: Entity<Stack>,
    pending: crate::runtime_pending::PendingPanel,
    composer: Entity<TextArea>,
    composer_dock: Entity<Card>,
    pub(crate) todo_panel: crate::todo_panel::TodoPanel,
    composer_actions: Entity<Stack>,
    composer_generation: ComposerGeneration,
    send: Entity<IconButton>,
    interrupt: Entity<IconButton>,
}

fn action_button(label: &str, kind: ButtonKind) -> Button {
    Button::new(label).kind(kind).size(ControlSize::Small)
}

pub fn mount_conversation_status(
    snapshot: &ConversationStatusSnapshot,
    sink: IntentSink,
) -> Result<(nana_ui::runtime::RuntimeDocument, ConversationStatusHandles), FrameworkError> {
    let document_id = DocumentId::new(CONVERSATION_STATUS_DOCUMENT).expect("status document");
    let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
    let context = document.context_mut();
    let _ = context.set_theme(snapshot.theme);

    let title = context.create_detached_component(document_id, Text::new("会话状态"))?;
    let error = context.create_detached_component(
        document_id,
        Text::new(snapshot.error.clone().unwrap_or_default()),
    )?;
    let list = context.create_detached_component(document_id, List::new())?;
    let actions = context.create_detached_component(document_id, Stack::row(8.0))?;
    let pin = context.create_detached_component(
        document_id,
        action_button(
            if snapshot.pinned {
                "取消置顶"
            } else {
                "置顶"
            },
            ButtonKind::Subtle,
        ),
    )?;
    let new_chat = context
        .create_detached_component(document_id, action_button("新会话", ButtonKind::Primary))?;
    let close = context
        .create_detached_component(document_id, action_button("关闭", ButtonKind::Subtle))?;
    bind_activate(
        context,
        pin,
        Arc::clone(&sink),
        ShellIntent::ToggleConversationStatusPin,
    )?;
    bind_activate(
        context,
        new_chat,
        Arc::clone(&sink),
        ShellIntent::OpenConversationStatusNewChat,
    )?;
    bind_activate(
        context,
        close,
        Arc::clone(&sink),
        ShellIntent::CloseConversationStatus,
    )?;
    context.append_child(actions, pin)?;
    context.append_child(actions, new_chat)?;
    context.append_child(actions, close)?;

    let page =
        context.create_detached_component(document_id, Stack::fill_column(10.0).padding(16.0))?;
    context.append_child(page, title)?;
    context.append_child(page, error)?;
    context.append_child(page, list)?;
    context.append_child(page, actions)?;

    let title_trailing = context.create_detached_component(document_id, Stack::row(6.0))?;
    if WindowChrome::platform_default().uses_custom_controls() {
        let close_win = context.create_detached_component(
            document_id,
            window_control(nana_ui::Icon::Close, "关闭", ButtonKind::Text),
        )?;
        context.append_child(title_trailing, close_win)?;
        bind_activate(
            context,
            close_win,
            Arc::clone(&sink),
            ShellIntent::CloseConversationStatus,
        )?;
    }

    let shell = context.create_component(
        document_id,
        DesktopShell::from_model(nana_ui::WorkspaceModel::new())
            .title("会话状态")
            .title_center(title.stable_id())
            .title_trailing(title_trailing.stable_id())
            .primary(page.stable_id()),
    )?;
    context.assemble_desktop_shell(shell)?;

    let mut handles = ConversationStatusHandles {
        sink,
        shell,
        title,
        error,
        list,
        rows: HashMap::new(),
        pin,
    };
    handles.sync_rows(context, document_id, snapshot)?;
    Ok((document, handles))
}

impl ConversationStatusHandles {
    pub fn sync(
        &mut self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        snapshot: &ConversationStatusSnapshot,
    ) -> Result<(), FrameworkError> {
        let document_id = document.document();
        let context = document.context_mut();
        let _ = context.set_theme(snapshot.theme);
        context.update_component(self.title, |title, _| {
            *title = Text::new("会话状态");
        })?;
        context.update_component(self.error, |error, _| {
            *error = Text::new(snapshot.error.clone().unwrap_or_default());
        })?;
        context.update_component(self.pin, |button, _| {
            *button = action_button(
                if snapshot.pinned {
                    "取消置顶"
                } else {
                    "置顶"
                },
                ButtonKind::Subtle,
            );
        })?;
        self.sync_rows(context, document_id, snapshot)?;
        context.assemble_desktop_shell(self.shell)?;
        Ok(())
    }

    fn sync_rows(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &ConversationStatusSnapshot,
    ) -> Result<(), FrameworkError> {
        let mut keep = HashSet::new();
        let mut order = Vec::new();
        for entry in &snapshot.entries {
            let key = entry.task_id.as_str().to_owned();
            keep.insert(key.clone());
            let label = format!(
                "{} · {} · {} · {}",
                entry.title, entry.project_name, entry.status, entry.phase
            );
            let row = if let Some(row) = self.rows.get(&key).copied() {
                row
            } else {
                let row =
                    context.create_detached_component(document_id, Stack::fill_column(4.0))?;
                let text =
                    context.create_detached_component(document_id, Text::new(label.clone()))?;
                let open = context.create_detached_component(
                    document_id,
                    action_button("打开", ButtonKind::Subtle),
                )?;
                bind_activate(
                    context,
                    open,
                    Arc::clone(&self.sink),
                    ShellIntent::OpenStatusTask(entry.task_id.clone()),
                )?;
                context.append_child(row, text)?;
                context.append_child(row, open)?;
                if entry.can_stop {
                    let stop = context.create_detached_component(
                        document_id,
                        action_button("停止", ButtonKind::Danger),
                    )?;
                    bind_activate(
                        context,
                        stop,
                        Arc::clone(&self.sink),
                        ShellIntent::StopStatusTask(entry.task_id.clone()),
                    )?;
                    context.append_child(row, stop)?;
                }
                self.rows.insert(key, row);
                row
            };
            let children = context
                .world()
                .node(row.stable_id())
                .map(|node| node.children.clone())
                .unwrap_or_default();
            if let Some(first) = children.first() {
                let _ =
                    context.update_component(Entity::<Text>::from_stable_id(*first), |text, _| {
                        *text = Text::new(label);
                    });
            }
            order.push(row.stable_id());
        }
        let stale: Vec<_> = self
            .rows
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(row) = self.rows.remove(&key) {
                let _ = context.remove_view(row);
            }
        }
        context
            .reconcile_children(self.list.stable_id(), &order)
            .map(|_| ())
    }
}

pub fn mount_task_popup(
    snapshot: &TaskPopupSnapshot,
    sink: IntentSink,
) -> Result<(nana_ui::runtime::RuntimeDocument, TaskPopupHandles), FrameworkError> {
    let document_id =
        DocumentId::new(10_000u64.saturating_add(snapshot.window_id.0)).expect("popup document");
    let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
    let context = document.context_mut();
    let _ = context.set_theme(snapshot.theme);

    let (heading_slot, heading, heading_actions) = crate::runtime_layout::mount_empty_headline(
        context,
        document_id,
        snapshot.heading.clone(),
    )?;
    let error = context.create_detached_component(
        document_id,
        Text::new(snapshot.error.clone().unwrap_or_default()),
    )?;
    let timeline_scroll = context.create_detached_component(
        document_id,
        ScrollView::new(ScrollAxes::Vertical).style(
            crate::runtime_conversation::timeline_container()
                .grow(1.0)
                .shrink(1.0)
                .min_height(nana_ui::runtime::LengthSpec::Px(0.0))
                .node_style(),
        ),
    )?;
    let composer = context.create_detached_component(
        document_id,
        crate::runtime_layout::flatten_composer_textarea(
            TextArea::new(snapshot.composer.clone())
                .atom_spans(crate::runtime_shell::composer_atoms(
                    &snapshot.composer_atom_spans,
                ))
                .height(snapshot.composer_height),
        ),
    )?;
    let composer_sink = Arc::clone(&sink);
    let window_id = snapshot.window_id;
    context.on(composer, move |_, event: &TextChanged, _| {
        emit(
            &composer_sink,
            ShellIntent::TaskPopupComposerChanged {
                window_id,
                value: event.value.clone(),
            },
        );
    })?;
    let send =
        context.create_detached_component(document_id, composer_send_button(snapshot.can_send))?;
    let interrupt = context.create_detached_component(
        document_id,
        composer_interrupt_button(snapshot.can_interrupt),
    )?;
    bind_activate(
        context,
        send,
        Arc::clone(&sink),
        ShellIntent::TaskPopupSubmit(window_id),
    )?;
    bind_activate(
        context,
        interrupt,
        Arc::clone(&sink),
        ShellIntent::TaskPopupInterrupt(window_id),
    )?;
    if snapshot.composer_disabled {
        context.update_component(composer, |editor, _| {
            editor.disabled = true;
        })?;
    }

    let actions = context.create_detached_component(document_id, Stack::row(8.0))?;
    if snapshot.can_interrupt {
        context.append_child(actions, interrupt)?;
    } else {
        context.append_child(actions, send)?;
    }
    let pending =
        crate::runtime_pending::PendingPanel::mount(context, document_id, window_id, sink.clone())?;
    let conversation_controls = crate::runtime_conversation::ConversationControlsHandles::mount(
        context,
        document_id,
        window_id,
        sink.clone(),
        true,
    )?;
    conversation_controls.bind_composer_keyboard(context, composer)?;
    let todo_panel = crate::todo_panel::TodoPanel::mount(
        context,
        document_id,
        Arc::clone(&sink),
        snapshot.window_id,
    )?;
    let composer_dock =
        context.create_detached_component(document_id, crate::runtime_layout::composer_card())?;
    let toolbar = context.create_detached_component(
        document_id,
        Stack::bar(8.0)
            .justify(nana_ui::runtime::JustifySpec::SpaceBetween)
            .wrap(true),
    )?;
    context.append_child(toolbar, conversation_controls.toolbar)?;
    context.append_child(toolbar, actions)?;
    context.append_child(composer_dock, conversation_controls.root)?;
    context.append_child(composer_dock, composer)?;
    context.append_child(composer_dock, toolbar)?;
    let page = context.create_detached_component(
        document_id,
        Stack::fill_column(12.0)
            .max_width(860.0)
            .with_layout(|layout| {
                layout.margin_left = Some(nana_ui::runtime::LengthSpec::Auto);
                layout.margin_right = Some(nana_ui::runtime::LengthSpec::Auto);
            }),
    )?;
    let conversation_body =
        context.create_detached_component(document_id, Stack::fill_column(12.0))?;
    context.append_child(conversation_body, heading_slot)?;
    context.append_child(conversation_body, error)?;
    context.append_child(conversation_body, timeline_scroll)?;
    context.append_child(page, conversation_body)?;
    context.append_child(page, composer_dock)?;
    let conversation = context
        .create_detached_component(document_id, crate::runtime_shell::conversation_root())?;
    context.append_child(conversation, page)?;

    let title =
        context.create_detached_component(document_id, Text::new(snapshot.title.clone()))?;
    let shell = context.create_component(
        document_id,
        DesktopShell::from_model(nana_ui::WorkspaceModel::new())
            .title(snapshot.title.clone())
            .title_center(title.stable_id())
            .primary(conversation.stable_id()),
    )?;
    context.assemble_desktop_shell(shell)?;

    let mut handles = TaskPopupHandles {
        window_id: snapshot.window_id,
        sink,
        shell,
        image_viewer: None,
        image_viewer_source: None,
        heading,
        heading_slot,
        heading_actions,
        empty_suggestions: Default::default(),
        conversation_body,
        error,
        timeline_scroll,
        timeline_items: HashMap::new(),
        load_earlier: None,
        conversation_controls,
        page,
        pending,
        composer,
        composer_dock,
        todo_panel,
        composer_actions: actions,
        composer_generation: ComposerGeneration::default(),
        send,
        interrupt,
    };
    handles.conversation_controls.sync_composer_keyboard(
        &snapshot.conversation_controls,
        &snapshot.composer,
        snapshot.can_send,
    );
    handles
        .conversation_controls
        .sync(context, document_id, &snapshot.conversation_controls)?;
    handles.sync_timeline(context, document_id, snapshot)?;
    handles.sync_suggestions(context, document_id, snapshot)?;
    crate::runtime_layout::sync_conversation_body(
        context,
        handles.conversation_body.stable_id(),
        (!snapshot.heading.trim().is_empty() && snapshot.timeline.is_empty())
            .then(|| handles.heading_slot.stable_id()),
        snapshot
            .error
            .as_ref()
            .filter(|error| !error.is_empty())
            .map(|_| handles.error.stable_id()),
        handles.timeline_scroll.stable_id(),
        None,
    )?;
    handles.sync_pending(context, document_id, snapshot)?;
    Ok((document, handles))
}

impl TaskPopupHandles {
    pub(crate) fn composer_node(&self) -> nana_ui::runtime::StableNodeId {
        self.composer.stable_id()
    }

    pub fn sync(
        &mut self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        snapshot: &TaskPopupSnapshot,
    ) -> Result<(), FrameworkError> {
        let document_id = document.document();
        let context = document.context_mut();
        let _ = context.set_theme(snapshot.theme);
        context.update_component(self.heading, |text, _| {
            *text = crate::runtime_layout::conversation_headline(snapshot.heading.clone());
        })?;
        context.update_component(self.heading_slot, |slot, _| {
            *slot = crate::runtime_layout::headline_slot(!snapshot.heading.trim().is_empty());
        })?;
        context.update_component(self.error, |text, _| {
            *text = Text::new(snapshot.error.clone().unwrap_or_default());
        })?;
        let composer_generation = ComposerGeneration::new(
            snapshot.composer_task_id.clone(),
            snapshot.composer_revision,
            None,
        );
        if self.composer_generation.task_changed(&composer_generation) {
            context.clear_text_history(self.composer.stable_id())?;
        }
        let write_composer = !composer_is_focused(context, self.composer)
            || self.composer_generation != composer_generation;
        context.update_component(self.composer, |editor, _| {
            if write_composer && editor.state.value != snapshot.composer {
                editor.state.replace_value(snapshot.composer.clone());
            }
            if editor.state.value == snapshot.composer {
                editor.atom_spans =
                    crate::runtime_shell::composer_atoms(&snapshot.composer_atom_spans);
            }
            editor.disabled = snapshot.composer_disabled;
            Arc::make_mut(&mut editor.style.layout).height =
                Some(nana_ui::runtime::LengthSpec::Px(snapshot.composer_height));
        })?;
        self.composer_generation = composer_generation;
        context.update_component(self.send, |button, _| {
            *button = composer_send_button(snapshot.can_send);
        })?;
        context.update_component(self.interrupt, |button, _| {
            *button = composer_interrupt_button(snapshot.can_interrupt);
        })?;
        context.reconcile_children(
            self.composer_actions.stable_id(),
            &[if snapshot.can_interrupt {
                self.interrupt.stable_id()
            } else {
                self.send.stable_id()
            }],
        )?;
        self.sync_timeline(context, document_id, snapshot)?;
        self.sync_suggestions(context, document_id, snapshot)?;
        crate::runtime_layout::sync_conversation_body(
            context,
            self.conversation_body.stable_id(),
            (!snapshot.heading.trim().is_empty() && snapshot.timeline.is_empty())
                .then(|| self.heading_slot.stable_id()),
            snapshot
                .error
                .as_ref()
                .filter(|error| !error.is_empty())
                .map(|_| self.error.stable_id()),
            self.timeline_scroll.stable_id(),
            None,
        )?;
        self.conversation_controls.sync_composer_keyboard(
            &snapshot.conversation_controls,
            &snapshot.composer,
            snapshot.can_send,
        );
        self.conversation_controls
            .sync(context, document_id, &snapshot.conversation_controls)?;
        self.sync_pending(context, document_id, snapshot)?;
        self.sync_image_preview(context, document_id, snapshot)?;
        context.assemble_desktop_shell(self.shell)?;
        Ok(())
    }

    fn sync_suggestions(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TaskPopupSnapshot,
    ) -> Result<(), FrameworkError> {
        self.empty_suggestions.sync(
            context,
            document_id,
            self.heading_actions,
            &snapshot.suggestions,
            snapshot.timeline.is_empty() && snapshot.composer.trim().is_empty(),
            self.window_id,
            self.sink.clone(),
        )
    }

    fn sync_image_preview(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TaskPopupSnapshot,
    ) -> Result<(), FrameworkError> {
        let host = context.read(self.shell, |shell| {
            shell.overlay.map(Entity::<OverlayHost>::from_stable_id)
        })?;
        let Some(host) = host else {
            return Ok(());
        };
        let mut overlays = Vec::new();
        if let Some(preview) = &snapshot.markdown_preview {
            let source_changed =
                self.image_viewer_source.as_deref() != Some(preview.source.as_str());
            self.image_viewer_source = Some(preview.source.clone());
            let viewer = if let Some(viewer) = self.image_viewer {
                context.update_component(viewer, |viewer, _| {
                    crate::runtime_shell::sync_markdown_image_viewer(
                        viewer,
                        preview,
                        source_changed,
                    );
                })?;
                viewer
            } else {
                let viewer = context.create_detached_component(
                    document_id,
                    crate::runtime_shell::markdown_image_viewer(preview),
                )?;
                let sink = self.sink.clone();
                let window_id = self.window_id;
                context.on(viewer, move |_, event: &ImageViewerEvent, _| {
                    if matches!(event, ImageViewerEvent::Close | ImageViewerEvent::Outside) {
                        emit(
                            &sink,
                            crate::runtime_conversation::intent(
                                window_id,
                                crate::runtime_conversation::ConversationAction::CloseImage,
                            ),
                        );
                    }
                })?;
                context.append_child(host, viewer)?;
                self.image_viewer = Some(viewer);
                viewer
            };
            overlays.push(viewer.stable_id());
            context.activate_overlay(host, viewer)?;
        } else if let Some(viewer) = self.image_viewer.take() {
            let _ = context.remove_view(viewer);
        }
        context.update_component(self.shell, |shell, _| {
            shell.overlays = overlays;
        })?;
        Ok(())
    }

    fn sync_pending(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TaskPopupSnapshot,
    ) -> Result<(), FrameworkError> {
        self.pending
            .sync(context, document_id, snapshot.pending.as_ref())?;
        self.todo_panel
            .sync(context, document_id, &snapshot.todo_panel)?;
        let mut order = vec![self.conversation_body.stable_id()];
        if snapshot.todo_panel.visible {
            order.push(self.todo_panel.root.stable_id());
        }
        if snapshot.pending.is_some() {
            order.push(self.pending.pending_panel.stable_id());
        } else {
            order.push(self.composer_dock.stable_id());
        }
        context
            .reconcile_children(self.page.stable_id(), &order)
            .map(|_| ())
    }

    fn sync_timeline(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &TaskPopupSnapshot,
    ) -> Result<(), FrameworkError> {
        let mut keep = HashSet::new();
        let mut order = Vec::new();
        if snapshot.timeline_can_load_earlier {
            let button = if let Some(button) = self.load_earlier {
                button
            } else {
                let button = context.create_detached_component(
                    document_id,
                    action_button("加载更早", ButtonKind::Subtle),
                )?;
                bind_activate(
                    context,
                    button,
                    self.sink.clone(),
                    crate::runtime_conversation::intent(
                        self.window_id,
                        crate::runtime_conversation::ConversationAction::LoadEarlier,
                    ),
                )?;
                self.load_earlier = Some(button);
                button
            };
            order.push(button.stable_id());
        }
        for item in &snapshot.timeline {
            let mut item = item.clone();
            if snapshot.can_interrupt || snapshot.pending.is_some() {
                item.can_branch = false;
                item.can_apply = false;
            }
            keep.insert(item.id.clone());
            if !self.timeline_items.contains_key(&item.id) {
                let root = context.create_detached_component(
                    document_id,
                    crate::runtime_conversation::timeline_stack(&item),
                )?;
                let content =
                    crate::runtime_conversation::TimelineContent::mount(context, document_id)?;
                self.timeline_items.insert(item.id.clone(), (root, content));
            }
            let (root, content) = self
                .timeline_items
                .get_mut(&item.id)
                .expect("timeline item");
            content.sync(
                context,
                document_id,
                *root,
                self.window_id,
                &item,
                &self.sink,
            )?;
            order.push(root.stable_id());
        }
        let stale = self
            .timeline_items
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            if let Some((root, content)) = self.timeline_items.remove(&key) {
                context.remove_view(root)?;
                content.dispose_owned(context)?;
            }
        }
        context
            .reconcile_children(self.timeline_scroll.stable_id(), &order)
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_retained_adapter_dispatches_normal_input_and_rejects_disabled_send() {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = events.clone();
        let mut snapshot = popup_snapshot();
        let (mut document, mut handles) = mount_task_popup(
            &snapshot,
            Arc::new(move |intent| captured.lock().unwrap().push(intent)),
        )
        .unwrap();
        let doc = DocumentId::new(10_000 + snapshot.window_id.0).unwrap();
        document
            .context_mut()
            .layout_document(doc, nana_ui::runtime::LayoutViewport::new(430.0, 760.0))
            .unwrap();
        assert!(handles
            .debug_retained_command(
                &mut document,
                &crate::agent_debug::DebugCommand::UiInput {
                    target_id: "lilia.ui.popup.composer".into(),
                    text: "真实弹窗输入\n第二行".into(),
                }
            )
            .unwrap()
            .is_some());
        assert!(events.lock().unwrap().iter().any(|event| matches!(event, ShellIntent::TaskPopupComposerChanged { window_id, value } if *window_id == snapshot.window_id && value == "真实弹窗输入\n第二行")));
        assert!(handles
            .debug_retained_command(
                &mut document,
                &crate::agent_debug::DebugCommand::UiClick {
                    target_id: "lilia.ui.popup.send".into()
                }
            )
            .unwrap()
            .is_some());
        assert!(events.lock().unwrap().iter().any(|event| matches!(event, ShellIntent::TaskPopupSubmit(window) if *window == snapshot.window_id)));
        snapshot.can_send = false;
        handles.sync(&mut document, &snapshot).unwrap();
        document
            .context_mut()
            .layout_document(doc, nana_ui::runtime::LayoutViewport::new(430.0, 760.0))
            .unwrap();
        assert!(handles
            .debug_retained_command(
                &mut document,
                &crate::agent_debug::DebugCommand::UiClick {
                    target_id: "lilia.ui.popup.send".into()
                }
            )
            .unwrap()
            .is_none());
    }

    #[test]
    fn popup_history_does_not_cross_same_value_task_switch() {
        let mut snapshot = popup_snapshot();
        snapshot.composer.clear();
        let (mut document, mut handles) = mount_task_popup(&snapshot, Arc::new(|_| {})).unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let doc = DocumentId::new(10_000 + snapshot.window_id.0).unwrap();
        let editor = handles.composer.stable_id();
        document.context_mut().focus_node(doc, editor).unwrap();
        let mut adapter = nana_ui::RuntimeInputAdapter::default();
        let mut key = nana_ui_platform::InputEvent::Keyboard {
            pressed: true,
            key: "a".into(),
            code: "KeyA".into(),
            text: Some("a".into()),
            repeat: false,
            modifiers: Default::default(),
        };
        adapter.dispatch(document.context_mut(), doc, &key).unwrap();
        snapshot.composer = "a".into();
        snapshot.composer_task_id = Some("another-task".into());
        handles.sync(&mut document, &snapshot).unwrap();
        key = nana_ui_platform::InputEvent::Keyboard {
            pressed: true,
            key: "z".into(),
            code: "KeyZ".into(),
            text: None,
            repeat: false,
            modifiers: nana_ui_platform::InputModifiers {
                control: true,
                ..Default::default()
            },
        };
        adapter.dispatch(document.context_mut(), doc, &key).unwrap();
        assert_eq!(document.context().world().text(editor), Some("a"));
    }

    fn popup_snapshot() -> TaskPopupSnapshot {
        TaskPopupSnapshot {
            window_id: WindowId(46),
            theme: ThemeMode::Dark,
            title: "测试对话".into(),
            heading: "今天想做什么？".into(),
            suggestions: Default::default(),
            error: None,
            timeline: Vec::new(),
            composer: "检查项目".into(),
            composer_atom_spans: Vec::new(),
            composer_task_id: Some("task-46".into()),
            composer_revision: 1,
            composer_height: 32.0,
            conversation_controls: crate::runtime_conversation::ConversationControls {
                model_label: "自动 · gpt-5.4-mini".into(),
                models: vec![(String::new(), "自动".into())],
                reasoning: "medium".into(),
                permission: "ask".into(),
                worktree: Some(("existing".into(), "feature-long-worktree-name".into())),
                context_label: Some("上下文 75% · 压缩".into()),
                can_compact: true,
                can_optimize: true,
                ..Default::default()
            },
            todo_panel: Default::default(),
            markdown_preview: None,
            timeline_can_load_earlier: false,
            composer_disabled: false,
            can_send: true,
            can_interrupt: false,
            pending: None,
        }
    }

    #[test]
    fn popup_empty_suggestions_live_under_the_headline_and_target_the_popup() {
        let mut snapshot = popup_snapshot();
        snapshot.composer.clear();
        snapshot
            .suggestions
            .items
            .push(crate::runtime_shell::ShellSuggestionRow {
                id: "popup-suggestion".into(),
                label: "继续当前项目".into(),
                source: None,
                prompt: "检查当前项目".into(),
            });
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let events = received.clone();
        let (mut document, mut handles) = mount_task_popup(
            &snapshot,
            Arc::new(move |intent| events.lock().unwrap().push(intent)),
        )
        .unwrap();
        let document_id = document.document();
        document
            .context_mut()
            .layout_document(
                document_id,
                nana_ui::runtime::LayoutViewport::new(430.0, 760.0),
            )
            .unwrap();
        let button_id = document
            .context()
            .world()
            .node(handles.heading_actions.stable_id())
            .unwrap()
            .children[0];
        assert!(document.context().world().is_mounted(button_id));
        let bounds = document.context().world().layout_box(button_id).unwrap();
        assert!((bounds.height - 24.0).abs() < 0.5);
        assert!(bounds.x >= 0.0 && bounds.x + bounds.width <= 430.5);
        document
            .context_mut()
            .update_component(
                Entity::<nana_ui::runtime::ListItem>::from_stable_id(button_id),
                |_, cx| cx.emit(nana_ui::runtime::Activate),
            )
            .unwrap();
        assert!(received.lock().unwrap().iter().any(|event| matches!(event,
            ShellIntent::ApplySuggestion { window_id, item_id }
                if *window_id == snapshot.window_id && item_id == "popup-suggestion")));

        snapshot.composer = "用户新输入".into();
        handles.sync(&mut document, &snapshot).unwrap();
        document
            .context_mut()
            .layout_document(
                document_id,
                nana_ui::runtime::LayoutViewport::new(430.0, 760.0),
            )
            .unwrap();
        assert!(!document.context().world().is_mounted(button_id));
        let actions = document
            .context()
            .world()
            .layout_box(handles.heading_actions.stable_id())
            .unwrap();
        assert!(actions.height >= 24.0);
    }

    #[test]
    fn popup_timeline_removal_releases_parked_optional_content() {
        let mut snapshot = popup_snapshot();
        let (mut document, mut handles) = mount_task_popup(&snapshot, Arc::new(|_| {})).unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let baseline = document.context().world().len();
        for pass in 0..3 {
            snapshot.timeline = vec![crate::runtime_shell::ShellTimelineRow {
                selected_text: None,
                attachments: Vec::new(),
                images: Vec::new(),
                id: format!("popup-event-{pass}"),
                title: String::new(),
                role: "assistant".into(),
                status: "completed".into(),
                markdown: "A popup message".into(),
                expanded: false,
                can_expand: false,
                can_copy: true,
                can_retry: false,
                can_branch: false,
                can_apply: false,
            }];
            handles.sync(&mut document, &snapshot).unwrap();
            let root = handles.timeline_items.values().next().unwrap().0;
            snapshot.timeline.clear();
            handles.sync(&mut document, &snapshot).unwrap();
            assert!(document.context().world().node(root.stable_id()).is_none());
            assert_eq!(
                document.context().world().len(),
                baseline,
                "popup removal must release attached and parked row nodes"
            );
        }
    }

    #[test]
    fn popup_toolbar_controls_fit_the_real_430_pixel_window() {
        let mut snapshot = popup_snapshot();
        let (mut document, mut handles) = mount_task_popup(&snapshot, Arc::new(|_| {})).unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let document_id = document.document();
        let all_text = document.context().world().document_order(document_id);
        document.context_mut().resolve_styles(&all_text).unwrap();
        document
            .context_mut()
            .shape_text(&all_text, &mut nana_ui::NanaTextShaper::default())
            .unwrap();
        document
            .context_mut()
            .layout_document(
                document_id,
                nana_ui::runtime::LayoutViewport::new(430.0, 760.0),
            )
            .unwrap();
        let ids = document.context().world().document_order(document_id);
        document.context_mut().resolve_styles(&ids).unwrap();
        document
            .context_mut()
            .shape_text(
                &[handles.heading.stable_id()],
                &mut nana_ui::NanaTextShaper::default(),
            )
            .unwrap();
        document
            .context_mut()
            .layout_document(
                document_id,
                nana_ui::runtime::LayoutViewport::new(430.0, 760.0),
            )
            .unwrap();
        let world = document.context().world();
        let body = world
            .layout_box(handles.conversation_body.stable_id())
            .unwrap();
        let headline = world.layout_box(handles.heading.stable_id()).unwrap();
        assert!(
            (headline.y + headline.height / 2.0
                - (body.y + body.height / 2.0 - 760.0 * 0.08 - 19.0))
                .abs()
                <= 1.0
        );
        assert!(!world.is_mounted(handles.timeline_scroll.stable_id()));
        let dock = world.layout_box(handles.composer_dock.stable_id()).unwrap();
        let targets = handles
            .conversation_controls
            .debug_targets()
            .into_iter()
            .collect::<HashMap<_, _>>();
        let mut bounds = Vec::new();
        for name in [
            "tools",
            "permission",
            "worktree",
            "model",
            "reasoning",
            "optimize",
            "compact",
        ] {
            let rect = world
                .layout_box(targets[name])
                .unwrap_or_else(|| panic!("{name} must be mounted"));
            assert!(
                rect.width > 0.0 && rect.height > 0.0,
                "{name} must have usable bounds"
            );
            assert!(
                rect.x >= dock.x && rect.x + rect.width <= dock.x + dock.width + 0.5,
                "{name} must fit the composer: control={rect:?}, dock={dock:?}"
            );
            assert!(rect.x + rect.width <= 430.5, "{name} exceeds window width");
            bounds.push((name, rect));
        }
        bounds.push(("send", world.layout_box(handles.send.stable_id()).unwrap()));
        for (index, (name, rect)) in bounds.iter().enumerate() {
            assert!(
                rect.x >= dock.x && rect.x + rect.width <= dock.x + dock.width + 0.5,
                "{name} exceeds dock"
            );
            for (other_name, other) in bounds.iter().skip(index + 1) {
                let overlaps = rect.x < other.x + other.width - 0.5
                    && other.x < rect.x + rect.width - 0.5
                    && rect.y < other.y + other.height - 0.5
                    && other.y < rect.y + rect.height - 0.5;
                assert!(!overlaps, "{name} overlaps {other_name}");
            }
        }
        snapshot.pending = Some(crate::runtime_shell::ShellPending {
            request_id: "popup-approval".into(),
            kind: crate::runtime_shell::ShellPendingKind::PlanApproval,
            title: "确认计划".into(),
            prompt: "检查后继续".into(),
            draft: "修改计划备注".into(),
            options: Vec::new(),
            tool: None,
            ask: None,
            mcp: None,
        });
        handles.sync(&mut document, &snapshot).unwrap();
        assert!(document
            .context()
            .world()
            .node(handles.composer_dock.stable_id())
            .unwrap()
            .parent
            .is_none());
        assert_eq!(
            document
                .context()
                .read(handles.composer, |view| view.state.value.clone())
                .unwrap(),
            "检查项目"
        );
        snapshot.pending = None;
        handles.sync(&mut document, &snapshot).unwrap();
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.composer_dock.stable_id())
                .unwrap()
                .parent,
            Some(handles.page.stable_id())
        );
        assert_eq!(
            document
                .context()
                .read(handles.composer, |view| view.state.value.clone())
                .unwrap(),
            "检查项目"
        );
    }
}
