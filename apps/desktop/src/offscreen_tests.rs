//! Offscreen GPU snapshots of the primary product shell.
//!
//! `nana-ui-devtools` CPU readback is test-only and never reaches the product
//! present path. `cargo xtask screenshot` drives these tests.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use nana_ui::runtime::{LayoutViewport, RuntimeDocument, SemanticColorRole};
use nana_ui::{NanaTextShaper, ThemeMode};
use nana_ui_devtools::offscreen::{self, Size};

use crate::module::timeline::view::{StepKind, TimelineRole, TimelineRow, TimelineTone};
use crate::runtime_shell::{
    empty_snapshot, mount_primary_shell, PrimaryShellSnapshot, ShellHandles, ShellNavItem,
    ShellSidebarAttention, ShellSidebarKind, ShellSidebarRow,
};

const DEFAULT_WIDTH: u32 = 1180;
const DEFAULT_HEIGHT: u32 = 760;

struct Capture {
    dir: Option<PathBuf>,
    theme: ThemeMode,
    width: u32,
    height: u32,
}

impl Capture {
    fn from_env() -> Self {
        let number = |name: &str, fallback: u32| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(fallback)
        };
        Self {
            dir: std::env::var_os("LILIA_OFFSCREEN_DIR").map(PathBuf::from),
            theme: match std::env::var("LILIA_OFFSCREEN_THEME").as_deref() {
                Ok("dark") => ThemeMode::Dark,
                _ => ThemeMode::Light,
            },
            width: number("LILIA_OFFSCREEN_WIDTH", DEFAULT_WIDTH),
            height: number("LILIA_OFFSCREEN_HEIGHT", DEFAULT_HEIGHT),
        }
    }

    fn theme_name(&self) -> &'static str {
        if self.theme == ThemeMode::Dark {
            "dark"
        } else {
            "light"
        }
    }

    fn paint(&self, scene: &str, snapshot: PrimaryShellSnapshot) {
        self.paint_with(scene, snapshot, |_, _, _, _| {});
    }

    /// Paint after `prepare` has acted on the laid-out shell, as a user would.
    fn paint_with(
        &self,
        scene: &str,
        mut snapshot: PrimaryShellSnapshot,
        prepare: impl FnOnce(
            &mut RuntimeDocument,
            &mut ShellHandles,
            &mut PrimaryShellSnapshot,
            LayoutViewport,
        ),
    ) {
        let Some(mut gpu) = offscreen::optional() else {
            assert!(
                self.dir.is_none(),
                "LILIA_OFFSCREEN_DIR is set but no offscreen GPU adapter is available"
            );
            return;
        };
        snapshot.theme = self.theme;
        let (mut document, mut handles) =
            mount_primary_shell(&snapshot, Arc::new(|_| {})).expect("mount product shell");
        handles
            .sync(&mut document, &snapshot)
            .expect("sync product shell");
        prepare(
            &mut document,
            &mut handles,
            &mut snapshot,
            LayoutViewport::new(self.width as f32, self.height as f32),
        );
        let pixels = render(&mut document, &mut gpu, self.width, self.height);
        let colors = unique_colors(&pixels);
        assert!(colors > 8, "{scene} painted only {colors} colours");
        let Some(dir) = &self.dir else {
            return;
        };
        let path = dir.join(format!(
            "{scene}-{}x{}-{}.png",
            self.width,
            self.height,
            self.theme_name()
        ));
        offscreen::write_png(&path, Size::new(self.width, self.height), &pixels)
            .expect("write snapshot png");
        println!("offscreen snapshot: {}", path.display());
    }
}

fn render(
    document: &mut RuntimeDocument,
    gpu: &mut offscreen::OffscreenSnapshots,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut shaper = NanaTextShaper::default();
    let viewport = LayoutViewport::new(width as f32, height as f32);
    // The first flush settles measured text; the second places dependent rows.
    document.flush(viewport, &mut shaper).expect("flush");
    document.flush(viewport, &mut shaper).expect("flush");
    let renderers = gpu.default_gpu_renderers();
    let color = document
        .context()
        .world()
        .style_model()
        .color(SemanticColorRole::Background);
    gpu.paint(
        document.scene(),
        Size::new(width, height),
        [color.r, color.g, color.b, color.a],
        None,
        Some(&renderers),
    )
    .expect("paint scene")
}

fn unique_colors(pixels: &[u8]) -> usize {
    pixels
        .chunks_exact(4)
        .map(|pixel| u32::from_be_bytes([0, pixel[0], pixel[1], pixel[2]]))
        .collect::<HashSet<_>>()
        .len()
}

fn sidebar_row(id: &str, label: &str, kind: ShellSidebarKind) -> ShellSidebarRow {
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
        can_menu: true,
        can_draft: false,
        attention: ShellSidebarAttention::Quiet,
    }
}

fn message(id: &str, role: TimelineRole, markdown: &str) -> TimelineRow {
    TimelineRow {
        id: id.to_owned(),
        role,
        tone: TimelineTone::Settled,
        title: String::new(),
        detail: String::new(),
        markdown: markdown.to_owned(),
        images: Vec::new(),
        expanded: true,
        can_expand: false,
        can_retry: false,
        can_copy: true,
        can_branch: role == TimelineRole::Reply,
        key_node: true,
    }
}

fn step(id: &str, kind: StepKind, tone: TimelineTone, detail: &str, body: &str) -> TimelineRow {
    TimelineRow {
        id: id.to_owned(),
        role: TimelineRole::Step(kind),
        tone,
        title: kind.label().to_owned(),
        detail: detail.to_owned(),
        markdown: body.to_owned(),
        images: Vec::new(),
        expanded: false,
        can_expand: !body.is_empty(),
        can_retry: tone == TimelineTone::Failed,
        can_copy: false,
        can_branch: false,
        key_node: false,
    }
}

/// A populated conversation: projects and tasks in the sidebar, a short
/// exchange in the timeline, and the composer at rest.
pub(crate) fn conversation_fixture() -> PrimaryShellSnapshot {
    let mut snapshot = empty_snapshot();
    snapshot.title_parent = "LiliaCode".to_owned();
    snapshot.title_context = "整理桌面视图层".to_owned();
    snapshot.heading = String::new();
    snapshot.provider_badge = "Claude".to_owned();
    let mut project = sidebar_row("project:lilia", "LiliaCode", ShellSidebarKind::Project);
    project.expanded = Some(true);
    project.ancestor = true;
    let mut active = sidebar_row("task:redesign", "整理桌面视图层", ShellSidebarKind::Task);
    active.depth = 1;
    active.selected = true;
    let mut running = sidebar_row("task:tests", "补齐离屏截图", ShellSidebarKind::Task);
    running.depth = 1;
    running.can_stop = true;
    running.attention = ShellSidebarAttention::Running;
    running.stop_turn_id = Some("turn-1".to_owned());
    let mut waiting = sidebar_row("task:review", "审阅改动", ShellSidebarKind::Task);
    waiting.depth = 1;
    waiting.attention = ShellSidebarAttention::Waiting;
    snapshot.sidebar_rows = vec![
        sidebar_row("projects-header", "项目", ShellSidebarKind::Header),
        project,
        active,
        running,
        waiting,
        sidebar_row("project:nana", "NanaUI", ShellSidebarKind::Project),
        {
            let mut inbox = sidebar_row("inbox", "收集箱", ShellSidebarKind::Inbox);
            inbox.expanded = Some(true);
            inbox
        },
        {
            let mut task = sidebar_row("task:inbox", "随手记一个想法", ShellSidebarKind::Task);
            task.depth = 0;
            task
        },
    ];
    snapshot.timeline.target.task_id = Some("task:redesign".to_owned());
    let mut expanded = step(
        "step-run",
        StepKind::Run,
        TimelineTone::Settled,
        "cargo test -p lilia-desktop",
        "```text
test result: ok. 656 passed; 0 failed
```",
    );
    expanded.expanded = true;
    snapshot.timeline.rows = vec![
        message(
            "user-1",
            TimelineRole::User,
            "把桌面视图层按表面拆开，并先恢复离屏截图。",
        ),
        step(
            "step-read",
            StepKind::Read,
            TimelineTone::Settled,
            "apps/desktop/src/runtime_shell.rs",
            "",
        ),
        step(
            "step-edit",
            StepKind::Edit,
            TimelineTone::Settled,
            "apps/desktop/src/ui/timeline.rs",
            "新增步骤类型映射",
        ),
        expanded,
        step(
            "step-search",
            StepKind::Search,
            TimelineTone::Failed,
            "rg TimelineRow",
            "",
        ),
        step(
            "step-think",
            StepKind::Thinking,
            TimelineTone::Running,
            "对照旧版时间线的轨道与节点",
            "",
        ),
        message(
            "assistant-1",
            TimelineRole::Reply,
            "已拆出合同层，测试全部通过。

- 删除未编译的旧文件
- 恢复 `cargo xtask screenshot`

```rust
fn main() {
    println!(\"hello\");
}
```",
        ),
    ];
    snapshot
}

/// The conversation with the work panel open on the session tab.
pub(crate) fn work_panel_fixture() -> PrimaryShellSnapshot {
    let mut snapshot = conversation_fixture();
    snapshot.inspector_title = "会话详情".to_owned();
    snapshot.inspector_kind = "task".to_owned();
    snapshot.inspector_body = "待办 2 · 产物 0 · 待处理 0".to_owned();
    snapshot.todo_panel = crate::todo_panel::TodoPanelSnapshot {
        visible: true,
        locked: false,
        todos: [
            (
                "todo-1",
                "补齐离屏截图场景",
                crate::application::DesktopTodoSource::Agent,
                None,
            ),
            (
                "guide-1",
                "完成后对照旧版侧栏再检查一遍",
                crate::application::DesktopTodoSource::Lilia,
                Some(crate::application::DesktopTodoGuideStatus::Pending),
            ),
        ]
        .into_iter()
        .map(
            |(id, text, source, guide_status)| crate::application::DesktopTaskTodo {
                id: id.to_owned(),
                task_id: lilia_contracts::TaskId::new("task:redesign").unwrap(),
                text: text.to_owned(),
                done: false,
                order: 0,
                source,
                priority: crate::application::DesktopTodoPriority::Normal,
                guide_status,
                attachments: Vec::new(),
                conversation_references: Vec::new(),
                workflow: None,
                created_at: 0,
                updated_at: 0,
            },
        )
        .collect(),
        goal: Some(crate::application::DesktopGoalSnapshot {
            thread_id: "thread".to_owned(),
            objective: "按 Tauri 风格还原桌面界面".to_owned(),
            status: crate::application::DesktopGoalStatus::Active,
            token_budget: Some(200_000),
            tokens_used: 48_210,
            time_used_seconds: 600,
            created_at: 0,
            updated_at: 0,
        }),
        editor: None,
        draft: String::new(),
        error: None,
    };
    snapshot.inspector_tabs = [
        ("task-inspector", "会话", true),
        ("coding-tools", "编码", false),
        ("iab", "浏览器", false),
    ]
    .into_iter()
    .map(|(id, label, selected)| ShellNavItem {
        id: id.to_owned(),
        label: label.to_owned(),
        settings: false,
        selected,
    })
    .collect();
    snapshot
        .workspace
        .layout_mut()
        .set_hidden(&nana_ui::RegionId::Inspector, false);
    snapshot
}

/// A project's session list.
pub(crate) fn sessions_fixture() -> PrimaryShellSnapshot {
    let mut snapshot = conversation_fixture();
    snapshot.timeline.rows.clear();
    snapshot.project_page = Some(crate::runtime_shell::ShellProjectPage::Sessions);
    snapshot.project_page_title = "LiliaCode".to_owned();
    snapshot.project_page_body = "5 个会话".to_owned();
    snapshot.session_cards = [
        "整理桌面视图层",
        "补齐离屏截图",
        "审阅改动",
        "验证 Native MCP 表单交互",
        "验证 Native 计划重启回放",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, title)| crate::runtime_shell::ShellTaskRow {
        id: lilia_contracts::TaskId::new(format!("task-{index}")).unwrap(),
        title: title.to_owned(),
        selected: index == 0,
    })
    .collect();
    snapshot
}

/// An agent question waiting above the composer.
pub(crate) fn pending_fixture() -> PrimaryShellSnapshot {
    use crate::module::composer::pending_view::{
        AskUserPending, PendingKind, PendingOption, PendingSnapshot,
    };
    let mut snapshot = conversation_fixture();
    snapshot.pending = Some(PendingSnapshot {
        stop_target: None,
        request_id: "request-1".to_owned(),
        kind: PendingKind::AskUser,
        title: "选择拆分方式".to_owned(),
        prompt: "视图层要按哪种粒度拆开？".to_owned(),
        draft: String::new(),
        options: [
            ("surface", "按表面拆分", true),
            ("feature", "按功能模块拆分", false),
        ]
        .into_iter()
        .map(|(id, label, selected)| PendingOption {
            id: id.to_owned(),
            label: label.to_owned(),
            selected,
            danger: false,
        })
        .collect(),
        tool: None,
        ask: Some(AskUserPending {
            show_other: true,
            other_selected: false,
            freeform: String::new(),
            show_freeform: false,
            show_skip: true,
            show_back: false,
            show_cancel: false,
            show_reject: false,
            can_submit: true,
            submit_label: "继续".to_owned(),
            reject_label: "拒绝".to_owned(),
        }),
        mcp: None,
    });
    snapshot.composer.pending_blocks_send = true;
    snapshot
}

/// The coding tab with uncommitted changes and one diff open.
pub(crate) fn coding_fixture() -> PrimaryShellSnapshot {
    use crate::runtime_shell::{ShellActionRow, ShellCodingChange, ShellCodingSnapshot};
    let mut snapshot = work_panel_fixture();
    snapshot.inspector_kind = "coding".to_owned();
    for tab in &mut snapshot.inspector_tabs {
        tab.selected = tab.id == "coding-tools";
    }
    let patch = [
        "diff --git a/apps/desktop/src/ui/theme.rs b/apps/desktop/src/ui/theme.rs",
        "--- a/apps/desktop/src/ui/theme.rs",
        "+++ b/apps/desktop/src/ui/theme.rs",
        "@@ -10,4 +10,6 @@",
        " pub(crate) const TIMELINE_WIDTH: f32 = 760.0;",
        "-pub(crate) const TIMELINE_RAIL: f32 = 24.0;",
        "+pub(crate) const TIMELINE_RAIL: f32 = 28.0;",
        "+/// Step icon disc on the rail.",
        "+pub(crate) const TIMELINE_NODE: f32 = 22.0;",
        " pub(crate) const BUBBLE_MAX_WIDTH: f32 = 620.0;",
    ]
    .join("\n");
    let diff = crate::application::DocumentDiff::parse_unified(&patch)
        .into_iter()
        .next()
        .map(|(_, diff)| diff);
    snapshot.coding = Some(ShellCodingSnapshot {
        query: String::new(),
        mode_label: "文本".to_owned(),
        scope_label: "当前项目".to_owned(),
        busy: false,
        git: "main · 3 项更改".to_owned(),
        diff_scope_label: "工作区".to_owned(),
        changes: vec![
            ShellCodingChange {
                path: "apps/desktop/src/ui/theme.rs".to_owned(),
                status: "M",
                additions: 3,
                deletions: 1,
            },
            ShellCodingChange {
                path: "apps/desktop/src/ui/coding_panel.rs".to_owned(),
                status: "A",
                additions: 412,
                deletions: 0,
            },
            ShellCodingChange {
                path: "apps/desktop/src/runtime_pending.rs".to_owned(),
                status: "D",
                additions: 0,
                deletions: 824,
            },
        ],
        selected_change: Some("apps/desktop/src/ui/theme.rs".to_owned()),
        selected_diff: diff,
        diff_truncated: false,
        files: Vec::new(),
        hits: Vec::new(),
        terminals: vec![ShellActionRow {
            id: "terminal-1".to_owned(),
            label: "cargo test -p lilia-desktop".to_owned(),
        }],
        tasks: Vec::new(),
    });
    snapshot
}

/// The settings page open on `tab`.
pub(crate) fn settings_fixture(tab: &str) -> PrimaryShellSnapshot {
    let mut snapshot = empty_snapshot();
    snapshot.navigation = crate::navigation::WindowRoute::Settings;
    snapshot.title_context = "设置".to_owned();
    let model = crate::desktop::product_settings_model().expect("settings model");
    let mut state = nana_ui::SettingsState::new(&model);
    state.select(&model, &nana_ui::SettingsTabId::new(tab));
    assert_eq!(state.active_tab().as_str(), tab);
    snapshot.settings.model = model;
    snapshot.settings.state = state;
    snapshot
}

/// The skills page with one skill open.
pub(crate) fn extensions_fixture() -> PrimaryShellSnapshot {
    use crate::module::extensions::ExtensionEntry;
    use crate::runtime_shell::ShellIntent;
    use crate::runtime_surface::SurfaceControl;
    let mut snapshot = settings_fixture("extensions");
    let entry = |key: &str, label: &str, meta: &str, enabled| ExtensionEntry {
        key: key.to_owned(),
        label: label.to_owned(),
        meta: meta.to_owned(),
        enabled,
    };
    snapshot.settings.extensions = Some(crate::runtime_extensions::ExtensionBrowserSnapshot {
        tab: "extensions".to_owned(),
        query: String::new(),
        entries: vec![
            entry("skill:user:review", "code-review", "用户技能", true),
            entry("skill:project:release", "release-notes", "项目技能", true),
            entry("skill:plugin:lint", "lint-fix", "插件技能", false),
        ],
        selected: Some("skill:user:review".to_owned()),
        title: "code-review · 用户技能".to_owned(),
        toolbar: vec![
            SurfaceControl::action("refresh", "刷新", ShellIntent::RefreshExtensions),
            SurfaceControl::action("create", "新建技能", ShellIntent::RefreshExtensions),
        ],
        detail_actions: vec![
            SurfaceControl::action("toggle", "停用技能", ShellIntent::RefreshExtensions),
            SurfaceControl::action("open", "打开技能文件", ShellIntent::RefreshExtensions),
        ],
        detail: vec![SurfaceControl::text(
            "detail",
            "code-review · 用户技能\n审阅当前分支的改动，按严重程度列出问题。\nC:\\Users\\me\\.lilia\\skills\\code-review\\SKILL.md",
        )],
        editor: None,
    });
    snapshot
}

/// The automations page with one published workflow selected.
pub(crate) fn automation_fixture() -> PrimaryShellSnapshot {
    use crate::module::automation::view::{AutomationRow, AutomationTarget};
    let mut snapshot = empty_snapshot();
    snapshot.navigation = crate::navigation::WindowRoute::Automations;
    snapshot.title_context = "自动化".to_owned();
    snapshot.automation.rows = vec![
        AutomationRow {
            id: "workflow-1".to_owned(),
            label: "每日回归".to_owned(),
            selected: true,
        },
        AutomationRow {
            id: "workflow-2".to_owned(),
            label: "提交前审阅".to_owned(),
            selected: false,
        },
    ];
    snapshot.automation.target = Some(AutomationTarget {
        window_id: crate::runtime_compat::HostedWindowId::PRIMARY,
        workflow_id: "workflow-1".to_owned(),
        modified_at: 0,
    });
    snapshot.automation.name = "每日回归".to_owned();
    snapshot.automation.published = true;
    snapshot.automation.enabled = true;
    snapshot
}

#[test]
fn product_shell_paints_offscreen() {
    let capture = Capture::from_env();
    capture.paint("empty", empty_snapshot());
    capture.paint("conversation", conversation_fixture());
    capture.paint("work-panel", work_panel_fixture());
    capture.paint("sessions", sessions_fixture());
    capture.paint("pending", pending_fixture());
    capture.paint("coding", coding_fixture());
    capture.paint("automation", automation_fixture());
    let mut toast = conversation_fixture();
    toast.toast = Some(crate::runtime_shell::ShellToast {
        key: crate::runtime_shell::ShellToastKey::Error,
        title: "无法写入终端：会话已关闭".to_owned(),
    });
    capture.paint("toast", toast);
    let mut reading = conversation_fixture();
    reading.timeline.layout =
        nana_ui::VirtualListLayout::new(std::iter::repeat_n(400.0, reading.timeline.rows.len()));
    capture.paint("reading", reading);
    capture.paint_with(
        "selection",
        conversation_fixture(),
        |document, handles, snapshot, viewport| {
            let mut shaper = NanaTextShaper::default();
            document.flush(viewport, &mut shaper).expect("flush");
            let reply = snapshot
                .timeline
                .rows
                .iter()
                .rev()
                .find(|row| row.role == TimelineRole::Reply)
                .map(|row| row.id.clone())
                .expect("a reply");
            let markdown = handles.timeline_view().timeline_markdown[&reply];
            let context = document.context_mut();
            let area = context
                .world()
                .canonical_layout_box(markdown.stable_id())
                .expect("laid-out reply");
            let line = area.y + 8.0;
            context
                .update_component(markdown, |markdown, _| {
                    markdown.pointer_down(area.x + 1.0, line, area);
                    markdown.pointer_move(area.x + 100.0, line, area);
                    markdown.pointer_up(area.x + 100.0, line, area);
                })
                .expect("select reply text");
            snapshot.timeline.selection = Some(reply);
            handles.sync(document, snapshot).expect("sync selection");
            document.flush(viewport, &mut shaper).expect("flush");
            handles
                .sync(document, snapshot)
                .expect("place selection bar");
        },
    );
    let mut completing = conversation_fixture();
    completing.composer.composer = "/".to_owned();
    completing.composer.slash_items = [
        ("review", "审阅改动"),
        ("plan", "先出计划"),
        ("compact", "压缩上下文"),
    ]
    .into_iter()
    .map(
        |(name, label)| crate::module::composer::presentation::ComposerSlashItem {
            name: name.to_owned(),
            label: label.to_owned(),
        },
    )
    .collect();
    capture.paint("completion", completing);
    for tab in ["appearance", "provider", "agent", "remote", "about"] {
        capture.paint(&format!("settings-{tab}"), settings_fixture(tab));
    }
    capture.paint("settings-extensions", extensions_fixture());
}
