#[cfg(test)]
use crate::module::composer::presentation::{
    COMPOSER_PERMISSION_OPTIONS, COMPOSER_WORKTREE_OPTIONS, ComposerSlashItem,
};
#[cfg(test)]
use crate::module::composer::view::ComposerBinding;
pub(crate) use crate::module::composer::view::{ComposerInputAction, ComposerTarget};
use crate::module::settings::view::SettingsSnapshot;
#[cfg(test)]
use nana_ui::{AppearanceSettings, SettingsModel, SettingsState};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use lilia_contracts::TaskId;
use lilia_feature_workspace::PROJECTS_WORKSPACE_ITEM_KIND;
#[cfg(test)]
use nana_ui::runtime::VirtualListLayout;
use nana_ui::runtime::{
    Activate, AppContext, Breadcrumb, BreadcrumbItem, BreadcrumbTone, Button, CodeEditing,
    CommandPalette, ConfirmDialog, ConfirmIntent, ConfirmSlots, ContextMenu, ContextMenuEvent,
    ContextMenuItem, DesktopShell, DocumentId, EmptyState, Entity, FrameworkError,
    HighlightRequest, IconButton, ImageViewer, ImageViewerContent, ImageViewerEvent,
    LengthSpec, List, ListItem, NodeStyle, OverlayChanged, OverlayClosing,
    OverlayHost, PaneChrome, PaneChromeAction, PaneChromeActionKind, ReorderItem, ReorderList,
    ReorderListEvent, ScrollAxes, ScrollView, SecondaryPress, SemanticColorRole, SettingsPage, SidebarFooter,
    SidebarFooterButton, SidebarFrame, SidebarRow, SidebarRowIcon, SidebarRowState, SidebarSection,
    SidebarSectionState, SplitPane, StableNodeId, Stack, TabOption, Tabs, TabsEvent, Text,
    TextArea, TextChanged, TextDiagnosticSeverity, TextDiagnosticSpan, TextFindScope, TextInput,
    TextSearchOptions, TreeDropPosition, TreeView, TreeViewEvent, View, sidebar_row_tool_button,
    sidebar_section_tool_button, sidebar_top_bar_tool_button,
};
use nana_ui::{
    AppearanceEvent, ButtonKind, CommandPaletteEvent, CommandPaletteItem, ControlSize, Icon,
    SettingsTabId, SplitAxis, SplitPaneModel, ThemeMode, UI_METRICS, WindowChrome,
    WindowChromeAction, WindowChromeEvent, WorkspaceModel,
};

use crate::application::{
    ARCHITECTURE_WORKSPACE_ITEM_KIND, AUTOMATION_WORKSPACE_ITEM_KIND, DOCUMENT_WORKSPACE_ITEM_KIND,
    Diagnostic, DiagnosticSeverity, MEMORY_WORKSPACE_ITEM_KIND, PROJECT_FILES_WORKSPACE_ITEM_KIND,
    ROADMAP_WORKSPACE_ITEM_KIND, SETTINGS_WORKSPACE_ITEM_KIND, TASK_WORKSPACE_ITEM_KIND,
    TERMINAL_WORKSPACE_ITEM_KIND,
};
use crate::navigation::WindowRoute;
use crate::runtime_compat::{HostedUiCommand, HostedWindowId};
use crate::runtime_layout::{
    inspector_header_bar, pill_button, reconcile_children, sidebar_icon_button, window_control,
};
use crate::target_ids;

#[cfg(debug_assertions)]
mod debug;
pub(crate) mod quota;

const PRIMARY_DOCUMENT: u64 = 1;
const SESSIONS_EMPTY_TEXT: &str = "还没有会话";
const INBOX_EMPTY_TEXT: &str = "没有未绑定的对话";
const PROJECTS_EMPTY_TEXT: &str = "暂无项目";
#[cfg(test)]
const COMPOSER_MIN_HEIGHT: f32 = UI_METRICS.control_height;
#[cfg(test)]
use crate::module::task::view::CHAT_CONTENT_MAX_WIDTH;
const CONVERSATION_WORKSPACE_SPLIT_SIZE: f32 = 420.0;
const CONVERSATION_WORKSPACE_SPLIT_MIN: f32 = 280.0;
const TITLE_BREADCRUMB_WIDTH: f32 = 440.0;
#[cfg(test)]
const TIMELINE_DEFAULT_VIEWPORT_EXTENT: f32 = 720.0;

fn sidebar_row_icon(kind: ShellSidebarKind, id: &str) -> Icon {
    if id == "projects-empty" {
        return Icon::Folder;
    }
    match kind {
        ShellSidebarKind::Header
        | ShellSidebarKind::DropHint
        | ShellSidebarKind::Project
        | ShellSidebarKind::Archived
        | ShellSidebarKind::SearchProject => Icon::Folder,
        ShellSidebarKind::Task | ShellSidebarKind::SearchTask | ShellSidebarKind::Empty => {
            Icon::MessageSquarePlus
        }
        ShellSidebarKind::Running => Icon::Activity,
        ShellSidebarKind::Inbox => Icon::Package,
        ShellSidebarKind::Reveal => Icon::More,
    }
}

fn footer_nav_icon(settings: bool) -> Icon {
    if settings {
        Icon::Settings
    } else {
        Icon::Nodes
    }
}

fn workspace_kind_icon(kind: &str) -> Icon {
    match kind {
        TASK_WORKSPACE_ITEM_KIND => Icon::MessageSquarePlus,
        DOCUMENT_WORKSPACE_ITEM_KIND => Icon::File,
        TERMINAL_WORKSPACE_ITEM_KIND => Icon::Activity,
        ROADMAP_WORKSPACE_ITEM_KIND => Icon::Chart,
        MEMORY_WORKSPACE_ITEM_KIND => Icon::Atom,
        ARCHITECTURE_WORKSPACE_ITEM_KIND | AUTOMATION_WORKSPACE_ITEM_KIND => Icon::Nodes,
        PROJECT_FILES_WORKSPACE_ITEM_KIND => Icon::Folder,
        SETTINGS_WORKSPACE_ITEM_KIND => Icon::Settings,
        PROJECTS_WORKSPACE_ITEM_KIND => Icon::Workspace,
        _ => Icon::File,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellTaskRow {
    pub id: TaskId,
    pub title: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellSidebarKind {
    Header,
    DropHint,
    Running,
    Project,
    Task,
    Inbox,
    Reveal,
    Empty,
    Archived,
    SearchProject,
    SearchTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarDropPosition {
    Before,
    Inside,
    After,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellSidebarRow {
    pub id: String,
    pub label: String,
    pub kind: ShellSidebarKind,
    pub selected: bool,
    pub ancestor: bool,
    pub depth: u16,
    pub expanded: Option<bool>,
    pub can_stop: bool,
    pub stop_turn_id: Option<String>,
    pub can_menu: bool,
    pub can_draft: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellConfirmKind {
    ArchiveConversations,
    RemoveProject,
    Update,
    RevokeRemote,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellConfirm {
    pub kind: ShellConfirmKind,
    pub title: String,
    pub message: String,
    pub confirm_label: String,
    pub cancel_label: String,
    pub danger: bool,
    pub busy: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellTodoRow {
    pub id: String,
    pub label: String,
    pub done: bool,
}

pub use crate::module::composer::pending_view::{
    AskUserPending as ShellAskUserPending, McpField as ShellMcpField,
    McpFieldOption as ShellMcpFieldOption, McpPending as ShellMcpPending,
    PendingKind as ShellPendingKind, PendingOption as ShellPendingOption,
    PendingSnapshot as ShellPending, ToolConsentPending as ShellToolConsentPending, TurnStopTarget,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellProjectPage {
    Overview,
    Sessions,
    Clone,
    Roadmap,
    Memory,
    Architecture,
    Settings,
    Files,
}

impl From<crate::application::ProjectWorkspaceSurface> for ShellProjectPage {
    fn from(surface: crate::application::ProjectWorkspaceSurface) -> Self {
        use crate::application::ProjectWorkspaceSurface;
        match surface {
            ProjectWorkspaceSurface::Roadmap => Self::Roadmap,
            ProjectWorkspaceSurface::Memory => Self::Memory,
            ProjectWorkspaceSurface::Architecture => Self::Architecture,
            ProjectWorkspaceSurface::Files => Self::Files,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellNavItem {
    pub id: String,
    pub label: String,
    pub settings: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellMenuItem {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellProjectCard {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellCodingFile {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellCodingHit {
    pub id: String,
    pub label: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellCodingSnapshot {
    pub query: String,
    pub mode_label: String,
    pub scope_label: String,
    pub busy: bool,
    pub git: String,
    pub files: Vec<ShellCodingFile>,
    pub hits: Vec<ShellCodingHit>,
    pub terminals: Vec<ShellActionRow>,
    pub tasks: Vec<ShellActionRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellActionRow {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellPaneItem {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub selected: bool,
    pub closable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellPaneRow {
    pub id: String,
    pub active: bool,
    pub items: Vec<ShellPaneItem>,
    pub document: Option<ShellDocumentSnapshot>,
    pub terminal: Option<ShellTerminalSnapshot>,
    pub browser: Option<crate::browser_workbench::BrowserPresentation>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShellPaneLayout {
    Leaf(String),
    Split {
        horizontal: bool,
        ratio: f32,
        first: Box<ShellPaneLayout>,
        second: Box<ShellPaneLayout>,
    },
}

impl Default for ShellPaneLayout {
    fn default() -> Self {
        Self::Leaf(String::new())
    }
}

impl ShellPaneLayout {
    fn split_keys(&self, keys: &mut HashSet<String>) {
        if let Self::Split { first, second, .. } = self {
            keys.insert(format!("{}:{}", first.first_leaf(), second.first_leaf()));
            first.split_keys(keys);
            second.split_keys(keys);
        }
    }

    pub(crate) fn filtered(&self, keep: &impl Fn(&str) -> bool) -> Option<Self> {
        match self {
            Self::Leaf(id) => keep(id).then(|| self.clone()),
            Self::Split {
                horizontal,
                ratio,
                first,
                second,
            } => match (first.filtered(keep), second.filtered(keep)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    horizontal: *horizontal,
                    ratio: *ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (first, second) => first.or(second),
            },
        }
    }

    pub(crate) fn first_leaf(&self) -> &str {
        match self {
            Self::Leaf(id) => id,
            Self::Split { first, .. } => first.first_leaf(),
        }
    }

    pub(crate) fn leaf_ids(&self) -> Vec<&str> {
        match self {
            Self::Leaf(id) => vec![id.as_str()],
            Self::Split { first, second, .. } => {
                let mut ids = first.leaf_ids();
                ids.extend(second.leaf_ids());
                ids
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellPaneTarget {
    pub window_id: HostedWindowId,
    pub pane_id: String,
    pub item_id: String,
}

impl ShellPaneTarget {
    fn primary(pane_id: &str, item_id: &str) -> Self {
        Self {
            window_id: HostedWindowId::PRIMARY,
            pane_id: pane_id.to_owned(),
            item_id: item_id.to_owned(),
        }
    }
}

#[derive(Clone, Default)]
struct PaneInputBindings {
    conflicted: bool,
    document: Option<(ShellPaneTarget, u64, String)>,
    terminal: Option<(ShellPaneTarget, String)>,
}

impl PaneInputBindings {
    fn projected(
        pane_id: &str,
        document: Option<&ShellDocumentSnapshot>,
        terminal: Option<&ShellTerminalSnapshot>,
    ) -> Self {
        Self::projected_in(HostedWindowId::PRIMARY, pane_id, document, terminal)
    }
    fn projected_in(
        window_id: HostedWindowId,
        pane_id: &str,
        document: Option<&ShellDocumentSnapshot>,
        terminal: Option<&ShellTerminalSnapshot>,
    ) -> Self {
        Self {
            conflicted: document.is_some_and(|document| document.conflicted),
            document: document.map(|document| {
                (
                    ShellPaneTarget {
                        window_id,
                        pane_id: pane_id.to_owned(),
                        item_id: document.item_id.clone(),
                    },
                    document.revision,
                    document.text.clone(),
                )
            }),
            terminal: terminal.map(|terminal| {
                (
                    ShellPaneTarget {
                        window_id,
                        pane_id: pane_id.to_owned(),
                        item_id: terminal.item_id.clone(),
                    },
                    terminal.session_id.clone(),
                )
            }),
        }
    }

    fn edit(&mut self, value: String) -> Option<ShellIntent> {
        let (target, revision, previous) = self.document.as_mut()?;
        if *previous == value {
            return None;
        }
        let intent = ShellIntent::DocumentChanged {
            target: target.clone(),
            revision: *revision,
            value: value.clone(),
        };
        if !self.conflicted {
            *revision = revision.checked_add(1)?;
        }
        *previous = value;
        Some(intent)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellDocumentSnapshot {
    pub item_id: String,
    pub revision: u64,
    pub conflicted: bool,
    pub title: String,
    pub text: String,
    pub language: String,
    pub status: String,
    pub read_only: bool,
    pub dirty: bool,
    pub diagnostics: Vec<ShellDiagnosticRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellDiagnosticRow {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub start_offset: usize,
    pub end_offset: usize,
}

impl From<&Diagnostic> for ShellDiagnosticRow {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self {
            severity: diagnostic.severity,
            message: diagnostic.message.clone(),
            start_offset: diagnostic.start_offset,
            end_offset: diagnostic.end_offset,
        }
    }
}

impl ShellDiagnosticRow {
    fn severity_label(&self) -> &'static str {
        match self.severity {
            DiagnosticSeverity::Error => "错误",
            DiagnosticSeverity::Warning => "警告",
            DiagnosticSeverity::Information => "信息",
            DiagnosticSeverity::Hint => "提示",
        }
    }

    fn editor_span(&self, text: &str) -> Option<TextDiagnosticSpan> {
        if self.start_offset > self.end_offset
            || self.end_offset > text.len()
            || !text.is_char_boundary(self.start_offset)
            || !text.is_char_boundary(self.end_offset)
        {
            return None;
        }
        let severity = match self.severity {
            DiagnosticSeverity::Error => TextDiagnosticSeverity::Error,
            DiagnosticSeverity::Warning => TextDiagnosticSeverity::Warning,
            DiagnosticSeverity::Information => TextDiagnosticSeverity::Information,
            DiagnosticSeverity::Hint => TextDiagnosticSeverity::Hint,
        };
        Some(
            TextDiagnosticSpan::new(
                self.start_offset,
                self.end_offset - self.start_offset,
                severity,
            )
            .with_message(self.message.clone()),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellMarkdownPreview {
    pub title: String,
    pub metadata: String,
    pub intrinsic_size: Option<(u32, u32)>,
    pub texture_slot: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellTerminalSnapshot {
    pub item_id: String,
    pub session_id: String,
    pub output: String,
    pub notice: Option<String>,
    pub screen: nana_ui::runtime::TerminalScreen,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellFilesSnapshot {
    pub tree: TreeView,
    pub preview: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PrimaryShellSnapshot {
    pub composer: crate::module::composer::view::ComposerViewSnapshot,
    pub theme: ThemeMode,
    pub title_parent: String,
    pub title_context: String,
    pub heading: String,
    pub error: Option<String>,
    pub navigation: WindowRoute,
    pub sidebar_collapsed: bool,
    pub sidebar_search_open: bool,
    pub sidebar_search_query: String,
    pub provider_badge: String,
    pub provider_badge_icon: Icon,
    pub nav_items: Vec<ShellNavItem>,
    pub sidebar_rows: Vec<ShellSidebarRow>,
    pub sidebar_menu: Vec<ShellMenuItem>,
    pub sidebar_menu_anchor: Option<(f32, f32)>,
    pub sidebar_menu_owner: Option<String>,
    pub add_project_menu_open: bool,
    pub workspace: WorkspaceModel,
    pub tasks: Vec<ShellTaskRow>,
    pub timeline: crate::module::timeline::view::TimelineViewSnapshot,
    pub clone_repository: String,
    pub clone_parent: String,
    pub roadmap: crate::module::roadmap::view::RoadmapViewSnapshot,
    pub command_palette_open: bool,
    pub command_palette_query: String,
    pub command_palette_selected: usize,
    pub command_palette_items: Vec<CommandPaletteItem>,
    pub settings: SettingsSnapshot,
    pub document: Option<ShellDocumentSnapshot>,
    pub files: Option<ShellFilesSnapshot>,
    pub terminal: Option<ShellTerminalSnapshot>,
    pub browser: Option<crate::browser_workbench::BrowserPresentation>,
    pub markdown_preview: Option<ShellMarkdownPreview>,
    pub inspector_title: String,
    pub inspector_body: String,
    pub inspector_todos: Vec<ShellTodoRow>,
    pub todo_panel: crate::todo_panel::TodoPanelSnapshot,
    pub confirm: Option<ShellConfirm>,
    pub pending: Option<ShellPending>,
    pub project_page: Option<ShellProjectPage>,
    pub project_page_title: String,
    pub project_page_body: String,
    pub project_cards: Vec<ShellProjectCard>,
    pub session_search: String,
    pub session_page: usize,
    pub session_page_count: usize,
    pub session_cards: Vec<ShellTaskRow>,
    pub memory: crate::module::memory::view::MemoryViewSnapshot,
    pub architecture: crate::module::architecture::view::ArchitectureViewSnapshot,
    pub inspector_kind: String,
    pub coding: Option<ShellCodingSnapshot>,
    pub pane_can_move_window: bool,
    pub pane_can_move_next: bool,
    pub titlebar_menu_open: bool,
    pub titlebar_has_task: bool,
    pub titlebar_can_split: bool,
    pub titlebar_can_close: bool,
    pub automation: crate::module::automation::view::AutomationViewSnapshot,
    pub panes: Vec<ShellPaneRow>,
    pub pane_layout: ShellPaneLayout,
}

#[derive(Debug, Clone)]
pub enum ShellIntent {
    AddressedTimeline {
        target: crate::module::timeline::view::TimelineTarget,
        action: crate::module::timeline::view::TimelineAction,
    },
    WorkspacePane {
        window_id: HostedWindowId,
        pane_id: String,
        intent: Box<ShellIntent>,
    },
    AddressedComposer {
        target: ComposerTarget,
        action: ComposerInputAction,
    },
    OverlayPresenceChanged,
    Todo {
        window_id: HostedWindowId,
        action: crate::todo_panel::TodoAction,
    },
    SessionPageChanged(isize),
    ToggleSidebar,
    NewConversation,
    SelectTask(TaskId),
    ToggleSidebarSearch,
    SidebarSearchChanged(String),
    ToggleSidebarInbox,
    RevealSidebarProject(String),
    RevealSidebarInbox,
    OpenProjectsOverview,
    OpenAddProjectMenu,
    OpenProjectMenu {
        id: String,
        anchor: Option<(f32, f32)>,
    },
    OpenTaskMenu {
        id: String,
        anchor: Option<(f32, f32)>,
    },
    /// 行体右键（列表冒泡解析）弹同款菜单，锚点为光标点。
    OpenRowMenu {
        id: String,
        anchor: (f32, f32),
    },
    ReorderSidebar {
        source: String,
        before: Option<String>,
    },
    SidebarTreeDrop {
        source: String,
        target: String,
        position: SidebarDropPosition,
    },
    OpenProjectDraft(String),
    RestoreProject(String),
    SelectProject(String),
    StopSidebarTask(TurnStopTarget),
    SidebarMenuAction(String),
    OpenAutomations,
    CloseAutomations,
    ConfirmDestructive,
    CancelDestructive,
    SelectPaneTab {
        pane_id: String,
        item_id: Option<String>,
    },
    ReorderPaneTab {
        pane_id: String,
        item_id: String,
        before: Option<String>,
    },
    ClosePaneTab {
        item_id: String,
    },
    TransferPaneTab {
        source_strip: String,
        target_strip: String,
        item_id: String,
        before: Option<String>,
    },
    CloseMarkdownPreview,
    MarkdownImageViewerInteraction,

    StartGitHubBinding,
    CancelGitHubBinding,
    BeginShortcutCapture,
    SaveShortcut,
    ClearShortcut,
    CodingQueryChanged(String),
    SearchCoding,
    RefreshCoding,
    CycleCodingMode,
    ToggleCodingScope,
    OpenCodingHit(String),
    OpenCodingWorkspace,
    OpenCodingTerminal,
    SelectRoadmapMilestone(String),
    RefreshArchitecture,
    RollbackArchitecture,
    ArchitectureGraph(nana_ui::GraphCanvasEvent),
    RespondApproval {
        request_id: String,
        approved: bool,
    },
    RespondTitle {
        request_id: String,
        accepted: bool,
    },
    RespondArchitecture {
        request_id: String,
        approved: bool,
    },
    RespondPlan {
        request_id: String,
        action: String,
    },
    RespondToolConsent {
        request_id: String,
        approved: bool,
    },
    ToolConsentDraftChanged {
        request_id: String,
        command: String,
        message: String,
    },
    AskUserPending {
        request_id: String,
        action: String,
        value: String,
    },
    PendingDraftChanged {
        request_id: String,
        value: String,
    },
    SelectPendingOption {
        request_id: String,
        option_id: String,
    },
    RespondMcp {
        request_id: String,
        action: String,
    },
    McpFieldChanged {
        request_id: String,
        field_key: String,
        value: String,
    },
    McpRawJsonChanged {
        request_id: String,
        value: String,
    },
    McpToggleOption {
        request_id: String,
        field_key: String,
        value: String,
        multi: bool,
    },
    McpToggleBoolean {
        request_id: String,
        field_key: String,
    },
    OpenMarkdownLink(String),
    CloneRepositoryChanged(String),
    PickCloneParent,
    StartClone,
    CancelClone,
    MilestoneTitleChanged(String),
    MilestoneDescriptionChanged(String),
    MilestoneDueDateChanged(String),
    CycleMilestoneStatus,
    MoveMilestone(isize),
    DeleteMilestone,
    CreateMilestone,
    SaveMilestone,
    ToggleMilestoneTask(String),
    SelectMemory(String),
    MemoryTitleChanged(String),
    MemoryBodyChanged(String),
    MemoryTagsChanged(String),
    ToggleMemoryScope,
    NewMemory,
    SaveMemory,
    DeleteMemory,
    MemoryAction(crate::module::memory::MemoryMessage),
    MovePaneToWindow,
    MovePaneToNext,
    OpenSettings,
    CloseSettings,
    InterruptTurn(Option<TurnStopTarget>),
    WindowChrome(WindowChromeEvent),
    ToggleCommandPalette,
    CommandPalette(CommandPaletteEvent),
    SelectSettingsTab(SettingsTabId),
    Appearance(AppearanceEvent),
    SetSidebarDisplayMode(String),
    ProjectNameChanged(String),
    SaveProjectSettings,
    PickProjectWorkspace,
    SelectProvider(String),
    RefreshProvider,
    ToggleAgent(String),
    RefreshQuota,
    RefreshExtensions,
    ExtensionsSearchChanged(String),
    ExtensionsCommand(crate::shell::ExtensionsMessage),
    OpenPath(String),
    ToggleRemoteHost,
    ToggleRemoteKeepAwake,
    RemoteNameChanged(String),
    SaveRemoteName,
    RefreshRemote,
    StartRemotePairing,
    CancelRemotePairing,
    RevokeRemoteDevice(String),
    CopyPairingUri,
    SetProjectWorktreeMode(String),
    PickProjectWorktreeParent,
    ProjectWorktreeInstructionsChanged(String),
    ToggleWorktreeCleanup,
    CheckForUpdate,
    PickDataImportSource,
    ExecuteDataImport,
    ResetDataImport,
    DocumentChanged {
        target: ShellPaneTarget,
        revision: u64,
        value: String,
    },
    SearchDocument {
        target: ShellPaneTarget,
        editor: StableNodeId,
        feedback: StableNodeId,
        query: String,
        replacement: String,
        action: DocumentSearchAction,
    },
    SaveDocument(ShellPaneTarget),
    DiscardDocument(ShellPaneTarget),
    TerminalWrite {
        target: ShellPaneTarget,
        session_id: String,
        bytes: Vec<u8>,
    },
    OpenBrowser {
        window_id: HostedWindowId,
        task_id: String,
    },
    Browser {
        target: ShellPaneTarget,
        action: crate::browser_workbench::BrowserAction,
    },
    TerminalResize {
        target: ShellPaneTarget,
        session_id: String,
        columns: u16,
        rows: u16,
    },
    TerminalInterrupt {
        target: ShellPaneTarget,
        session_id: String,
    },
    ToggleProjectFile(String),
    OpenProjectFile(String),
    RefreshProjectFiles,
    ProviderSecretChanged(String),
    SaveProviderCredential,
    RevokeProviderCredential {
        credential_id: String,
        revision: u64,
    },
    ProviderModelChanged(String),
    ProviderOpenAiEndpointChanged(String),
    ProviderAnthropicEndpointChanged(String),
    SaveProviderRuntimeSettings,
    ResetProviderRuntimeSettings,
    AgentNameChanged(String),
    AgentDescriptionChanged(String),
    AgentInstructionChanged(String),
    NewCustomAgent,
    EditCustomAgent(String),
    SaveCustomAgent,
    CancelCustomAgentEdit,
    ToggleCustomAgent(String),
    DeleteCustomAgent(String),
    CycleQuotaDays,
    CycleQuotaBackend,
    SkillIdChanged(String),
    SkillDescriptionChanged(String),
    CreateSkill,
    ToggleSkill(String),
    NewMcpServer,
    EditMcpServer(String),
    McpServerIdChanged(String),
    CycleMcpTransport,
    McpLocationChanged(String),
    McpArgsChanged(String),
    ToggleMcpEditorEnabled,
    SaveMcpServer,
    CancelMcpEditor,
    ToggleMcpServer(String),
    ToggleTitlebarMenu,
    BackToTaskList,
    OpenTaskPopup,
    AskTaskPopup,
    ToggleTaskInspector,
    CloseInspectorDock,
    SplitWorkspaceHorizontal,
    SplitWorkspaceVertical,
    ResizeWorkspaceSplit {
        first_pane_id: String,
        second_pane_id: String,
        ratio: f32,
    },
    CloseCurrentWorkspaceItem,
    OpenConversationStatus,
    CloseConversationStatus,
    ToggleConversationStatusPin,
    OpenConversationStatusNewChat,
    OpenStatusTask(TaskId),
    StopStatusTask(TurnStopTarget),
    FocusWorkspacePane(String),
    Automation {
        target: crate::module::automation::view::AutomationTarget,
        action: crate::module::automation::view::AutomationAction,
    },
    SelectAutomation(String),
    CreateAutomation,
    RefreshAutomations,
    AddressedPending {
        target: crate::module::composer::pending_view::PendingTarget,
        action: crate::module::composer::pending_view::PendingAction,
    },
}

pub(crate) type IntentSink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

/// 上一次真正同步进 `RuntimeDocument` 的输入。`sync` 是快照的纯函数，
/// 所以输入不变时重跑 reconcile 只会产生同一棵树，可以整段跳过。
#[derive(Default)]
struct SyncedInputs {
    sidebar_rows: Vec<ShellSidebarRow>,
    sidebar_tasks: Vec<ShellTaskRow>,
    sidebar_search_open: bool,
    workspace: Option<WorkspaceInputs>,
    inspector: Option<InspectorInputs>,
}

/// 工作区页、面板标签条与诊断面板共同的输入。
#[derive(PartialEq)]
struct WorkspaceInputs {
    panes: Vec<ShellPaneRow>,
    pane_layout: ShellPaneLayout,
    document: Option<ShellDocumentSnapshot>,
    terminal: Option<ShellTerminalSnapshot>,
    browser: Option<crate::browser_workbench::BrowserPresentation>,
    files: Option<ShellFilesSnapshot>,
}

#[derive(PartialEq)]
struct InspectorInputs {
    kind: String,
    todos: Vec<ShellTodoRow>,
    body: String,
    architecture_resource: Option<String>,
    coding: Option<ShellCodingSnapshot>,
    todo_panel: crate::todo_panel::TodoPanelSnapshot,
}

pub struct ShellHandles {
    automation_view: crate::module::automation::view::AutomationView,
    settings_view: crate::module::settings::view::SettingsView,
    project_fields: crate::form_view::ProductFields,
    task_view: crate::module::task::view::TaskView,
    sink: IntentSink,
    shell: Entity<DesktopShell>,
    overlay_host: Option<Entity<OverlayHost>>,
    pending_overlay_dismissals: Arc<Mutex<Vec<StableNodeId>>>,
    palette: Option<Entity<CommandPalette>>,
    more_menu: Option<Entity<ContextMenu>>,
    titlebar_menu: Option<Entity<ContextMenu>>,
    sidebar_toggle: Entity<IconButton>,
    footer_more: Entity<SidebarFooterButton>,
    pane_bar: Entity<Stack>,
    pane_buttons: HashMap<String, Entity<Button>>,
    title_breadcrumb: Entity<Breadcrumb>,
    title_leading: Entity<Stack>,
    title_trailing: Entity<Stack>,
    conversation_sidebar: Entity<SidebarFrame>,
    sidebar_top: Entity<Stack>,
    new_conversation: Entity<SidebarRow>,
    search_toggle: Entity<IconButton>,
    search_input: Entity<TextArea>,
    search_close: Entity<IconButton>,
    sidebar_scroll: Entity<ScrollView>,
    conversation_section: Entity<SidebarSection>,
    task_body: Entity<List>,
    task_reorder: Entity<ReorderList>,
    project_section: Entity<SidebarSection>,
    #[cfg(test)]
    project_header: Entity<ListItem>,
    project_body: Entity<List>,
    project_reorder: Entity<ReorderList>,
    add_project_menu: Entity<IconButton>,
    inbox_section: Entity<SidebarSection>,
    inbox_body: Entity<List>,
    inbox_reorder: Entity<ReorderList>,
    task_rows: HashMap<String, Entity<SidebarRow>>,
    row_kinds: HashMap<String, ShellSidebarKind>,
    row_tools: HashMap<String, Entity<Stack>>,
    row_tool_buttons: HashMap<String, RowToolButton>,
    footer_nav: HashMap<String, Entity<SidebarFooterButton>>,
    provider_badge: Entity<SidebarFooterButton>,
    conversation: Entity<Stack>,
    synced: SyncedInputs,
    shell_assembled: bool,
    extra_buttons: HashMap<String, Entity<Button>>,
    project_page: Entity<ScrollView>,
    project_page_title: Entity<Text>,
    project_page_body: Entity<Text>,
    project_cards: HashMap<String, Entity<Button>>,
    memory_view: Option<crate::module::memory::view::MemoryView>,
    roadmap_view: Option<crate::module::roadmap::view::RoadmapView>,
    architecture_view: Option<crate::module::architecture::view::ArchitectureView>,
    workspace_page: Entity<Stack>,
    terminal_page: Entity<Stack>,
    workbench_bottom: Entity<Stack>,
    compact_workbench: crate::workspace_view::CompactWorkbench,
    conversation_workspace: Entity<SplitPane>,
    pane_chrome: Entity<PaneChrome>,
    pane_tabs: Entity<Tabs>,
    workspace_content: Entity<Stack>,
    workspace_heading: Entity<Text>,
    workspace_status: Entity<Text>,
    workspace_editor: Entity<TextArea>,
    workspace_search: EditorSearchView,
    workspace_bindings: Arc<Mutex<PaneInputBindings>>,
    workspace_log: Entity<nana_ui::runtime::TerminalView>,
    workspace_terminal_session: Option<String>,
    workspace_browser: crate::browser_workbench::BrowserView,
    diagnostics_panel: Entity<Stack>,
    diagnostic_rows: HashMap<String, Entity<Text>>,
    image_viewer: Option<Entity<ImageViewer>>,
    workspace_actions: Entity<Stack>,
    workspace_buttons: HashMap<String, Entity<Button>>,
    workspace_tree: Entity<TreeView>,
    inspector: Entity<Stack>,
    inspector_header: Entity<Stack>,
    inspector_close: Entity<IconButton>,
    inspector_heading: Entity<Text>,
    inspector_body: Entity<Text>,
    inspector_todos: Entity<Stack>,
    inspector_todo_rows: HashMap<String, Entity<Text>>,
    todo_panel: crate::todo_panel::TodoPanel,
    coding_panel: Entity<Stack>,
    coding_query: Entity<TextArea>,
    coding_rows: HashMap<String, Entity<Button>>,
    pane_move_window: Entity<IconButton>,
    pane_move_next: Entity<IconButton>,
    extra_workspace_panes: HashMap<String, WorkspacePaneView>,
    workspace_splits: HashMap<String, Entity<SplitPane>>,
    workspace_split_handles: HashMap<String, Entity<Stack>>,
    iab_empty: Entity<EmptyState>,
    confirm: Option<Entity<ConfirmDialog>>,
    confirm_cancel: Option<Entity<Button>>,
    confirm_commit: Option<Entity<Button>>,
    focus_targets: HashMap<String, StableNodeId>,
}

#[derive(Clone)]
pub(crate) struct WorkspacePaneView {
    bindings: Arc<Mutex<PaneInputBindings>>,
    chrome_actions: [Entity<IconButton>; 4],
    save: Entity<Button>,
    discard: Entity<Button>,
    interrupt: Entity<Button>,
    chrome: Entity<PaneChrome>,
    tabs: Entity<Tabs>,
    content: Entity<Stack>,
    heading: Entity<Text>,
    status: Entity<Text>,
    editor: Entity<TextArea>,
    search: EditorSearchView,
    log: Entity<nana_ui::runtime::TerminalView>,
    terminal_session: Option<String>,
    pub(crate) browser: crate::browser_workbench::BrowserView,
    tree: Entity<TreeView>,
    actions: Entity<Stack>,
}

impl WorkspacePaneView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        window: HostedWindowId,
        pane: &str,
        sink: &IntentSink,
    ) -> Result<Self, FrameworkError> {
        mount_workspace_pane_view_in(context, document, window, pane, sink)
    }
    pub(crate) fn root(&self) -> StableNodeId {
        self.chrome.stable_id()
    }
    pub(crate) fn matches_document_search(
        &self,
        target: &ShellPaneTarget,
        editor: StableNodeId,
        feedback: StableNodeId,
    ) -> bool {
        self.editor.stable_id() == editor
            && self.search.feedback.stable_id() == feedback
            && self.search.draft.lock().unwrap().expanded
            && self
                .bindings
                .lock()
                .unwrap()
                .document
                .as_ref()
                .is_some_and(|(current, _, _)| current == target)
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        window_id: HostedWindowId,
        pane: &ShellPaneRow,
        files: Option<&ShellFilesSnapshot>,
        conversation: Option<StableNodeId>,
    ) -> Result<(), FrameworkError> {
        let (selected, options) = pane_tab_options_for(pane);
        context.update_component(self.tabs, |tabs, _| {
            *tabs = Tabs::new(selected)
                .options(options)
                .strip_id(workspace_strip_id(window_id, &pane.id))
                .fill(true);
        })?;
        let kind = pane
            .items
            .iter()
            .find(|item| item.selected)
            .map(|item| item.kind.as_str());
        let document = pane.document.as_ref();
        let terminal = pane.terminal.as_ref();
        *self.bindings.lock().unwrap() =
            PaneInputBindings::projected_in(window_id, &pane.id, document, terminal);
        self.search
            .sync(context, document.map(|document| document.item_id.as_str()))?;
        self.search
            .set_read_only(context, document.is_some_and(|document| document.read_only))?;
        let (title, status) = match kind {
            Some("document-editor") => document
                .map(|document| (document.title.clone(), document.status.clone()))
                .unwrap_or_default(),
            Some("terminal") => (
                "终端".to_owned(),
                terminal
                    .and_then(|terminal| terminal.notice.clone())
                    .unwrap_or_default(),
            ),
            Some("project-files") => (
                "项目文件".to_owned(),
                files
                    .and_then(|files| files.preview.clone())
                    .unwrap_or_default(),
            ),
            _ => Default::default(),
        };
        let show_heading = !title.is_empty() && kind == Some("project-files");
        let show_status = !status.is_empty();
        context.update_component(self.heading, |text, _| {
            *text = Text::new(title);
        })?;
        context.update_component(self.status, |text, _| {
            *text = Text::new(status);
        })?;
        if let Some(document) = document {
            context.update_component(self.editor, |editor_view, _| {
                if editor_view.state.value != document.text {
                    editor_view.state.replace_value(document.text.clone());
                }
                editor_view.read_only = document.read_only;
                apply_workspace_editor_chrome(editor_view, Some(document.language.as_str()));
                editor_view.diagnostics = document
                    .diagnostics
                    .iter()
                    .filter_map(|diagnostic| diagnostic.editor_span(&document.text))
                    .collect::<Vec<_>>()
                    .into();
            })?;
        }
        if let Some(terminal) = terminal {
            if self.terminal_session.as_deref() != Some(&terminal.session_id) {
                context.update_component(self.log, |grid, _| {
                    *grid = nana_ui::runtime::TerminalView::new(terminal.screen.clone());
                })?;
                self.terminal_session = Some(terminal.session_id.clone());
            }
            context.update_component(self.log, |log, _| {
                log.read_only = !terminal.running;
            })?;
            context.sync_terminal_screen(self.log, terminal.screen.clone())?;
        }
        if let Some(browser) = &pane.browser {
            self.browser.sync(
                context,
                ShellPaneTarget {
                    window_id,
                    pane_id: pane.id.clone(),
                    item_id: browser.resource.clone(),
                },
                browser,
            )?;
        }
        let mut actions = Vec::new();
        if let Some(document) = document.filter(|document| document.dirty && !document.read_only) {
            context.update_component(self.save, |button, _| {
                *button = extra_button(
                    if document.conflicted {
                        "保留并保存"
                    } else {
                        "保存"
                    },
                    ButtonKind::Primary,
                );
            })?;
            context.update_component(self.discard, |button, _| {
                *button = extra_button(
                    if document.conflicted {
                        "重新载入"
                    } else {
                        "放弃"
                    },
                    ButtonKind::Subtle,
                );
            })?;
            actions.extend([self.save.stable_id(), self.discard.stable_id()]);
        } else if terminal.is_some_and(|terminal| terminal.running) {
            actions.push(self.interrupt.stable_id());
        }
        reconcile_children(context, self.actions.stable_id(), &actions)?;
        let mut order = Vec::new();
        if show_heading {
            order.push(self.heading.stable_id());
        }
        if show_status {
            order.push(self.status.stable_id());
        }
        match kind {
            Some("document-editor") => {
                if document.is_some() {
                    order.push(self.search.root.stable_id());
                }
                order.push(self.editor.stable_id());
                order.push(self.actions.stable_id());
            }
            Some("project-files") => {
                order.push(self.tree.stable_id());
                order.push(self.actions.stable_id());
            }
            Some("task-browser") => {
                order.push(self.browser.root.stable_id());
            }
            Some("terminal") => {
                order.push(self.log.stable_id());
                order.push(self.actions.stable_id());
            }
            _ => {}
        }
        if let Some(conversation) = conversation {
            order = vec![conversation];
        }
        reconcile_children(context, self.content.stable_id(), &order)?;
        assemble_workspace_chrome(context, self.chrome)?;
        Ok(())
    }

    pub(crate) fn dispose(self, context: &mut AppContext) -> Result<(), FrameworkError> {
        macro_rules! remove {
            ($entity:expr) => {
                if context.world().contains($entity.stable_id()) {
                    context.remove_view($entity)?;
                }
            };
        }
        remove!(self.chrome);
        remove!(self.search.panel);
        remove!(self.search.root);
        remove!(self.editor);
        remove!(self.log);
        self.browser.dispose(context)?;
        remove!(self.tree);
        remove!(self.actions);
        remove!(self.save);
        remove!(self.discard);
        remove!(self.interrupt);
        for button in self.chrome_actions {
            remove!(button);
        }
        Ok(())
    }
}

pub(crate) fn emit(sink: &IntentSink, intent: ShellIntent) {
    sink(intent);
}

pub(crate) fn bind_activate<V: View>(
    context: &mut AppContext,
    entity: Entity<V>,
    sink: IntentSink,
    intent: ShellIntent,
) -> Result<(), FrameworkError> {
    context.on(entity, move |_, _event: &Activate, _| {
        emit(&sink, intent.clone());
    })
}

fn sidebar_toggle_button(collapsed: bool) -> IconButton {
    IconButton::new(
        Icon::Sidebar,
        if collapsed {
            "显示会话栏"
        } else {
            "隐藏会话栏"
        },
    )
}

fn new_conversation_row(leading: StableNodeId) -> SidebarRow {
    let mut row = SidebarRow::new("新对话").size(ControlSize::Medium).slots(
        nana_ui::runtime::ListItemSlots {
            leading: Some(leading),
            content: None,
            trailing: None,
        },
    );
    row.style = sidebar_row_style();
    row
}

fn sidebar_row_style() -> NodeStyle {
    let mut style = NodeStyle::default();
    let layout = Arc::make_mut(&mut style.layout);
    // Rows are fixed height; a wrapping title would spill over the row tools.
    layout.white_space_nowrap = true;
    layout.text_overflow_ellipsis = true;
    style
}

fn sidebar_search_toggle() -> IconButton {
    sidebar_top_bar_tool_button(Icon::Search, "搜索")
}

fn sidebar_search_close() -> IconButton {
    sidebar_top_bar_tool_button(Icon::Close, "关闭搜索")
}

fn iab_unavailable_state() -> EmptyState {
    EmptyState::new("无法浏览网页")
        .message("没有可打开的页面。")
        .compact(true)
}

fn breadcrumb_items(parent: &str, current: &str) -> Vec<BreadcrumbItem> {
    vec![
        BreadcrumbItem::new(parent).tone(BreadcrumbTone::Parent),
        BreadcrumbItem::new(current).tone(BreadcrumbTone::Current),
    ]
}

fn bind_document_input(
    context: &mut AppContext,
    editor: Entity<TextArea>,
    sink: &IntentSink,
    bindings: &Arc<Mutex<PaneInputBindings>>,
) -> Result<(), FrameworkError> {
    let sink = Arc::clone(sink);
    let bindings = Arc::clone(bindings);
    context.on(editor, move |editor, event: &TextChanged, _| {
        editor.diagnostics = Arc::from([]);
        let intent = bindings.lock().unwrap().edit(event.value.clone());
        if let Some(intent) = intent {
            emit(&sink, intent);
        }
    })
}

#[derive(Clone, Copy)]
enum PaneAction {
    Save,
    Discard,
    Interrupt,
}

fn pane_bound_action(
    context: &mut AppContext,
    document_id: DocumentId,
    sink: &IntentSink,
    bindings: &Arc<Mutex<PaneInputBindings>>,
    label: &str,
    action: PaneAction,
) -> Result<Entity<Button>, FrameworkError> {
    let kind = match action {
        PaneAction::Save => ButtonKind::Primary,
        PaneAction::Discard => ButtonKind::Subtle,
        PaneAction::Interrupt => ButtonKind::Danger,
    };
    let button = context.create_detached_component(document_id, extra_button(label, kind))?;
    let sink = Arc::clone(sink);
    let bindings = Arc::clone(bindings);
    context.on(button, move |_, _: &Activate, _| {
        let bindings = bindings.lock().unwrap().clone();
        let intent = match action {
            PaneAction::Save => bindings
                .document
                .map(|(target, _, _)| ShellIntent::SaveDocument(target)),
            PaneAction::Discard => bindings
                .document
                .map(|(target, _, _)| ShellIntent::DiscardDocument(target)),
            PaneAction::Interrupt => bindings
                .terminal
                .map(|(target, session_id)| ShellIntent::TerminalInterrupt { target, session_id }),
        };
        if let Some(intent) = intent {
            emit(&sink, intent);
        }
    })?;
    Ok(button)
}

fn mount_workspace_pane_view(
    context: &mut AppContext,
    document_id: DocumentId,
    pane_id: &str,
    sink: &IntentSink,
) -> Result<WorkspacePaneView, FrameworkError> {
    mount_workspace_pane_view_in(context, document_id, HostedWindowId::PRIMARY, pane_id, sink)
}

fn assemble_workspace_chrome(
    context: &mut AppContext,
    chrome: Entity<PaneChrome>,
) -> Result<(), FrameworkError> {
    context.update_component(chrome, |chrome, _| {
        for action in &mut chrome.actions {
            action.icon = match action.kind {
                PaneChromeActionKind::SplitHorizontal => Some(Icon::Sidebar),
                PaneChromeActionKind::SplitVertical => Some(Icon::Workspace),
                PaneChromeActionKind::MoveToWindow => Some(Icon::Restore),
                PaneChromeActionKind::MoveToNextPane => Some(Icon::ArrowRight),
                _ => action.icon,
            };
        }
    })?;
    let (header, tabs, body, actions) = context.read(chrome, |chrome| {
        (
            chrome.header,
            chrome.tabs,
            chrome.body,
            chrome
                .actions
                .iter()
                .filter_map(|action| action.target)
                .collect::<Vec<_>>(),
        )
    })?;
    if let Some(header) = header {
        let children = tabs.into_iter().chain(actions).collect::<Vec<_>>();
        reconcile_children(context, header, &children)?;
    }
    reconcile_children(
        context,
        chrome.stable_id(),
        &header.into_iter().chain(body).collect::<Vec<_>>(),
    )
}

fn workspace_strip_id(window: HostedWindowId, pane: &str) -> String {
    if window == HostedWindowId::PRIMARY {
        format!("workspace/main/pane/{pane}")
    } else {
        format!("workspace/window/{}/pane/{pane}", window.0)
    }
}

fn mount_workspace_pane_view_in(
    context: &mut AppContext,
    document_id: DocumentId,
    window_id: HostedWindowId,
    pane_id: &str,
    sink: &IntentSink,
) -> Result<WorkspacePaneView, FrameworkError> {
    let chrome_sink: IntentSink = {
        let sink = Arc::clone(sink);
        let pane_id = pane_id.to_owned();
        Arc::new(move |intent| {
            emit(
                &sink,
                ShellIntent::WorkspacePane {
                    window_id,
                    pane_id: pane_id.clone(),
                    intent: Box::new(intent),
                },
            )
        })
    };
    let header = context.create_detached_component(document_id, Stack::bar(6.0))?;
    let (selected, options) = (String::new(), Vec::new());
    let tabs = context.create_detached_component(
        document_id,
        Tabs::new(selected)
            .options(options)
            .strip_id(workspace_strip_id(window_id, pane_id))
            .fill(true),
    )?;
    let pane_id_owned = pane_id.to_owned();
    let tab_sink = Arc::clone(&chrome_sink);
    context.on(tabs, move |_, event: &TabsEvent, _| {
        emit(&tab_sink, workspace_tabs_intent_for(&pane_id_owned, event));
    })?;
    let split_h = context.create_detached_component(
        document_id,
        workspace_chrome_button("左右分栏", Icon::Sidebar),
    )?;
    let split_v = context.create_detached_component(
        document_id,
        workspace_chrome_button("上下分栏", Icon::Workspace),
    )?;
    bind_activate(
        context,
        split_h,
        Arc::clone(&chrome_sink),
        ShellIntent::SplitWorkspaceHorizontal,
    )?;
    bind_activate(
        context,
        split_v,
        Arc::clone(&chrome_sink),
        ShellIntent::SplitWorkspaceVertical,
    )?;
    let move_window = context.create_detached_component(
        document_id,
        workspace_chrome_button(
            if window_id == HostedWindowId::PRIMARY {
                "移至新窗口"
            } else {
                "移回主窗口"
            },
            Icon::Restore,
        ),
    )?;
    let move_next = context.create_detached_component(
        document_id,
        workspace_chrome_button("移至下一窗格", Icon::ArrowRight),
    )?;
    bind_activate(
        context,
        move_window,
        Arc::clone(&chrome_sink),
        ShellIntent::MovePaneToWindow,
    )?;
    bind_activate(
        context,
        move_next,
        Arc::clone(&chrome_sink),
        ShellIntent::MovePaneToNext,
    )?;
    let body = context.create_detached_component(document_id, Stack::fill_column(0.0))?;
    let chrome = context.create_detached_component(
        document_id,
        PaneChrome::new()
            .header(header.stable_id())
            .tabs(tabs.stable_id())
            .body(body.stable_id())
            .actions([
                PaneChromeAction::new(PaneChromeActionKind::SplitHorizontal, "左右分栏")
                    .target(split_h.stable_id()),
                PaneChromeAction::new(PaneChromeActionKind::SplitVertical, "上下分栏")
                    .target(split_v.stable_id()),
                PaneChromeAction::new(
                    PaneChromeActionKind::MoveToWindow,
                    if window_id == HostedWindowId::PRIMARY {
                        "移至新窗口"
                    } else {
                        "移回主窗口"
                    },
                )
                .target(move_window.stable_id()),
                PaneChromeAction::new(PaneChromeActionKind::MoveToNextPane, "移至下一窗格")
                    .target(move_next.stable_id()),
            ]),
    )?;
    assemble_workspace_chrome(context, chrome)?;
    let content =
        context.create_detached_component(document_id, Stack::fill_column(12.0).padding(16.0))?;
    let heading = context.create_detached_component(document_id, Text::new(String::new()))?;
    let status = context.create_detached_component(document_id, Text::new(String::new()))?;
    let editor = context
        .create_detached_component(document_id, fill_workspace_editor(String::new(), None))?;
    let bindings = Arc::new(Mutex::new(PaneInputBindings::default()));
    bind_document_input(context, editor, sink, &bindings)?;
    let search = EditorSearchView::mount(context, document_id, editor, &bindings, sink)?;
    let log = mount_terminal(context, document_id, sink, &bindings)?;
    let browser = crate::browser_workbench::BrowserView::mount(context, document_id, sink)?;
    let tree = context.create_detached_component(document_id, TreeView::new(Vec::new()))?;
    let actions = context.create_detached_component(document_id, Stack::row(8.0))?;
    let save = pane_bound_action(
        context,
        document_id,
        sink,
        &bindings,
        "保存",
        PaneAction::Save,
    )?;
    let discard = pane_bound_action(
        context,
        document_id,
        sink,
        &bindings,
        "放弃",
        PaneAction::Discard,
    )?;
    let interrupt = pane_bound_action(
        context,
        document_id,
        sink,
        &bindings,
        "停止",
        PaneAction::Interrupt,
    )?;
    context.append_child(content, heading)?;
    context.append_child(content, status)?;
    context.append_child(body, content)?;
    Ok(WorkspacePaneView {
        bindings,
        chrome_actions: [split_h, split_v, move_window, move_next],
        save,
        discard,
        interrupt,
        chrome,
        tabs,
        content,
        heading,
        status,
        editor,
        search,
        log,
        terminal_session: None,
        browser,
        tree,
        actions,
    })
}

fn extra_button(label: &str, kind: ButtonKind) -> Button {
    pill_button(label, kind)
}

fn workspace_chrome_button(label: &'static str, icon: Icon) -> IconButton {
    IconButton::new(icon, label)
        .kind(ButtonKind::Text)
        .size(ControlSize::Small)
        .with_tooltip(label)
}

fn markdown_image_viewer(preview: &ShellMarkdownPreview) -> ImageViewer {
    let mut viewer = ImageViewer::new(
        preview
            .texture_slot
            .as_ref()
            .map(|slot| ImageViewerContent::host_texture(slot.clone()))
            .unwrap_or_default(),
    )
    .name(preview.title.clone())
    .metadata(preview.metadata.clone());
    viewer.intrinsic_size = preview.intrinsic_size;
    viewer
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentSearchAction {
    Next,
    Previous,
    Replace,
    ReplaceAll,
}

pub fn search_document(
    context: &mut AppContext,
    editor: StableNodeId,
    feedback: StableNodeId,
    query: &str,
    replacement: &str,
    action: DocumentSearchAction,
) -> Result<(), FrameworkError> {
    let Some(document) = context.world().node(editor).map(|node| node.document) else {
        return Ok(());
    };
    let read_only = context.read(Entity::<TextArea>::from_stable_id(editor), |area| {
        area.read_only
    })?;
    let message = if read_only
        && matches!(
            action,
            DocumentSearchAction::Replace | DocumentSearchAction::ReplaceAll
        ) {
        "此文档为只读".to_owned()
    } else if query.is_empty() {
        "请输入查找内容".to_owned()
    } else {
        context.focus_node(document, editor)?;
        if context.world().focused(document) != Some(editor) {
            return Ok(());
        }
        let options = TextSearchOptions::default();
        match action {
            DocumentSearchAction::Next | DocumentSearchAction::Previous => {
                let found = if action == DocumentSearchAction::Next {
                    context.find_next_focused_text_match(
                        document,
                        query,
                        options,
                        TextFindScope::Document,
                    )?
                } else {
                    context.find_previous_focused_text_match(
                        document,
                        query,
                        options,
                        TextFindScope::Document,
                    )?
                };
                if found {
                    String::new()
                } else {
                    "没有匹配内容".to_owned()
                }
            }
            DocumentSearchAction::Replace => {
                let replaced = context.replace_focused_text_match(
                    document,
                    query,
                    options,
                    replacement,
                    false,
                )?;
                let found = context.find_next_focused_text_match(
                    document,
                    query,
                    options,
                    TextFindScope::Document,
                )?;
                if replaced {
                    "已替换 1 处".to_owned()
                } else if found {
                    "已选中匹配内容，再次点击替换".to_owned()
                } else {
                    "没有匹配内容".to_owned()
                }
            }
            DocumentSearchAction::ReplaceAll => {
                let count = context.replace_all_focused_text_matches(
                    document,
                    query,
                    options,
                    replacement,
                    TextFindScope::Document,
                    false,
                )?;
                if count == 0 {
                    "没有匹配内容".to_owned()
                } else {
                    format!("已替换 {count} 处")
                }
            }
        }
    };
    context.update_component(Entity::<Text>::from_stable_id(feedback), |text, _| {
        *text = Text::new(message);
    })
}

#[derive(Default)]
struct EditorSearchDraft {
    item: Option<String>,
    read_only: bool,
    expanded: bool,
    query: String,
    replacement: String,
}

#[derive(Clone)]
struct EditorSearchView {
    replace_actions: Vec<Entity<Button>>,
    root: Entity<Stack>,
    panel: Entity<Stack>,
    toggle: Entity<Button>,
    query: Entity<TextInput>,
    replacement: Entity<TextInput>,
    feedback: Entity<Text>,
    draft: Arc<Mutex<EditorSearchDraft>>,
}

impl EditorSearchView {
    fn mount(
        context: &mut AppContext,
        document: DocumentId,
        editor: Entity<TextArea>,
        bindings: &Arc<Mutex<PaneInputBindings>>,
        sink: &IntentSink,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, Stack::column(4.0))?;
        let header = context.create_detached_component(document, Stack::row(0.0))?;
        let toggle = context
            .create_detached_component(document, extra_button("查找替换", ButtonKind::Text))?;
        let panel = context.create_detached_component(document, Stack::column(4.0))?;
        context.append_child(header, toggle)?;
        context.append_child(root, header)?;
        let find_row = context.create_detached_component(document, Stack::bar(4.0))?;
        let replace_row = context.create_detached_component(document, Stack::bar(4.0))?;
        let input = |placeholder: &str| {
            let mut input = TextInput::new(String::new()).placeholder(placeholder.to_owned());
            let layout = Arc::make_mut(&mut input.style.layout);
            layout.flex_grow = Some(1.0);
            layout.flex_shrink = Some(1.0);
            layout.min_width = Some(LengthSpec::Px(0.0));
            input
        };
        let query = context.create_detached_component(document, input("查找"))?;
        let replacement = context.create_detached_component(document, input("替换为"))?;
        let feedback = context.create_detached_component(document, Text::new(String::new()))?;
        let draft = Arc::new(Mutex::new(EditorSearchDraft::default()));
        let toggle_draft = Arc::clone(&draft);
        context.on(toggle, move |button, _: &Activate, cx| {
            let mut draft = toggle_draft.lock().unwrap();
            draft.expanded = !draft.expanded;
            *button = extra_button(
                if draft.expanded {
                    "收起查找"
                } else if draft.read_only {
                    "查找"
                } else {
                    "查找替换"
                },
                ButtonKind::Text,
            );
            if draft.expanded {
                cx.mutations()
                    .insert(root.stable_id(), panel.stable_id(), None);
                cx.mutations()
                    .request_focus(document, Some(query.stable_id()));
            } else {
                cx.mutations().park_subtree(panel.stable_id());
                cx.mutations()
                    .request_focus(document, Some(editor.stable_id()));
            }
        })?;
        let find_draft = Arc::clone(&draft);
        context.on(query, move |_, event: &TextChanged, _| {
            find_draft.lock().unwrap().query = event.value.clone();
        })?;
        let replace_draft = Arc::clone(&draft);
        context.on(replacement, move |_, event: &TextChanged, _| {
            replace_draft.lock().unwrap().replacement = event.value.clone();
        })?;
        context.append_child(find_row, query)?;
        context.append_child(replace_row, replacement)?;
        let mut replace_actions = Vec::new();
        for (row, label, action) in [
            (find_row, "上一处", DocumentSearchAction::Previous),
            (find_row, "下一处", DocumentSearchAction::Next),
            (replace_row, "替换", DocumentSearchAction::Replace),
            (replace_row, "全部替换", DocumentSearchAction::ReplaceAll),
        ] {
            let button = context
                .create_detached_component(document, extra_button(label, ButtonKind::Subtle))?;
            if matches!(
                action,
                DocumentSearchAction::Replace | DocumentSearchAction::ReplaceAll
            ) {
                replace_actions.push(button);
            }
            let bindings = Arc::clone(bindings);
            let draft = Arc::clone(&draft);
            let sink = Arc::clone(sink);
            context.on(button, move |_, _: &Activate, _| {
                let target = bindings
                    .lock()
                    .unwrap()
                    .document
                    .as_ref()
                    .map(|(target, _, _)| target.clone());
                let draft = draft.lock().unwrap();
                if let Some(target) =
                    target.filter(|target| draft.item.as_deref() == Some(target.item_id.as_str()))
                {
                    emit(
                        &sink,
                        ShellIntent::SearchDocument {
                            target,
                            editor: editor.stable_id(),
                            feedback: feedback.stable_id(),
                            query: draft.query.clone(),
                            replacement: draft.replacement.clone(),
                            action,
                        },
                    );
                }
            })?;
            context.append_child(row, button)?;
        }
        context.append_child(panel, find_row)?;
        context.append_child(panel, replace_row)?;
        context.append_child(panel, feedback)?;
        Ok(Self {
            replace_actions,
            root,
            panel,
            toggle,
            query,
            replacement,
            feedback,
            draft,
        })
    }

    fn set_read_only(
        &self,
        context: &mut AppContext,
        read_only: bool,
    ) -> Result<(), FrameworkError> {
        let mut draft = self.draft.lock().unwrap();
        draft.read_only = read_only;
        let label = if draft.expanded {
            "收起查找"
        } else if read_only {
            "查找"
        } else {
            "查找替换"
        };
        drop(draft);
        context.update_component(self.toggle, |button, _| {
            *button = extra_button(label, ButtonKind::Text);
        })?;
        context.update_component(self.replacement, |input, _| {
            input.disabled = read_only;
        })?;
        for button in &self.replace_actions {
            context.update_component(*button, |button, _| {
                button.disabled = read_only;
            })?;
        }
        Ok(())
    }

    fn sync(&self, context: &mut AppContext, item: Option<&str>) -> Result<(), FrameworkError> {
        let mut draft = self.draft.lock().unwrap();
        if draft.item.as_deref() == item {
            return Ok(());
        }
        *draft = EditorSearchDraft {
            item: item.map(str::to_owned),
            ..Default::default()
        };
        drop(draft);
        context.update_component(self.toggle, |button, cx| {
            *button = extra_button("查找替换", ButtonKind::Text);
            cx.mutations().park_subtree(self.panel.stable_id());
        })?;
        context.update_component(self.query, |input, _| {
            input.state.replace_value(String::new());
        })?;
        context.update_component(self.replacement, |input, _| {
            input.state.replace_value(String::new());
        })?;
        context.update_component(self.feedback, |text, _| {
            *text = Text::new(String::new());
        })
    }
}

fn fill_workspace_editor(value: impl Into<String>, language: Option<&str>) -> TextArea {
    let mut editor = TextArea::new(value.into());
    apply_workspace_editor_chrome(&mut editor, language);
    editor
}

fn apply_workspace_editor_chrome(editor: &mut TextArea, language: Option<&str>) {
    editor.highlight = language
        .filter(|language| !language.is_empty())
        .map(|language| HighlightRequest::highlight(language.to_owned()));
    editor.line_numbers = language.is_some();
    editor.code_editing = language.and_then(|language| {
        let (comment, indent) = match language.to_ascii_lowercase().as_str() {
            "rust" | "javascript" | "typescript" | "tsx" | "jsx" => ("//", "    "),
            "python" | "toml" | "shell" => ("#", "    "),
            "yaml" => ("#", "  "),
            _ => return None,
        };
        Some(CodeEditing::new(comment, indent))
    });
    fill_workspace_surface(editor);
    let digits = editor
        .state
        .value
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        .saturating_add(1)
        .ilog10()
        .saturating_add(1)
        .max(3);
    Arc::make_mut(&mut editor.style.layout).padding_left = Some(LengthSpec::Px(
        UI_METRICS.field_padding_x
            + if editor.line_numbers {
                digits as f32 * 8.0 + UI_METRICS.field_padding_x
            } else {
                0.0
            },
    ));
}

fn mount_terminal(
    context: &mut AppContext,
    document_id: DocumentId,
    sink: &IntentSink,
    bindings: &Arc<Mutex<PaneInputBindings>>,
) -> Result<Entity<nana_ui::runtime::TerminalView>, FrameworkError> {
    use nana_ui::runtime::{TerminalEvent, TerminalScreen, TerminalView};
    let terminal = context
        .create_detached_component(document_id, TerminalView::new(TerminalScreen::blank(1, 1)))?;
    let bindings = Arc::clone(bindings);
    let sink = Arc::clone(sink);
    context.on(terminal, move |_, event: &TerminalEvent, _| {
        let Some((target, session_id)) = bindings.lock().unwrap().terminal.clone() else {
            return;
        };
        match event {
            TerminalEvent::Input(bytes) => emit(
                &sink,
                ShellIntent::TerminalWrite {
                    target,
                    session_id,
                    bytes: bytes.clone(),
                },
            ),
            TerminalEvent::Resize { columns, rows } => emit(
                &sink,
                ShellIntent::TerminalResize {
                    target,
                    session_id,
                    columns: *columns,
                    rows: *rows,
                },
            ),
            TerminalEvent::SelectionChanged(_) => {}
        }
    })?;
    Ok(terminal)
}

fn fill_workspace_surface(area: &mut TextArea) {
    let layout = Arc::make_mut(&mut area.style.layout);
    layout.height = Some(LengthSpec::Fill);
    layout.flex_grow = Some(1.0);
    layout.min_height = Some(LengthSpec::Px(0.0));
}

/// 侧边栏菜单锚定来源；节点变体在同步时解析为按钮下方（右下展开）坐标。
#[derive(Clone, Copy, Debug, PartialEq)]
enum SidebarMenuAnchor {
    AddProjectButton(Option<(f32, f32)>),
    RowMenuButton(StableNodeId),
    Point((f32, f32)),
}

#[derive(Clone, Copy)]
enum RowToolButton {
    Stop(Entity<Button>),
    Tool(Entity<IconButton>),
}

impl RowToolButton {
    fn stable_id(self) -> StableNodeId {
        match self {
            Self::Stop(button) => button.stable_id(),
            Self::Tool(button) => button.stable_id(),
        }
    }

    fn attach(self, context: &mut AppContext, host: Entity<Stack>) -> Result<(), FrameworkError> {
        match self {
            Self::Stop(button) => context.append_child(host, button),
            Self::Tool(button) => context.append_child(host, button),
        }
    }

    fn remove(self, context: &mut AppContext) -> Result<(), FrameworkError> {
        match self {
            Self::Stop(button) => context.remove_view(button).map(|_| ()),
            Self::Tool(button) => context.remove_view(button).map(|_| ()),
        }
    }
}

fn row_menu_button() -> IconButton {
    sidebar_row_tool_button(Icon::More, "更多")
}

fn row_stop_button() -> Button {
    Button::new("停止")
        .kind(ButtonKind::Danger)
        .size(ControlSize::Small)
}

fn row_draft_button() -> IconButton {
    sidebar_row_tool_button(Icon::MessageSquarePlus, "新对话")
}

fn pane_tab_options(snapshot: &PrimaryShellSnapshot) -> (String, Vec<TabOption>) {
    snapshot
        .panes
        .iter()
        .find(|pane| pane.active)
        .or_else(|| snapshot.panes.first())
        .map(pane_tab_options_for)
        .unwrap_or_default()
}

fn pane_tab_options_for(pane: &ShellPaneRow) -> (String, Vec<TabOption>) {
    let selected = pane
        .items
        .iter()
        .find(|item| item.selected)
        .map(|item| item.id.clone())
        .unwrap_or_default();
    let options = pane
        .items
        .iter()
        .map(|item| {
            TabOption::new(item.id.clone(), item.title.clone())
                .icon(workspace_kind_icon(&item.kind))
                .closable(item.closable)
        })
        .collect();
    (selected, options)
}

fn active_pane_strip_id(snapshot: &PrimaryShellSnapshot) -> String {
    snapshot
        .panes
        .iter()
        .find(|pane| pane.active)
        .or_else(|| snapshot.panes.first())
        .map(|pane| format!("workspace/main/pane/{}", pane.id))
        .unwrap_or_else(|| "workspace/main/pane/active".to_owned())
}

fn workspace_tabs_intent(event: &TabsEvent) -> ShellIntent {
    workspace_tabs_intent_for("active", event)
}

fn workspace_tabs_intent_for(pane_id: &str, event: &TabsEvent) -> ShellIntent {
    match event {
        TabsEvent::Select(value) => {
            let item_id = if value.as_ref() == "conversation" {
                None
            } else {
                Some(value.to_string())
            };
            ShellIntent::SelectPaneTab {
                pane_id: pane_id.to_owned(),
                item_id,
            }
        }
        TabsEvent::Reorder { value, before } => ShellIntent::ReorderPaneTab {
            pane_id: pane_id.to_owned(),
            item_id: value.to_string(),
            before: before.as_ref().map(|value| value.to_string()),
        },
        TabsEvent::Close(value) => ShellIntent::ClosePaneTab {
            item_id: value.to_string(),
        },
        TabsEvent::Transfer {
            source_strip,
            value,
            target_strip,
            before,
        } => ShellIntent::TransferPaneTab {
            source_strip: source_strip.to_string(),
            target_strip: target_strip.to_string(),
            item_id: value.to_string(),
            before: before.as_ref().map(|value| value.to_string()),
        },
    }
}

pub(crate) fn pending_action_intent(
    request_id: String,
    action: crate::module::composer::pending_view::PendingAction,
) -> ShellIntent {
    use crate::module::composer::pending_view::PendingAction;
    match action {
        PendingAction::RespondApproval { approved } => ShellIntent::RespondApproval {
            request_id,
            approved,
        },
        PendingAction::RespondTitle { accepted } => ShellIntent::RespondTitle {
            request_id,
            accepted,
        },
        PendingAction::RespondArchitecture { approved } => ShellIntent::RespondArchitecture {
            request_id,
            approved,
        },
        PendingAction::RespondPlan { action } => ShellIntent::RespondPlan { request_id, action },
        PendingAction::RespondToolConsent { approved } => ShellIntent::RespondToolConsent {
            request_id,
            approved,
        },
        PendingAction::ToolConsentDraftChanged { command, message } => {
            ShellIntent::ToolConsentDraftChanged {
                request_id,
                command,
                message,
            }
        }
        PendingAction::AskUserPending { action, value } => ShellIntent::AskUserPending {
            request_id,
            action,
            value,
        },
        PendingAction::PendingDraftChanged { value } => {
            ShellIntent::PendingDraftChanged { request_id, value }
        }
        PendingAction::SelectPendingOption { option_id } => ShellIntent::SelectPendingOption {
            request_id,
            option_id,
        },
        PendingAction::RespondMcp { action } => ShellIntent::RespondMcp { request_id, action },
        PendingAction::McpFieldChanged { field_key, value } => ShellIntent::McpFieldChanged {
            request_id,
            field_key,
            value,
        },
        PendingAction::McpRawJsonChanged { value } => {
            ShellIntent::McpRawJsonChanged { request_id, value }
        }
        PendingAction::McpToggleOption {
            field_key,
            value,
            multi,
        } => ShellIntent::McpToggleOption {
            request_id,
            field_key,
            value,
            multi,
        },
        PendingAction::McpToggleBoolean { field_key } => ShellIntent::McpToggleBoolean {
            request_id,
            field_key,
        },
        PendingAction::OpenMarkdownLink(url) => ShellIntent::OpenMarkdownLink(url),
        PendingAction::InterruptTurn(target) => ShellIntent::InterruptTurn(target),
    }
}

fn command_palette_view(snapshot: &PrimaryShellSnapshot) -> CommandPalette {
    CommandPalette::new("命令", snapshot.command_palette_items.clone())
        .placeholder("搜索命令")
        .query(snapshot.command_palette_query.clone())
}

pub fn mount_primary_shell(
    snapshot: &PrimaryShellSnapshot,
    sink: IntentSink,
) -> Result<(nana_ui::runtime::RuntimeDocument, ShellHandles), FrameworkError> {
    let document_id = DocumentId::new(PRIMARY_DOCUMENT).expect("primary document id");
    let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
    let context = document.context_mut();
    let _ = context.set_theme(snapshot.theme);

    let title_leading = context.create_detached_component(document_id, Stack::row(0.0))?;
    let sidebar_toggle = context.create_detached_component(
        document_id,
        sidebar_toggle_button(snapshot.sidebar_collapsed),
    )?;
    context.append_child(title_leading, sidebar_toggle)?;
    bind_activate(
        context,
        sidebar_toggle,
        Arc::clone(&sink),
        ShellIntent::ToggleSidebar,
    )?;

    let title_breadcrumb = context.create_detached_component(document_id, Breadcrumb::new())?;
    context.set_breadcrumb_items(
        title_breadcrumb,
        breadcrumb_items(&snapshot.title_parent, &snapshot.title_context),
    )?;
    // The reference chrome keeps window controls alone on the trailing edge; the
    // command palette and inspector already live in the titlebar more menu, which
    // now hangs off the sidebar footer.
    let title_trailing = context.create_detached_component(document_id, Stack::row(6.0))?;
    if WindowChrome::platform_default().uses_custom_controls() {
        let minimize = context.create_detached_component(
            document_id,
            window_control(Icon::Minimize, "最小化", ButtonKind::Text),
        )?;
        let maximize = context.create_detached_component(
            document_id,
            window_control(Icon::Maximize, "最大化", ButtonKind::Text),
        )?;
        let close = context.create_detached_component(
            document_id,
            window_control(Icon::Close, "关闭", ButtonKind::Text),
        )?;
        context.append_child(title_trailing, minimize)?;
        context.append_child(title_trailing, maximize)?;
        context.append_child(title_trailing, close)?;
        bind_activate(
            context,
            minimize,
            Arc::clone(&sink),
            ShellIntent::WindowChrome(WindowChromeEvent::Action(WindowChromeAction::Minimize)),
        )?;
        bind_activate(
            context,
            maximize,
            Arc::clone(&sink),
            ShellIntent::WindowChrome(WindowChromeEvent::Action(
                WindowChromeAction::ToggleMaximize,
            )),
        )?;
        bind_activate(
            context,
            close,
            Arc::clone(&sink),
            ShellIntent::WindowChrome(WindowChromeEvent::Action(WindowChromeAction::Close)),
        )?;
    }

    let sidebar_top = context.create_detached_component(document_id, Stack::bar(6.0))?;
    let new_conversation_icon = context
        .create_detached_component(document_id, SidebarRowIcon::new(Icon::MessageSquarePlus))?;
    let new_conversation = context.create_detached_component(
        document_id,
        new_conversation_row(new_conversation_icon.stable_id()),
    )?;
    context.append_child(new_conversation, new_conversation_icon)?;
    let search_toggle = context.create_detached_component(document_id, sidebar_search_toggle())?;
    let search_close = context.create_detached_component(document_id, sidebar_search_close())?;
    let search_input = context.create_detached_component(
        document_id,
        TextArea::new(snapshot.sidebar_search_query.clone())
            .placeholder("搜索项目和会话")
            .height(32.0),
    )?;
    let search_sink = Arc::clone(&sink);
    context.on(search_input, move |_, event: &TextChanged, _| {
        emit(
            &search_sink,
            ShellIntent::SidebarSearchChanged(event.value.clone()),
        );
    })?;
    bind_activate(
        context,
        new_conversation,
        Arc::clone(&sink),
        ShellIntent::NewConversation,
    )?;
    bind_activate(
        context,
        search_toggle,
        Arc::clone(&sink),
        ShellIntent::ToggleSidebarSearch,
    )?;
    bind_activate(
        context,
        search_close,
        Arc::clone(&sink),
        ShellIntent::ToggleSidebarSearch,
    )?;
    if snapshot.sidebar_search_open {
        context.append_child(sidebar_top, search_input)?;
        context.append_child(sidebar_top, search_close)?;
    } else {
        context.append_child(sidebar_top, new_conversation)?;
        context.append_child(sidebar_top, search_toggle)?;
    }

    let add_project_menu = context.create_detached_component(
        document_id,
        sidebar_section_tool_button(Icon::Add, "添加项目"),
    )?;
    bind_activate(
        context,
        add_project_menu,
        Arc::clone(&sink),
        ShellIntent::OpenAddProjectMenu,
    )?;
    let (section, _session_header, task_body) = mount_sidebar_section(
        context,
        document_id,
        "会话",
        Some(SESSIONS_EMPTY_TEXT),
        None,
    )?;
    let (project_section, project_header, project_body) = mount_sidebar_section(
        context,
        document_id,
        "项目",
        Some(PROJECTS_EMPTY_TEXT),
        Some(add_project_menu),
    )?;
    bind_activate(
        context,
        project_header,
        Arc::clone(&sink),
        ShellIntent::OpenProjectsOverview,
    )?;
    let (inbox_section, inbox_header, inbox_body) =
        mount_sidebar_section(context, document_id, "收集箱", Some(INBOX_EMPTY_TEXT), None)?;
    bind_activate(
        context,
        inbox_header,
        Arc::clone(&sink),
        ShellIntent::ToggleSidebarInbox,
    )?;
    let task_reorder =
        mount_sidebar_reorder(context, document_id, "会话", false, Arc::clone(&sink))?;
    let project_reorder =
        mount_sidebar_reorder(context, document_id, "项目", true, Arc::clone(&sink))?;
    let inbox_reorder =
        mount_sidebar_reorder(context, document_id, "收集箱", false, Arc::clone(&sink))?;
    let scroll =
        context.create_detached_component(document_id, SidebarFrame::vertical_body_scroll())?;
    context.append_child(scroll, section)?;
    let footer = context.create_detached_component(document_id, SidebarFooter::new())?;
    let mut footer_nav = HashMap::new();
    for item in &snapshot.nav_items {
        let button = context.create_detached_component(
            document_id,
            SidebarFooterButton::new(item.label.clone(), footer_nav_icon(item.settings))
                .selected(item.selected),
        )?;
        context.append_child(footer, button)?;
        bind_activate(
            context,
            button,
            Arc::clone(&sink),
            if item.settings {
                ShellIntent::OpenSettings
            } else {
                ShellIntent::OpenAutomations
            },
        )?;
        footer_nav.insert(item.id.clone(), button);
    }
    let more = context
        .create_detached_component(document_id, SidebarFooterButton::new("更多", Icon::More))?;
    context.append_child(footer, more)?;
    bind_activate(
        context,
        more,
        Arc::clone(&sink),
        ShellIntent::ToggleTitlebarMenu,
    )?;
    let provider_badge = context.create_detached_component(
        document_id,
        SidebarFooterButton::new(
            snapshot.provider_badge.clone(),
            snapshot.provider_badge_icon,
        ),
    )?;
    context.append_child(footer, provider_badge)?;
    bind_activate(
        context,
        provider_badge,
        Arc::clone(&sink),
        ShellIntent::OpenSettings,
    )?;
    let conversation_sidebar = context.create_detached_component(
        document_id,
        SidebarFrame::new()
            .top(sidebar_top.stable_id())
            .body(scroll.stable_id())
            .footer(footer.stable_id()),
    )?;
    context.append_child(conversation_sidebar, sidebar_top)?;
    context.append_child(conversation_sidebar, scroll)?;
    context.append_child(conversation_sidebar, footer)?;

    let conversation = context.create_detached_component(document_id, conversation_root())?;
    let task_view = crate::module::task::view::TaskView::mount(
        context,
        document_id,
        snapshot.task_input(),
        Arc::clone(&sink),
    )?;
    context.append_child(conversation, task_view.conversation_column)?;

    let settings_view = crate::module::settings::view::SettingsView::mount(
        context,
        document_id,
        &snapshot.settings,
        snapshot.theme,
        Arc::clone(&sink),
    )?;
    let workspace_page = context.create_detached_component(document_id, Stack::fill_column(0.0))?;
    let terminal_page = context.create_detached_component(document_id, Stack::fill_column(0.0))?;
    let workbench_bottom =
        context.create_detached_component(document_id, Stack::fill_column(0.0))?;
    let compact_workbench = crate::workspace_view::CompactWorkbench::mount(
        context,
        document_id,
        conversation.stable_id(),
    )?;
    let pane_header = context.create_detached_component(document_id, Stack::bar(6.0))?;
    let (pane_selected, pane_options) = pane_tab_options(snapshot);
    let pane_tabs = context.create_detached_component(
        document_id,
        Tabs::new(pane_selected)
            .options(pane_options)
            .strip_id(active_pane_strip_id(snapshot))
            .fill(true),
    )?;
    let pane_sink = Arc::clone(&sink);
    context.on(pane_tabs, move |_, event: &TabsEvent, _| {
        emit(&pane_sink, workspace_tabs_intent(event));
    })?;
    let pane_split_h = context.create_detached_component(
        document_id,
        workspace_chrome_button("左右分栏", Icon::Sidebar),
    )?;
    let pane_split_v = context.create_detached_component(
        document_id,
        workspace_chrome_button("上下分栏", Icon::Workspace),
    )?;
    bind_activate(
        context,
        pane_split_h,
        Arc::clone(&sink),
        ShellIntent::SplitWorkspaceHorizontal,
    )?;
    bind_activate(
        context,
        pane_split_v,
        Arc::clone(&sink),
        ShellIntent::SplitWorkspaceVertical,
    )?;
    let pane_move_window = context.create_detached_component(
        document_id,
        workspace_chrome_button("移至新窗口", Icon::Restore),
    )?;
    let pane_move_next = context.create_detached_component(
        document_id,
        workspace_chrome_button("移至下一窗格", Icon::ArrowRight),
    )?;
    bind_activate(
        context,
        pane_move_window,
        Arc::clone(&sink),
        ShellIntent::MovePaneToWindow,
    )?;
    bind_activate(
        context,
        pane_move_next,
        Arc::clone(&sink),
        ShellIntent::MovePaneToNext,
    )?;
    let pane_body = context.create_detached_component(document_id, Stack::fill_column(0.0))?;
    let pane_chrome = context.create_detached_component(
        document_id,
        PaneChrome::new()
            .header(pane_header.stable_id())
            .tabs(pane_tabs.stable_id())
            .body(pane_body.stable_id())
            .actions([
                PaneChromeAction::new(PaneChromeActionKind::SplitHorizontal, "左右分栏")
                    .target(pane_split_h.stable_id()),
                PaneChromeAction::new(PaneChromeActionKind::SplitVertical, "上下分栏")
                    .target(pane_split_v.stable_id()),
                PaneChromeAction::new(PaneChromeActionKind::MoveToWindow, "移至新窗口")
                    .target(pane_move_window.stable_id()),
                PaneChromeAction::new(PaneChromeActionKind::MoveToNextPane, "移至下一窗格")
                    .target(pane_move_next.stable_id()),
            ]),
    )?;
    assemble_workspace_chrome(context, pane_chrome)?;
    context.append_child(workspace_page, pane_chrome)?;
    let pane_bar = context.create_detached_component(document_id, Stack::row(8.0))?;
    context.append_child(workspace_page, pane_bar)?;
    let workspace_content =
        context.create_detached_component(document_id, Stack::fill_column(12.0).padding(16.0))?;
    let workspace_heading =
        context.create_detached_component(document_id, Text::new(String::new()))?;
    let workspace_status =
        context.create_detached_component(document_id, Text::new(String::new()))?;
    let workspace_editor = context
        .create_detached_component(document_id, fill_workspace_editor(String::new(), None))?;
    let workspace_bindings = Arc::new(Mutex::new(PaneInputBindings::default()));
    bind_document_input(context, workspace_editor, &sink, &workspace_bindings)?;
    let workspace_search = EditorSearchView::mount(
        context,
        document_id,
        workspace_editor,
        &workspace_bindings,
        &sink,
    )?;
    let workspace_log = mount_terminal(context, document_id, &sink, &workspace_bindings)?;
    let workspace_browser =
        crate::browser_workbench::BrowserView::mount(context, document_id, &sink)?;
    let workspace_tree =
        context.create_detached_component(document_id, TreeView::new(Vec::new()))?;
    let tree_sink = Arc::clone(&sink);
    context.on(
        workspace_tree,
        move |_, event: &TreeViewEvent<Arc<str>>, _| match event {
            TreeViewEvent::Toggle(path) => {
                emit(&tree_sink, ShellIntent::ToggleProjectFile(path.to_string()));
            }
            TreeViewEvent::Select(path) => {
                emit(&tree_sink, ShellIntent::OpenProjectFile(path.to_string()));
            }
        },
    )?;
    let workspace_actions = context.create_detached_component(document_id, Stack::row(8.0))?;
    context.append_child(workspace_content, workspace_heading)?;
    context.append_child(workspace_content, workspace_status)?;
    context.append_child(pane_body, workspace_content)?;

    let inspector =
        context.create_detached_component(document_id, Stack::fill_column(8.0).padding(12.0))?;
    let inspector_header =
        context.create_detached_component(document_id, inspector_header_bar())?;
    let inspector_heading = context
        .create_detached_component(document_id, Text::new(snapshot.inspector_title.clone()))?;
    let inspector_close = context
        .create_detached_component(document_id, sidebar_icon_button(Icon::Close, "关闭检查器"))?;
    bind_activate(
        context,
        inspector_close,
        Arc::clone(&sink),
        ShellIntent::CloseInspectorDock,
    )?;
    context.append_child(inspector_header, inspector_heading)?;
    context.append_child(inspector_header, inspector_close)?;
    let inspector_body = context
        .create_detached_component(document_id, Text::new(snapshot.inspector_body.clone()))?;
    context.append_child(inspector, inspector_header)?;
    context.append_child(inspector, inspector_body)?;
    let inspector_todos = context.create_detached_component(document_id, Stack::column(4.0))?;
    let todo_panel = crate::todo_panel::TodoPanel::mount(
        context,
        document_id,
        Arc::clone(&sink),
        HostedWindowId::PRIMARY,
    )?;
    context.append_child(inspector_todos, todo_panel.root)?;
    context.append_child(inspector, inspector_todos)?;
    let iab_empty = context.create_detached_component(document_id, iab_unavailable_state())?;
    context.append_child(inspector, iab_empty)?;
    let diagnostics_panel =
        context.create_detached_component(document_id, Stack::column(4.0).padding(8.0))?;
    let coding_panel = context.create_detached_component(document_id, Stack::fill_column(8.0))?;
    let coding_query = context.create_detached_component(
        document_id,
        TextArea::new(String::new())
            .placeholder("搜索工作区")
            .height(36.0),
    )?;
    context.on(coding_query, {
        let sink = Arc::clone(&sink);
        move |_, event: &TextChanged, _| {
            emit(&sink, ShellIntent::CodingQueryChanged(event.value.clone()))
        }
    })?;
    context.append_child(coding_panel, coding_query)?;
    context.append_child(inspector, coding_panel)?;

    let automation_view = crate::module::automation::view::AutomationView::mount(
        context,
        document_id,
        &snapshot.automation,
        snapshot.navigation.is_automations(),
        Arc::clone(&sink),
    )?;

    let project_page = context.create_detached_component(
        document_id,
        ScrollView::new(ScrollAxes::Vertical)
            .style(Stack::fill_column(12.0).padding(16.0).node_style()),
    )?;
    let project_page_title =
        context.create_detached_component(document_id, Text::new(String::new()))?;
    let project_page_body =
        context.create_detached_component(document_id, Text::new(String::new()))?;
    context.append_child(project_page, project_page_title)?;
    context.append_child(project_page, project_page_body)?;

    let conversation_workspace = context.create_detached_component(
        document_id,
        // SplitPane 每次装配会全量重投影根节点，自带 Background 承接 Primary 区域底色。
        SplitPane::from_model(
            &SplitPaneModel::new(
                SplitAxis::Horizontal,
                CONVERSATION_WORKSPACE_SPLIT_SIZE,
                CONVERSATION_WORKSPACE_SPLIT_MIN,
                10_000.0,
            ),
            conversation.stable_id(),
            workspace_page.stable_id(),
        )
        .surface(SemanticColorRole::Background),
    )?;
    let navigation = if snapshot.navigation.is_settings() {
        settings_view.settings_sidebar.stable_id()
    } else if snapshot.navigation.is_automations() {
        automation_view.sidebar.stable_id()
    } else {
        conversation_sidebar.stable_id()
    };
    let primary = primary_content_id(
        snapshot,
        conversation,
        settings_view.settings_page,
        conversation_workspace,
        automation_view.page,
        project_page,
    );
    let mut shell_builder = DesktopShell::from_model(snapshot.workspace.clone())
        .title(snapshot.title_context.clone())
        .title_leading(title_leading.stable_id())
        .title_center(title_breadcrumb.stable_id())
        .title_center_width(TITLE_BREADCRUMB_WIDTH)
        // The trailing slot already owns bound window controls; the shell's own
        // strip would only add a second, inert set.
        .title_window_controls(false)
        .title_trailing(title_trailing.stable_id())
        .navigation(navigation)
        .primary(primary);
    if !snapshot.inspector_title.is_empty() {
        shell_builder = shell_builder.inspector(inspector.stable_id());
    }
    let shell = context.create_component(document_id, shell_builder)?;
    context.assemble_desktop_shell(shell)?;
    if primary == conversation_workspace.stable_id() {
        assemble_conversation_workspace(
            context,
            conversation_workspace,
            conversation,
            workspace_page,
        )?;
    }

    let overlay_host = context
        .read(shell, |shell| {
            shell.overlay.map(Entity::<OverlayHost>::from_stable_id)
        })
        .ok()
        .flatten();

    let pending_overlay_dismissals = Arc::new(Mutex::new(Vec::new()));
    if let Some(host) = overlay_host {
        let pending = Arc::clone(&pending_overlay_dismissals);
        let sink = Arc::clone(&sink);
        let closing_sink = Arc::clone(&sink);
        context.on(host, move |_, event: &OverlayClosing, _| {
            pending
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(event.root);
            emit(&closing_sink, ShellIntent::OverlayPresenceChanged);
        })?;
        context.on(host, move |_, _: &OverlayChanged, _| {
            emit(&sink, ShellIntent::OverlayPresenceChanged);
        })?;
    }

    let mut handles = ShellHandles {
        automation_view,
        project_fields: crate::form_view::ProductFields::new(Arc::clone(&sink)),
        settings_view,
        task_view,
        sink,
        shell,
        overlay_host,
        pending_overlay_dismissals,
        palette: None,
        more_menu: None,
        titlebar_menu: None,
        sidebar_toggle,
        footer_more: more,
        pane_bar,
        pane_buttons: HashMap::new(),
        title_breadcrumb,
        title_leading,
        title_trailing,
        conversation_sidebar,
        sidebar_top,
        new_conversation,
        search_toggle,
        search_input,
        search_close,
        sidebar_scroll: scroll,
        conversation_section: section,
        task_body,
        task_reorder,
        project_section,
        #[cfg(test)]
        project_header,
        project_body,
        project_reorder,
        add_project_menu,
        inbox_section,
        inbox_body,
        inbox_reorder,
        task_rows: HashMap::new(),
        row_kinds: HashMap::new(),
        row_tools: HashMap::new(),
        row_tool_buttons: HashMap::new(),
        footer_nav,
        provider_badge,
        conversation,
        synced: SyncedInputs::default(),
        shell_assembled: false,
        extra_buttons: HashMap::new(),
        project_page,
        project_page_title,
        project_page_body,
        project_cards: HashMap::new(),
        memory_view: None,
        roadmap_view: None,
        architecture_view: None,
        workspace_page,
        terminal_page,
        workbench_bottom,
        compact_workbench,
        conversation_workspace,
        pane_chrome,
        pane_tabs,
        workspace_content,
        workspace_heading,
        workspace_status,
        workspace_editor,
        workspace_search,
        workspace_bindings,
        workspace_log,
        workspace_terminal_session: None,
        workspace_browser,
        diagnostics_panel,
        diagnostic_rows: HashMap::new(),
        image_viewer: None,
        workspace_actions,
        workspace_buttons: HashMap::new(),
        workspace_tree,
        inspector,
        inspector_header,
        inspector_close,
        inspector_heading,
        inspector_body,
        inspector_todos,
        inspector_todo_rows: HashMap::new(),
        todo_panel,
        coding_panel,
        coding_query,
        coding_rows: HashMap::new(),
        pane_move_window,
        pane_move_next,
        extra_workspace_panes: HashMap::new(),
        workspace_splits: HashMap::new(),
        workspace_split_handles: HashMap::new(),
        iab_empty,
        confirm: None,
        confirm_cancel: None,
        confirm_commit: None,
        focus_targets: HashMap::new(),
    };
    handles.focus_targets.insert(
        target_ids::COMMAND_PALETTE_OPEN.to_owned(),
        more.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::SIDEBAR_NEW_CONVERSATION.to_owned(),
        new_conversation.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::SIDEBAR_SEARCH_TOGGLE.to_owned(),
        search_toggle.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::SIDEBAR_SEARCH_INPUT.to_owned(),
        search_input.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::SIDEBAR_PROJECTS_OVERVIEW.to_owned(),
        project_header.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::SIDEBAR_PROJECTS_ADD.to_owned(),
        add_project_menu.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::COMPOSER_INPUT.to_owned(),
        handles.task_view.composer_view.composer.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::TASK_SESSION_PENDING.to_owned(),
        handles.task_view.pending_view.root.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::TASK_SESSION_INSPECTOR.to_owned(),
        inspector.stable_id(),
    );
    handles.focus_targets.insert(
        target_ids::TASK_SESSION_INSPECTOR_CLOSE.to_owned(),
        inspector_close.stable_id(),
    );
    handles.sync_lists(context, snapshot)?;
    handles.settings_view.sync(
        context,
        document_id,
        &snapshot.settings,
        snapshot.theme,
        snapshot.navigation.is_settings(),
        snapshot.workspace.viewport_geometry().logical_size.0,
    )?;
    handles.sync_workspace_page(context, document_id, snapshot)?;
    handles.sync_overlay(context, document_id, snapshot)?;
    Ok((document, handles))
}

fn active_pane_item_kind(snapshot: &PrimaryShellSnapshot) -> Option<&str> {
    snapshot
        .panes
        .iter()
        .find(|pane| pane.active)
        .or_else(|| snapshot.panes.first())
        .and_then(|pane| pane.items.iter().find(|item| item.selected))
        .map(|item| item.kind.as_str())
}

fn workspace_pane_kind(snapshot: &PrimaryShellSnapshot) -> Option<&str> {
    if snapshot.project_page == Some(ShellProjectPage::Files) {
        Some("project-files")
    } else {
        active_pane_item_kind(snapshot)
    }
}

fn has_workspace_primary_content(snapshot: &PrimaryShellSnapshot) -> bool {
    if snapshot
        .panes
        .iter()
        .any(|pane| pane_has_resource(pane) && !pane_is_terminal(pane))
    {
        return true;
    }
    if snapshot.panes.iter().any(pane_is_terminal) {
        return false;
    }
    match workspace_pane_kind(snapshot) {
        Some("document-editor") => snapshot.document.is_some(),
        Some("terminal") => snapshot.terminal.is_some(),
        Some("task-browser") => snapshot.browser.is_some(),
        Some("project-files") => true,
        _ => false,
    }
}

pub(crate) fn pane_is_terminal(pane: &ShellPaneRow) -> bool {
    !pane.items.is_empty() && pane.items.iter().all(|item| item.kind == "terminal")
}

fn pane_has_resource(pane: &ShellPaneRow) -> bool {
    pane.items.iter().any(|item| {
        item.selected
            && matches!(
                item.kind.as_str(),
                "document-editor" | "terminal" | "task-browser" | "project-files"
            )
    })
}

fn conversation_root() -> Stack {
    // 每帧整体重投影会抹掉 Primary 区域涂在节点上的底色，因此自带 Background。
    Stack::fill_column(0.0)
        .padding_xy(24.0, 20.0)
        .radius(UI_METRICS.radius_lg)
        .surface(SemanticColorRole::Background)
}

fn assemble_conversation_workspace(
    context: &mut AppContext,
    split: Entity<SplitPane>,
    conversation: Entity<Stack>,
    workspace_page: Entity<Stack>,
) -> Result<(), FrameworkError> {
    context.update_component(split, |pane, _| {
        pane.first = Some(conversation.stable_id());
        pane.second = Some(workspace_page.stable_id());
    })?;
    context.assemble_split_pane(split)?;
    Ok(())
}

fn primary_content_id(
    snapshot: &PrimaryShellSnapshot,
    conversation: Entity<Stack>,
    settings_page: Entity<SettingsPage>,
    conversation_workspace: Entity<SplitPane>,
    automations_page: Entity<Stack>,
    project_page: Entity<ScrollView>,
) -> StableNodeId {
    if snapshot.navigation.is_settings() {
        settings_page.stable_id()
    } else if snapshot.navigation.is_automations() {
        automations_page.stable_id()
    } else if has_workspace_primary_content(snapshot) {
        conversation_workspace.stable_id()
    } else if snapshot.project_page.is_some() {
        project_page.stable_id()
    } else {
        conversation.stable_id()
    }
}

fn sidebar_menu_view(
    context: &AppContext,
    host: Entity<OverlayHost>,
    anchor: (f32, f32),
    items: Vec<ContextMenuItem>,
) -> ContextMenu {
    let mut view = ContextMenu::new(anchor.0, anchor.1).items(items).open(true);
    if let Some(viewport) = context.world().layout_box(host.stable_id()) {
        view.place_in(viewport);
    }
    view
}

fn overlay_anchor(
    context: &AppContext,
    node: StableNodeId,
    below: bool,
    fallback: Option<(f32, f32)>,
) -> (f32, f32) {
    context
        .world()
        .layout_box(node)
        .map(|bounds| {
            (
                bounds.x,
                if below {
                    bounds.y + bounds.height
                } else {
                    bounds.y
                },
            )
        })
        .or(fallback)
        .unwrap_or((0.0, 0.0))
}

fn close_shell_overlay<V: View>(
    context: &mut AppContext,
    host: Entity<OverlayHost>,
    overlay: &mut Option<Entity<V>>,
) -> Result<(), FrameworkError> {
    let Some(entity) = *overlay else {
        return Ok(());
    };
    let is_active = |context: &AppContext| {
        context
            .world()
            .overlay_host(host.stable_id())
            .is_some_and(|state| state.active == Some(entity.stable_id()))
    };
    if is_active(context) {
        context.dismiss_overlay(host)?;
    }
    if !is_active(context) {
        context.remove_view(entity)?;
        *overlay = None;
    }
    Ok(())
}

impl ShellHandles {
    #[cfg(debug_assertions)]
    pub(crate) fn automation_graph_is_mounted(
        &self,
        document: &nana_ui::runtime::RuntimeDocument,
    ) -> bool {
        self.automation_view.graph_is_mounted(document.context())
    }

    pub(crate) fn reveal_workspace_resources(&self) {
        self.compact_workbench.reveal_resources();
    }

    #[cfg(debug_assertions)]
    pub(crate) fn debug_browser_views(&self) -> Vec<crate::browser_workbench::BrowserView> {
        std::iter::once(self.workspace_browser.clone())
            .chain(
                self.extra_workspace_panes
                    .values()
                    .map(|pane| pane.browser.clone()),
            )
            .collect()
    }

    pub(crate) fn matches_document_search(
        &self,
        target: &ShellPaneTarget,
        editor: StableNodeId,
        feedback: StableNodeId,
    ) -> bool {
        let matches = |bindings: &Arc<Mutex<PaneInputBindings>>,
                       view: &EditorSearchView,
                       node: StableNodeId| {
            node == editor
                && view.feedback.stable_id() == feedback
                && bindings
                    .lock()
                    .unwrap()
                    .document
                    .as_ref()
                    .is_some_and(|(current, _, _)| current == target)
                && view.draft.lock().unwrap().expanded
        };
        matches(
            &self.workspace_bindings,
            &self.workspace_search,
            self.workspace_editor.stable_id(),
        ) || self
            .extra_workspace_panes
            .values()
            .any(|view| matches(&view.bindings, &view.search, view.editor.stable_id()))
    }

    /// 拖动工作区分隔条由 NanaUI 输入层直接改写 shell 模型且不通知宿主，
    /// 宿主在每次同步前经此拉取，保持平行 controller 与 shell 一致。
    pub fn live_workspace_model(
        &self,
        document: &mut nana_ui::runtime::RuntimeDocument,
    ) -> Option<WorkspaceModel> {
        document
            .context_mut()
            .read(self.shell, |shell| shell.model.clone())
            .ok()
    }

    pub fn sync(
        &mut self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let context = document.context_mut();
        let _ = context.set_theme(snapshot.theme);
        context.update_component(self.sidebar_toggle, |button, _| {
            *button = sidebar_toggle_button(snapshot.sidebar_collapsed);
        })?;
        context.update_component(self.footer_more, |button, _| {
            *button =
                SidebarFooterButton::new("更多", Icon::More).selected(snapshot.titlebar_menu_open);
        })?;
        context.set_breadcrumb_items(
            self.title_breadcrumb,
            breadcrumb_items(&snapshot.title_parent, &snapshot.title_context),
        )?;
        context.update_component(self.search_toggle, |button, _| {
            *button = sidebar_search_toggle();
        })?;
        context.update_component(self.search_close, |button, _| {
            *button = sidebar_search_close();
        })?;
        context.update_component(self.search_input, |editor, _| {
            if editor.state.value != snapshot.sidebar_search_query {
                editor
                    .state
                    .replace_value(snapshot.sidebar_search_query.clone());
            }
        })?;
        context.update_component(self.provider_badge, |button, _| {
            *button = SidebarFooterButton::new(
                snapshot.provider_badge.clone(),
                snapshot.provider_badge_icon,
            );
        })?;
        context.update_component(self.inspector_heading, |text, _| {
            *text = Text::new(snapshot.inspector_title.clone());
        })?;
        context.update_component(self.inspector_body, |text, _| {
            *text = Text::new(snapshot.inspector_body.clone());
        })?;
        let document_id = context
            .world()
            .node(self.task_body.stable_id())
            .map(|node| node.document)
            .ok_or(FrameworkError::MissingView(self.task_body.stable_id()))?;
        self.sync_lists(context, snapshot)?;
        self.settings_view.sync(
            context,
            document_id,
            &snapshot.settings,
            snapshot.theme,
            snapshot.navigation.is_settings(),
            snapshot.workspace.viewport_geometry().logical_size.0,
        )?;
        let document_id = context
            .world()
            .node(self.workspace_page.stable_id())
            .map(|node| node.document)
            .ok_or(FrameworkError::MissingView(self.workspace_page.stable_id()))?;
        let workspace_inputs = WorkspaceInputs {
            panes: snapshot.panes.clone(),
            pane_layout: snapshot.pane_layout.clone(),
            document: snapshot.document.clone(),
            terminal: snapshot.terminal.clone(),
            browser: snapshot.browser.clone(),
            files: snapshot.files.clone(),
        };
        if self.synced.workspace.as_ref() != Some(&workspace_inputs) {
            self.sync_workspace_page(context, document_id, snapshot)?;
            self.sync_panes(context, document_id, snapshot)?;
            self.sync_diagnostics(context, document_id, snapshot)?;
            self.synced.workspace = Some(workspace_inputs);
        }
        self.automation_view.sync(
            context,
            document_id,
            &snapshot.automation,
            snapshot.navigation.is_automations(),
        )?;
        self.sync_task_view(context, document_id, snapshot)?;
        self.sync_project_page(context, document_id, snapshot)?;
        let inspector_inputs = InspectorInputs {
            kind: snapshot.inspector_kind.clone(),
            todos: snapshot.inspector_todos.clone(),
            body: snapshot.inspector_body.clone(),
            architecture_resource: snapshot.architecture.project_id.clone(),
            coding: snapshot.coding.clone(),
            todo_panel: snapshot.todo_panel.clone(),
        };
        if self.synced.inspector.as_ref() != Some(&inspector_inputs)
            || snapshot.inspector_kind == "architecture"
        {
            self.sync_inspector_details(context, document_id, snapshot)?;
            self.synced.inspector = Some(inspector_inputs);
        }
        self.sync_overlay(context, document_id, snapshot)?;
        let navigation = if snapshot.navigation.is_settings() {
            self.settings_view.settings_sidebar.stable_id()
        } else if snapshot.navigation.is_automations() {
            self.automation_view.sidebar.stable_id()
        } else {
            self.conversation_sidebar.stable_id()
        };
        let mut primary = primary_content_id(
            snapshot,
            self.conversation,
            self.settings_view.settings_page,
            self.conversation_workspace,
            self.automation_view.page,
            self.project_page,
        );
        let inspector = (!snapshot.inspector_title.is_empty()).then(|| self.inspector.stable_id());
        let diagnostics = snapshot
            .document
            .as_ref()
            .is_some_and(|document| !document.diagnostics.is_empty())
            .then(|| self.diagnostics_panel.stable_id());
        let task_surface = primary == self.conversation.stable_id()
            || primary == self.conversation_workspace.stable_id();
        let has_terminal = task_surface && snapshot.panes.iter().any(pane_is_terminal);
        let compact = task_surface && snapshot.workspace.inline_size() < 1000.0;
        let bottom = if compact {
            let mut resources = Vec::new();
            if has_workspace_primary_content(snapshot) {
                resources.push(self.workspace_page.stable_id());
            }
            if has_terminal {
                resources.push(self.terminal_page.stable_id());
            }
            resources.extend(diagnostics);
            self.compact_workbench.sync(context, &resources)?;
            primary = self.compact_workbench.root.stable_id();
            None
        } else if has_terminal {
            let mut children = vec![self.terminal_page.stable_id()];
            children.extend(diagnostics);
            reconcile_children(context, self.workbench_bottom.stable_id(), &children)?;
            Some(self.workbench_bottom.stable_id())
        } else {
            diagnostics
        };
        let mut shell_changed = !self.shell_assembled;
        context.update_component(self.shell, |shell, _| {
            shell_changed = shell_changed
                || shell.model != snapshot.workspace
                || shell.title.as_deref() != Some(snapshot.title_context.as_str())
                || shell.navigation != Some(navigation)
                || shell.primary != Some(primary)
                || shell.inspector != inspector
                || shell.bottom != bottom;
            shell.model = snapshot.workspace.clone();
            shell.title = Some(Arc::from(snapshot.title_context.as_str()));
            shell.title_leading = Some(self.title_leading.stable_id());
            shell.title_center = Some(self.title_breadcrumb.stable_id());
            shell.title_trailing = Some(self.title_trailing.stable_id());
            shell.navigation = Some(navigation);
            shell.primary = Some(primary);
            shell.inspector = inspector;
            shell.bottom = bottom;
        })?;
        if shell_changed {
            context.assemble_desktop_shell(self.shell)?;
            self.shell_assembled = true;
        }
        if primary == self.conversation_workspace.stable_id() {
            assemble_conversation_workspace(
                context,
                self.conversation_workspace,
                self.conversation,
                self.workspace_page,
            )?;
        } else if primary == self.conversation.stable_id() {
            context.update_component(self.conversation, |stack, _| {
                *stack = conversation_root();
            })?;
        }
        if primary == self.project_page.stable_id()
            && snapshot.project_page == Some(ShellProjectPage::Memory)
        {
            if let Some(view) = &mut self.memory_view {
                view.restore_focus(context)?;
            }
        }
        if primary == self.project_page.stable_id()
            && snapshot.project_page == Some(ShellProjectPage::Roadmap)
        {
            if let Some(view) = &mut self.roadmap_view {
                view.restore_focus(context)?;
            }
        }
        if primary == self.project_page.stable_id()
            && snapshot.project_page == Some(ShellProjectPage::Architecture)
        {
            if let Some(view) = &mut self.architecture_view {
                view.restore_focus(context)?;
            }
        }
        self.overlay_host = context
            .read(self.shell, |shell| {
                shell.overlay.map(Entity::<OverlayHost>::from_stable_id)
            })
            .ok()
            .flatten();
        Ok(())
    }

    pub fn apply_ui_commands(
        &mut self,
        document: &mut nana_ui::runtime::RuntimeDocument,
        window_id: HostedWindowId,
        commands: impl IntoIterator<Item = HostedUiCommand>,
    ) -> Result<(), FrameworkError> {
        let document_id = document.document();
        let context = document.context_mut();
        for command in commands {
            match command {
                HostedUiCommand::Focus {
                    window_id: target_window,
                    target,
                } if target_window == window_id => {
                    if let Some(node) = self.focus_targets.get(&target).copied() {
                        let _ = context.focus_node(document_id, node)?;
                    } else if target == target_ids::COMMAND_PALETTE_INPUT {
                        if let Some(palette) = self.palette {
                            let _ = context.focus_node(document_id, palette.stable_id())?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn sync_lists(
        &mut self,
        context: &mut AppContext,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let document_id = context
            .world()
            .node(self.task_body.stable_id())
            .map(|node| node.document)
            .ok_or(FrameworkError::MissingView(self.task_body.stable_id()))?;
        if self.synced.sidebar_rows != snapshot.sidebar_rows
            || self.synced.sidebar_tasks != snapshot.tasks
            || self.synced.sidebar_search_open != snapshot.sidebar_search_open
        {
            let groups = partition_sidebar_rows(snapshot);
            self.reconcile_task_rows(context, document_id, &groups)?;
            self.sync_sidebar_sections(context, snapshot, &groups)?;
            self.synced.sidebar_rows = snapshot.sidebar_rows.clone();
            self.synced.sidebar_tasks = snapshot.tasks.clone();
            self.synced.sidebar_search_open = snapshot.sidebar_search_open;
        }
        self.sync_sidebar_chrome(context, snapshot)?;
        Ok(())
    }

    fn sync_sidebar_sections(
        &mut self,
        context: &mut AppContext,
        snapshot: &PrimaryShellSnapshot,
        groups: &SidebarRowGroups,
    ) -> Result<(), FrameworkError> {
        context.update_component(self.conversation_section, |section, _| {
            section.title = Arc::from(if snapshot.sidebar_search_open {
                "搜索"
            } else {
                "会话"
            });
            section.count = Some(groups.sessions.len());
            section.empty_text = (!snapshot.sidebar_search_open && groups.sessions.is_empty())
                .then(|| Arc::from(SESSIONS_EMPTY_TEXT));
        })?;
        context.update_component(self.project_section, |section, _| {
            section.title = Arc::from("项目");
            section.count = Some(sidebar_project_entry_count(&groups.projects));
            section.empty_text = groups
                .projects
                .is_empty()
                .then(|| Arc::from(PROJECTS_EMPTY_TEXT));
        })?;
        context.update_component(self.inbox_section, |section, _| {
            section.title = Arc::from("收集箱");
            section.count = Some(groups.inbox.len());
            section.empty_text = groups.inbox.is_empty().then(|| Arc::from(INBOX_EMPTY_TEXT));
            section.collapsible = true;
            section.state = SidebarSectionState::new(groups.inbox_expanded);
        })?;
        let sections = if snapshot.sidebar_search_open {
            vec![self.conversation_section.stable_id()]
        } else if groups.grouped {
            vec![
                self.project_section.stable_id(),
                self.inbox_section.stable_id(),
            ]
        } else {
            vec![self.conversation_section.stable_id()]
        };
        reconcile_children(context, self.sidebar_scroll.stable_id(), &sections)
    }

    fn sync_sidebar_chrome(
        &mut self,
        context: &mut AppContext,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let document_id = context
            .world()
            .node(self.sidebar_top.stable_id())
            .map(|node| node.document)
            .ok_or(FrameworkError::MissingView(self.sidebar_top.stable_id()))?;
        let top = if snapshot.sidebar_search_open {
            vec![self.search_input.stable_id(), self.search_close.stable_id()]
        } else {
            vec![
                self.new_conversation.stable_id(),
                self.search_toggle.stable_id(),
            ]
        };
        reconcile_children(context, self.sidebar_top.stable_id(), &top)?;
        let mut footer = Vec::new();
        let mut keep = HashSet::new();
        for item in &snapshot.nav_items {
            keep.insert(item.id.clone());
            let button = if let Some(button) = self.footer_nav.get(&item.id).copied() {
                context.update_component(button, |button, _| {
                    *button = SidebarFooterButton::new(
                        item.label.clone(),
                        footer_nav_icon(item.settings),
                    )
                    .selected(item.selected);
                })?;
                button
            } else {
                let button = context.create_detached_component(
                    document_id,
                    SidebarFooterButton::new(item.label.clone(), footer_nav_icon(item.settings))
                        .selected(item.selected),
                )?;
                bind_activate(
                    context,
                    button,
                    Arc::clone(&self.sink),
                    if item.settings {
                        ShellIntent::OpenSettings
                    } else {
                        ShellIntent::OpenAutomations
                    },
                )?;
                self.footer_nav.insert(item.id.clone(), button);
                button
            };
            footer.push(button.stable_id());
        }
        let stale: Vec<_> = self
            .footer_nav
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect();
        for id in stale {
            if let Some(button) = self.footer_nav.remove(&id) {
                let _ = context.remove_view(button);
            }
        }
        footer.push(self.footer_more.stable_id());
        footer.push(self.provider_badge.stable_id());
        let footer_id = context
            .world()
            .node(self.provider_badge.stable_id())
            .and_then(|node| node.parent)
            .ok_or(FrameworkError::MissingView(self.provider_badge.stable_id()))?;
        reconcile_children(context, footer_id, &footer)
    }

    fn sync_row_tools(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        item: &ShellSidebarRow,
        row: Entity<SidebarRow>,
    ) -> Result<(), FrameworkError> {
        let stop_prefix = format!("{}-stop-", item.id);
        let expected_stop = item
            .stop_turn_id
            .as_ref()
            .filter(|_| item.can_stop)
            .map(|turn| format!("{stop_prefix}{turn}"));
        let stale = self
            .row_tool_buttons
            .keys()
            .filter(|key| {
                key.starts_with(&stop_prefix) && Some(key.as_str()) != expected_stop.as_deref()
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(button) = self.row_tool_buttons.remove(&key) {
                button.remove(context)?;
            }
        }
        let mut tools = Vec::new();
        if item.can_stop {
            if let (Ok(task_id), Some(turn_id)) = (TaskId::new(&item.id), item.stop_turn_id.clone())
            {
                let id = format!("{}-stop-{}", item.id, turn_id);
                let button =
                    if let Some(RowToolButton::Stop(button)) = self.row_tool_buttons.get(&id) {
                        *button
                    } else {
                        let button =
                            context.create_detached_component(document_id, row_stop_button())?;
                        bind_activate(
                            context,
                            button,
                            Arc::clone(&self.sink),
                            ShellIntent::StopSidebarTask(TurnStopTarget { task_id, turn_id }),
                        )?;
                        self.row_tool_buttons
                            .insert(id, RowToolButton::Stop(button));
                        button
                    };
                tools.push(RowToolButton::Stop(button));
            }
        }
        if item.can_draft {
            let id = format!("{}-draft", item.id);
            let button = if let Some(RowToolButton::Tool(button)) = self.row_tool_buttons.get(&id) {
                *button
            } else {
                let button = context.create_detached_component(document_id, row_draft_button())?;
                bind_activate(
                    context,
                    button,
                    Arc::clone(&self.sink),
                    ShellIntent::OpenProjectDraft(item.id.clone()),
                )?;
                self.row_tool_buttons
                    .insert(id, RowToolButton::Tool(button));
                button
            };
            tools.push(RowToolButton::Tool(button));
        }
        if item.can_menu {
            let id = format!("{}-menu", item.id);
            let button = if let Some(RowToolButton::Tool(button)) = self.row_tool_buttons.get(&id) {
                *button
            } else {
                let button = context.create_detached_component(document_id, row_menu_button())?;
                let intent = match item.kind {
                    ShellSidebarKind::Task
                    | ShellSidebarKind::SearchTask
                    | ShellSidebarKind::Running => ShellIntent::OpenTaskMenu {
                        id: item.id.clone(),
                        anchor: None,
                    },
                    _ => ShellIntent::OpenProjectMenu {
                        id: item.id.clone(),
                        anchor: None,
                    },
                };
                bind_activate(context, button, Arc::clone(&self.sink), intent)?;
                self.row_tool_buttons
                    .insert(id, RowToolButton::Tool(button));
                button
            };
            tools.push(RowToolButton::Tool(button));
        }
        if tools.is_empty() {
            return self.clear_row_tools(context, &item.id, Some(row));
        }
        let host = if let Some(host) = self.row_tools.get(&item.id).copied() {
            host
        } else {
            let host = context.create_detached_component(document_id, Stack::row(2.0))?;
            context.update_component(row, |row, _| {
                row.tools = Some(host.stable_id());
            })?;
            context.append_child(row, host)?;
            self.row_tools.insert(item.id.clone(), host);
            host
        };
        let order = tools
            .iter()
            .map(|tool| tool.stable_id())
            .collect::<Vec<_>>();
        for tool in tools {
            if context
                .world()
                .node(tool.stable_id())
                .and_then(|node| node.parent)
                != Some(host.stable_id())
            {
                tool.attach(context, host)?;
            }
        }
        reconcile_children(context, host.stable_id(), &order)
    }

    fn sync_sidebar_row_group(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        items: &[ShellSidebarRow],
        keep: &mut HashSet<String>,
    ) -> Result<Vec<StableNodeId>, FrameworkError> {
        let mut order = Vec::new();
        for item in items {
            keep.insert(item.id.clone());
            if self
                .row_kinds
                .get(&item.id)
                .is_some_and(|kind| *kind != item.kind)
            {
                self.remove_sidebar_row(context, &item.id);
            }
            let state = if item.selected {
                SidebarRowState::Active
            } else if item.ancestor {
                SidebarRowState::AncestorActive
            } else {
                SidebarRowState::Idle
            };
            let row = if let Some(row) = self.task_rows.get(&item.id).copied() {
                context.update_component(row, |row, _| {
                    row.label = Arc::from(item.label.as_str());
                    row.state = state;
                    row.depth = item.depth;
                    row.disclosure = item.expanded;
                })?;
                row
            } else {
                // Nested session rows read as children of their project through
                // indentation alone; a glyph there only competes with the label.
                let leading = if item.depth == 0 {
                    Some(context.create_detached_component(
                        document_id,
                        SidebarRowIcon::new(sidebar_row_icon(item.kind, &item.id)),
                    )?)
                } else {
                    None
                };
                let mut row_view = SidebarRow::new(item.label.clone())
                    .state(state)
                    .depth(item.depth)
                    .slots(nana_ui::runtime::ListItemSlots {
                        leading: leading.map(|leading| leading.stable_id()),
                        content: None,
                        trailing: None,
                    });
                row_view.style = sidebar_row_style();
                if let Some(expanded) = item.expanded {
                    row_view = row_view.disclosure(expanded);
                }
                let row = context.create_detached_component(document_id, row_view)?;
                if let Some(leading) = leading {
                    context.append_child(row, leading)?;
                }
                if let Some(intent) = sidebar_row_intent(item) {
                    bind_activate(context, row, Arc::clone(&self.sink), intent)?;
                }
                let sink = Arc::clone(&self.sink);
                let row_id = item.id.clone();
                let row_kind = item.kind;
                context.on(row, move |_, press: &SecondaryPress, _| {
                    if let Some(intent) =
                        sidebar_row_menu_intent(row_kind, row_id.as_str(), (press.x, press.y))
                    {
                        emit(&sink, intent);
                    }
                })?;
                self.task_rows.insert(item.id.clone(), row);
                self.row_kinds.insert(item.id.clone(), item.kind);
                row
            };
            self.sync_row_tools(context, document_id, item, row)?;
            order.push(row.stable_id());
        }
        Ok(order)
    }

    fn clear_row_tools(
        &mut self,
        context: &mut AppContext,
        id: &str,
        row: Option<Entity<SidebarRow>>,
    ) -> Result<(), FrameworkError> {
        if let Some(row) = row {
            context.update_component(row, |row, _| {
                row.tools = None;
            })?;
        }
        if let Some(host) = self.row_tools.remove(id) {
            let _ = context.remove_view(host);
        }
        let stopped = self
            .row_tool_buttons
            .keys()
            .filter(|key| key.starts_with(&format!("{id}-stop-")))
            .cloned()
            .collect::<Vec<_>>();
        for key in stopped {
            if let Some(button) = self.row_tool_buttons.remove(&key) {
                let _ = button.remove(context);
            }
        }
        for suffix in ["stop", "draft", "menu"] {
            if let Some(button) = self.row_tool_buttons.remove(&format!("{id}-{suffix}")) {
                let _ = button.remove(context);
            }
        }
        Ok(())
    }

    fn remove_sidebar_row(&mut self, context: &mut AppContext, id: &str) {
        self.row_kinds.remove(id);
        let row = self.task_rows.remove(id);
        let _ = self.clear_row_tools(context, id, row);
        if let Some(row) = row {
            let _ = context.remove_view(row);
        }
    }

    fn reconcile_task_rows(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        groups: &SidebarRowGroups,
    ) -> Result<(), FrameworkError> {
        let mut keep = HashSet::new();
        let session_order =
            self.sync_sidebar_row_group(context, document_id, &groups.sessions, &mut keep)?;
        let project_order =
            self.sync_sidebar_row_group(context, document_id, &groups.projects, &mut keep)?;
        let inbox_order =
            self.sync_sidebar_row_group(context, document_id, &groups.inbox, &mut keep)?;
        let stale: Vec<_> = self
            .task_rows
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            self.remove_sidebar_row(context, &key);
        }
        self.sync_reorder_list(
            context,
            self.task_reorder,
            self.task_body,
            &groups.sessions,
            &session_order,
            false,
        )?;
        self.sync_reorder_list(
            context,
            self.project_reorder,
            self.project_body,
            &groups.projects,
            &project_order,
            groups.grouped,
        )?;
        self.sync_reorder_list(
            context,
            self.inbox_reorder,
            self.inbox_body,
            &groups.inbox,
            &inbox_order,
            false,
        )
    }

    fn sync_reorder_list(
        &self,
        context: &mut AppContext,
        list: Entity<ReorderList>,
        body: Entity<List>,
        items: &[ShellSidebarRow],
        order: &[StableNodeId],
        tree_drop: bool,
    ) -> Result<(), FrameworkError> {
        if order.is_empty() {
            return reconcile_children(context, body.stable_id(), &[]);
        }
        let entries = items
            .iter()
            .map(|item| {
                sidebar_reorder_item(
                    item,
                    self.row_tools.get(&item.id).map(|host| host.stable_id()),
                )
            })
            .collect::<Vec<_>>();
        context.update_component(list, |list, _| {
            list.items = entries;
            list.tree_drop = tree_drop;
        })?;
        reconcile_children(context, list.stable_id(), order)?;
        reconcile_children(context, body.stable_id(), &[list.stable_id()])
    }

    fn sync_task_view(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        self.task_view
            .sync(context, document, snapshot.task_input())
    }

    fn upsert_tagged_button(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        id: &str,
        label: &str,
        kind: ButtonKind,
        intent: ShellIntent,
        disabled: bool,
    ) -> Result<Entity<Button>, FrameworkError> {
        if let Some(button) = self.extra_buttons.get(id).copied() {
            context.update_component(button, |button, _| {
                *button = extra_button(label, kind);
                button.disabled = disabled;
            })?;
            Ok(button)
        } else {
            let mut view = extra_button(label, kind);
            view.disabled = disabled;
            let button = context.create_detached_component(document_id, view)?;
            bind_activate(context, button, Arc::clone(&self.sink), intent)?;
            self.extra_buttons.insert(id.to_owned(), button);
            Ok(button)
        }
    }

    fn sync_inspector_details(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        context.update_component(self.iab_empty, |empty, _| {
            *empty = iab_unavailable_state();
        })?;
        self.todo_panel
            .sync(context, document_id, &snapshot.todo_panel)?;
        let mut order = Vec::new();
        if snapshot.todo_panel.visible {
            for (_, row) in self.inspector_todo_rows.drain() {
                let _ = context.remove_view(row);
            }
            order.push(self.todo_panel.root.stable_id());
        } else {
            let inspector_rows: Vec<(String, String)> = snapshot
                .inspector_todos
                .iter()
                .map(|todo| {
                    (
                        todo.id.clone(),
                        format!("{} {}", if todo.done { "✓" } else { "○" }, todo.label),
                    )
                })
                .collect();
            for (id, label) in &inspector_rows {
                let row = if let Some(row) = self.inspector_todo_rows.get(id).copied() {
                    context.update_component(row, |text, _| {
                        *text = Text::new(label.clone());
                    })?;
                    row
                } else {
                    let row =
                        context.create_detached_component(document_id, Text::new(label.clone()))?;
                    self.inspector_todo_rows.insert(id.clone(), row);
                    row
                };
                order.push(row.stable_id());
            }
            let stale: Vec<_> = self
                .inspector_todo_rows
                .keys()
                .filter(|id| inspector_rows.iter().all(|(keep, _)| keep != *id))
                .cloned()
                .collect();
            for id in stale {
                if let Some(row) = self.inspector_todo_rows.remove(&id) {
                    let _ = context.remove_view(row);
                }
            }
        }
        reconcile_children(context, self.inspector_todos.stable_id(), &order)?;
        self.sync_coding_tools(context, document_id, snapshot)?;
        let mut inspector_order = vec![self.inspector_header.stable_id()];
        match snapshot.inspector_kind.as_str() {
            "coding" => inspector_order.push(self.coding_panel.stable_id()),
            "iab" => inspector_order.push(self.iab_empty.stable_id()),
            "architecture" => {
                if let Some(view) = &self.architecture_view {
                    inspector_order.push(view.inspector.stable_id());
                }
            }
            _ => {
                inspector_order.push(self.inspector_body.stable_id());
                inspector_order.push(self.inspector_todos.stable_id());
            }
        }
        reconcile_children(
            context,
            self.inspector_header.stable_id(),
            &[
                self.inspector_heading.stable_id(),
                self.inspector_close.stable_id(),
            ],
        )?;
        reconcile_children(context, self.inspector.stable_id(), &inspector_order)
    }

    fn sync_coding_tools(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let Some(coding) = &snapshot.coding else {
            reconcile_children(context, self.coding_panel.stable_id(), &[])?;
            return Ok(());
        };
        context.update_component(self.coding_query, |editor, _| {
            if editor.state.value != coding.query {
                editor.state.replace_value(coding.query.clone());
            }
        })?;
        let mut keep = HashSet::new();
        let mut order = vec![self.coding_query.stable_id()];
        for (id, label, intent) in [
            ("coding-search", "搜索", ShellIntent::SearchCoding),
            (
                "coding-refresh",
                if coding.busy { "处理中" } else { "刷新" },
                ShellIntent::RefreshCoding,
            ),
            (
                "coding-mode",
                coding.mode_label.as_str(),
                ShellIntent::CycleCodingMode,
            ),
            (
                "coding-scope",
                coding.scope_label.as_str(),
                ShellIntent::ToggleCodingScope,
            ),
            (
                "coding-files",
                "文件管理器",
                ShellIntent::OpenCodingWorkspace,
            ),
            (
                "coding-terminal",
                "工作区终端",
                ShellIntent::OpenCodingTerminal,
            ),
        ] {
            keep.insert(id.to_owned());
            let button = self.upsert_coding_button(context, document_id, id, label, intent)?;
            order.push(button.stable_id());
        }
        if !coding.git.is_empty() {
            keep.insert("coding-git".into());
            let button = self.upsert_coding_button(
                context,
                document_id,
                "coding-git",
                &coding.git,
                ShellIntent::RefreshCoding,
            )?;
            order.push(button.stable_id());
        }
        for file in &coding.files {
            let id = format!("coding-file-{}", file.path);
            keep.insert(id.clone());
            let button = self.upsert_coding_button(
                context,
                document_id,
                &id,
                &format!("{} · {}", file.path, file.kind),
                ShellIntent::OpenProjectFile(file.path.clone()),
            )?;
            order.push(button.stable_id());
        }
        for hit in &coding.hits {
            let id = format!("coding-hit-{}", hit.id);
            keep.insert(id.clone());
            let label = if hit.summary.is_empty() {
                hit.label.clone()
            } else {
                format!("{}\n{}", hit.label, hit.summary)
            };
            let button = self.upsert_coding_button(
                context,
                document_id,
                &id,
                &label,
                ShellIntent::OpenCodingHit(hit.id.clone()),
            )?;
            order.push(button.stable_id());
        }
        for row in coding.terminals.iter().chain(coding.tasks.iter()) {
            let id = format!("coding-row-{}", row.id);
            keep.insert(id.clone());
            let button = self.upsert_coding_button(
                context,
                document_id,
                &id,
                &row.label,
                ShellIntent::OpenCodingTerminal,
            )?;
            order.push(button.stable_id());
        }
        let stale: Vec<_> = self
            .coding_rows
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(button) = self.coding_rows.remove(&key) {
                let _ = context.remove_view(button);
            }
        }
        reconcile_children(context, self.coding_panel.stable_id(), &order)
    }

    fn upsert_coding_button(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        id: &str,
        label: &str,
        intent: ShellIntent,
    ) -> Result<Entity<Button>, FrameworkError> {
        if let Some(button) = self.coding_rows.get(id).copied() {
            context.update_component(button, |button, _| {
                *button = extra_button(label, ButtonKind::Subtle);
            })?;
            Ok(button)
        } else {
            let button = context
                .create_detached_component(document_id, extra_button(label, ButtonKind::Subtle))?;
            bind_activate(context, button, Arc::clone(&self.sink), intent)?;
            self.coding_rows.insert(id.to_owned(), button);
            Ok(button)
        }
    }

    fn sync_project_page(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let project_visible = !snapshot.navigation.is_management();
        let memory_visible =
            project_visible && snapshot.project_page == Some(ShellProjectPage::Memory);
        let roadmap_visible =
            project_visible && snapshot.project_page == Some(ShellProjectPage::Roadmap);
        if !memory_visible {
            if let Some(view) = &mut self.memory_view {
                view.suspend(context)?;
            }
        }
        if !roadmap_visible {
            if let Some(view) = &mut self.roadmap_view {
                view.suspend(context)?;
            }
        }
        if !(project_visible && snapshot.project_page == Some(ShellProjectPage::Architecture)) {
            if let Some(view) = &mut self.architecture_view {
                view.suspend(context)?;
            }
        }
        if !project_visible {
            return Ok(());
        }
        if snapshot.project_page.is_none() {
            return Ok(());
        }
        context.update_component(self.project_page_title, |text, _| {
            *text = Text::new(snapshot.project_page_title.clone());
        })?;
        context.update_component(self.project_page_body, |text, _| {
            *text = Text::new(snapshot.project_page_body.clone());
        })?;
        let mut keep = HashSet::new();
        let mut field_keep = HashSet::new();
        let mut order = vec![self.project_page_title.stable_id()];
        match snapshot.project_page {
            Some(ShellProjectPage::Clone) => {
                order.push(self.project_page_body.stable_id());
                self.project_fields.upsert(
                    context,
                    document_id,
                    &mut field_keep,
                    &mut order,
                    "project-clone-repository",
                    &snapshot.clone_repository,
                    ShellIntent::CloneRepositoryChanged,
                )?;
                let parent = self.upsert_tagged_button(
                    context,
                    document_id,
                    "project-clone-parent",
                    if snapshot.clone_parent.is_empty() {
                        "选择父目录"
                    } else {
                        snapshot.clone_parent.as_str()
                    },
                    ButtonKind::Subtle,
                    ShellIntent::PickCloneParent,
                    false,
                )?;
                order.push(parent.stable_id());
                let start = self.upsert_tagged_button(
                    context,
                    document_id,
                    "project-clone-start",
                    "开始克隆",
                    ButtonKind::Primary,
                    ShellIntent::StartClone,
                    false,
                )?;
                order.push(start.stable_id());
                let cancel = self.upsert_tagged_button(
                    context,
                    document_id,
                    "project-clone-cancel",
                    "取消",
                    ButtonKind::Subtle,
                    ShellIntent::CancelClone,
                    false,
                )?;
                order.push(cancel.stable_id());
            }
            Some(ShellProjectPage::Settings) => {
                order.push(self.project_page_body.stable_id());
                self.project_fields.upsert(
                    context,
                    document_id,
                    &mut field_keep,
                    &mut order,
                    "project-settings-name",
                    &snapshot.settings.project_name,
                    ShellIntent::ProjectNameChanged,
                )?;
                let workspace = self.upsert_tagged_button(
                    context,
                    document_id,
                    "project-settings-workspace",
                    if snapshot.settings.project_workspace.is_empty() {
                        "选择工作区"
                    } else {
                        snapshot.settings.project_workspace.as_str()
                    },
                    ButtonKind::Subtle,
                    ShellIntent::PickProjectWorkspace,
                    false,
                )?;
                order.push(workspace.stable_id());
                let save = self.upsert_tagged_button(
                    context,
                    document_id,
                    "project-settings-save",
                    "保存项目",
                    ButtonKind::Primary,
                    ShellIntent::SaveProjectSettings,
                    false,
                )?;
                order.push(save.stable_id());
            }
            Some(ShellProjectPage::Roadmap) => {
                if self
                    .roadmap_view
                    .as_ref()
                    .is_some_and(|view| !view.belongs_to(&snapshot.roadmap))
                {
                    self.roadmap_view.take().unwrap().dispose(context)?;
                }
                if self.roadmap_view.is_none() {
                    let sink = Arc::clone(&self.sink);
                    self.roadmap_view = Some(crate::module::roadmap::view::RoadmapView::mount(
                        context,
                        document_id,
                        Arc::new(move |message| {
                            use crate::module::roadmap::RoadmapMessage;
                            let intent = match message {
                                RoadmapMessage::Select(id) => {
                                    ShellIntent::SelectRoadmapMilestone(id)
                                }
                                RoadmapMessage::TitleChanged(value) => {
                                    ShellIntent::MilestoneTitleChanged(value)
                                }
                                RoadmapMessage::DescriptionChanged(value) => {
                                    ShellIntent::MilestoneDescriptionChanged(value)
                                }
                                RoadmapMessage::DueDateChanged(value) => {
                                    ShellIntent::MilestoneDueDateChanged(value)
                                }
                                RoadmapMessage::Create => ShellIntent::CreateMilestone,
                                RoadmapMessage::Save => ShellIntent::SaveMilestone,
                                RoadmapMessage::CycleStatus => ShellIntent::CycleMilestoneStatus,
                                RoadmapMessage::Move(offset) => ShellIntent::MoveMilestone(offset),
                                RoadmapMessage::Delete => ShellIntent::DeleteMilestone,
                                RoadmapMessage::ToggleTask(id) => {
                                    ShellIntent::ToggleMilestoneTask(id)
                                }
                                _ => return,
                            };
                            emit(&sink, intent);
                        }),
                    )?);
                }
                let view = self.roadmap_view.as_mut().unwrap();
                view.sync(context, document_id, &snapshot.roadmap)?;
                order = vec![view.root.stable_id()];
            }
            Some(ShellProjectPage::Memory) => {
                if self
                    .memory_view
                    .as_ref()
                    .is_some_and(|view| !view.belongs_to(&snapshot.memory))
                {
                    self.memory_view.take().unwrap().dispose(context)?;
                }
                if self.memory_view.is_none() {
                    let sink = Arc::clone(&self.sink);
                    self.memory_view = Some(crate::module::memory::view::MemoryView::mount(
                        context,
                        document_id,
                        Arc::new(move |message| {
                            use crate::module::memory::MemoryMessage;
                            let intent = match message {
                                MemoryMessage::New => ShellIntent::NewMemory,
                                MemoryMessage::Select(id) => ShellIntent::SelectMemory(id),
                                MemoryMessage::TitleChanged(value) => {
                                    ShellIntent::MemoryTitleChanged(value)
                                }
                                MemoryMessage::BodyReplaced(value) => {
                                    ShellIntent::MemoryBodyChanged(value)
                                }
                                MemoryMessage::TagsChanged(value) => {
                                    ShellIntent::MemoryTagsChanged(value)
                                }
                                MemoryMessage::ToggleScope => ShellIntent::ToggleMemoryScope,
                                MemoryMessage::Save => ShellIntent::SaveMemory,
                                MemoryMessage::Delete => ShellIntent::DeleteMemory,
                                other => ShellIntent::MemoryAction(other),
                            };
                            emit(&sink, intent);
                        }),
                    )?);
                }
                let view = self.memory_view.as_mut().unwrap();
                view.sync(context, document_id, &snapshot.memory)?;
                order = vec![view.root.stable_id()];
            }
            Some(ShellProjectPage::Architecture) => {
                if self
                    .architecture_view
                    .as_ref()
                    .is_some_and(|view| !view.belongs_to(&snapshot.architecture))
                {
                    self.architecture_view.take().unwrap().dispose(context)?;
                }
                if self.architecture_view.is_none() {
                    let sink = Arc::clone(&self.sink);
                    self.architecture_view =
                        Some(crate::module::architecture::view::ArchitectureView::mount(
                            context,
                            document_id,
                            Arc::new(move |message| {
                                use crate::module::architecture::ArchitectureMessage;
                                let intent = match message {
                                    ArchitectureMessage::Refresh => {
                                        ShellIntent::RefreshArchitecture
                                    }
                                    ArchitectureMessage::Rollback => {
                                        ShellIntent::RollbackArchitecture
                                    }
                                    ArchitectureMessage::Graph(event) => {
                                        ShellIntent::ArchitectureGraph(event)
                                    }
                                    ArchitectureMessage::Open => return,
                                };
                                emit(&sink, intent);
                            }),
                        )?);
                }
                let view = self.architecture_view.as_mut().unwrap();
                view.sync(context, document_id, &snapshot.architecture)?;
                order = vec![view.root.stable_id()];
            }
            Some(ShellProjectPage::Sessions) => {
                if !snapshot.project_page_body.is_empty() {
                    order.push(self.project_page_body.stable_id());
                }
                if snapshot.session_page > 0 {
                    let previous = self.upsert_tagged_button(
                        context,
                        document_id,
                        "session-page-previous",
                        "上一页",
                        ButtonKind::Subtle,
                        ShellIntent::SessionPageChanged(-1),
                        false,
                    )?;
                    order.push(previous.stable_id());
                }
                if snapshot.session_page + 1 < snapshot.session_page_count {
                    let next = self.upsert_tagged_button(
                        context,
                        document_id,
                        "session-next",
                        "下一页",
                        ButtonKind::Subtle,
                        ShellIntent::SessionPageChanged(1),
                        false,
                    )?;
                    order.push(next.stable_id());
                }
            }
            _ => {
                if !snapshot.project_page_body.is_empty() {
                    order.push(self.project_page_body.stable_id());
                }
            }
        }
        let cards: Vec<(String, String, String, Option<ShellIntent>)> = match snapshot.project_page
        {
            Some(ShellProjectPage::Overview) => snapshot
                .project_cards
                .iter()
                .map(|card| {
                    (
                        format!("overview-{}", card.id),
                        card.title.clone(),
                        card.subtitle.clone(),
                        Some(ShellIntent::SelectProject(card.id.clone())),
                    )
                })
                .collect(),
            Some(ShellProjectPage::Sessions) => snapshot
                .session_cards
                .iter()
                .map(|row| {
                    (
                        format!("session-{}", row.id.as_str()),
                        row.title.clone(),
                        String::new(),
                        Some(ShellIntent::SelectTask(row.id.clone())),
                    )
                })
                .collect(),
            _ => Vec::new(),
        };
        for (id, title, _subtitle, intent) in cards {
            keep.insert(id.clone());
            let card = if let Some(card) = self.project_cards.get(&id).copied() {
                context.update_component(card, |button, _| {
                    button.label = title.clone();
                })?;
                card
            } else {
                let card = context.create_detached_component(
                    document_id,
                    Button::new(title.clone()).kind(ButtonKind::Subtle),
                )?;
                if let Some(intent) = intent {
                    bind_activate(context, card, Arc::clone(&self.sink), intent)?;
                }
                self.project_cards.insert(id.clone(), card);
                card
            };
            order.push(card.stable_id());
        }
        let stale: Vec<_> = self
            .project_cards
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(card) = self.project_cards.remove(&key) {
                let _ = context.remove_view(card);
            }
        }
        self.project_fields.retain(context, &field_keep)?;
        reconcile_children(context, self.project_page.stable_id(), &order)
    }

    fn sync_workspace_page(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let pane_kind = workspace_pane_kind(snapshot);
        let pane_id = snapshot
            .panes
            .iter()
            .find(|pane| pane.active)
            .or_else(|| snapshot.panes.first())
            .map(|pane| pane.id.as_str())
            .unwrap_or_default();
        *self.workspace_bindings.lock().unwrap() = PaneInputBindings::projected(
            pane_id,
            snapshot.document.as_ref(),
            snapshot.terminal.as_ref(),
        );
        self.workspace_search.sync(
            context,
            snapshot
                .document
                .as_ref()
                .map(|document| document.item_id.as_str()),
        )?;
        self.workspace_search.set_read_only(
            context,
            snapshot
                .document
                .as_ref()
                .is_some_and(|document| document.read_only),
        )?;
        let (title, status) = match pane_kind {
            Some("document-editor") => snapshot
                .document
                .as_ref()
                .map(|document| (document.title.clone(), document.status.clone()))
                .unwrap_or_default(),
            Some("terminal") => (
                "终端".to_owned(),
                snapshot
                    .terminal
                    .as_ref()
                    .and_then(|terminal| terminal.notice.clone())
                    .unwrap_or_default(),
            ),
            Some("project-files") => (
                "项目文件".to_owned(),
                snapshot
                    .files
                    .as_ref()
                    .and_then(|files| files.preview.clone())
                    .unwrap_or_default(),
            ),
            _ => Default::default(),
        };
        let show_heading = !title.is_empty() && pane_kind == Some("project-files");
        let show_status = !status.is_empty();
        context.update_component(self.workspace_heading, |text, _| {
            *text = Text::new(title);
        })?;
        context.update_component(self.workspace_status, |text, _| {
            *text = Text::new(status);
        })?;
        if let Some(document) = &snapshot.document {
            context.update_component(self.workspace_editor, |editor_view, _| {
                if editor_view.state.value != document.text {
                    editor_view.state.replace_value(document.text.clone());
                }
                editor_view.read_only = document.read_only;
                apply_workspace_editor_chrome(editor_view, Some(document.language.as_str()));
                editor_view.diagnostics = document
                    .diagnostics
                    .iter()
                    .filter_map(|diagnostic| diagnostic.editor_span(&document.text))
                    .collect::<Vec<_>>()
                    .into();
            })?;
        }
        if let Some(terminal) = &snapshot.terminal {
            if self.workspace_terminal_session.as_deref() != Some(&terminal.session_id) {
                context.update_component(self.workspace_log, |view, _| {
                    *view = nana_ui::runtime::TerminalView::new(terminal.screen.clone());
                })?;
                self.workspace_terminal_session = Some(terminal.session_id.clone());
            }
            context.update_component(self.workspace_log, |log, _| {
                log.read_only = !terminal.running;
            })?;
            context.sync_terminal_screen(self.workspace_log, terminal.screen.clone())?;
        }
        if let Some(files) = &snapshot.files {
            context.update_component(self.workspace_tree, |tree, _| {
                *tree = files.tree.clone();
            })?;
        } else {
            context.update_component(self.workspace_tree, |tree, _| {
                *tree = TreeView::new(Vec::new());
            })?;
        }
        if let Some(browser) = &snapshot.browser {
            self.workspace_browser.sync(
                context,
                ShellPaneTarget::primary(&pane_id, &browser.resource),
                browser,
            )?;
        }
        self.reconcile_workspace_actions(context, document_id, snapshot)?;
        let mut order = Vec::new();
        if show_heading {
            order.push(self.workspace_heading.stable_id());
        }
        if show_status {
            order.push(self.workspace_status.stable_id());
        }
        match pane_kind {
            Some("document-editor") => {
                if snapshot.document.is_some() {
                    order.push(self.workspace_search.root.stable_id());
                }
                order.push(self.workspace_editor.stable_id());
                order.push(self.workspace_actions.stable_id());
            }
            Some("project-files") => {
                order.push(self.workspace_tree.stable_id());
                order.push(self.workspace_actions.stable_id());
            }
            Some("task-browser") => {
                order.push(self.workspace_browser.root.stable_id());
            }
            Some("terminal") => {
                order.push(self.workspace_log.stable_id());
                order.push(self.workspace_actions.stable_id());
            }
            _ => {}
        }
        reconcile_children(context, self.workspace_content.stable_id(), &order)?;
        self.sync_live_workspace_panes(context, document_id, snapshot)
    }

    fn reconcile_workspace_actions(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let mut desired = Vec::new();
        let bindings = self.workspace_bindings.lock().unwrap().clone();
        match workspace_pane_kind(snapshot) {
            Some("document-editor") => {
                if let (Some(document), Some((target, _, _))) =
                    (&snapshot.document, &bindings.document)
                {
                    if document.dirty && !document.read_only {
                        desired.push((
                            "save",
                            if document.conflicted {
                                "保留并保存"
                            } else {
                                "保存"
                            },
                            ButtonKind::Primary,
                            ShellIntent::SaveDocument(target.clone()),
                        ));
                        desired.push((
                            "discard",
                            if document.conflicted {
                                "重新载入"
                            } else {
                                "放弃"
                            },
                            ButtonKind::Subtle,
                            ShellIntent::DiscardDocument(target.clone()),
                        ));
                    }
                }
            }
            Some("project-files") => {
                if snapshot.files.is_some() {
                    desired.push((
                        "refresh_files",
                        "刷新",
                        ButtonKind::Subtle,
                        ShellIntent::RefreshProjectFiles,
                    ));
                }
            }
            Some("terminal") => {
                if let Some((target, session_id)) = &bindings.terminal {
                    desired.push((
                        "terminal_interrupt",
                        "停止",
                        ButtonKind::Danger,
                        ShellIntent::TerminalInterrupt {
                            target: target.clone(),
                            session_id: session_id.clone(),
                        },
                    ));
                }
            }
            _ => {}
        }
        let mut keep = HashSet::new();
        let mut order = Vec::new();
        for (id, label, kind, intent) in desired {
            let id = format!("{id}:{intent:?}");
            keep.insert(id.clone());
            let button = if let Some(button) = self.workspace_buttons.get(&id).copied() {
                context.update_component(button, |button, _| {
                    *button = extra_button(label, kind);
                })?;
                button
            } else {
                let button =
                    context.create_detached_component(document_id, extra_button(label, kind))?;
                bind_activate(context, button, Arc::clone(&self.sink), intent)?;
                self.workspace_buttons.insert(id.to_owned(), button);
                button
            };
            order.push(button.stable_id());
        }
        let stale: Vec<_> = self
            .workspace_buttons
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(button) = self.workspace_buttons.remove(&key) {
                let _ = context.remove_view(button);
            }
        }
        reconcile_children(context, self.workspace_actions.stable_id(), &order)
    }

    fn sync_panes(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let (pane_selected, pane_options) = pane_tab_options(snapshot);
        context.update_component(self.pane_tabs, |tabs, _| {
            *tabs = Tabs::new(pane_selected)
                .options(pane_options)
                .strip_id(active_pane_strip_id(snapshot))
                .fill(true);
        })?;
        let mut keep = HashSet::new();
        let mut order = Vec::new();
        if snapshot.panes.len() > 1 && matches!(snapshot.pane_layout, ShellPaneLayout::Leaf(_)) {
            for pane in &snapshot.panes {
                let id = format!("pane-{}", pane.id);
                keep.insert(id.clone());
                let label = if pane.active {
                    format!("窗格 {}", pane.id)
                } else {
                    format!("切换 {}", pane.id)
                };
                let button = self.upsert_chrome_button(
                    context,
                    document_id,
                    &id,
                    &label,
                    if pane.active {
                        ButtonKind::Primary
                    } else {
                        ButtonKind::Subtle
                    },
                    ShellIntent::FocusWorkspacePane(pane.id.clone()),
                )?;
                order.push(button.stable_id());
            }
        }
        let stale: Vec<_> = self
            .pane_buttons
            .keys()
            .filter(|key| !key.starts_with("auto-") && !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(button) = self.pane_buttons.remove(&key) {
                let _ = context.remove_view(button);
            }
        }
        reconcile_children(context, self.pane_bar.stable_id(), &order)?;
        context.update_component(self.pane_chrome, |chrome, _| {
            let mut actions: Vec<PaneChromeAction> = chrome
                .actions
                .iter()
                .filter(|action| {
                    matches!(
                        action.kind,
                        PaneChromeActionKind::SplitHorizontal | PaneChromeActionKind::SplitVertical
                    )
                })
                .cloned()
                .collect();
            if snapshot.pane_can_move_window {
                actions.push(
                    PaneChromeAction::new(PaneChromeActionKind::MoveToWindow, "移至新窗口")
                        .target(self.pane_move_window.stable_id()),
                );
            }
            if snapshot.pane_can_move_next {
                actions.push(
                    PaneChromeAction::new(PaneChromeActionKind::MoveToNextPane, "移至下一窗格")
                        .target(self.pane_move_next.stable_id()),
                );
            }
            chrome.actions = actions;
        })?;
        assemble_workspace_chrome(context, self.pane_chrome)
    }

    fn sync_live_workspace_panes(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let leaf_ids = snapshot
            .pane_layout
            .leaf_ids()
            .into_iter()
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let primary_id = snapshot
            .panes
            .iter()
            .find(|pane| pane.active)
            .or_else(|| snapshot.panes.first())
            .map(|pane| pane.id.clone())
            .unwrap_or_else(|| leaf_ids.first().cloned().unwrap_or_default());
        for pane in &snapshot.panes {
            if pane.id == primary_id {
                continue;
            }
            if !self.extra_workspace_panes.contains_key(&pane.id) {
                let view = mount_workspace_pane_view(context, document_id, &pane.id, &self.sink)?;
                self.extra_workspace_panes.insert(pane.id.clone(), view);
            }
        }
        let stale: Vec<_> = self
            .extra_workspace_panes
            .keys()
            .filter(|id| snapshot.panes.iter().all(|pane| pane.id != **id))
            .cloned()
            .collect();
        for id in stale {
            if let Some(view) = self.extra_workspace_panes.remove(&id) {
                view.dispose(context)?;
            }
        }
        for pane in &snapshot.panes {
            if pane.id == primary_id {
                continue;
            }
            if let Some(view) = self.extra_workspace_panes.get(&pane.id).cloned() {
                self.sync_extra_workspace_pane(context, snapshot, pane, view)?;
            }
        }
        let terminal_layout = snapshot.pane_layout.filtered(&|id| {
            snapshot
                .panes
                .iter()
                .any(|pane| pane.id == id && pane_is_terminal(pane))
        });
        let resource_layout = snapshot.pane_layout.filtered(&|id| {
            snapshot
                .panes
                .iter()
                .any(|pane| pane.id == id && !pane_is_terminal(pane))
        });
        let mut keep_splits = HashSet::new();
        for layout in [&terminal_layout, &resource_layout].into_iter().flatten() {
            layout.split_keys(&mut keep_splits);
        }
        for (page, layout) in [
            (self.terminal_page, terminal_layout),
            (self.workspace_page, resource_layout),
        ] {
            let mut children = Vec::new();
            if let Some(layout) = layout {
                children.push(self.mount_pane_layout(
                    context,
                    document_id,
                    snapshot,
                    &layout,
                    &primary_id,
                )?);
                if leaf_ids.len() < 2 && page == self.workspace_page {
                    children.push(self.pane_bar.stable_id());
                }
            }
            reconcile_children(context, page.stable_id(), &children)?;
        }
        let stale = self
            .workspace_splits
            .keys()
            .filter(|key| !keep_splits.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            let split = self.workspace_splits.remove(&key).unwrap();
            let pane_roots = std::iter::once(self.pane_chrome.stable_id()).chain(
                self.extra_workspace_panes
                    .values()
                    .map(WorkspacePaneView::root),
            );
            for root in pane_roots {
                let mut ancestor = context.world().node(root).and_then(|node| node.parent);
                while let Some(node) = ancestor {
                    if node == split.stable_id() {
                        let mut mutations = nana_ui::runtime::MutationQueue::new();
                        mutations.park_subtree(root);
                        context.commit_mutations(mutations)?;
                        break;
                    }
                    ancestor = context.world().node(node).and_then(|node| node.parent);
                }
            }
            if context.world().contains(split.stable_id()) {
                context.remove_view(split)?;
            }
            if let Some(handle) = self.workspace_split_handles.remove(&key) {
                if context.world().contains(handle.stable_id()) {
                    context.remove_view(handle)?;
                }
            }
        }
        Ok(())
    }

    fn mount_pane_layout(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
        layout: &ShellPaneLayout,
        primary_id: &str,
    ) -> Result<StableNodeId, FrameworkError> {
        match layout {
            ShellPaneLayout::Leaf(id) => Ok(self.pane_chrome_for(id, primary_id)),
            ShellPaneLayout::Split {
                horizontal,
                ratio,
                first,
                second,
            } => {
                let first_id =
                    self.mount_pane_layout(context, document_id, snapshot, first, primary_id)?;
                let second_id =
                    self.mount_pane_layout(context, document_id, snapshot, second, primary_id)?;
                let key = format!("{}:{}", first.first_leaf(), second.first_leaf());
                let snapshot_ratio = *ratio;
                let extent = 800.0;
                let size = (extent * snapshot_ratio.clamp(0.15, 0.85)).max(80.0);
                let axis = if *horizontal {
                    SplitAxis::Horizontal
                } else {
                    SplitAxis::Vertical
                };
                let handle = if let Some(handle) = self.workspace_split_handles.get(&key).copied() {
                    handle
                } else {
                    let handle = context.create_detached_component(document_id, Stack::bar(0.0))?;
                    self.workspace_split_handles.insert(key.clone(), handle);
                    handle
                };
                let split = if let Some(split) = self.workspace_splits.get(&key).copied() {
                    context.update_component(split, |split, _| {
                        split.first = Some(first_id);
                        split.second = Some(second_id);
                        split.handle = Some(handle.stable_id());
                    })?;
                    split
                } else {
                    let split = context.create_detached_component(
                        document_id,
                        SplitPane::from_model(
                            &SplitPaneModel::new(axis, size, 80.0, 10_000.0),
                            first_id,
                            second_id,
                        )
                        .handle(handle.stable_id()),
                    )?;
                    self.workspace_splits.insert(key.clone(), split);
                    split
                };
                reconcile_children(
                    context,
                    split.stable_id(),
                    &[first_id, handle.stable_id(), second_id],
                )?;
                if let Ok(current) = context.read(split, |pane| pane.model.size()) {
                    let live_ratio = (current / extent).clamp(0.15, 0.85);
                    if (live_ratio - snapshot_ratio).abs() > 0.01 {
                        emit(
                            &self.sink,
                            ShellIntent::ResizeWorkspaceSplit {
                                first_pane_id: first.first_leaf().to_owned(),
                                second_pane_id: second.first_leaf().to_owned(),
                                ratio: live_ratio,
                            },
                        );
                    }
                }
                Ok(split.stable_id())
            }
        }
    }

    fn pane_chrome_for(&self, pane_id: &str, primary_id: &str) -> StableNodeId {
        if pane_id == primary_id {
            self.pane_chrome.stable_id()
        } else {
            self.extra_workspace_panes
                .get(pane_id)
                .map(|view| view.chrome.stable_id())
                .unwrap_or_else(|| self.pane_chrome.stable_id())
        }
    }

    fn sync_extra_workspace_pane(
        &mut self,
        context: &mut AppContext,
        snapshot: &PrimaryShellSnapshot,
        pane: &ShellPaneRow,
        mut view: WorkspacePaneView,
    ) -> Result<(), FrameworkError> {
        view.sync(
            context,
            HostedWindowId::PRIMARY,
            pane,
            snapshot.files.as_ref(),
            None,
        )?;
        self.extra_workspace_panes.insert(pane.id.clone(), view);
        Ok(())
    }

    fn upsert_chrome_button(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        id: &str,
        label: &str,
        kind: ButtonKind,
        intent: ShellIntent,
    ) -> Result<Entity<Button>, FrameworkError> {
        if let Some(button) = self.pane_buttons.get(id).copied() {
            context.update_component(button, |button, _| {
                *button = extra_button(label, kind);
            })?;
            Ok(button)
        } else {
            let button =
                context.create_detached_component(document_id, extra_button(label, kind))?;
            bind_activate(context, button, Arc::clone(&self.sink), intent)?;
            self.pane_buttons.insert(id.to_owned(), button);
            Ok(button)
        }
    }

    fn titlebar_more_items(
        snapshot: &PrimaryShellSnapshot,
    ) -> Vec<(&'static str, String, ShellIntent)> {
        let mut items = vec![
            (
                "more-palette",
                "命令面板".to_owned(),
                ShellIntent::ToggleCommandPalette,
            ),
            (
                "more-status",
                "会话状态".to_owned(),
                ShellIntent::OpenConversationStatus,
            ),
        ];
        if snapshot.titlebar_has_task {
            items.extend([
                (
                    "more-back",
                    "返回任务列表".to_owned(),
                    ShellIntent::BackToTaskList,
                ),
                (
                    "more-popup",
                    "在弹出窗口继续".to_owned(),
                    ShellIntent::OpenTaskPopup,
                ),
                (
                    "more-ask",
                    "在弹出窗口询问".to_owned(),
                    ShellIntent::AskTaskPopup,
                ),
                (
                    "more-inspector",
                    "会话详情".to_owned(),
                    ShellIntent::ToggleTaskInspector,
                ),
            ]);
        }
        if snapshot.titlebar_can_split {
            items.extend([
                (
                    "more-split-h",
                    "横向拆分".to_owned(),
                    ShellIntent::SplitWorkspaceHorizontal,
                ),
                (
                    "more-split-v",
                    "纵向拆分".to_owned(),
                    ShellIntent::SplitWorkspaceVertical,
                ),
            ]);
        }
        if snapshot.titlebar_can_close {
            items.push((
                "more-close",
                "关闭当前".to_owned(),
                ShellIntent::CloseCurrentWorkspaceItem,
            ));
        }
        items
    }

    /// 侧边栏菜单锚点来源：加项目菜单锚在区块加号按钮下方；行菜单优先用
    /// 右键光标点，否则锚在该行 more 按钮下方，向右下角展开。
    fn sidebar_menu_anchor_source(&self, snapshot: &PrimaryShellSnapshot) -> SidebarMenuAnchor {
        if snapshot.add_project_menu_open {
            return SidebarMenuAnchor::AddProjectButton(snapshot.sidebar_menu_anchor);
        }
        if let Some(anchor) = snapshot.sidebar_menu_anchor {
            return SidebarMenuAnchor::Point(anchor);
        }
        snapshot
            .sidebar_menu_owner
            .as_deref()
            .and_then(
                |owner| match self.row_tool_buttons.get(format!("{owner}-menu").as_str()) {
                    Some(RowToolButton::Tool(button)) => Some(button.stable_id()),
                    _ => None,
                },
            )
            .map_or(
                SidebarMenuAnchor::AddProjectButton(None),
                SidebarMenuAnchor::RowMenuButton,
            )
    }

    pub(crate) fn take_overlay_dismissals(&self, context: &AppContext) -> Vec<ShellIntent> {
        let pending = std::mem::take(
            &mut *self
                .pending_overlay_dismissals
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        );
        pending
            .into_iter()
            .filter_map(|root| self.dismissed_overlay_intent(context, root))
            .collect()
    }

    fn dismissed_overlay_intent(
        &self,
        context: &AppContext,
        root: StableNodeId,
    ) -> Option<ShellIntent> {
        let host = self.overlay_host?;
        let active = context.world().overlay_host(host.stable_id())?.active;
        let document = context.world().node(host.stable_id())?.document;
        if active.is_some_and(|active| active != root)
            || context
                .active_runtime_overlay(document)
                .is_some_and(|overlay| overlay.root == root)
        {
            return None;
        }
        if self.settings_view.extensions.dialog_id() == Some(root) {
            Some(ShellIntent::ExtensionsCommand(
                crate::shell::ExtensionsMessage::CancelEditor,
            ))
        } else if self.confirm.is_some_and(|view| view.stable_id() == root) {
            Some(ShellIntent::CancelDestructive)
        } else if self.palette.is_some_and(|view| view.stable_id() == root) {
            Some(ShellIntent::CommandPalette(CommandPaletteEvent::Dismiss))
        } else if self.more_menu.is_some_and(|view| view.stable_id() == root) {
            Some(ShellIntent::SidebarMenuAction(String::new()))
        } else if self
            .titlebar_menu
            .is_some_and(|view| view.stable_id() == root)
        {
            Some(ShellIntent::ToggleTitlebarMenu)
        } else if self
            .image_viewer
            .is_some_and(|view| view.stable_id() == root)
        {
            Some(ShellIntent::CloseMarkdownPreview)
        } else {
            None
        }
    }

    fn sync_overlay(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let Some(host) = self.overlay_host else {
            return Ok(());
        };
        if self.settings_view.extensions.sync_dialog(
            context,
            document_id,
            host,
            snapshot
                .settings
                .extensions
                .as_ref()
                .and_then(|extensions| extensions.editor.as_ref()),
            self.sink.clone(),
        )? {
            context.update_component(self.shell, |shell, _| {
                shell.overlays = self
                    .settings_view
                    .extensions
                    .dialog_id()
                    .into_iter()
                    .collect();
            })?;
            return Ok(());
        }
        if snapshot.confirm.is_none() {
            close_shell_overlay(context, host, &mut self.confirm)?;
            if self.confirm.is_none() {
                self.confirm_cancel = None;
                self.confirm_commit = None;
            }
        }
        if !snapshot.command_palette_open {
            close_shell_overlay(context, host, &mut self.palette)?;
            self.focus_targets.remove(target_ids::COMMAND_PALETTE_INPUT);
        }
        if snapshot.sidebar_menu.is_empty() {
            close_shell_overlay(context, host, &mut self.more_menu)?;
        }
        if !snapshot.titlebar_menu_open {
            close_shell_overlay(context, host, &mut self.titlebar_menu)?;
        }
        if snapshot.markdown_preview.is_none() {
            close_shell_overlay(context, host, &mut self.image_viewer)?;
        }
        if let Some(confirm) = &snapshot.confirm {
            let dialog = if let Some(dialog) = self.confirm {
                context.update_component(dialog, |view, _| {
                    view.title = confirm.title.clone().into();
                    view.message = confirm.message.clone().into();
                    view.danger = confirm.danger;
                    view.busy = confirm.busy;
                })?;
                dialog
            } else {
                let dialog = context.create_detached_component(
                    document_id,
                    ConfirmDialog::new(confirm.title.clone(), confirm.message.clone()),
                )?;
                let cancel = context.create_detached_component(
                    document_id,
                    extra_button(&confirm.cancel_label, ButtonKind::Subtle),
                )?;
                let commit = context.create_detached_component(
                    document_id,
                    extra_button(
                        &confirm.confirm_label,
                        if confirm.danger {
                            ButtonKind::Danger
                        } else {
                            ButtonKind::Primary
                        },
                    ),
                )?;
                context.set_confirm_slots(
                    dialog,
                    ConfirmSlots {
                        body: None,
                        close_action: None,
                        cancel: cancel.stable_id(),
                        secondary: None,
                        confirm: commit.stable_id(),
                    },
                )?;
                let sink = Arc::clone(&self.sink);
                context.on(dialog, move |_, intent: &ConfirmIntent, _| {
                    emit(
                        &sink,
                        match intent {
                            ConfirmIntent::Confirm { .. } => ShellIntent::ConfirmDestructive,
                            ConfirmIntent::Cancel | ConfirmIntent::Secondary => {
                                ShellIntent::CancelDestructive
                            }
                        },
                    );
                })?;
                context.append_child(host, dialog)?;
                self.confirm = Some(dialog);
                self.confirm_cancel = Some(cancel);
                self.confirm_commit = Some(commit);
                dialog
            };
            if let (Some(cancel), Some(commit)) = (self.confirm_cancel, self.confirm_commit) {
                context.update_component(cancel, |button, _| {
                    *button = extra_button(&confirm.cancel_label, ButtonKind::Subtle);
                    button.disabled = confirm.busy;
                })?;
                context.update_component(commit, |button, _| {
                    *button = extra_button(
                        &confirm.confirm_label,
                        if confirm.danger {
                            ButtonKind::Danger
                        } else {
                            ButtonKind::Primary
                        },
                    );
                    button.disabled = confirm.busy;
                })?;
            }
            let _ = context.set_confirm_state(dialog, confirm.busy, confirm.danger);
            context.update_component(self.shell, |shell, _| {
                shell.overlays = vec![dialog.stable_id()];
            })?;
            context.activate_overlay(host, dialog)?;
            return Ok(());
        }

        if snapshot.command_palette_open {
            let palette = if let Some(palette) = self.palette {
                context.update_component(palette, |view, _| {
                    *view = command_palette_view(snapshot);
                    view.selected = snapshot.command_palette_selected;
                })?;
                palette
            } else {
                let palette = context
                    .create_detached_component(document_id, command_palette_view(snapshot))?;
                let sink = Arc::clone(&self.sink);
                context.on(palette, move |_, event: &CommandPaletteEvent, _| {
                    emit(&sink, ShellIntent::CommandPalette(event.clone()));
                })?;
                context.append_child(host, palette)?;
                self.palette = Some(palette);
                palette
            };
            context.update_component(self.shell, |shell, _| {
                shell.overlays = vec![palette.stable_id()];
            })?;
            context.activate_overlay(host, palette)?;
            self.focus_targets.insert(
                target_ids::COMMAND_PALETTE_INPUT.to_owned(),
                palette.stable_id(),
            );
            return Ok(());
        }

        let mut overlays = Vec::new();
        if !snapshot.sidebar_menu.is_empty() {
            let items: Vec<ContextMenuItem> = snapshot
                .sidebar_menu
                .iter()
                .map(|item| ContextMenuItem::new(item.id.clone(), item.label.clone()))
                .collect();
            let anchor = match self.sidebar_menu_anchor_source(snapshot) {
                SidebarMenuAnchor::AddProjectButton(fallback) => {
                    overlay_anchor(context, self.add_project_menu.stable_id(), true, fallback)
                }
                SidebarMenuAnchor::RowMenuButton(button) => {
                    overlay_anchor(context, button, true, None)
                }
                SidebarMenuAnchor::Point(point) => point,
            };
            let menu = if let Some(menu) = self.more_menu {
                let view = sidebar_menu_view(context, host, anchor, items);
                context.update_component(menu, |slot, _| {
                    *slot = view;
                })?;
                menu
            } else {
                let view = sidebar_menu_view(context, host, anchor, items);
                let menu = context.create_detached_component(document_id, view)?;
                let sink = Arc::clone(&self.sink);
                context.on(menu, move |_, event: &ContextMenuEvent, _| match event {
                    ContextMenuEvent::Select(value) => {
                        emit(&sink, ShellIntent::SidebarMenuAction(value.to_string()));
                    }
                    ContextMenuEvent::Dismiss => {
                        emit(&sink, ShellIntent::SidebarMenuAction(String::new()));
                    }
                    ContextMenuEvent::Search(_) => {}
                })?;
                context.append_child(host, menu)?;
                self.more_menu = Some(menu);
                menu
            };
            overlays.push(menu.stable_id());
            context.activate_overlay(host, menu)?;
        }

        if snapshot.titlebar_menu_open {
            let items: Vec<ContextMenuItem> = Self::titlebar_more_items(snapshot)
                .into_iter()
                .map(|(id, label, _)| ContextMenuItem::new(id, label))
                .collect();
            let (anchor_x, anchor_y) =
                overlay_anchor(context, self.footer_more.stable_id(), false, None);
            let view = sidebar_menu_view(context, host, (anchor_x, anchor_y), items);
            let menu = if let Some(menu) = self.titlebar_menu {
                context.update_component(menu, |slot, _| {
                    *slot = view;
                })?;
                menu
            } else {
                let menu = context.create_detached_component(document_id, view)?;
                let sink = Arc::clone(&self.sink);
                context.on(menu, move |_, event: &ContextMenuEvent, _| match event {
                    ContextMenuEvent::Select(value) => {
                        emit(&sink, titlebar_menu_intent(value.as_ref()));
                    }
                    ContextMenuEvent::Dismiss => {
                        emit(&sink, ShellIntent::ToggleTitlebarMenu);
                    }
                    ContextMenuEvent::Search(_) => {}
                })?;
                context.append_child(host, menu)?;
                self.titlebar_menu = Some(menu);
                menu
            };
            overlays.push(menu.stable_id());
            context.activate_overlay(host, menu)?;
        }

        if let Some(preview) = &snapshot.markdown_preview {
            let viewer = if let Some(viewer) = self.image_viewer {
                context.update_component(viewer, |view, _| {
                    *view = markdown_image_viewer(preview);
                })?;
                viewer
            } else {
                let viewer = context
                    .create_detached_component(document_id, markdown_image_viewer(preview))?;
                let sink = Arc::clone(&self.sink);
                context.on(viewer, move |_, event: &ImageViewerEvent, _| match event {
                    ImageViewerEvent::Close | ImageViewerEvent::Outside => {
                        emit(&sink, ShellIntent::CloseMarkdownPreview);
                    }
                    ImageViewerEvent::Interaction => {
                        emit(&sink, ShellIntent::MarkdownImageViewerInteraction);
                    }
                })?;
                context.append_child(host, viewer)?;
                self.image_viewer = Some(viewer);
                viewer
            };
            overlays.push(viewer.stable_id());
            context.activate_overlay(host, viewer)?;
        }

        if let Some(active) = context
            .world()
            .overlay_host(host.stable_id())
            .and_then(|state| state.active)
            .filter(|active| !overlays.contains(active))
        {
            overlays.push(active);
        }
        context.update_component(self.shell, |shell, _| {
            shell.overlays = overlays;
        })?;
        Ok(())
    }

    fn sync_diagnostics(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        snapshot: &PrimaryShellSnapshot,
    ) -> Result<(), FrameworkError> {
        let diagnostics = snapshot
            .document
            .as_ref()
            .map(|document| document.diagnostics.as_slice())
            .unwrap_or(&[]);
        let mut keep = HashSet::new();
        let mut order = Vec::new();
        for (index, diagnostic) in diagnostics.iter().enumerate() {
            let id = format!("{index}");
            keep.insert(id.clone());
            let label = format!("{}  {}", diagnostic.severity_label(), diagnostic.message);
            let row = if let Some(row) = self.diagnostic_rows.get(&id).copied() {
                context.update_component(row, |text, _| {
                    *text = Text::new(label);
                })?;
                row
            } else {
                let row = context.create_detached_component(document_id, Text::new(label))?;
                self.diagnostic_rows.insert(id, row);
                row
            };
            order.push(row.stable_id());
        }
        let stale: Vec<_> = self
            .diagnostic_rows
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(row) = self.diagnostic_rows.remove(&key) {
                let _ = context.remove_view(row);
            }
        }
        reconcile_children(context, self.diagnostics_panel.stable_id(), &order)
    }
}

fn titlebar_menu_intent(id: &str) -> ShellIntent {
    match id {
        "more-palette" => ShellIntent::ToggleCommandPalette,
        "more-status" => ShellIntent::OpenConversationStatus,
        "more-back" => ShellIntent::BackToTaskList,
        "more-popup" => ShellIntent::OpenTaskPopup,
        "more-ask" => ShellIntent::AskTaskPopup,
        "more-inspector" => ShellIntent::ToggleTaskInspector,
        "more-split-h" => ShellIntent::SplitWorkspaceHorizontal,
        "more-split-v" => ShellIntent::SplitWorkspaceVertical,
        "more-close" => ShellIntent::CloseCurrentWorkspaceItem,
        _ => ShellIntent::ToggleTitlebarMenu,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarBucket {
    Session,
    Project,
    Inbox,
}

struct SidebarRowGroups {
    sessions: Vec<ShellSidebarRow>,
    projects: Vec<ShellSidebarRow>,
    inbox: Vec<ShellSidebarRow>,
    grouped: bool,
    inbox_expanded: bool,
}

fn sidebar_project_entry_count(rows: &[ShellSidebarRow]) -> usize {
    rows.iter()
        .filter(|row| {
            matches!(
                row.kind,
                ShellSidebarKind::Project | ShellSidebarKind::Archived
            )
        })
        .count()
}

fn sidebar_row_is_section_chrome(row: &ShellSidebarRow) -> bool {
    matches!(row.kind, ShellSidebarKind::Header | ShellSidebarKind::Inbox)
        || matches!(
            row.id.as_str(),
            "sessions-empty" | "projects-empty" | "inbox-empty"
        )
}

fn partition_sidebar_rows(snapshot: &PrimaryShellSnapshot) -> SidebarRowGroups {
    let grouped = !snapshot.sidebar_search_open
        && snapshot
            .sidebar_rows
            .iter()
            .any(|row| row.kind == ShellSidebarKind::Inbox || row.id == "projects-header");
    let inbox_expanded = snapshot
        .sidebar_rows
        .iter()
        .find(|row| row.kind == ShellSidebarKind::Inbox)
        .and_then(|row| row.expanded)
        .unwrap_or(true);
    if snapshot.sidebar_rows.is_empty() {
        return SidebarRowGroups {
            sessions: snapshot
                .tasks
                .iter()
                .map(|task| ShellSidebarRow {
                    id: task.id.as_str().to_owned(),
                    label: task.title.clone(),
                    kind: ShellSidebarKind::Task,
                    selected: task.selected,
                    ancestor: false,
                    depth: 0,
                    expanded: None,
                    can_stop: false,
                    stop_turn_id: None,
                    can_menu: true,
                    can_draft: false,
                })
                .collect(),
            projects: Vec::new(),
            inbox: Vec::new(),
            grouped: false,
            inbox_expanded,
        };
    }
    if snapshot.sidebar_search_open {
        return SidebarRowGroups {
            sessions: snapshot
                .sidebar_rows
                .iter()
                .filter(|row| !sidebar_row_is_section_chrome(row))
                .cloned()
                .collect(),
            projects: Vec::new(),
            inbox: Vec::new(),
            grouped: false,
            inbox_expanded,
        };
    }
    let mut sessions = Vec::new();
    let mut projects = Vec::new();
    let mut inbox = Vec::new();
    let mut bucket = SidebarBucket::Session;
    for row in &snapshot.sidebar_rows {
        if row.kind == ShellSidebarKind::Header && row.id == "projects-header" {
            bucket = SidebarBucket::Project;
            continue;
        }
        if row.kind == ShellSidebarKind::Inbox {
            bucket = SidebarBucket::Inbox;
            continue;
        }
        if sidebar_row_is_section_chrome(row) {
            continue;
        }
        let target = match row.kind {
            ShellSidebarKind::DropHint | ShellSidebarKind::Archived if grouped => {
                SidebarBucket::Project
            }
            _ => bucket,
        };
        match target {
            SidebarBucket::Session => sessions.push(row.clone()),
            SidebarBucket::Project => projects.push(row.clone()),
            SidebarBucket::Inbox => inbox.push(row.clone()),
        }
    }
    SidebarRowGroups {
        sessions,
        projects,
        inbox,
        grouped,
        inbox_expanded,
    }
}

fn mount_sidebar_section(
    context: &mut AppContext,
    document_id: DocumentId,
    title: &str,
    empty: Option<&str>,
    tool: Option<Entity<IconButton>>,
) -> Result<(Entity<SidebarSection>, Entity<ListItem>, Entity<List>), FrameworkError> {
    let mut spec = SidebarSection::new(title);
    if let Some(empty) = empty {
        spec = spec.empty_text(empty);
    }
    let title_label = context.create_detached_component(document_id, spec.title_label())?;
    spec = spec.title_slot(title_label.stable_id());
    let header = context.create_detached_component(document_id, spec.header_item())?;
    context.append_child(header, title_label)?;
    if let Some(tool) = tool {
        context.append_child(header, tool)?;
    }
    let body = context.create_detached_component(document_id, SidebarSection::body_port())?;
    let section = context.create_detached_component(
        document_id,
        spec.header(header.stable_id()).body(body.stable_id()),
    )?;
    context.append_child(section, header)?;
    context.append_child(section, body)?;
    Ok((section, header, body))
}

fn mount_sidebar_reorder(
    context: &mut AppContext,
    document_id: DocumentId,
    label: &str,
    tree_drop: bool,
    sink: IntentSink,
) -> Result<Entity<ReorderList>, FrameworkError> {
    let list = context.create_detached_component(
        document_id,
        ReorderList::new([])
            .size(ControlSize::Medium)
            .spacing(1.0)
            .tree_drop(tree_drop)
            .label(label),
    )?;
    context.on(list, move |_, event: &ReorderListEvent, _| {
        if let Some(intent) = sidebar_reorder_intent(event) {
            emit(&sink, intent);
        }
    })?;
    Ok(list)
}

fn sidebar_row_is_task(item: &ShellSidebarRow) -> bool {
    matches!(
        item.kind,
        ShellSidebarKind::Task | ShellSidebarKind::Running
    )
}

fn sidebar_reorder_item(item: &ShellSidebarRow, tools: Option<StableNodeId>) -> ReorderItem {
    let draggable = sidebar_row_is_task(item);
    let drop_target = draggable || item.kind == ShellSidebarKind::Project;
    let mut entry = ReorderItem::new(item.id.clone(), item.label.clone())
        .draggable(draggable)
        .drop_target(drop_target)
        .selected(item.selected);
    if let Some(tools) = tools {
        entry = entry.tools(tools);
    }
    entry
}

fn sidebar_reorder_intent(event: &ReorderListEvent) -> Option<ShellIntent> {
    match event {
        ReorderListEvent::Reorder { source, before } => Some(ShellIntent::ReorderSidebar {
            source: source.to_string(),
            before: before.as_ref().map(|value| value.to_string()),
        }),
        ReorderListEvent::Secondary { source, x, y } => Some(ShellIntent::OpenRowMenu {
            id: source.to_string(),
            anchor: (*x, *y),
        }),
        ReorderListEvent::TreeDrop { source, intent } => Some(ShellIntent::SidebarTreeDrop {
            source: source.to_string(),
            target: intent.target.to_string(),
            position: match intent.position {
                TreeDropPosition::Before => SidebarDropPosition::Before,
                TreeDropPosition::Inside => SidebarDropPosition::Inside,
                TreeDropPosition::After => SidebarDropPosition::After,
            },
        }),
        ReorderListEvent::Select(_) | ReorderListEvent::Cancelled => None,
    }
}

/// 行右键菜单与行内 more 按钮同源：项目类行弹项目菜单，会话类行弹任务
/// 菜单；`anchor` 为右键光标点，菜单从该点向右下角展开。
fn sidebar_row_menu_intent(
    kind: ShellSidebarKind,
    id: &str,
    anchor: (f32, f32),
) -> Option<ShellIntent> {
    match kind {
        ShellSidebarKind::Project | ShellSidebarKind::SearchProject => {
            Some(ShellIntent::OpenProjectMenu {
                id: id.to_owned(),
                anchor: Some(anchor),
            })
        }
        ShellSidebarKind::Task | ShellSidebarKind::SearchTask | ShellSidebarKind::Running => {
            TaskId::new(id).ok().map(|_| ShellIntent::OpenTaskMenu {
                id: id.to_owned(),
                anchor: Some(anchor),
            })
        }
        _ => None,
    }
}

fn sidebar_row_intent(row: &ShellSidebarRow) -> Option<ShellIntent> {
    match row.kind {
        ShellSidebarKind::Header if row.id == "projects-header" => {
            Some(ShellIntent::OpenProjectsOverview)
        }
        ShellSidebarKind::Header => None,
        ShellSidebarKind::DropHint => None,
        ShellSidebarKind::Empty => None,
        ShellSidebarKind::Project | ShellSidebarKind::SearchProject => {
            Some(ShellIntent::SelectProject(row.id.clone()))
        }
        ShellSidebarKind::Task | ShellSidebarKind::SearchTask | ShellSidebarKind::Running => {
            TaskId::new(&row.id).ok().map(ShellIntent::SelectTask)
        }
        ShellSidebarKind::Inbox => Some(ShellIntent::ToggleSidebarInbox),
        ShellSidebarKind::Reveal if row.id == "inbox-reveal" => {
            Some(ShellIntent::RevealSidebarInbox)
        }
        ShellSidebarKind::Reveal => Some(ShellIntent::RevealSidebarProject(
            row.id.strip_prefix("reveal-").unwrap_or(&row.id).to_owned(),
        )),
        ShellSidebarKind::Archived => Some(ShellIntent::RestoreProject(row.id.clone())),
    }
}

/// A projection with every field at rest.
///
/// Lives outside the test module because the UI module tests assert on which
/// fields a module writes, and that only means something against a baseline
/// where nothing is set. Not a `Default` impl: `SettingsModel` is a NanaUI type
/// and giving it a default is not this crate's call.
#[cfg(test)]
pub(crate) fn empty_snapshot() -> PrimaryShellSnapshot {
    PrimaryShellSnapshot {
        theme: ThemeMode::Light,
        title_parent: "LiliaCode".to_owned(),
        title_context: "今天想做什么？".to_owned(),
        heading: "今天想做什么？".to_owned(),
        error: None,
        navigation: WindowRoute::Workspace,
        sidebar_collapsed: false,
        sidebar_search_open: false,
        sidebar_search_query: String::new(),
        provider_badge: "未连接".to_owned(),
        provider_badge_icon: Icon::Cpu,
        nav_items: Vec::new(),
        sidebar_rows: Vec::new(),
        sidebar_menu: Vec::new(),
        sidebar_menu_anchor: None,
        sidebar_menu_owner: None,
        add_project_menu_open: false,
        workspace: WorkspaceModel::new(),
        tasks: Vec::new(),
        timeline: crate::module::timeline::view::TimelineViewSnapshot {
            target: crate::module::timeline::view::TimelineTarget {
                window_id: HostedWindowId::PRIMARY,
                task_id: None,
            },
            rows: Vec::new(),
            layout: VirtualListLayout::default(),
            scroll_offset: 0.0,
            viewport_extent: TIMELINE_DEFAULT_VIEWPORT_EXTENT,
            can_load_earlier: false,
        },
        clone_repository: String::new(),
        clone_parent: String::new(),
        roadmap: Default::default(),
        command_palette_open: false,
        command_palette_query: String::new(),
        command_palette_selected: 0,
        command_palette_items: Vec::new(),
        settings: {
            let model = SettingsModel::new(
                "appearance",
                [nana_ui::SettingsTab::new("appearance", "外观")],
            )
            .expect("settings model");
            let state = SettingsState::new(&model);
            SettingsSnapshot {
                model,
                state,
                appearance: AppearanceSettings::default(),
                material_status: String::new(),
                project_name: String::new(),
                project_workspace: String::new(),
                project_error: None,
                providers: Vec::new(),
                provider_status: String::new(),
                agent_actions: Vec::new(),
                quota_status: String::new(),
                extensions_status: String::new(),
                extensions_search: String::new(),
                extensions: None,
                remote_status: String::new(),
                remote_host_enabled: false,
                remote_keep_awake: false,
                remote_pc_name: String::new(),
                remote_pairing_active: false,
                remote_pairing_uri: String::new(),
                remote_devices: Vec::new(),
                project_clone_parent: String::new(),
                project_worktree_mode: String::new(),
                project_worktree_parent: String::new(),
                project_worktree_instructions: String::new(),
                project_cleanup_on_archive: false,
                desktop_status: String::new(),
                data_status: String::new(),
                data_can_import: false,
                provider_secret: String::new(),
                provider_model: String::new(),
                provider_openai_endpoint: String::new(),
                provider_anthropic_endpoint: String::new(),
                can_save_credential: false,
                credentials: Vec::new(),
                custom_agents: Vec::new(),
                custom_agent_editor_open: false,
                custom_agent_name: String::new(),
                custom_agent_description: String::new(),
                custom_agent_instruction: String::new(),
                quota_days_label: String::new(),
                quota_backend_label: String::new(),
                quota_values: Vec::new(),
                quota_axis_labels: Vec::new(),
                quota_daily: Vec::new(),
                quota_project_slices: Vec::new(),
                quota_conversation_slices: Vec::new(),
                quota_tool_slices: Vec::new(),
                skills: Vec::new(),
                skill_id: String::new(),
                skill_description: String::new(),
                can_create_skill: false,
                mcp_servers: Vec::new(),
                mcp_editor: None,
                github_state: String::new(),
                github_login: String::new(),
                github_busy: false,
                github_can_bind: false,
                shortcut: String::new(),
                shortcut_capturing: false,
                shortcut_registered: false,
                sidebar_display_mode: "grouped".to_owned(),
            }
        },
        document: None,
        files: None,
        terminal: None,
        browser: None,
        markdown_preview: None,
        inspector_title: String::new(),
        inspector_body: String::new(),
        inspector_todos: Vec::new(),
        todo_panel: Default::default(),
        confirm: None,
        pending: None,
        project_page: None,
        project_page_title: String::new(),
        project_page_body: String::new(),
        project_cards: Vec::new(),
        session_search: String::new(),
        session_page: 0,
        session_page_count: 1,
        session_cards: Vec::new(),
        memory: Default::default(),
        architecture: Default::default(),
        inspector_kind: String::new(),
        coding: None,
        pane_can_move_window: false,
        pane_can_move_next: false,
        titlebar_menu_open: false,
        titlebar_has_task: false,
        titlebar_can_split: false,
        titlebar_can_close: false,
        automation: crate::module::automation::view::AutomationViewSnapshot::default(),
        panes: Vec::new(),
        pane_layout: ShellPaneLayout::default(),
        composer: crate::module::composer::view::ComposerViewSnapshot {
            window_id: HostedWindowId::PRIMARY,
            can_open_browser: false,
            composer: String::new(),
            composer_atom_spans: Vec::new(),
            composer_task_id: None,
            composer_revision: 0,
            composer_height: COMPOSER_MIN_HEIGHT,
            composer_placeholder: "输入消息".to_owned(),
            composer_disabled: true,
            can_send: false,
            can_interrupt: false,
            composer_turn_id: None,
            pending_blocks_send: false,
            attachments: Vec::new(),
            plan_mode: false,
            goal_mode: false,
            permission_label: "询问".to_owned(),
            permission_selection: "ask".to_owned(),
            reasoning: "medium".to_owned(),
            model: String::new(),
            model_label: "自动选择".to_owned(),
            models: vec![
                (String::new(), "自动选择".to_owned()),
                ("native-debug".to_owned(), "Native Debug".to_owned()),
            ],
            worktree_label: None,
            worktree_selection: "current".to_owned(),
            suggestions: Vec::new(),
            suggestions_can_refresh: false,
            slash_items: Vec::new(),
            mention_items: Vec::new(),
            reference_items: Vec::new(),
            composer_plus_open: false,
            composer_permission_menu_open: false,
            composer_worktree_menu_open: false,
            branch_label: None,
            review_target: None,
            review_value: String::new(),
            can_manage_todos: false,
            apply_failed: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_layout::COMPOSER_CARD_RADIUS;

    fn snapshot_with_empty_primary_pane() -> PrimaryShellSnapshot {
        let mut snapshot = empty_snapshot();
        snapshot.panes = vec![ShellPaneRow {
            id: "primary".to_owned(),
            active: true,
            items: Vec::new(),
            document: None,
            terminal: None,
            browser: None,
        }];
        snapshot.pane_layout = ShellPaneLayout::Leaf("primary".to_owned());
        snapshot
    }

    fn test_sidebar_row(id: &str, label: &str, kind: ShellSidebarKind) -> ShellSidebarRow {
        ShellSidebarRow {
            id: id.to_owned(),
            label: label.to_owned(),
            kind,
            selected: false,
            ancestor: false,
            depth: 0,
            expanded: None,
            can_stop: false,
            stop_turn_id: None,
            can_menu: false,
            can_draft: false,
        }
    }

    #[test]
    fn composer_input_binding_keeps_window_task_base_text_and_revision_for_queued_edits() {
        let mut binding = ComposerBinding::new(
            nana_ui_platform::WindowId(42),
            Some("task-a".to_owned()),
            5,
            "a".to_owned(),
            Some("turn-a".into()),
        );
        assert!(binding.edit("a".to_owned()).is_none());
        for (before, value, revision) in [("a", "ab", 5), ("ab", "abc", 6), ("abc", "", 7)] {
            match binding.edit(value.to_owned()).unwrap() {
                ShellIntent::AddressedComposer {
                    target,
                    action:
                        ComposerInputAction::SetContent {
                            value: actual,
                            expected_content,
                        },
                } => {
                    assert_eq!(target.window_id, nana_ui_platform::WindowId(42));
                    assert_eq!(target.turn_id.as_deref(), Some("turn-a"));
                    assert_eq!(target.task_id.as_deref(), Some("task-a"));
                    assert_eq!(target.revision, revision);
                    assert_eq!(expected_content, before);
                    assert_eq!(actual, value);
                }
                _ => panic!("unexpected composer intent"),
            }
        }
        assert_eq!(binding.target.revision, 8);
    }

    #[test]
    fn queued_stop_keeps_its_turn_when_the_same_task_binding_advances() {
        let mut binding = ComposerBinding::new(
            HostedWindowId::PRIMARY,
            Some("task".into()),
            2,
            "draft".into(),
            Some("turn-a".into()),
        );
        let queued = ShellIntent::AddressedComposer {
            target: binding.target.clone(),
            action: ComposerInputAction::Interrupt,
        };
        binding.edit("new draft".into()).unwrap();
        assert_eq!(binding.target.turn_id.as_deref(), Some("turn-a"));
        binding = ComposerBinding::new(
            HostedWindowId::PRIMARY,
            Some("task".into()),
            3,
            "new draft".into(),
            Some("turn-b".into()),
        );
        assert_eq!(binding.target.turn_id.as_deref(), Some("turn-b"));
        let ShellIntent::AddressedComposer {
            target,
            action: ComposerInputAction::Interrupt,
        } = queued
        else {
            panic!("stop intent");
        };
        assert_eq!(target.turn_id.as_deref(), Some("turn-a"));
    }

    #[test]
    fn pending_conflict_draft_edits_keep_the_canonical_revision() {
        let target = ShellPaneTarget::primary("secondary", "document-b");
        let mut bindings = PaneInputBindings {
            conflicted: true,
            document: Some((target.clone(), 7, "draft".to_owned())),
            terminal: None,
        };
        for value in ["draft a", "draft ab", ""] {
            match bindings.edit(value.to_owned()).unwrap() {
                ShellIntent::DocumentChanged {
                    target: actual,
                    revision,
                    value: actual_value,
                } => {
                    assert_eq!(actual, target);
                    assert_eq!(revision, 7);
                    assert_eq!(actual_value, value);
                }
                _ => panic!("editor emitted a different action"),
            }
        }
    }

    #[test]
    fn pane_editor_events_keep_identity_and_advance_only_for_actual_edits() {
        let target = ShellPaneTarget::primary("secondary", "document-b");
        let mut bindings = PaneInputBindings {
            conflicted: false,
            document: Some((target.clone(), 7, "a".to_owned())),
            terminal: None,
        };
        assert!(bindings.edit("a".to_owned()).is_none());
        for (value, expected) in [("ab", 7), ("abc", 8), ("", 9)] {
            match bindings.edit(value.to_owned()).unwrap() {
                ShellIntent::DocumentChanged {
                    target: actual,
                    revision,
                    value: actual_value,
                } => {
                    assert_eq!(actual, target);
                    assert_eq!(revision, expected);
                    assert_eq!(actual_value, value);
                }
                _ => panic!("editor emitted a different action"),
            }
        }
        assert!(bindings.edit(String::new()).is_none());
        bindings.document = None;
        assert!(bindings.edit("late event".to_owned()).is_none());
    }

    #[test]
    fn project_and_search_rows_both_activate_the_project() {
        let project = test_sidebar_row("project-lilia", "LiliaCode", ShellSidebarKind::Project);
        let search = test_sidebar_row(
            "project-lilia",
            "LiliaCode",
            ShellSidebarKind::SearchProject,
        );
        for row in [project, search] {
            assert!(matches!(
                sidebar_row_intent(&row),
                Some(ShellIntent::SelectProject(id)) if id == "project-lilia"
            ));
        }
    }

    fn section_row_ids(
        document: &nana_ui::runtime::RuntimeDocument,
        body: StableNodeId,
    ) -> Vec<StableNodeId> {
        let children = document
            .context()
            .world()
            .node(body)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        if children.len() == 1 && document.context().is_reorder_list(children[0]) {
            return document
                .context()
                .world()
                .node(children[0])
                .map(|node| node.children.clone())
                .unwrap_or_default();
        }
        children
    }

    fn mounted_primary(
        snapshot: &PrimaryShellSnapshot,
    ) -> (
        nana_ui::runtime::RuntimeDocument,
        ShellHandles,
        Option<StableNodeId>,
    ) {
        let (mut document, mut handles) =
            mount_primary_shell(snapshot, Arc::new(|_| {})).expect("mount shell");
        handles.sync(&mut document, snapshot).expect("sync shell");
        let primary = document
            .context_mut()
            .read(handles.shell, |shell| shell.primary)
            .expect("read shell primary");
        (document, handles, primary)
    }

    /// NanaUI SplitPane wraps each pane body in a split-owned `split-pane-slot`
    /// shell and places the resize handle between the shells; the host content
    /// sits one level inside the shells.
    fn split_pane_content_children(
        document: &nana_ui::runtime::RuntimeDocument,
        pane: StableNodeId,
    ) -> Vec<StableNodeId> {
        let world = document.context().world();
        world
            .node(pane)
            .map(|node| node.children.clone())
            .unwrap_or_default()
            .into_iter()
            .flat_map(|child| match world.node(child).map(|node| node.kind) {
                Some(nana_ui::runtime::NodeKind::Element { tag }) if tag == "split-pane-slot" => {
                    world
                        .node(child)
                        .map(|node| node.children.clone())
                        .unwrap_or_default()
                }
                Some(nana_ui::runtime::NodeKind::Element { tag }) if tag == "split-handle" => {
                    Vec::new()
                }
                _ => vec![child],
            })
            .collect()
    }

    fn assert_conversation_beside_workspace(
        document: &nana_ui::runtime::RuntimeDocument,
        handles: &ShellHandles,
        primary: Option<StableNodeId>,
    ) {
        assert_eq!(primary, Some(handles.conversation_workspace.stable_id()));
        let children =
            split_pane_content_children(document, handles.conversation_workspace.stable_id());
        assert_eq!(
            children.first().copied(),
            Some(handles.conversation.stable_id())
        );
        assert_eq!(
            children.last().copied(),
            Some(handles.workspace_page.stable_id())
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.composer_view.composer.stable_id())
                .and_then(|node| node.parent),
            Some(handles.task_view.composer_view.composer_dock.stable_id())
        );
    }

    #[test]
    fn confirm_exit_retains_slots_and_reopening_cancels_pending_release() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        let confirm = ShellConfirm {
            kind: ShellConfirmKind::RemoveProject,
            title: "Delete".into(),
            message: "Remove this item?".into(),
            cancel_label: "Cancel".into(),
            confirm_label: "Delete".into(),
            danger: true,
            busy: false,
        };
        snapshot.confirm = Some(confirm.clone());
        let (mut document, mut handles, _) = mounted_primary(&snapshot);
        let dialog = handles.confirm.unwrap();
        let cancel = handles.confirm_cancel.unwrap();
        let host = handles.overlay_host.unwrap();
        document
            .context_mut()
            .advance_animations(std::time::Duration::from_millis(140));
        snapshot.confirm = None;
        handles.sync(&mut document, &snapshot).unwrap();
        assert!(document.context().world().is_mounted(dialog.stable_id()));
        assert!(document.context().world().is_mounted(cancel.stable_id()));
        assert!(
            document
                .context()
                .active_runtime_overlay(document.document())
                .is_none()
        );
        document
            .context_mut()
            .advance_animations(std::time::Duration::from_millis(200));
        snapshot.confirm = Some(confirm);
        handles.sync(&mut document, &snapshot).unwrap();
        assert_eq!(handles.confirm.unwrap(), dialog);
        document
            .context_mut()
            .advance_animations(std::time::Duration::from_millis(340));
        handles.sync(&mut document, &snapshot).unwrap();
        assert_eq!(
            document
                .context()
                .world()
                .overlay_host(host.stable_id())
                .unwrap()
                .active,
            Some(dialog.stable_id())
        );
        snapshot.confirm = None;
        handles.sync(&mut document, &snapshot).unwrap();
        document
            .context_mut()
            .advance_animations(std::time::Duration::from_millis(480));
        handles.sync(&mut document, &snapshot).unwrap();
        assert!(handles.confirm.is_none());
        assert!(handles.confirm_cancel.is_none());
        assert!(!document.context().world().contains(dialog.stable_id()));
        assert!(!document.context().world().contains(cancel.stable_id()));
    }

    #[test]
    fn shell_menu_escape_updates_business_presence_and_ignores_a_stale_close_after_reopen() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.titlebar_menu_open = true;
        let (mut document, mut handles, _) = mounted_primary(&snapshot);
        let menu = handles.titlebar_menu.unwrap();
        let doc = document.document();
        document
            .context_mut()
            .advance_animations(std::time::Duration::from_millis(180));
        assert!(
            document
                .context_mut()
                .route_overlay_key(doc, nana_ui::runtime::OverlayKey::Escape)
                .unwrap()
        );
        assert!(matches!(
            handles
                .take_overlay_dismissals(document.context())
                .as_slice(),
            [ShellIntent::ToggleTitlebarMenu]
        ));
        assert!(
            handles
                .take_overlay_dismissals(document.context())
                .is_empty()
        );
        snapshot.titlebar_menu_open = false;
        handles.sync(&mut document, &snapshot).unwrap();
        document
            .context_mut()
            .advance_animations(std::time::Duration::from_millis(200));
        snapshot.titlebar_menu_open = true;
        handles.sync(&mut document, &snapshot).unwrap();
        assert!(
            handles
                .dismissed_overlay_intent(document.context(), menu.stable_id())
                .is_none()
        );
        document
            .context_mut()
            .advance_animations(std::time::Duration::from_millis(380));
        handles.sync(&mut document, &snapshot).unwrap();
        assert_eq!(handles.titlebar_menu.unwrap(), menu);
        assert!(document.context().active_runtime_overlay(doc).is_some());
    }

    #[test]
    fn shell_menu_exit_notifies_the_host_before_releasing_its_retained_tree() {
        for sidebar in [true, false] {
            let mut snapshot = snapshot_with_empty_primary_pane();
            if sidebar {
                snapshot.sidebar_menu = vec![ShellMenuItem {
                    id: "open".into(),
                    label: "Open".into(),
                }];
            } else {
                snapshot.titlebar_menu_open = true;
            }
            let events = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&events);
            let (mut document, mut handles) = mount_primary_shell(
                &snapshot,
                Arc::new(move |event| sink.lock().unwrap().push(event)),
            )
            .unwrap();
            handles.sync(&mut document, &snapshot).unwrap();
            assert!(
                document
                    .context()
                    .world()
                    .is_mounted(handles.footer_more.stable_id())
            );
            let menu = if sidebar {
                handles.more_menu.unwrap()
            } else {
                handles.titlebar_menu.unwrap()
            };
            document
                .context_mut()
                .advance_animations(std::time::Duration::from_millis(180));
            events.lock().unwrap().clear();
            snapshot.sidebar_menu.clear();
            snapshot.titlebar_menu_open = false;
            handles.sync(&mut document, &snapshot).unwrap();
            assert!(
                events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|event| matches!(event, ShellIntent::OverlayPresenceChanged))
            );
            events.lock().unwrap().clear();
            document
                .context_mut()
                .advance_animations(std::time::Duration::from_millis(359));
            assert!(document.context().world().is_mounted(menu.stable_id()));
            assert!(events.lock().unwrap().is_empty());
            document
                .context_mut()
                .advance_animations(std::time::Duration::from_millis(360));
            assert!(
                events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|event| matches!(event, ShellIntent::OverlayPresenceChanged))
            );
            handles.sync(&mut document, &snapshot).unwrap();
            assert!(!document.context().world().contains(menu.stable_id()));
        }
    }

    #[test]
    fn mounts_a_primary_shell_document() {
        let (document, _handles) =
            mount_primary_shell(&empty_snapshot(), Arc::new(|_| {})).expect("mount shell");
        assert_eq!(document.document(), DocumentId::new(1).unwrap());
    }

    #[test]
    fn default_empty_layout_selects_conversation_primary() {
        let (document, handles, primary) = mounted_primary(&snapshot_with_empty_primary_pane());
        assert_eq!(primary, Some(handles.conversation.stable_id()));
        assert_ne!(primary, Some(handles.workspace_page.stable_id()));

        let timeline = handles.task_view.timeline_view.root.stable_id();
        let heading = handles.task_view.heading_slot.stable_id();
        let body = document
            .context()
            .world()
            .node(timeline)
            .and_then(|node| node.parent)
            .expect("conversation body");
        assert_eq!(
            document
                .context()
                .world()
                .node(heading)
                .and_then(|node| node.parent),
            Some(body)
        );
        let body_layout = &document
            .context()
            .world()
            .node_style(body)
            .expect("conversation body style")
            .layout;
        assert_eq!(body_layout.flex_grow, Some(1.0));
        assert_eq!(
            body_layout.min_height,
            Some(nana_ui::runtime::LengthSpec::Px(0.0))
        );

        let extras = handles.task_view.composer_view.extras.stable_id();
        let send = handles.task_view.composer_view.send.stable_id();
        let dock = document
            .context()
            .world()
            .node(handles.task_view.composer_view.composer.stable_id())
            .and_then(|node| node.parent)
            .expect("composer dock");
        assert_eq!(
            document
                .context()
                .world()
                .node(extras)
                .and_then(|node| node.parent),
            Some(handles.task_view.composer_view.composer_toolbar.stable_id())
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(send)
                .and_then(|node| node.parent),
            Some(handles.task_view.composer_view.composer_actions.stable_id())
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.composer_view.composer_toolbar.stable_id())
                .and_then(|node| node.parent),
            Some(dock)
        );
        let dock_layout = &document
            .context()
            .world()
            .node_style(dock)
            .expect("composer dock style")
            .layout;
        assert_eq!(dock_layout.flex_grow.unwrap_or_default(), 0.0);
        assert_eq!(
            dock_layout
                .height
                .unwrap_or(nana_ui::runtime::LengthSpec::Shrink),
            nana_ui::runtime::LengthSpec::Shrink
        );
        assert_eq!(dock_layout.border_radius, Some(COMPOSER_CARD_RADIUS));
        assert_eq!(
            document
                .context()
                .world()
                .node_style(dock)
                .expect("composer dock style")
                .background,
            Some(nana_ui::runtime::SemanticColorRole::Surface)
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.conversation.stable_id())
                .map(|node| node.children),
            Some(vec![handles.task_view.conversation_column.stable_id()])
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.conversation_column.stable_id())
                .map(|node| node.children),
            Some(vec![body, dock])
        );
        assert_eq!(
            dock,
            handles.task_view.composer_view.composer_dock.stable_id()
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.pending_view.root.stable_id())
                .and_then(|node| node.parent),
            None
        );
    }

    #[test]
    fn pending_interaction_sits_above_composer_not_inside_it() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.pending = Some(ShellPending {
            stop_target: None,
            request_id: "pending-1".to_owned(),
            kind: ShellPendingKind::PermissionApproval,
            title: "允许读取文件".to_owned(),
            prompt: "Agent 想读取 src/lib.rs".to_owned(),
            draft: String::new(),
            options: Vec::new(),
            tool: None,
            ask: None,
            mcp: None,
        });
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let body = handles.task_view.conversation_body.stable_id();
        let pending = handles.task_view.pending_view.root.stable_id();
        let dock = handles.task_view.composer_view.composer_dock.stable_id();
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.conversation_column.stable_id())
                .map(|node| node.children),
            Some(vec![body, pending])
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(dock)
                .and_then(|node| node.parent),
            None
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.composer_view.composer.stable_id())
                .and_then(|node| node.parent),
            Some(dock)
        );
        assert!(
            document
                .context()
                .world()
                .node(pending)
                .map(|node| handles
                    .task_view
                    .pending_view
                    .actions()
                    .is_some_and(|actions| node.children.contains(&actions)))
                .unwrap_or(false)
        );
        assert_eq!(
            handles
                .focus_targets
                .get(target_ids::TASK_SESSION_PENDING)
                .copied(),
            Some(pending)
        );
    }

    #[test]
    fn inspector_header_owns_close_control() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.inspector_title = "任务".to_owned();
        snapshot.inspector_body = "进行中".to_owned();
        let (document, handles, _primary) = mounted_primary(&snapshot);
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.inspector_header.stable_id())
                .map(|node| node.children),
            Some(vec![
                handles.inspector_heading.stable_id(),
                handles.inspector_close.stable_id()
            ])
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.inspector.stable_id())
                .map(|node| node.children.first().copied()),
            Some(Some(handles.inspector_header.stable_id()))
        );
        assert_eq!(
            handles
                .focus_targets
                .get(target_ids::TASK_SESSION_INSPECTOR_CLOSE)
                .copied(),
            Some(handles.inspector_close.stable_id())
        );
    }

    #[test]
    fn iab_inspector_shows_unavailable_state_without_browse_actions() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.inspector_title = "浏览器".to_owned();
        snapshot.inspector_kind = "iab".to_owned();
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let inspector = document
            .context()
            .world()
            .node(handles.inspector.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert!(inspector.contains(&handles.inspector_header.stable_id()));
        assert!(inspector.contains(&handles.iab_empty.stable_id()));
        assert!(!inspector.contains(&handles.inspector_body.stable_id()));
        assert!(!inspector.contains(&handles.inspector_todos.stable_id()));
        assert!(!inspector.contains(&handles.coding_panel.stable_id()));
    }

    #[test]
    fn architecture_page_mounts_its_own_main_and_inspector_regions() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.project_page = Some(ShellProjectPage::Architecture);
        snapshot.architecture.summary = "当前图".into();
        snapshot.architecture.records =
            vec![crate::module::architecture::view::ArchitectureRecord {
                id: "change-1".into(),
                title: "新增服务".into(),
                status: "已应用".into(),
            }];
        snapshot.inspector_kind = "architecture".into();
        let (document, handles, primary) = mounted_primary(&snapshot);
        let view = handles.architecture_view.as_ref().unwrap();
        assert_eq!(primary, Some(handles.project_page.stable_id()));
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.project_page.stable_id())
                .unwrap()
                .children,
            vec![view.root.stable_id()]
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.inspector.stable_id())
                .unwrap()
                .children,
            vec![
                handles.inspector_header.stable_id(),
                view.inspector.stable_id()
            ]
        );
        assert!(handles.inspector_todo_rows.is_empty());
    }

    #[test]
    fn open_document_selects_workspace_primary() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            text: String::new(),
            language: "markdown".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            kind: "document-editor".to_owned(),
            selected: true,
            closable: true,
        });
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_conversation_beside_workspace(&document, &handles, primary);
    }

    #[test]
    fn open_files_selects_workspace_primary() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.files = Some(ShellFilesSnapshot {
            tree: TreeView::new(Vec::new()),
            preview: None,
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "files".to_owned(),
            title: "文件".to_owned(),
            kind: "project-files".to_owned(),
            selected: true,
            closable: true,
        });
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_conversation_beside_workspace(&document, &handles, primary);
    }

    #[test]
    fn files_project_page_selects_workspace_tree() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.project_page = Some(ShellProjectPage::Files);
        snapshot.project_page_title = "项目文件".to_owned();
        snapshot.files = Some(ShellFilesSnapshot {
            tree: TreeView::new(Vec::new()),
            preview: None,
        });
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_conversation_beside_workspace(&document, &handles, primary);
        let content = document
            .context()
            .world()
            .node(handles.workspace_content.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert!(content.contains(&handles.workspace_tree.stable_id()));
        assert!(!content.contains(&handles.workspace_editor.stable_id()));
        assert!(!content.contains(&handles.workspace_log.stable_id()));
    }

    #[test]
    fn document_workspace_hides_file_tree_and_terminal() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            text: String::new(),
            language: "markdown".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.files = Some(ShellFilesSnapshot {
            tree: TreeView::new(Vec::new()),
            preview: None,
        });
        snapshot.terminal = Some(ShellTerminalSnapshot {
            item_id: "terminal-item".to_owned(),
            session_id: "terminal-session".to_owned(),
            output: String::new(),
            notice: None,
            screen: nana_ui::runtime::TerminalScreen::blank(80, 24),
            running: true,
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            kind: "document-editor".to_owned(),
            selected: true,
            closable: true,
        });
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_conversation_beside_workspace(&document, &handles, primary);
        let content = document
            .context()
            .world()
            .node(handles.workspace_content.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert!(content.contains(&handles.workspace_editor.stable_id()));
        assert!(!content.contains(&handles.workspace_tree.stable_id()));
        assert!(!content.contains(&handles.workspace_log.stable_id()));
    }

    #[test]
    fn open_terminal_uses_bottom_and_keeps_conversation_primary() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.terminal = Some(ShellTerminalSnapshot {
            item_id: "terminal-item".to_owned(),
            session_id: "terminal-session".to_owned(),
            output: String::new(),
            notice: None,
            screen: nana_ui::runtime::TerminalScreen::blank(80, 24),
            running: true,
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "term".to_owned(),
            title: "终端".to_owned(),
            kind: "terminal".to_owned(),
            selected: true,
            closable: true,
        });
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_eq!(primary, Some(handles.conversation.stable_id()));
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.terminal_page.stable_id())
                .unwrap()
                .children,
            vec![handles.pane_chrome.stable_id()]
        );
        assert_eq!(
            document
                .context()
                .read(handles.shell, |shell| shell.bottom)
                .unwrap(),
            Some(handles.workbench_bottom.stable_id())
        );
    }

    #[test]
    fn split_workspace_paints_two_live_pane_bodies() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "doc-1".to_owned(),
            title: "main.rs".to_owned(),
            text: "fn main() {}".to_owned(),
            language: "rust".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes = vec![
            ShellPaneRow {
                id: "left".to_owned(),
                active: true,
                items: vec![ShellPaneItem {
                    id: "doc-1".to_owned(),
                    title: "main.rs".to_owned(),
                    kind: "document-editor".to_owned(),
                    selected: true,
                    closable: true,
                }],
                document: snapshot.document.clone(),
                terminal: None,
                browser: None,
            },
            ShellPaneRow {
                id: "right".to_owned(),
                active: false,
                items: vec![ShellPaneItem {
                    id: "term-1".to_owned(),
                    title: "终端".to_owned(),
                    kind: "terminal".to_owned(),
                    selected: true,
                    closable: true,
                }],
                document: None,
                browser: None,
                terminal: Some(ShellTerminalSnapshot {
                    item_id: "terminal-item".to_owned(),
                    session_id: "terminal-session".to_owned(),
                    output: "$ ls".to_owned(),
                    notice: None,
                    screen: nana_ui::runtime::TerminalScreen::blank(80, 24),
                    running: true,
                }),
            },
        ];
        snapshot.pane_layout = ShellPaneLayout::Split {
            horizontal: true,
            ratio: 0.5,
            first: Box::new(ShellPaneLayout::Leaf("left".to_owned())),
            second: Box::new(ShellPaneLayout::Leaf("right".to_owned())),
        };
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_conversation_beside_workspace(&document, &handles, primary);
        assert_eq!(handles.extra_workspace_panes.len(), 1);
        let right = handles
            .extra_workspace_panes
            .get("right")
            .expect("right pane");
        let page = document
            .context()
            .world()
            .node(handles.workspace_page.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0], handles.pane_chrome.stable_id());
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.terminal_page.stable_id())
                .unwrap()
                .children,
            vec![right.chrome.stable_id()]
        );
        let right_content = document
            .context()
            .world()
            .node(right.content.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert!(right_content.contains(&right.log.stable_id()));
        assert!(!right_content.contains(&right.editor.stable_id()));
        let left_content = document
            .context()
            .world()
            .node(handles.workspace_content.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert!(left_content.contains(&handles.workspace_editor.stable_id()));
        assert!(!left_content.contains(&handles.workspace_log.stable_id()));
    }

    #[test]
    fn pane_with_workspace_item_selects_workspace_primary() {
        let mut snapshot = empty_snapshot();
        snapshot.panes = vec![ShellPaneRow {
            id: "primary".to_owned(),
            active: true,
            items: vec![ShellPaneItem {
                id: "item-1".to_owned(),
                title: "会话".to_owned(),
                kind: "task".to_owned(),
                selected: true,
                closable: true,
            }],
            document: None,
            terminal: None,
            browser: None,
        }];
        snapshot.pane_layout = ShellPaneLayout::Leaf("primary".to_owned());
        let (_document, handles, primary) = mounted_primary(&snapshot);
        assert_eq!(primary, Some(handles.conversation.stable_id()));
    }

    #[test]
    fn document_pane_item_selects_workspace_primary() {
        let mut snapshot = empty_snapshot();
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "item-1".to_owned(),
            title: "main.rs".to_owned(),
            text: String::new(),
            language: "rust".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes = vec![ShellPaneRow {
            id: "primary".to_owned(),
            active: true,
            items: vec![ShellPaneItem {
                id: "item-1".to_owned(),
                title: "main.rs".to_owned(),
                kind: "document-editor".to_owned(),
                selected: true,
                closable: true,
            }],
            document: None,
            terminal: None,
            browser: None,
        }];
        snapshot.pane_layout = ShellPaneLayout::Leaf("primary".to_owned());
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_conversation_beside_workspace(&document, &handles, primary);
    }

    #[test]
    fn settings_open_selects_settings_primary() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.navigation = WindowRoute::Settings;
        let (_document, handles, primary) = mounted_primary(&snapshot);
        assert_eq!(
            primary,
            Some(handles.settings_view.settings_page.stable_id())
        );
    }

    #[test]
    fn automations_open_selects_automations_primary() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.navigation = WindowRoute::Automations;
        let (document, handles, primary) = mounted_primary(&snapshot);
        assert_eq!(primary, Some(handles.automation_view.page.stable_id()));
        let navigation = document
            .context()
            .read(handles.shell, |shell| shell.navigation)
            .expect("read navigation");
        assert_eq!(
            navigation,
            Some(handles.automation_view.sidebar.stable_id())
        );
    }

    #[test]
    fn settings_open_with_document_stays_exclusive() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.navigation = WindowRoute::Settings;
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            text: String::new(),
            language: "markdown".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            kind: "document-editor".to_owned(),
            selected: true,
            closable: true,
        });
        let (_document, handles, primary) = mounted_primary(&snapshot);
        assert_eq!(
            primary,
            Some(handles.settings_view.settings_page.stable_id())
        );
        assert_ne!(primary, Some(handles.conversation_workspace.stable_id()));
    }

    #[test]
    fn closing_workspace_restores_conversation_primary() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            text: String::new(),
            language: "markdown".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "doc-1".to_owned(),
            title: "notes.md".to_owned(),
            kind: "document-editor".to_owned(),
            selected: true,
            closable: true,
        });
        let (mut document, mut handles, primary) = mounted_primary(&snapshot);
        assert_conversation_beside_workspace(&document, &handles, primary);
        snapshot.document = None;
        snapshot.panes[0].items.clear();
        handles
            .sync(&mut document, &snapshot)
            .expect("sync closed workspace");
        let primary = document
            .context_mut()
            .read(handles.shell, |shell| shell.primary)
            .expect("read shell primary");
        assert_eq!(primary, Some(handles.conversation.stable_id()));
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.composer_view.composer.stable_id())
                .and_then(|node| node.parent),
            Some(handles.task_view.composer_view.composer_dock.stable_id())
        );
    }

    #[test]
    fn conversation_column_keeps_chat_max_width() {
        let (document, handles, _primary) = mounted_primary(&snapshot_with_empty_primary_pane());
        let width = document
            .context()
            .world()
            .node_style(handles.task_view.conversation_column.stable_id())
            .expect("conversation column style")
            .layout
            .max_width;
        assert_eq!(
            width,
            Some(nana_ui::runtime::LengthSpec::Px(CHAT_CONTENT_MAX_WIDTH))
        );
    }

    #[test]
    fn composer_dock_stays_centered_when_chat_column_is_capped() {
        let (mut document, handles, _primary) =
            mounted_primary(&snapshot_with_empty_primary_pane());
        document
            .context_mut()
            .layout_document(
                DocumentId::new(PRIMARY_DOCUMENT).expect("primary document id"),
                nana_ui::runtime::LayoutViewport::new(1400.0, 900.0),
            )
            .expect("layout primary shell");
        let world = document.context().world();
        let root = world
            .layout_box(handles.conversation.stable_id())
            .expect("conversation root box");
        let dock = world
            .layout_box(handles.task_view.composer_view.composer_dock.stable_id())
            .expect("composer dock box");
        assert!(
            (dock.width - CHAT_CONTENT_MAX_WIDTH).abs() < 0.5,
            "wide viewport must exercise the chat column cap, dock width={}",
            dock.width
        );
        let root_center = root.x + root.width / 2.0;
        let dock_center = dock.x + dock.width / 2.0;
        assert!(
            (root_center - dock_center).abs() < 0.5,
            "composer dock must stay centered in the conversation pane: \
             root_center={root_center}, dock_center={dock_center}"
        );
    }

    #[test]
    fn composer_permission_menu_marks_current_without_moving_the_toolbar() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.composer.permission_selection = "readonly".to_owned();
        snapshot.composer.composer_permission_menu_open = true;
        let (mut document, handles, _primary) = mounted_primary(&snapshot);
        document
            .context_mut()
            .layout_document(
                DocumentId::new(PRIMARY_DOCUMENT).expect("primary document id"),
                nana_ui::runtime::LayoutViewport::new(1400.0, 900.0),
            )
            .expect("layout open permission menu");
        let world = document.context().world();
        assert_eq!(
            handles.task_view.composer_view.permission_items.len(),
            COMPOSER_PERMISSION_OPTIONS.len()
        );
        let readonly_item =
            handles.task_view.composer_view.permission_items["readonly"].stable_id();
        let ask_item = handles.task_view.composer_view.permission_items["ask"].stable_id();
        assert_eq!(
            world
                .node_style(readonly_item)
                .and_then(|style| style.background),
            Some(SemanticColorRole::Hover)
        );
        assert_eq!(
            world
                .node_style(ask_item)
                .and_then(|style| style.background),
            None
        );
        let menu_style = world
            .node_style(handles.task_view.composer_view.permission_menu.stable_id())
            .expect("permission menu style");
        assert_eq!(
            menu_style.layout.position,
            nana_ui::runtime::PositionSpec::Static
        );
        let open_toolbar = world
            .layout_box(handles.task_view.composer_view.composer_toolbar.stable_id())
            .expect("open toolbar");
        let open_trigger = world
            .layout_box(handles.task_view.composer_view.permission_menu.stable_id())
            .expect("open trigger");
        let open_item = world.layout_box(ask_item).expect("open menu item");
        assert!(
            open_item.y + open_item.height <= open_trigger.y + 1.0,
            "permission items must open above the trigger: trigger={open_trigger:?} item={open_item:?}"
        );
        drop(document);
        let (mut closed_document, closed_handles, _primary) =
            mounted_primary(&snapshot_with_empty_primary_pane());
        closed_document
            .context_mut()
            .layout_document(
                DocumentId::new(PRIMARY_DOCUMENT).expect("primary document id"),
                nana_ui::runtime::LayoutViewport::new(1400.0, 900.0),
            )
            .expect("layout closed permission menu");
        assert!(
            closed_handles
                .task_view
                .composer_view
                .permission_items
                .is_empty()
        );
        let closed_world = closed_document.context().world();
        let closed_toolbar = closed_world
            .layout_box(
                closed_handles
                    .task_view
                    .composer_view
                    .composer_toolbar
                    .stable_id(),
            )
            .expect("closed toolbar");
        let closed_trigger = closed_world
            .layout_box(
                closed_handles
                    .task_view
                    .composer_view
                    .permission_menu
                    .stable_id(),
            )
            .expect("closed trigger");
        assert_eq!(closed_toolbar, open_toolbar);
        assert_eq!(closed_trigger, open_trigger);
    }

    #[test]
    fn composer_worktree_menu_marks_current_option() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.composer.worktree_label = Some("新建工作树".to_owned());
        snapshot.composer.worktree_selection = "create".to_owned();
        snapshot.composer.composer_worktree_menu_open = true;
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let world = document.context().world();
        assert_eq!(
            handles.task_view.composer_view.worktree_items.len(),
            COMPOSER_WORKTREE_OPTIONS.len()
        );
        let create_item = handles.task_view.composer_view.worktree_items["create"].stable_id();
        let current_item = handles.task_view.composer_view.worktree_items["current"].stable_id();
        assert_eq!(
            world
                .node_style(create_item)
                .and_then(|style| style.background),
            Some(SemanticColorRole::Hover)
        );
        assert_eq!(
            world
                .node_style(current_item)
                .and_then(|style| style.background),
            None
        );
        let extras = world
            .node(handles.task_view.composer_view.extras.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            extras,
            vec![
                handles.task_view.composer_view.plus_slot.stable_id(),
                handles.task_view.composer_view.attach.stable_id(),
                handles.task_view.composer_view.permission_slot.stable_id(),
                handles.task_view.composer_view.worktree_slot.stable_id()
            ]
        );
    }

    #[test]
    fn composer_plus_menu_stays_in_the_toolbar() {
        let (document, handles, _primary) = mounted_primary(&snapshot_with_empty_primary_pane());
        let extras = document
            .context()
            .world()
            .node(handles.task_view.composer_view.extras.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            extras,
            vec![
                handles.task_view.composer_view.plus_slot.stable_id(),
                handles.task_view.composer_view.attach.stable_id(),
                handles.task_view.composer_view.permission_slot.stable_id()
            ]
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.composer_view.plus_slot.stable_id())
                .map(|node| node.children.clone())
                .unwrap_or_default(),
            vec![handles.task_view.composer_view.plus_menu.stable_id()]
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.composer_view.permission_slot.stable_id())
                .map(|node| node.children.clone())
                .unwrap_or_default(),
            vec![
                handles.task_view.composer_view.permission_icon.stable_id(),
                handles.task_view.composer_view.permission_menu.stable_id()
            ]
        );
        assert!(handles.task_view.composer_view.plus_items.is_empty());
        assert!(handles.task_view.composer_view.permission_items.is_empty());
        let actions = document
            .context()
            .world()
            .node(handles.task_view.composer_view.composer_actions.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            actions,
            vec![
                handles.task_view.composer_view.browser_open.stable_id(),
                handles.task_view.composer_view.send.stable_id()
            ]
        );
        assert!(handles.task_view.timeline_view.load_earlier.is_none());
        let body = document
            .context()
            .world()
            .node(handles.task_view.conversation_body.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            body,
            vec![
                handles.task_view.heading_slot.stable_id(),
                handles.task_view.error.stable_id(),
                handles.task_view.timeline_view.root.stable_id(),
            ]
        );
    }

    #[test]
    fn slash_items_mount_above_the_composer() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.composer.slash_items = vec![ComposerSlashItem {
            name: "status".to_owned(),
            label: "查看状态".to_owned(),
        }];
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let extras = document
            .context()
            .world()
            .node(handles.task_view.composer_view.extras.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            extras,
            vec![
                handles.task_view.composer_view.plus_slot.stable_id(),
                handles.task_view.composer_view.attach.stable_id(),
                handles.task_view.composer_view.permission_slot.stable_id()
            ]
        );
        let dock = document
            .context()
            .world()
            .node(handles.task_view.composer_view.composer.stable_id())
            .and_then(|node| node.parent)
            .expect("composer dock");
        assert_eq!(
            document
                .context()
                .world()
                .node(dock)
                .map(|node| node.children.clone())
                .unwrap_or_default(),
            vec![
                handles.task_view.composer_view.completion_slot.stable_id(),
                handles.task_view.composer_view.composer.stable_id(),
                handles.task_view.composer_view.composer_toolbar.stable_id(),
            ]
        );
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.composer_view.completion_slot.stable_id())
                .map(|node| node.children.clone())
                .unwrap_or_default()
                .first()
                .copied(),
            handles
                .task_view
                .composer_view
                .completion_items
                .get("slash-status")
                .map(|item| item.stable_id())
        );
    }

    #[test]
    fn project_row_menu_anchors_to_its_more_button() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        let mut project = test_sidebar_row("project-lilia", "LiliaCode", ShellSidebarKind::Project);
        project.can_menu = true;
        snapshot.sidebar_rows = vec![project];
        snapshot.sidebar_menu = vec![ShellMenuItem {
            id: "open-project".to_owned(),
            label: "进入项目".to_owned(),
        }];
        snapshot.sidebar_menu_owner = Some("project-lilia".to_owned());
        let (mut document, mut handles, _primary) = mounted_primary(&snapshot);
        handles.sync(&mut document, &snapshot).expect("sync shell");

        let button = match handles.row_tool_buttons.get("project-lilia-menu") {
            Some(RowToolButton::Tool(button)) => button.stable_id(),
            _ => panic!("project row menu button must be mounted"),
        };
        assert_eq!(
            handles.sidebar_menu_anchor_source(&snapshot),
            SidebarMenuAnchor::RowMenuButton(button)
        );
    }

    #[test]
    fn right_click_anchor_wins_and_add_project_menu_keeps_its_button() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.sidebar_menu = vec![ShellMenuItem {
            id: "open-project".to_owned(),
            label: "进入项目".to_owned(),
        }];
        snapshot.sidebar_menu_owner = Some("project-lilia".to_owned());
        snapshot.sidebar_menu_anchor = Some((40.0, 220.0));
        let (_document, handles, _primary) = mounted_primary(&snapshot);
        assert_eq!(
            handles.sidebar_menu_anchor_source(&snapshot),
            SidebarMenuAnchor::Point((40.0, 220.0))
        );

        snapshot.sidebar_menu_anchor = None;
        snapshot.add_project_menu_open = true;
        assert_eq!(
            handles.sidebar_menu_anchor_source(&snapshot),
            SidebarMenuAnchor::AddProjectButton(None)
        );
    }

    #[test]
    fn add_project_menu_stays_on_the_section_header() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.add_project_menu_open = true;
        snapshot.sidebar_menu_anchor = Some((24.0, 96.0));
        snapshot.sidebar_menu = vec![ShellMenuItem {
            id: "add-local-folder".to_owned(),
            label: "使用本地文件夹".to_owned(),
        }];
        let (mut document, handles, _primary) = mounted_primary(&snapshot);
        let header_children = document
            .context()
            .world()
            .node(handles.project_header.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert!(header_children.contains(&handles.add_project_menu.stable_id()));
        let more_menu = handles
            .more_menu
            .expect("add-project items use the overlay");
        assert_eq!(
            document
                .context_mut()
                .read(more_menu, |menu| menu.items[0].value.to_string())
                .expect("read overlay menu"),
            "add-local-folder"
        );
    }

    #[test]
    fn sidebar_new_conversation_lives_in_the_top_slot() {
        let (document, handles, _primary) = mounted_primary(&snapshot_with_empty_primary_pane());
        let children = document
            .context()
            .world()
            .node(handles.sidebar_top.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            children.first().copied(),
            Some(handles.new_conversation.stable_id())
        );
    }

    #[test]
    fn an_enabled_composer_accepts_pointer_and_focus() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.composer.composer_disabled = false;
        let (mut document, handles, _primary) = mounted_primary(&snapshot);
        let disabled = document
            .context_mut()
            .read(handles.task_view.composer_view.composer, |composer| {
                composer.disabled
            })
            .expect("read composer");
        assert!(!disabled);
        assert_eq!(
            handles
                .focus_targets
                .get(target_ids::COMPOSER_INPUT)
                .copied(),
            Some(handles.task_view.composer_view.composer.stable_id())
        );
    }

    #[test]
    fn focused_composer_keeps_cleared_text_until_revision_changes() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.composer.composer = "a".to_owned();
        snapshot.composer.composer_revision = 1;
        snapshot.composer.composer_disabled = false;
        let (mut document, mut handles, _primary) = mounted_primary(&snapshot);

        let composer_id = handles.task_view.composer_view.composer.stable_id();
        let document_id = document.document();
        document
            .context_mut()
            .focus_node(document_id, composer_id)
            .expect("focus composer");
        document
            .context_mut()
            .update_component(handles.task_view.composer_view.composer, |composer, _| {
                composer.state.replace_value(String::new());
            })
            .expect("clear composer");

        handles
            .sync(&mut document, &snapshot)
            .expect("sync stale snapshot");
        let after_stale = document
            .context()
            .read(handles.task_view.composer_view.composer, |composer| {
                composer.state.value.clone()
            })
            .expect("read composer");
        assert_eq!(after_stale, "");

        snapshot.composer.composer = "@file".to_owned();
        snapshot.composer.composer_revision = 2;
        handles
            .sync(&mut document, &snapshot)
            .expect("sync revision bump");
        let after_revision = document
            .context()
            .read(handles.task_view.composer_view.composer, |composer| {
                composer.state.value.clone()
            })
            .expect("read composer");
        assert_eq!(after_revision, "@file");
    }

    #[test]
    fn empty_session_sidebar_uses_section_empty_state() {
        let (document, handles, _primary) = mounted_primary(&snapshot_with_empty_primary_pane());
        let empty_text = document
            .context()
            .read(handles.conversation_section, |section| {
                section.empty_text.clone()
            })
            .expect("read session section");
        assert_eq!(empty_text.as_deref(), Some(SESSIONS_EMPTY_TEXT));
        let children = document
            .context()
            .world()
            .node(handles.task_body.stable_id())
            .map(|node| node.children.len())
            .unwrap_or(usize::MAX);
        assert_eq!(children, 0);
    }

    #[test]
    fn session_rows_replace_sidebar_empty_state() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.tasks = vec![ShellTaskRow {
            id: TaskId::new("task-1").expect("task id"),
            title: "设计稿".to_owned(),
            selected: true,
        }];
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let children = section_row_ids(&document, handles.task_body.stable_id());
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0],
            handles
                .task_rows
                .get("task-1")
                .expect("task row")
                .stable_id()
        );
        let tools = handles.row_tools.get("task-1").map(|host| host.stable_id());
        let item_tools = document
            .context()
            .read(handles.task_reorder, |list| {
                list.items
                    .iter()
                    .find(|item| item.value.as_ref() == "task-1")
                    .and_then(|item| item.tools)
            })
            .expect("read reorder item");
        assert_eq!(item_tools, tools);
        assert!(tools.is_some());
    }

    #[test]
    fn grouped_sidebar_mounts_projects_and_inbox_only() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.sidebar_rows = vec![
            test_sidebar_row("projects-header", "项目", ShellSidebarKind::Header),
            test_sidebar_row("inbox", "收集箱", ShellSidebarKind::Inbox),
        ];
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let project_empty = document
            .context()
            .read(handles.project_section, |section| {
                section.empty_text.clone()
            })
            .expect("read project section");
        assert_eq!(project_empty.as_deref(), Some(PROJECTS_EMPTY_TEXT));
        let inbox_empty = document
            .context()
            .read(handles.inbox_section, |section| section.empty_text.clone())
            .expect("read inbox section");
        assert_eq!(inbox_empty.as_deref(), Some(INBOX_EMPTY_TEXT));
        let scroll = document
            .context()
            .world()
            .node(handles.sidebar_scroll.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            scroll,
            vec![
                handles.project_section.stable_id(),
                handles.inbox_section.stable_id(),
            ]
        );
        assert_eq!(
            handles
                .focus_targets
                .get(target_ids::SIDEBAR_PROJECTS_OVERVIEW)
                .copied(),
            Some(handles.project_header.stable_id())
        );
    }

    #[test]
    fn partition_sidebar_keeps_drop_hint_and_archived_with_projects() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.sidebar_rows = vec![
            test_sidebar_row("drop-hint", "松开以添加项目", ShellSidebarKind::DropHint),
            test_sidebar_row("projects-header", "项目", ShellSidebarKind::Header),
            test_sidebar_row("proj-1", "Demo", ShellSidebarKind::Project),
            test_sidebar_row("inbox", "收集箱", ShellSidebarKind::Inbox),
            test_sidebar_row("inbox-task", "未绑定", ShellSidebarKind::Task),
            test_sidebar_row("archived-1", "恢复 · 旧项目", ShellSidebarKind::Archived),
        ];
        let groups = partition_sidebar_rows(&snapshot);
        assert!(groups.grouped);
        assert_eq!(
            groups
                .projects
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            vec!["drop-hint", "proj-1", "archived-1"]
        );
        assert_eq!(
            groups
                .inbox
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            vec!["inbox-task"]
        );
        assert_eq!(sidebar_project_entry_count(&groups.projects), 2);
    }

    #[test]
    fn grouped_sidebar_project_count_excludes_nested_sessions() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        let mut project = test_sidebar_row("proj-1", "Demo", ShellSidebarKind::Project);
        project.expanded = Some(true);
        let mut nested = test_sidebar_row("task-1", "会话", ShellSidebarKind::Task);
        nested.depth = 1;
        snapshot.sidebar_rows = vec![
            test_sidebar_row("projects-header", "项目", ShellSidebarKind::Header),
            project,
            nested,
            test_sidebar_row("reveal-proj-1", "…", ShellSidebarKind::Reveal),
            test_sidebar_row("inbox", "收集箱", ShellSidebarKind::Inbox),
        ];
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let count = document
            .context()
            .read(handles.project_section, |section| section.count)
            .expect("read project count");
        assert_eq!(count, Some(1));
        let body = section_row_ids(&document, handles.project_body.stable_id()).len();
        assert_eq!(body, 3);
    }

    #[test]
    fn grouped_sidebar_rebuilds_row_when_kind_changes() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        let mut project = test_sidebar_row("proj-1", "Demo", ShellSidebarKind::Project);
        project.expanded = Some(true);
        project.can_menu = true;
        project.can_draft = true;
        snapshot.sidebar_rows = vec![
            test_sidebar_row("projects-header", "项目", ShellSidebarKind::Header),
            project,
            test_sidebar_row("inbox", "收集箱", ShellSidebarKind::Inbox),
        ];
        let (mut document, mut handles, _primary) = mounted_primary(&snapshot);
        let original = handles
            .task_rows
            .get("proj-1")
            .map(|row| row.stable_id())
            .expect("project row");
        assert!(handles.row_tools.contains_key("proj-1"));

        snapshot.sidebar_rows = vec![
            test_sidebar_row("projects-header", "项目", ShellSidebarKind::Header),
            test_sidebar_row("inbox", "收集箱", ShellSidebarKind::Inbox),
            test_sidebar_row("proj-1", "恢复 · Demo", ShellSidebarKind::Archived),
        ];
        handles.sync(&mut document, &snapshot).expect("resync");
        let rebuilt = handles
            .task_rows
            .get("proj-1")
            .map(|row| row.stable_id())
            .expect("archived row");
        assert_ne!(original, rebuilt);
        assert!(!handles.row_tools.contains_key("proj-1"));
        let tools = document
            .context()
            .read(
                *handles
                    .task_rows
                    .get("proj-1")
                    .expect("archived row entity"),
                |row| row.tools,
            )
            .expect("read tools");
        assert_eq!(tools, None);
        let project_children = section_row_ids(&document, handles.project_body.stable_id());
        assert_eq!(project_children, vec![rebuilt]);
    }

    #[test]
    fn long_timeline_materializes_only_the_visible_window() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.timeline.rows = (0..50)
            .map(|index| crate::module::timeline::view::TimelineRow {
                id: format!("event-{index}"),
                markdown: format!("行 {index}"),
                images: Vec::new(),
                expanded: false,
                can_expand: false,
                can_retry: false,
                can_copy: false,
                can_branch: false,
            })
            .collect();
        snapshot.timeline.layout = VirtualListLayout::new(std::iter::repeat(40.0).take(50));
        snapshot.timeline.scroll_offset = 0.0;
        snapshot.timeline.viewport_extent = 80.0;
        let (document, handles, _primary) = mounted_primary(&snapshot);
        let children = document
            .context()
            .world()
            .node(handles.task_view.timeline_view.timeline_list.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert!(
            children.len() < snapshot.timeline.rows.len(),
            "expected a window, got {} children",
            children.len()
        );
        assert!(!children.is_empty());
        assert_eq!(
            document
                .context()
                .world()
                .node(handles.task_view.timeline_view.timeline_scroll.stable_id())
                .map(|node| node.children),
            Some(vec![
                handles.task_view.timeline_view.timeline_list.stable_id()
            ])
        );
    }

    #[test]
    fn sidebar_reorder_events_map_to_intents() {
        assert!(matches!(
            sidebar_reorder_intent(&ReorderListEvent::Reorder {
                source: Arc::from("task-1"),
                before: Some(Arc::from("task-2")),
            }),
            Some(ShellIntent::ReorderSidebar { source, before })
                if source == "task-1" && before.as_deref() == Some("task-2")
        ));
        assert!(matches!(
            sidebar_reorder_intent(&ReorderListEvent::TreeDrop {
                source: Arc::from("task-1"),
                intent: nana_ui::runtime::TreeDropIntent {
                    target: Arc::from("proj-1"),
                    position: TreeDropPosition::Inside,
                },
            }),
            Some(ShellIntent::SidebarTreeDrop {
                source,
                target,
                position: SidebarDropPosition::Inside,
            }) if source == "task-1" && target == "proj-1"
        ));
        assert!(sidebar_reorder_intent(&ReorderListEvent::Select(Arc::from("task-1"))).is_none());
    }

    #[test]
    fn workspace_tabs_close_and_transfer_emit_intents() {
        assert!(matches!(
            workspace_tabs_intent(&TabsEvent::Close(Arc::from("doc-1"))),
            ShellIntent::ClosePaneTab { item_id, .. } if item_id == "doc-1"
        ));
        assert!(matches!(
            workspace_tabs_intent(&TabsEvent::Transfer {
                source_strip: Arc::from("workspace/main/pane/a"),
                value: Arc::from("doc-1"),
                target_strip: Arc::from("workspace/main/pane/b"),
                before: None,
            }),
            ShellIntent::TransferPaneTab { item_id, .. } if item_id == "doc-1"
        ));
    }

    #[test]
    fn editor_diagnostics_preserve_utf8_ranges_and_reject_invalid_spans() {
        let text = "let 变量 = 1;";
        let diagnostic = Diagnostic {
            message: "未使用".to_owned(),
            severity: DiagnosticSeverity::Warning,
            start_offset: 4,
            end_offset: 10,
            source: None,
            code: None,
        };
        let mut row = ShellDiagnosticRow::from(&diagnostic);
        let span = row.editor_span(text).expect("valid UTF-8 span");
        assert_eq!(&text[span.offset..span.offset + span.length], "变量");
        assert_eq!(span.severity, TextDiagnosticSeverity::Warning);
        assert_eq!(span.message, diagnostic.message);
        row.start_offset = 5;
        assert!(row.editor_span(text).is_none());
        row.start_offset = 11;
        assert!(row.editor_span(text).is_none());
        row.start_offset = 4;
        row.end_offset = text.len() + 1;
        assert!(row.editor_span(text).is_none());
    }

    #[test]
    fn document_editor_requests_syntax_highlight() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "doc-1".to_owned(),
            title: "main.rs".to_owned(),
            text: "fn main() {}".to_owned(),
            language: "rust".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "doc-1".to_owned(),
            title: "main.rs".to_owned(),
            kind: "document-editor".to_owned(),
            selected: true,
            closable: true,
        });
        let (mut document, handles, _) = mounted_primary(&snapshot);
        let language = document
            .context_mut()
            .read(handles.workspace_editor, |editor| {
                editor
                    .highlight
                    .as_ref()
                    .map(|request| request.language.to_string())
            })
            .expect("read editor");
        assert_eq!(language.as_deref(), Some("rust"));
    }

    #[test]
    fn read_only_document_search_is_available_in_both_panes_without_edit_events() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            item_id: "read-only".into(),
            revision: 7,
            conflicted: false,
            title: "reference.txt".into(),
            text: "猫 and 猫".into(),
            language: "plaintext".into(),
            status: String::new(),
            read_only: true,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes[0].document = snapshot.document.clone();
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "read-only".into(),
            title: "reference.txt".into(),
            kind: "document-editor".into(),
            selected: true,
            closable: true,
        });
        let mut second = snapshot.panes[0].clone();
        second.id = "second".into();
        second.active = false;
        snapshot.panes.push(second);
        snapshot.pane_layout = ShellPaneLayout::Split {
            horizontal: true,
            ratio: 0.5,
            first: Box::new(ShellPaneLayout::Leaf("primary".into())),
            second: Box::new(ShellPaneLayout::Leaf("second".into())),
        };
        let intents = Arc::new(Mutex::new(Vec::new()));
        let sink = intents.clone();
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| sink.lock().unwrap().push(intent)),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let second = handles.extra_workspace_panes.get("second").unwrap().clone();
        let context = document.context_mut();
        let document_id = context
            .world()
            .node(handles.workspace_editor.stable_id())
            .unwrap()
            .document;
        let first_selection = context
            .read(handles.workspace_editor, |editor| editor.state.selection)
            .unwrap();
        for (editor, search) in [
            (handles.workspace_editor, handles.workspace_search.clone()),
            (second.editor, second.search.clone()),
        ] {
            assert!(
                context
                    .read(editor, |editor| editor.read_only && !editor.disabled)
                    .unwrap()
            );
            assert!(
                context
                    .world()
                    .node(search.root.stable_id())
                    .unwrap()
                    .parent
                    .is_some()
            );
            context.activate_button(search.toggle).unwrap();
            assert!(
                context
                    .read(search.replacement, |input| input.disabled)
                    .unwrap()
            );
            for button in &search.replace_actions {
                assert!(!context.activate_button(*button).unwrap());
            }
            context
                .focus_node(document_id, search.query.stable_id())
                .unwrap();
            search_document(
                context,
                editor.stable_id(),
                search.feedback.stable_id(),
                "猫",
                "dog",
                DocumentSearchAction::Next,
            )
            .unwrap();
            assert_eq!(
                context.world().focused(document_id),
                Some(editor.stable_id())
            );
            assert_eq!(
                context.focused_selected_text(document_id).as_deref(),
                Some("猫")
            );
            search_document(
                context,
                editor.stable_id(),
                search.feedback.stable_id(),
                "猫",
                "dog",
                DocumentSearchAction::ReplaceAll,
            )
            .unwrap();
            assert_eq!(
                context
                    .read(editor, |editor| editor.state.value.clone())
                    .unwrap(),
                "猫 and 猫"
            );
        }
        assert_ne!(
            context
                .read(handles.workspace_editor, |editor| editor.state.selection)
                .unwrap(),
            first_selection
        );
        assert!(
            intents
                .lock()
                .unwrap()
                .iter()
                .all(|intent| !matches!(intent, ShellIntent::DocumentChanged { .. }))
        );
        let target = ShellPaneTarget::primary("second", "read-only");
        assert!(second.matches_document_search(
            &target,
            second.editor.stable_id(),
            second.search.feedback.stable_id()
        ));
        let stale = ShellPaneTarget::primary("second", "closed-document");
        assert!(!second.matches_document_search(
            &stale,
            second.editor.stable_id(),
            second.search.feedback.stable_id()
        ));
    }

    #[test]
    fn document_replace_uses_bound_editor_while_search_input_has_focus() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            item_id: "doc-search".to_owned(),
            revision: 7,
            conflicted: false,
            title: "notes.txt".to_owned(),
            text: "猫 and 猫".to_owned(),
            language: "plaintext".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: Vec::new(),
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "doc-search".to_owned(),
            title: "notes.txt".to_owned(),
            kind: "document-editor".to_owned(),
            selected: true,
            closable: true,
        });
        let intents = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&intents);
        let (mut document, mut handles) = mount_primary_shell(
            &snapshot,
            Arc::new(move |intent| {
                received.lock().unwrap().push(intent);
            }),
        )
        .unwrap();
        handles.sync(&mut document, &snapshot).unwrap();
        let context = document.context_mut();
        let editor = handles.workspace_editor.stable_id();
        let document_id = context.world().node(editor).unwrap().document;
        assert!(!handles.workspace_search.draft.lock().unwrap().expanded);
        context
            .activate_button(handles.workspace_search.toggle)
            .unwrap();
        assert!(handles.workspace_search.draft.lock().unwrap().expanded);
        context
            .update_component(handles.workspace_search.query, |input, _| {
                input.state.replace_value("猫".to_owned());
            })
            .unwrap();
        context
            .focus_node(document_id, handles.workspace_search.query.stable_id())
            .unwrap();
        search_document(
            context,
            editor,
            handles.workspace_search.feedback.stable_id(),
            "猫",
            "dog",
            DocumentSearchAction::ReplaceAll,
        )
        .unwrap();
        assert_eq!(
            context
                .read(handles.workspace_editor, |area| area.state.value.clone())
                .unwrap(),
            "dog and dog"
        );
        assert_eq!(
            context
                .read(handles.workspace_search.query, |input| input
                    .state
                    .value
                    .clone())
                .unwrap(),
            "猫"
        );
        assert!(intents.lock().unwrap().iter().any(|intent| matches!(intent,
            ShellIntent::DocumentChanged { target, revision: 7, value }
            if target.item_id == "doc-search" && target.pane_id == "primary" && value == "dog and dog"
        )));
        let count = intents
            .lock()
            .unwrap()
            .iter()
            .filter(|intent| matches!(intent, ShellIntent::DocumentChanged { .. }))
            .count();
        search_document(
            context,
            editor,
            handles.workspace_search.feedback.stable_id(),
            "missing",
            "",
            DocumentSearchAction::ReplaceAll,
        )
        .unwrap();
        assert_eq!(
            intents
                .lock()
                .unwrap()
                .iter()
                .filter(|intent| matches!(intent, ShellIntent::DocumentChanged { .. }))
                .count(),
            count
        );
        context
            .activate_button(handles.workspace_search.toggle)
            .unwrap();
        assert!(!handles.workspace_search.draft.lock().unwrap().expanded);
        assert_eq!(
            context
                .read(handles.workspace_search.query, |input| input
                    .state
                    .value
                    .clone())
                .unwrap(),
            "猫"
        );
        context
            .activate_button(handles.workspace_search.toggle)
            .unwrap();
        assert_eq!(
            context
                .read(handles.workspace_search.query, |input| input
                    .state
                    .value
                    .clone())
                .unwrap(),
            "猫"
        );
        let target = ShellPaneTarget::primary("primary", "doc-search");
        assert!(handles.matches_document_search(
            &target,
            editor,
            handles.workspace_search.feedback.stable_id()
        ));
        let mut stale_target = target.clone();
        stale_target.item_id = "other-document".to_owned();
        assert!(!handles.matches_document_search(
            &stale_target,
            editor,
            handles.workspace_search.feedback.stable_id()
        ));
        handles
            .workspace_search
            .sync(context, Some("different-document"))
            .unwrap();
        assert!(!handles.workspace_search.draft.lock().unwrap().expanded);
        assert_eq!(
            context
                .read(handles.workspace_search.query, |input| input
                    .state
                    .value
                    .clone())
                .unwrap(),
            ""
        );
        assert!(!handles.matches_document_search(
            &target,
            editor,
            handles.workspace_search.feedback.stable_id()
        ));
    }

    #[test]
    fn closing_an_extra_pane_releases_unmounted_search_and_parked_controls() {
        let snapshot = snapshot_with_empty_primary_pane();
        let (mut document, handles, _) = mounted_primary(&snapshot);
        let context = document.context_mut();
        let document_id = context
            .world()
            .node(handles.workspace_editor.stable_id())
            .unwrap()
            .document;
        let sink: IntentSink = Arc::new(|_| {});
        let pane = mount_workspace_pane_view(context, document_id, "temporary", &sink).unwrap();
        let owned = [
            pane.chrome.stable_id(),
            pane.editor.stable_id(),
            pane.log.stable_id(),
            pane.search.root.stable_id(),
            pane.search.panel.stable_id(),
            pane.search.query.stable_id(),
            pane.search.replacement.stable_id(),
            pane.search.feedback.stable_id(),
            pane.discard.stable_id(),
            pane.chrome_actions[0].stable_id(),
        ];
        assert!(owned.iter().all(|node| context.world().contains(*node)));
        pane.dispose(context).unwrap();
        assert!(owned.iter().all(|node| !context.world().contains(*node)));
    }

    #[test]
    fn terminal_pane_uses_interactive_grid() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.terminal = Some(ShellTerminalSnapshot {
            item_id: "terminal-item".to_owned(),
            session_id: "terminal-session".to_owned(),
            output: "$ ls".to_owned(),
            notice: None,
            screen: nana_ui::runtime::TerminalScreen::blank(80, 24),
            running: true,
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "term".to_owned(),
            title: "终端".to_owned(),
            kind: "terminal".to_owned(),
            selected: true,
            closable: true,
        });
        let (mut document, handles, _) = mounted_primary(&snapshot);
        let children = document
            .context()
            .world()
            .node(handles.workspace_content.stable_id())
            .map(|node| node.children.clone())
            .expect("workspace children");
        assert!(children.contains(&handles.workspace_log.stable_id()));
        assert!(!children.contains(&handles.workspace_editor.stable_id()));
        let (columns, rows, disabled) = document
            .context_mut()
            .read(handles.workspace_log, |terminal| {
                (
                    terminal.screen.columns,
                    terminal.screen.rows,
                    terminal.disabled,
                )
            })
            .expect("read terminal grid");
        assert_eq!((columns, rows), (80, 24));
        assert!(!disabled);
    }

    #[test]
    fn browser_region_uses_the_actual_workbench_layout() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.browser = Some(crate::browser_workbench::BrowserPresentation {
            resource: "browser:task-1".into(),
            url: "about:blank".into(),
            ready: true,
            failed: false,
            human_control: false,
            busy: false,
            status: String::new(),
            requests: vec![],
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "browser:task-1".into(),
            title: "浏览器".into(),
            kind: "task-browser".into(),
            selected: true,
            closable: true,
        });
        let (mut document, handles, _) = mounted_primary(&snapshot);
        document
            .flush(
                nana_ui::runtime::LayoutViewport::new(1400.0, 900.0),
                &mut nana_ui::NanaTextShaper::default(),
            )
            .unwrap();
        let viewport = document
            .scene()
            .node_bounds(handles.shell.stable_id())
            .unwrap();
        let regions = nana_ui::native_content_regions(document.scene(), viewport).unwrap();
        assert_eq!(regions.len(), 1);
        assert_eq!(&*regions[0].resource, "browser:task-1");
        let pane_height = assert_workspace_chrome_geometry(&document, handles.pane_chrome);
        assert!(
            regions[0].bounds.width > 100.0 && regions[0].bounds.height > pane_height * 0.65,
            "browser region {:?} must fill pane height {pane_height}",
            regions[0].bounds
        );
        let browser = document
            .context()
            .world()
            .layout_box(handles.workspace_browser.root.stable_id())
            .unwrap();
        let pane = document
            .context()
            .world()
            .layout_box(handles.pane_chrome.stable_id())
            .unwrap();
        assert!(
            browser.y - pane.y <= 56.0,
            "browser toolbar begins too low: pane={pane:?}, browser={browser:?}"
        );
    }

    fn assert_workspace_chrome_geometry(
        document: &nana_ui::runtime::RuntimeDocument,
        chrome: Entity<PaneChrome>,
    ) -> f32 {
        let context = document.context();
        let actions = context
            .read(chrome, |chrome| {
                chrome
                    .actions
                    .iter()
                    .filter_map(|action| action.target)
                    .collect::<Vec<_>>()
            })
            .unwrap();
        for action in actions {
            let bounds = context.world().layout_box(action).unwrap();
            assert!(
                bounds.width > 0.0 && bounds.width <= 36.0,
                "pane icon action must not consume label width: {bounds:?}"
            );
            assert!(
                context.world().text(action).is_none_or(str::is_empty),
                "pane icon action must not also draw its accessibility label"
            );
            assert!(matches!(
                context.world().standard_visual(action),
                Some(nana_ui::runtime::StandardVisual::Icon { .. })
            ));
        }
        let (header, tabs, body) = context
            .read(chrome, |chrome| {
                (
                    chrome.header.unwrap(),
                    chrome.tabs.unwrap(),
                    chrome.body.unwrap(),
                )
            })
            .unwrap();
        let pane = context.world().layout_box(chrome.stable_id()).unwrap();
        let header = context.world().layout_box(header).unwrap();
        let tabs = context.world().layout_box(tabs).unwrap();
        let body = context.world().layout_box(body).unwrap();
        assert!(
            (header.y - pane.y).abs() <= 1.0 && (30.0..=36.0).contains(&header.height),
            "header must be a compact top row: pane={pane:?}, header={header:?}"
        );
        assert!(
            tabs.height <= 36.0
                && tabs.width >= 48.0
                && tabs.y >= header.y
                && tabs.y + tabs.height <= header.y + header.height + 1.0,
            "tabs must fit header: header={header:?}, tabs={tabs:?}"
        );
        assert!(
            body.y <= header.y + header.height + 1.0 && body.height >= pane.height - 38.0,
            "body must receive the remaining pane height: pane={pane:?}, body={body:?}"
        );
        pane.height
    }

    #[test]
    fn secondary_and_popup_browser_chrome_stays_compact_after_resizing() {
        let document_id = DocumentId::new(812).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let host = context
            .create_component(document_id, Stack::fill_column(0.0))
            .unwrap();
        let sink: IntentSink = Arc::new(|_| {});
        let mut view = WorkspacePaneView::mount(
            context,
            document_id,
            nana_ui_platform::WindowId(83),
            "browser-pane",
            &sink,
        )
        .unwrap();
        context.append_child(host, view.chrome).unwrap();
        let pane = ShellPaneRow {
            id: "browser-pane".into(),
            active: true,
            document: None,
            terminal: None,
            items: vec![ShellPaneItem {
                id: "browser:task".into(),
                title: "浏览器".into(),
                kind: "task-browser".into(),
                selected: true,
                closable: true,
            }],
            browser: Some(crate::browser_workbench::BrowserPresentation {
                resource: "browser:task".into(),
                url: "about:blank".into(),
                ready: true,
                failed: false,
                human_control: false,
                busy: false,
                status: String::new(),
                requests: vec![],
            }),
        };
        for (width, height) in [(760.0, 900.0), (420.0, 640.0)] {
            view.sync(
                document.context_mut(),
                nana_ui_platform::WindowId(83),
                &pane,
                None,
                None,
            )
            .unwrap();
            document
                .flush(
                    nana_ui::runtime::LayoutViewport::new(width, height),
                    &mut nana_ui::NanaTextShaper::default(),
                )
                .unwrap();
            let pane_height = assert_workspace_chrome_geometry(&document, view.chrome);
            let browser = document
                .context()
                .world()
                .layout_box(view.browser.root.stable_id())
                .unwrap();
            assert!(
                browser.y <= 56.0 && browser.height >= pane_height * 0.8,
                "browser={browser:?}, pane height={pane_height}"
            );
        }
    }

    #[test]
    fn diagnostics_attach_bottom_slot() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.document = Some(ShellDocumentSnapshot {
            revision: 0,
            conflicted: false,
            item_id: "doc-1".to_owned(),
            title: "main.rs".to_owned(),
            text: String::new(),
            language: "rust".to_owned(),
            status: String::new(),
            read_only: false,
            dirty: false,
            diagnostics: vec![ShellDiagnosticRow {
                severity: DiagnosticSeverity::Error,
                start_offset: 0,
                end_offset: 1,
                message: "unused".to_owned(),
            }],
        });
        snapshot.panes[0].items.push(ShellPaneItem {
            id: "doc-1".to_owned(),
            title: "main.rs".to_owned(),
            kind: "document-editor".to_owned(),
            selected: true,
            closable: true,
        });
        let (mut document, handles, _) = mounted_primary(&snapshot);
        let bottom = document
            .context_mut()
            .read(handles.shell, |shell| shell.bottom)
            .expect("read bottom");
        assert_eq!(bottom, Some(handles.diagnostics_panel.stable_id()));
    }

    #[test]
    fn product_settings_do_not_add_a_second_card_padding_layer() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.navigation = WindowRoute::Settings;
        let model = SettingsModel::new(
            "provider",
            [nana_ui::SettingsTab::new("provider", "模型服务")],
        )
        .expect("settings model");
        snapshot.settings.state = SettingsState::new(&model);
        snapshot.settings.model = model;
        snapshot.settings.provider_status = "当前服务可用。".to_owned();
        let (document, handles, _) = mounted_primary(&snapshot);
        let stack_layout = &document
            .context()
            .world()
            .node_style(handles.settings_view.product_settings.stable_id())
            .expect("product settings stack")
            .layout;
        assert!(stack_layout.resolved_padding().is_zero());
        let title = document
            .context()
            .read(handles.settings_view.settings_card, |card| {
                card.title.to_string()
            })
            .expect("read settings card");
        assert!(title.is_empty());
        let card_padding = document
            .context()
            .world()
            .node_style(handles.settings_view.settings_card.stable_id())
            .expect("settings card style")
            .layout
            .resolved_padding();
        assert_eq!(card_padding.top, nana_ui::UI_METRICS.panel_padding_y);
        assert_eq!(card_padding.left, nana_ui::UI_METRICS.panel_padding_x);
        let children = document
            .context()
            .world()
            .node(handles.settings_view.product_settings.stable_id())
            .map(|node| node.children.clone())
            .unwrap_or_default();
        assert_eq!(
            children,
            vec![
                handles.settings_view.product_body.stable_id(),
                handles.settings_view.settings_card.stable_id(),
            ]
        );
    }

    #[test]
    fn provider_settings_use_form_fields() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.navigation = WindowRoute::Settings;
        let model = SettingsModel::new(
            "provider",
            [nana_ui::SettingsTab::new("provider", "模型服务")],
        )
        .expect("settings model");
        snapshot.settings.state = SettingsState::new(&model);
        snapshot.settings.model = model;
        snapshot.settings.provider_secret = "secret".to_owned();
        let (_document, handles, _) = mounted_primary(&snapshot);
        assert!(
            handles
                .settings_view
                .fields
                .wrappers
                .contains_key("provider_secret")
        );
        assert!(handles.settings_view.form_switches.is_empty());
    }

    #[test]
    fn remote_settings_use_switches() {
        let mut snapshot = snapshot_with_empty_primary_pane();
        snapshot.navigation = WindowRoute::Settings;
        let model = SettingsModel::new("remote", [nana_ui::SettingsTab::new("remote", "远程控制")])
            .expect("settings model");
        snapshot.settings.state = SettingsState::new(&model);
        snapshot.settings.model = model;
        snapshot.settings.remote_host_enabled = true;
        let (_document, handles, _) = mounted_primary(&snapshot);
        assert!(
            handles
                .settings_view
                .form_switches
                .contains_key("remote_host")
        );
        assert!(
            handles
                .settings_view
                .form_switches
                .contains_key("remote_keep_awake")
        );
    }
}

impl PrimaryShellSnapshot {
    fn task_input(&self) -> crate::module::task::view::TaskViewInput<'_> {
        crate::module::task::view::TaskViewInput {
            heading: &self.heading,
            error: self.error.as_deref(),
            timeline: &self.timeline,
            composer: &self.composer,
            pending: self.pending.as_ref(),
        }
    }
}
