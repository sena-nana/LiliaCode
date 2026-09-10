use std::collections::HashMap;
use std::sync::Arc;

use nana_ui::runtime::{
    Activate, AppContext, Dialog, DocumentId, Entity, FrameworkError, LengthSpec, ModalSlots,
    OverlayHost, ScrollAxes, ScrollView, SemanticColorRole, SidebarRow, SidebarRowState,
    StableNodeId, Stack, Text, TextChanged, TextInput,
};
use nana_ui::{DialogClosePolicy, DialogSize, UI_METRICS};

use crate::module::extensions::ExtensionEntry;
use crate::runtime_shell::ShellIntent;
use crate::runtime_surface::{SurfaceControl, SurfaceHandles};
use crate::shell::ExtensionsMessage;

type Sink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

#[derive(Clone, Debug)]
pub(crate) struct ExtensionBrowserSnapshot {
    pub tab: String,
    pub query: String,
    pub entries: Vec<ExtensionEntry>,
    pub selected: Option<String>,
    pub title: String,
    pub detail_actions: Vec<SurfaceControl>,
    pub toolbar: Vec<SurfaceControl>,
    pub detail: Vec<SurfaceControl>,
    pub editor: Option<ExtensionEditorSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::LayoutViewport;
    use std::sync::Mutex;

    fn browser_snapshot() -> ExtensionBrowserSnapshot {
        ExtensionBrowserSnapshot {
            tab: "extensions".into(),
            query: String::new(),
            selected: Some("skill:user:a".into()),
            title: "A".into(),
            entries: vec![
                ExtensionEntry {
                    key: "skill:user:a".into(),
                    label: "A".into(),
                    meta: "用户技能".into(),
                    enabled: true,
                },
                ExtensionEntry {
                    key: "skill:user:b".into(),
                    label: "B".into(),
                    meta: "用户技能".into(),
                    enabled: false,
                },
            ],
            toolbar: vec![],
            detail_actions: vec![],
            detail: vec![SurfaceControl::text("detail", "Detail")],
            editor: None,
        }
    }

    #[test]
    fn extensions_browser_uses_window_breakpoint_and_preserves_independent_scroll_regions() {
        let doc = DocumentId::new(791).unwrap();
        let mut cx = AppContext::new();
        let host = cx
            .create_component(doc, Stack::fill_column(0.0).width(LengthSpec::Px(640.0)))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        let sink: Sink = Arc::new(move |intent| output.lock().unwrap().push(intent));
        let mut browser = ExtensionBrowser::mount(&mut cx, doc, sink.clone()).unwrap();
        cx.append_child(host, browser.root).unwrap();
        let mut snapshot = browser_snapshot();
        for window_width in [960.0, 700.0, 960.0] {
            browser
                .sync(&mut cx, doc, &snapshot, window_width, sink.clone())
                .unwrap();
            cx.layout_document(doc, LayoutViewport::new(window_width, 800.0))
                .unwrap();
            let left = cx
                .world()
                .layout_box(browser.list_panel.stable_id())
                .unwrap();
            let right = cx
                .world()
                .layout_box(browser.detail_panel.stable_id())
                .unwrap();
            let list = cx
                .world()
                .layout_box(browser.list_scroll.stable_id())
                .unwrap();
            let detail = cx
                .world()
                .layout_box(browser.detail_scroll.stable_id())
                .unwrap();
            assert!(
                list.height > 0.0 && detail.height > 0.0,
                "both scroll regions must have usable viewports"
            );
            if window_width > 760.0 {
                assert!((left.width - 320.0).abs() < 1.0);
                assert!(right.x >= left.x + left.width - 1.0);
                assert!(
                    left.height > 600.0 && right.height > 600.0,
                    "wide columns fill remaining viewport"
                );
                assert!(cx
                    .world()
                    .node(browser.root.stable_id())
                    .unwrap()
                    .children
                    .contains(&browser.body.stable_id()));
            } else {
                assert!(left.height >= 260.0 && right.height >= 360.0);
                assert!(right.y >= left.y + left.height - 1.0);
                assert!(cx
                    .world()
                    .node(browser.content_scroll.stable_id())
                    .unwrap()
                    .children
                    .contains(&browser.body.stable_id()));
            }
        }
        assert!(cx
            .replace_text_input_selection(browser.search, "review")
            .unwrap());
        assert!(
            matches!(events.lock().unwrap().last(), Some(ShellIntent::ExtensionsCommand(ExtensionsMessage::SearchChanged(query))) if query == "review")
        );
        assert!(cx
            .activate_node(browser.rows["skill:user:b"].stable_id())
            .unwrap());
        assert!(
            matches!(events.lock().unwrap().last(), Some(ShellIntent::ExtensionsCommand(ExtensionsMessage::SelectEntry { key, .. })) if key == "skill:user:b")
        );
        snapshot.entries.remove(0);
        snapshot.selected = Some("skill:user:b".into());
        browser.sync(&mut cx, doc, &snapshot, 960.0, sink).unwrap();
        assert!(!browser.rows.contains_key("skill:user:a"));
        assert_eq!(
            cx.read(browser.rows["skill:user:b"], |row| row.state)
                .unwrap(),
            SidebarRowState::Active
        );
    }

    #[test]
    fn extensions_plugin_toolbar_keeps_install_visible_at_window_breakpoints() {
        let doc = DocumentId::new(793).unwrap();
        let mut cx = AppContext::new();
        let host = cx.create_component(doc, Stack::fill_column(0.0)).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        let sink: Sink = Arc::new(move |intent| output.lock().unwrap().push(intent));
        let mut browser = ExtensionBrowser::mount(&mut cx, doc, sink.clone()).unwrap();
        cx.append_child(host, browser.root).unwrap();
        let mut snapshot = browser_snapshot();
        snapshot.tab = "plugin-packages".into();
        snapshot.toolbar = vec![
            SurfaceControl::action(
                "extensions-refresh",
                "刷新",
                ShellIntent::ExtensionsCommand(ExtensionsMessage::Refresh),
            ),
            SurfaceControl::field(
                "plugin-source",
                "插件目录",
                "/tmp/a-long-plugin-directory-for-installation",
                false,
                |value| {
                    ShellIntent::ExtensionsCommand(ExtensionsMessage::PluginSourceChanged(value))
                },
            ),
            SurfaceControl::action(
                "plugin-pick",
                "选择插件目录",
                ShellIntent::ExtensionsCommand(ExtensionsMessage::PickPluginDirectory),
            ),
            SurfaceControl::action(
                "plugin-install",
                "安装插件",
                ShellIntent::ExtensionsCommand(ExtensionsMessage::InstallPlugin),
            ),
        ];
        let mut shaper = nana_ui::NanaTextShaper::default();
        for width in [1180.0, 960.0, 700.0, 960.0] {
            cx.update_component(host, |host, _| {
                *host = Stack::fill_column(0.0).width(LengthSpec::Px(width - 240.0));
            })
            .unwrap();
            browser
                .sync(&mut cx, doc, &snapshot, width, sink.clone())
                .unwrap();
            let text_nodes = cx.world().document_order(doc);
            cx.resolve_styles(&text_nodes).unwrap();
            cx.shape_text(&text_nodes, &mut shaper).unwrap();
            cx.layout_document(doc, LayoutViewport::new(width, 800.0))
                .unwrap();
            let toolbar = cx.world().layout_box(browser.toolbar.stable_id()).unwrap();
            let source = cx
                .world()
                .layout_box(browser.plugin_source.stable_id())
                .unwrap();
            let targets = browser.debug_nodes();
            for name in [
                "plugin-source",
                "extensions-refresh",
                "plugin-pick",
                "plugin-install",
            ] {
                let target = targets.iter().find(|(id, _)| id == name).unwrap().1;
                let bounds = cx.world().layout_box(target).unwrap();
                assert!(
                    bounds.width >= 32.0 && bounds.height >= 28.0,
                    "{width}: {name} has usable bounds {bounds:?}"
                );
                assert!(
                    bounds.x >= toolbar.x
                        && bounds.x + bounds.width <= toolbar.x + toolbar.width + 0.5,
                    "{width}: {name} exceeds toolbar: {bounds:?}, toolbar={toolbar:?}"
                );
                assert!(
                    bounds.y >= toolbar.y
                        && bounds.y + bounds.height <= toolbar.y + toolbar.height + 0.5
                );
            }
            for (name, expected) in [
                ("extensions-refresh", "刷新"),
                ("plugin-pick", "选择插件目录"),
                ("plugin-install", "安装插件"),
            ] {
                let target = targets.iter().find(|(id, _)| id == name).unwrap().1;
                let bounds = cx.world().layout_box(target).unwrap();
                let Some(nana_ui::runtime::ComponentGeometry::Button { label, .. }) =
                    cx.world().component_geometry(target)
                else {
                    panic!("expected real button geometry for {name}");
                };
                let measured = cx.world().text_metrics(target).unwrap();
                assert_eq!(label.content.as_ref(), expected);
                assert!(measured.width > 0.0);
                assert!((label.bounds.width - measured.width).abs() < 0.5,
                    "{width}: {name} must display its full shaped label: {label:?}, metrics={measured:?}");
                assert!(
                    label.bounds.x >= bounds.x
                        && label.bounds.x + label.bounds.width <= bounds.x + bounds.width + 0.5
                );
            }
            let actions = cx
                .world()
                .layout_box(browser.toolbar_actions.stable_id())
                .unwrap();
            if width > 760.0 {
                assert!(source.x + source.width <= actions.x);
                assert!(
                    (source.y - actions.y).abs() < 1.0,
                    "source={source:?}, actions={actions:?}"
                );
            } else {
                assert!(actions.y >= source.y + source.height);
            }
            let install = targets
                .iter()
                .find(|(id, _)| id == "plugin-install")
                .unwrap()
                .1;
            assert!(cx.activate_node(install).unwrap());
            assert!(matches!(
                events.lock().unwrap().pop(),
                Some(ShellIntent::ExtensionsCommand(
                    ExtensionsMessage::InstallPlugin
                ))
            ));
        }
        assert!(cx
            .replace_text_input_selection(browser.plugin_source, "-edited")
            .unwrap());
        assert!(
            matches!(events.lock().unwrap().last(), Some(ShellIntent::ExtensionsCommand(ExtensionsMessage::PluginSourceChanged(value))) if value.contains("-edited"))
        );
    }

    #[test]
    fn extensions_detail_header_wraps_actions_without_clipping_labels() {
        let doc = DocumentId::new(794).unwrap();
        let mut cx = AppContext::new();
        let host = cx.create_component(doc, Stack::fill_column(0.0)).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        let sink: Sink = Arc::new(move |intent| output.lock().unwrap().push(intent));
        let mut browser = ExtensionBrowser::mount(&mut cx, doc, sink.clone()).unwrap();
        cx.append_child(host, browser.root).unwrap();
        let mut snapshot = browser_snapshot();
        snapshot.title = "Native Local Fixture · 1.0.0".into();
        snapshot.entries[0].label = "很长的用户技能名称用于检验单行省略".into();
        snapshot.detail_actions = ["编辑技能", "打开目录", "删除技能"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                SurfaceControl::action(
                    format!("header-{index}"),
                    label,
                    ShellIntent::ExtensionsCommand(ExtensionsMessage::Refresh),
                )
            })
            .collect();
        for width in [960.0, 1440.0, 700.0] {
            cx.update_component(host, |host, _| {
                *host = Stack::fill_column(0.0).width(LengthSpec::Px(width - 240.0))
            })
            .unwrap();
            browser
                .sync(&mut cx, doc, &snapshot, width, sink.clone())
                .unwrap();
            let ids = cx.world().document_order(doc);
            cx.resolve_styles(&ids).unwrap();
            cx.shape_text(&ids, &mut nana_ui::NanaTextShaper::default())
                .unwrap();
            cx.layout_document(doc, LayoutViewport::new(width, 900.0))
                .unwrap();
            let header = cx
                .world()
                .layout_box(browser.detail_header.stable_id())
                .unwrap();
            let title = cx
                .world()
                .layout_box(browser.detail_title.stable_id())
                .unwrap();
            assert!(
                title.width >= 120.0 && title.height < 40.0,
                "{width}: {title:?}"
            );
            let row = cx
                .world()
                .layout_box(browser.rows["skill:user:a"].stable_id())
                .unwrap();
            assert!((row.height - 28.0).abs() < 0.5);
            let targets = browser.debug_nodes();
            for index in 0..3 {
                let target = targets
                    .iter()
                    .find(|(id, _)| id == &format!("header-{index}"))
                    .unwrap()
                    .1;
                let bounds = cx.world().layout_box(target).unwrap();
                assert!(
                    bounds.x >= header.x
                        && bounds.x + bounds.width <= header.x + header.width + 0.5
                        && bounds.y >= header.y
                        && bounds.y + bounds.height <= header.y + header.height + 0.5,
                    "{width}: action {bounds:?} outside {header:?}"
                );
                let Some(nana_ui::runtime::ComponentGeometry::Button { label, .. }) =
                    cx.world().component_geometry(target)
                else {
                    panic!("button");
                };
                let metrics = cx.world().text_metrics(target).unwrap();
                assert!(metrics.width > 20.0 && (label.bounds.width - metrics.width).abs() < 0.5);
                assert!(label.bounds.x + label.bounds.width <= bounds.x + bounds.width + 0.5);
                assert!(cx.activate_node(target).unwrap());
                assert!(matches!(
                    events.lock().unwrap().pop(),
                    Some(ShellIntent::ExtensionsCommand(ExtensionsMessage::Refresh))
                ));
            }
        }
    }

    #[test]
    fn extensions_editor_dialog_reopens_with_live_controls_and_dispatches_cancel() {
        let doc = DocumentId::new(792).unwrap();
        let mut cx = AppContext::new();
        let host = cx.create_component(doc, OverlayHost::new()).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        let sink: Sink = Arc::new(move |intent| output.lock().unwrap().push(intent));
        let mut browser = ExtensionBrowser::mount(&mut cx, doc, sink.clone()).unwrap();
        let mut editor = ExtensionEditorSnapshot {
            title: "编辑技能".into(),
            controls: vec![SurfaceControl::field(
                "name",
                "名称",
                "review",
                false,
                |value| ShellIntent::ExtensionsCommand(ExtensionsMessage::SkillIdChanged(value)),
            )],
            actions: vec![SurfaceControl::action(
                "cancel",
                "取消",
                ShellIntent::ExtensionsCommand(ExtensionsMessage::CancelEditor),
            )],
            busy: false,
        };
        for cycle in 0..3 {
            let started = cycle * 1_000;
            cx.advance_animations(std::time::Duration::from_millis(started));
            assert!(browser
                .sync_dialog(&mut cx, doc, host, Some(&editor), sink.clone())
                .unwrap());
            cx.layout_document(doc, LayoutViewport::new(960.0, 800.0))
                .unwrap();
            cx.advance_animations(std::time::Duration::from_millis(started + 400));
            let id = browser
                .debug_nodes()
                .into_iter()
                .find(|(key, _)| key == "cancel")
                .unwrap()
                .1;
            assert!(cx.activate_node(id).unwrap());
            assert!(matches!(
                events.lock().unwrap().pop(),
                Some(ShellIntent::ExtensionsCommand(
                    ExtensionsMessage::CancelEditor
                ))
            ));
            editor.busy = true;
            browser
                .sync_dialog(&mut cx, doc, host, Some(&editor), sink.clone())
                .unwrap();
            assert!(cx
                .read(browser.dialog.unwrap(), |dialog| dialog
                    .close_policy
                    .close_disabled)
                .unwrap());
            editor.busy = false;
            browser
                .sync_dialog(&mut cx, doc, host, Some(&editor), sink.clone())
                .unwrap();
            browser
                .sync_dialog(&mut cx, doc, host, None, sink.clone())
                .unwrap();
            assert!(
                browser.dialog_id().is_some(),
                "retain controls during the normal exit animation"
            );
            for frame in 1..=25 {
                cx.advance_animations(std::time::Duration::from_millis(started + 400 + frame * 16));
                browser
                    .sync_dialog(&mut cx, doc, host, None, sink.clone())
                    .unwrap();
            }
            assert!(browser.dialog_id().is_none());
            assert!(!cx.world().contains(id));
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExtensionEditorSnapshot {
    pub title: String,
    pub controls: Vec<SurfaceControl>,
    pub actions: Vec<SurfaceControl>,
    pub busy: bool,
}

pub(crate) struct ExtensionBrowser {
    pub root: Entity<Stack>,
    toolbar: Entity<Stack>,
    toolbar_actions: Entity<Stack>,
    plugin_source: Entity<TextInput>,
    #[cfg(debug_assertions)]
    plugin_source_visible: bool,
    body: Entity<Stack>,
    content_scroll: Entity<ScrollView>,
    list_panel: Entity<Stack>,
    search: Entity<TextInput>,
    list: Entity<Stack>,
    list_scroll: Entity<ScrollView>,
    detail: Entity<Stack>,
    detail_panel: Entity<Stack>,
    detail_header: Entity<Stack>,
    detail_title: Entity<Text>,
    detail_action_row: Entity<Stack>,
    detail_scroll: Entity<ScrollView>,
    empty: Entity<Text>,
    rows: HashMap<String, Entity<SidebarRow>>,
    toolbar_surface: SurfaceHandles,
    detail_surface: SurfaceHandles,
    detail_action_surface: SurfaceHandles,
    editor_surface: SurfaceHandles,
    editor_actions: SurfaceHandles,
    dialog: Option<Entity<Dialog>>,
    dialog_body: Option<Entity<Stack>>,
    dialog_footer: Option<Entity<Stack>>,
    dialog_scroll: Option<Entity<ScrollView>>,
    narrow: Option<bool>,
}

impl ExtensionBrowser {
    pub fn mount(cx: &mut AppContext, doc: DocumentId, sink: Sink) -> Result<Self, FrameworkError> {
        let root = cx.create_detached_component(
            doc,
            Stack::fill_column(0.0)
                .surface(SemanticColorRole::Surface)
                .outline(SemanticColorRole::BorderSoft, 1.0)
                .radius(UI_METRICS.radius_md),
        )?;
        let toolbar = cx.create_detached_component(
            doc,
            Stack::bar(8.0)
                .min_height(LengthSpec::Px(42.0))
                .padding_xy(8.0, 6.0),
        )?;
        let toolbar_actions = cx.create_detached_component(doc, Stack::row(8.0))?;
        let plugin_source =
            cx.create_detached_component(doc, TextInput::new("").placeholder("插件目录"))?;
        let dispatch = sink.clone();
        cx.on(plugin_source, move |_, event: &TextChanged, _| {
            dispatch(ShellIntent::ExtensionsCommand(
                ExtensionsMessage::PluginSourceChanged(event.value.clone()),
            ));
        })?;
        let body = cx.create_detached_component(doc, Stack::fill_row(0.0))?;
        let content_scroll = cx.create_detached_component(
            doc,
            ScrollView::new(ScrollAxes::Vertical).style(Stack::fill_column(0.0).node_style()),
        )?;
        let list_panel = cx.create_detached_component(
            doc,
            Stack::fill_column(8.0)
                .width(LengthSpec::Px(320.0))
                .min_width(LengthSpec::Px(250.0))
                .grow(0.0)
                .shrink(1.0)
                .padding(8.0),
        )?;
        let search = cx.create_detached_component(
            doc,
            TextInput::new("").placeholder("搜索当前列表").layout(
                Stack::bar(0.0)
                    .height(LengthSpec::Px(30.0))
                    .node_style()
                    .layout,
            ),
        )?;
        cx.on(search, move |_, event: &TextChanged, _| {
            sink(ShellIntent::ExtensionsCommand(
                ExtensionsMessage::SearchChanged(event.value.clone()),
            ))
        })?;
        let list = cx.create_detached_component(doc, Stack::column(4.0))?;
        let list_scroll = cx.create_detached_component(
            doc,
            ScrollView::new(ScrollAxes::Vertical).style(Stack::fill_column(0.0).node_style()),
        )?;
        let detail = cx.create_detached_component(doc, Stack::column(12.0).padding(16.0))?;
        let detail_panel = cx.create_detached_component(doc, Stack::fill_column(0.0))?;
        let detail_header = cx.create_detached_component(doc, detail_header_layout(false))?;
        let detail_title = cx.create_detached_component(doc, Text::new("选择一项"))?;
        let detail_action_row = cx.create_detached_component(
            doc,
            crate::runtime_conversation::wrapping_controls_row(6.0),
        )?;
        let detail_scroll = cx.create_detached_component(
            doc,
            ScrollView::new(ScrollAxes::Vertical).style(Stack::fill_column(0.0).node_style()),
        )?;
        let empty = cx.create_detached_component(doc, Text::new("没有匹配的条目"))?;
        cx.append_child(list_scroll, list)?;
        cx.append_child(detail_scroll, detail)?;
        cx.append_child(list_panel, search)?;
        cx.append_child(list_panel, list_scroll)?;
        cx.append_child(body, list_panel)?;
        cx.append_child(detail_header, detail_title)?;
        cx.append_child(detail_header, detail_action_row)?;
        cx.append_child(detail_panel, detail_header)?;
        cx.append_child(detail_panel, detail_scroll)?;
        cx.append_child(body, detail_panel)?;
        cx.append_child(root, toolbar)?;
        cx.append_child(root, body)?;
        Ok(Self {
            root,
            toolbar,
            toolbar_actions,
            plugin_source,
            #[cfg(debug_assertions)]
            plugin_source_visible: false,
            body,
            content_scroll,
            list_panel,
            search,
            list,
            list_scroll,
            detail,
            detail_panel,
            detail_header,
            detail_title,
            detail_action_row,
            detail_scroll,
            empty,
            rows: HashMap::new(),
            toolbar_surface: Default::default(),
            detail_surface: Default::default(),
            detail_action_surface: Default::default(),
            editor_surface: Default::default(),
            editor_actions: Default::default(),
            dialog: None,
            dialog_body: None,
            dialog_footer: None,
            dialog_scroll: None,
            narrow: None,
        })
    }

    pub fn sync(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        view: &ExtensionBrowserSnapshot,
        width: f32,
        sink: Sink,
    ) -> Result<(), FrameworkError> {
        let narrow = width <= 760.0;
        if self.narrow != Some(narrow) {
            cx.reconcile_children(self.root.stable_id(), &[self.toolbar.stable_id()])?;
            cx.reconcile_children(self.content_scroll.stable_id(), &[])?;
            if narrow {
                cx.append_child(self.content_scroll, self.body)?;
                cx.append_child(self.root, self.content_scroll)?;
            } else {
                cx.append_child(self.root, self.body)?;
            }
            self.narrow = Some(narrow);
        }
        cx.update_component(self.body, |body, _| {
            *body = if narrow {
                Stack::column(0.0)
            } else {
                Stack::fill_row(0.0)
            }
        })?;
        cx.update_component(self.list_panel, |panel, _| {
            *panel = if narrow {
                Stack::column(8.0)
                    .min_height(LengthSpec::Px(260.0))
                    .shrink(0.0)
                    .padding(8.0)
            } else {
                Stack::fill_column(8.0)
                    .width(LengthSpec::Px(320.0))
                    .min_width(LengthSpec::Px(250.0))
                    .grow(0.0)
                    .shrink(1.0)
                    .padding(8.0)
            }
        })?;
        cx.update_component(self.search, |search, _| {
            if search.state.value != view.query {
                search.state.replace_value(view.query.clone());
            }
        })?;
        cx.update_component(self.detail_panel, |panel, _| {
            *panel = if narrow {
                Stack::column(0.0)
                    .min_height(LengthSpec::Px(360.0))
                    .shrink(0.0)
            } else {
                Stack::fill_column(0.0)
            };
        })?;
        cx.update_component(self.detail_header, |header, _| {
            *header = detail_header_layout(narrow)
        })?;
        cx.update_component(self.detail_title, |title, _| {
            *title = Text::new(&view.title);
            let layout = Arc::make_mut(&mut title.style.layout);
            layout.width = Some(if narrow {
                LengthSpec::Fill
            } else {
                LengthSpec::Px(0.0)
            });
            layout.min_width = Some(LengthSpec::Px(120.0));
            layout.max_width = Some(LengthSpec::Percent(100.0));
            layout.flex_grow = Some(1.0);
            layout.flex_shrink = Some(1.0);
            layout.white_space = nana_ui_core::WhiteSpaceSpec::Nowrap;
            layout.white_space_nowrap = true;
            layout.text_overflow_ellipsis = true;
        })?;
        let actions =
            self.detail_action_surface
                .sync(cx, doc, &view.detail_actions, sink.clone())?;
        cx.reconcile_children(self.detail_action_row.stable_id(), &actions)?;
        let source = view.toolbar.iter().find_map(|control| match control {
            SurfaceControl::Field { id, value, .. } if id == "plugin-source" => Some(value),
            _ => None,
        });
        #[cfg(debug_assertions)]
        {
            self.plugin_source_visible = source.is_some();
        }
        if let Some(value) = source {
            cx.update_component(self.plugin_source, |input, _| {
                if input.state.value != *value {
                    input.state.replace_value(value.clone());
                }
                input.style.layout = if narrow {
                    Stack::bar(0.0).height(LengthSpec::Px(32.0))
                } else {
                    Stack::bar(0.0)
                        .width(LengthSpec::Px(0.0))
                        .min_width(LengthSpec::Px(120.0))
                        .grow(1.0)
                        .shrink(1.0)
                        .height(LengthSpec::Px(32.0))
                }
                .node_style()
                .layout;
            })?;
        }
        cx.update_component(self.toolbar, |toolbar, _| {
            *toolbar = if narrow && source.is_some() {
                Stack::column(8.0)
            } else {
                Stack::bar(8.0)
            }
            .min_height(LengthSpec::Px(42.0))
            .padding_xy(8.0, 6.0);
        })?;
        let controls = view.toolbar.iter()
            .filter(|control| !matches!(control, SurfaceControl::Field { id, .. } if id == "plugin-source"))
            .cloned().collect::<Vec<_>>();
        let actions = self
            .toolbar_surface
            .sync(cx, doc, &controls, sink.clone())?;
        cx.reconcile_children(self.toolbar_actions.stable_id(), &actions)?;
        let toolbar = if source.is_some() {
            vec![
                self.plugin_source.stable_id(),
                self.toolbar_actions.stable_id(),
            ]
        } else {
            vec![self.toolbar_actions.stable_id()]
        };
        cx.reconcile_children(self.toolbar.stable_id(), &toolbar)?;
        let mut order = Vec::new();
        for entry in &view.entries {
            let selected = view.selected.as_deref() == Some(entry.key.as_str());
            let mut row_view = SidebarRow::new(format!(
                "{} · {}{}",
                entry.label,
                entry.meta,
                if entry.enabled { "" } else { " · 已停用" }
            ))
            .state(if selected {
                SidebarRowState::Active
            } else {
                SidebarRowState::Idle
            })
            .size(nana_ui::ControlSize::Small);
            let row_layout = Arc::make_mut(&mut row_view.style.layout);
            row_layout.white_space = nana_ui_core::WhiteSpaceSpec::Nowrap;
            row_layout.white_space_nowrap = true;
            row_layout.text_overflow_ellipsis = true;
            row_layout.min_width = Some(LengthSpec::Px(0.0));
            row_layout.height = Some(LengthSpec::Px(28.0));
            let row = if let Some(row) = self.rows.get(&entry.key).copied() {
                cx.update_component(row, |r, _| *r = row_view)?;
                row
            } else {
                let row = cx.create_detached_component(doc, row_view)?;
                let target = entry.key.clone();
                let tab = view.tab.clone();
                let dispatch = sink.clone();
                cx.on(row, move |_, _: &Activate, _| {
                    dispatch(ShellIntent::ExtensionsCommand(
                        ExtensionsMessage::SelectEntry {
                            tab: tab.clone(),
                            key: target.clone(),
                        },
                    ))
                })?;
                self.rows.insert(entry.key.clone(), row);
                row
            };
            order.push(row.stable_id());
        }
        if order.is_empty() {
            order.push(self.empty.stable_id());
        }
        cx.reconcile_children(self.list.stable_id(), &order)?;
        let stale = self
            .rows
            .keys()
            .filter(|key| !view.entries.iter().any(|entry| &entry.key == *key))
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(row) = self.rows.remove(&key) {
                cx.remove_view(row)?;
            }
        }
        self.detail_surface.settings_width(width);
        let detail = self.detail_surface.sync(cx, doc, &view.detail, sink)?;
        cx.reconcile_children(self.detail.stable_id(), &detail)?;
        Ok(())
    }

    pub fn dialog_id(&self) -> Option<StableNodeId> {
        self.dialog.map(|dialog| dialog.stable_id())
    }

    pub fn sync_dialog(
        &mut self,
        cx: &mut AppContext,
        doc: DocumentId,
        host: Entity<OverlayHost>,
        editor: Option<&ExtensionEditorSnapshot>,
        sink: Sink,
    ) -> Result<bool, FrameworkError> {
        let Some(editor) = editor else {
            if let Some(dialog) = self.dialog {
                if cx
                    .world()
                    .overlay_host(host.stable_id())
                    .is_some_and(|state| state.active == Some(dialog.stable_id()))
                {
                    cx.dismiss_overlay(host)?;
                }
                if cx
                    .world()
                    .overlay_host(host.stable_id())
                    .is_some_and(|state| state.active == Some(dialog.stable_id()))
                {
                    return Ok(false);
                }
                let controls = self.editor_surface.sync(cx, doc, &[], sink.clone())?;
                if let Some(body) = self.dialog_body {
                    cx.reconcile_children(body.stable_id(), &controls)?;
                }
                let actions = self.editor_actions.sync(cx, doc, &[], sink)?;
                if let Some(footer) = self.dialog_footer {
                    cx.reconcile_children(footer.stable_id(), &actions)?;
                }
                cx.remove_view(dialog)?;
                self.dialog = None;
                self.dialog_body = None;
                self.dialog_footer = None;
                self.dialog_scroll = None;
            }
            return Ok(false);
        };
        let dialog = if let Some(dialog) = self.dialog {
            dialog
        } else {
            let dialog = cx.create_detached_component(
                doc,
                Dialog::new(editor.title.as_str()).size(DialogSize::Wide),
            )?;
            let body = cx.create_detached_component(doc, Stack::column(12.0).padding(8.0))?;
            let scroll = cx.create_detached_component(
                doc,
                ScrollView::new(ScrollAxes::Vertical).style(
                    Stack::column(0.0)
                        .height(LengthSpec::Px(380.0))
                        .node_style(),
                ),
            )?;
            let footer = cx.create_detached_component(doc, Stack::bar(8.0))?;
            cx.append_child(scroll, body)?;
            cx.set_modal_slots(
                dialog,
                ModalSlots {
                    body: Some(scroll.stable_id()),
                    footer: Some(footer.stable_id()),
                    ..Default::default()
                },
            )?;
            cx.append_child(host, dialog)?;
            self.dialog = Some(dialog);
            self.dialog_body = Some(body);
            self.dialog_footer = Some(footer);
            self.dialog_scroll = Some(scroll);
            dialog
        };
        cx.update_component(dialog, |dialog, _| {
            dialog.title = editor.title.clone().into();
            dialog.close_policy = DialogClosePolicy {
                close_disabled: editor.busy,
                ..Default::default()
            };
        })?;
        let controls = self
            .editor_surface
            .sync(cx, doc, &editor.controls, sink.clone())?;
        cx.reconcile_children(self.dialog_body.unwrap().stable_id(), &controls)?;
        let actions = self.editor_actions.sync(cx, doc, &editor.actions, sink)?;
        cx.reconcile_children(self.dialog_footer.unwrap().stable_id(), &actions)?;
        cx.activate_overlay(host, dialog)?;
        Ok(true)
    }

    #[cfg(debug_assertions)]
    pub fn debug_nodes(&self) -> Vec<(String, StableNodeId)> {
        let mut nodes = vec![
            (
                "extensions-content-scroll".into(),
                self.content_scroll.stable_id(),
            ),
            ("extensions-search".into(), self.search.stable_id()),
            (
                "extensions-list-scroll".into(),
                self.list_scroll.stable_id(),
            ),
            (
                "extensions-detail-scroll".into(),
                self.detail_scroll.stable_id(),
            ),
        ];
        if self.plugin_source_visible {
            nodes.push(("plugin-source".into(), self.plugin_source.stable_id()));
        }
        nodes.extend(
            self.rows
                .iter()
                .map(|(key, row)| (format!("extensions-entry-{key}"), row.stable_id())),
        );
        for surface in [
            &self.toolbar_surface,
            &self.detail_surface,
            &self.detail_action_surface,
            &self.editor_surface,
            &self.editor_actions,
        ] {
            nodes.extend(
                surface
                    .debug_nodes()
                    .into_iter()
                    .map(|(id, node, _)| (id, node)),
            );
        }
        if let Some(scroll) = self.dialog_scroll {
            nodes.push(("extensions-editor-scroll".into(), scroll.stable_id()));
        }
        nodes
    }
}

fn detail_header_layout(narrow: bool) -> Stack {
    if narrow {
        Stack::column(8.0)
    } else {
        Stack::bar(10.0).wrap(true)
    }
    .min_height(LengthSpec::Px(48.0))
    .padding(10.0)
}
