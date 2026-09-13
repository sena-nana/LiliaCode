use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lilia_contracts::TaskId;
use nana_ui::runtime::{
    AppContext, Button, DesktopShell, DocumentId, Entity, FrameworkError, List, Stack, Text,
};
use nana_ui::{ButtonKind, ControlSize, ThemeMode, WindowChrome};
use nana_ui_platform::WindowId;

use crate::module::composer::view::ComposerViewSnapshot;
use crate::module::task::view::{TaskView, TaskViewInput};
use crate::runtime_layout::{reconcile_children, window_control};
use crate::runtime_shell::{ShellIntent, bind_activate};

const CONVERSATION_STATUS_DOCUMENT: u64 = 10_001;

type IntentSink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct ConversationStatusRow {
    pub task_id: TaskId,
    pub title: String,
    pub project_name: String,
    pub status: String,
    pub phase: String,
    pub can_stop: bool,
    pub stop_turn_id: Option<String>,
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
    pub window_inline_size: f32,
    pub panes: Vec<crate::runtime_shell::ShellPaneRow>,
    pub pane_layout: crate::runtime_shell::ShellPaneLayout,
    pub window_id: WindowId,
    pub theme: ThemeMode,
    pub title: String,
    pub heading: String,
    pub error: Option<String>,
    pub timeline: crate::module::timeline::view::TimelineViewSnapshot,
    pub composer: ComposerViewSnapshot,
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
    workspace: crate::workspace_view::WorkspaceView,
    shell: Entity<DesktopShell>,
    page: Entity<Stack>,
    task_view: TaskView,
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
            let key = format!(
                "{}:{:?}:{}",
                entry.task_id.as_str(),
                entry.stop_turn_id,
                entry.can_stop
            );
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
                if let Some(turn_id) = entry.stop_turn_id.as_ref().filter(|_| entry.can_stop) {
                    let stop = context.create_detached_component(
                        document_id,
                        action_button("停止", ButtonKind::Danger),
                    )?;
                    bind_activate(
                        context,
                        stop,
                        Arc::clone(&self.sink),
                        ShellIntent::StopStatusTask(crate::runtime_shell::TurnStopTarget {
                            task_id: entry.task_id.clone(),
                            turn_id: turn_id.clone(),
                        }),
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
        reconcile_children(context, self.list.stable_id(), &order)
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

    let task_view = TaskView::mount(
        context,
        document_id,
        snapshot.task_input(),
        Arc::clone(&sink),
    )?;
    let page = context
        .create_detached_component(document_id, Stack::fill_column(0.0).padding_xy(24.0, 20.0))?;
    context.append_child(page, task_view.conversation_column)?;

    let title =
        context.create_detached_component(document_id, Text::new(snapshot.title.clone()))?;
    let workspace = crate::workspace_view::WorkspaceView::mount(
        context,
        document_id,
        snapshot.window_id,
        Arc::clone(&sink),
    )?;
    let shell = context.create_component(
        document_id,
        DesktopShell::from_model(nana_ui::WorkspaceModel::new())
            .title(snapshot.title.clone())
            .title_center(title.stable_id())
            .primary(workspace.root.stable_id()),
    )?;
    context.assemble_desktop_shell(shell)?;

    let mut handles = TaskPopupHandles {
        window_id: snapshot.window_id,
        workspace,
        shell,
        page,
        task_view,
    };
    handles.workspace.sync(
        context,
        document_id,
        &snapshot.panes,
        &snapshot.pane_layout,
        handles.page.stable_id(),
        snapshot.window_inline_size,
    )?;
    context.update_component(handles.shell, |shell, _| {
        shell.bottom = handles.workspace.bottom_slot();
    })?;
    context.assemble_desktop_shell(handles.shell)?;
    Ok((document, handles))
}

impl TaskPopupHandles {
    pub(crate) fn reveal_workspace_resources(&mut self) {
        self.workspace.reveal_resources();
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_browser_views(&self) -> Vec<crate::browser_workbench::BrowserView> {
        self.workspace.debug_browser_views()
    }

    pub(crate) fn matches_document_search(
        &self,
        target: &crate::runtime_shell::ShellPaneTarget,
        editor: nana_ui::runtime::StableNodeId,
        feedback: nana_ui::runtime::StableNodeId,
    ) -> bool {
        self.workspace
            .matches_document_search(target, editor, feedback)
    }

    pub fn sync(
        &mut self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        snapshot: &TaskPopupSnapshot,
    ) -> Result<(), FrameworkError> {
        let document_id = document.document();
        let context = document.context_mut();
        let _ = context.set_theme(snapshot.theme);
        self.task_view
            .sync(context, document_id, snapshot.task_input())?;
        self.workspace.sync(
            context,
            document_id,
            &snapshot.panes,
            &snapshot.pane_layout,
            self.page.stable_id(),
            snapshot.window_inline_size,
        )?;
        context.update_component(self.shell, |shell, _| {
            shell.bottom = self.workspace.bottom_slot();
        })?;
        context.assemble_desktop_shell(self.shell)?;
        Ok(())
    }
}

impl TaskPopupSnapshot {
    fn task_input(&self) -> TaskViewInput<'_> {
        TaskViewInput {
            heading: if self.timeline.rows.is_empty() {
                &self.heading
            } else {
                ""
            },
            error: self.error.as_deref(),
            timeline: &self.timeline,
            composer: &self.composer,
            pending: self.pending.as_ref(),
        }
    }
}

#[cfg(debug_assertions)]
#[path = "runtime_windows_debug.rs"]
mod debug;
