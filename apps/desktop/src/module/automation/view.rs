use crate::runtime_compat::HostedWindowId;
use crate::runtime_layout::reconcile_children;
use crate::runtime_shell::{IntentSink, ShellIntent, bind_activate, emit};
use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, EmptyState, Entity, FormField, FrameworkError,
    GraphCanvas, ScrollAxes, ScrollView, SearchDropdown, SearchDropdownEvent, SearchDropdownOption,
    SidebarFooter, SidebarFooterButton, SidebarFrame, SidebarRow, SidebarRowState, SidebarSection,
    StableNodeId, Stack, Switch, Text, TextArea, TextChanged, TextInput, ToggleChanged, View,
};
use nana_ui::{ButtonKind, GraphCanvasEvent, GraphModel, GraphSelection, GraphViewport, Icon};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationTarget {
    pub window_id: HostedWindowId,
    pub workflow_id: String,
    pub modified_at: i64,
}
#[derive(Clone, Debug)]
pub enum AutomationAction {
    Node {
        node_id: String,
        action: super::editor::NodeEditorAction,
    },
    Rename(String),
    Save,
    Publish,
    Run,
    ToggleEnabled,
    AddNode(String),
    Graph(GraphCanvasEvent),
    SelectRun(String),
    Respond {
        run_id: String,
        node_id: String,
        value: String,
    },
    Resume {
        run_id: String,
        node_id: String,
    },
    Cancel {
        run_id: String,
    },
    SetInspector(String),
    ToggleInbox,
    ToggleScope {
        field: String,
        value: String,
    },
}

pub(crate) const INSPECTOR_PANELS: &[(&str, &str)] = &[
    ("node", "节点"),
    ("workflow", "工作流"),
    ("scope", "触发范围"),
    ("runs", "运行记录"),
];

pub(crate) const EVENT_KIND_OPTIONS: &[(&str, &str)] = &[
    ("task_created", "新建任务"),
    ("task_status_changed", "任务状态"),
    ("task_updated", "任务更新"),
    ("timeline_event", "对话事件"),
    ("todo_changed", "待办更新"),
    ("interaction_request", "交互请求"),
];
#[derive(Clone, Debug, Default)]
pub struct AutomationRunView {
    pub id: String,
    pub label: String,
    pub error: Option<String>,
    pub prompt: Option<String>,
    pub waiting_node: Option<String>,
    pub can_cancel: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AutomationRow {
    pub id: String,
    pub label: String,
    pub selected: bool,
}
#[derive(Clone, Debug)]
pub struct AutomationViewSnapshot {
    pub rows: Vec<AutomationRow>,
    pub target: Option<AutomationTarget>,
    pub name: String,
    pub enabled: bool,
    pub published: bool,
    pub error: Option<String>,
    pub graph: GraphModel,
    pub viewport: GraphViewport,
    pub selection: Option<GraphSelection>,
    pub runs: Vec<AutomationRunView>,
    pub selected_run: Option<String>,
    pub response: String,
    pub editor: Option<super::editor::NodeEditorSnapshot>,
    pub compact: bool,
    pub operation_pending: bool,
    pub cancel_pending: bool,
    pub inspector_panel: String,
    pub include_inbox: bool,
    pub event_kinds: Vec<String>,
    pub projects: Vec<(String, String, bool)>,
}
impl Default for AutomationViewSnapshot {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            target: None,
            name: String::new(),
            enabled: false,
            published: false,
            error: None,
            graph: GraphModel::empty(),
            viewport: GraphViewport::default(),
            selection: None,
            runs: Vec::new(),
            selected_run: None,
            response: String::new(),
            editor: None,
            compact: false,
            operation_pending: false,
            cancel_pending: false,
            inspector_panel: "node".into(),
            include_inbox: false,
            event_kinds: Vec::new(),
            projects: Vec::new(),
        }
    }
}
fn bind_action<V: View>(
    context: &mut AppContext,
    control: Entity<V>,
    sink: IntentSink,
    target: Arc<Mutex<Option<AutomationTarget>>>,
    action: AutomationAction,
) -> Result<(), FrameworkError> {
    context.on(control, move |_, _: &Activate, _| {
        let selected = target.lock().unwrap().clone();
        if let Some(target) = selected {
            emit(
                &sink,
                ShellIntent::Automation {
                    target,
                    action: action.clone(),
                },
            );
        }
    })
}
pub(crate) struct AutomationView {
    pub(crate) page: Entity<Stack>,
    pub(crate) sidebar: Entity<SidebarFrame>,
    canvas: Entity<GraphCanvas>,
    body: Entity<Stack>,
    editor: super::editor_view::NodeEditorView,
    section: Entity<SidebarSection>,
    list: Entity<Stack>,
    empty: Entity<EmptyState>,
    toolbar: Entity<Stack>,
    node_tools: Entity<Stack>,
    name: Entity<TextInput>,
    name_field: Entity<FormField>,
    status: Entity<Text>,
    error: Entity<Text>,
    publish: Entity<Button>,
    run: Entity<Button>,
    toggle: Entity<Button>,
    add_human: Entity<Button>,
    create: Entity<SidebarFooterButton>,
    back: Entity<SidebarRow>,
    content_scroll: Entity<ScrollView>,
    run_panel: Entity<Stack>,
    run_picker: Entity<SearchDropdown>,
    run_detail: Entity<Text>,
    response: Entity<TextArea>,
    run_actions: Entity<Stack>,
    resume: Entity<Button>,
    cancel: Entity<Button>,
    inspector: Entity<SearchDropdown>,
    scope_panel: Entity<Stack>,
    inbox: Entity<Switch>,
    project_toggles: HashMap<String, Entity<Switch>>,
    event_toggles: HashMap<String, Entity<Switch>>,
    run_target: Arc<Mutex<Option<(AutomationTarget, String, Option<String>, bool)>>>,
    pub(crate) rows: HashMap<String, Entity<SidebarRow>>,
    target: Arc<Mutex<Option<AutomationTarget>>>,
    sink: IntentSink,
}
impl AutomationView {
    pub(crate) fn debug_nodes(&self) -> Vec<(String, StableNodeId)> {
        let mut nodes = vec![
            ("new".into(), self.create.stable_id()),
            ("back".into(), self.back.stable_id()),
            ("scroll".into(), self.content_scroll.stable_id()),
            ("canvas".into(), self.canvas.stable_id()),
            ("auto-name".into(), self.name.stable_id()),
            ("auto-add-human".into(), self.add_human.stable_id()),
            ("auto-publish".into(), self.publish.stable_id()),
            ("auto-toggle".into(), self.toggle.stable_id()),
            ("auto-response".into(), self.response.stable_id()),
            ("auto-resume".into(), self.resume.stable_id()),
            ("auto-cancel".into(), self.cancel.stable_id()),
            (
                "auto-inspector-panel".into(),
                self.inspector.stable_id(),
            ),
            ("auto-inbox".into(), self.inbox.stable_id()),
        ];
        if let Some(target) = self.target.lock().unwrap().as_ref() {
            nodes.push((
                format!("{}.auto-run", target.workflow_id),
                self.run.stable_id(),
            ));
        }
        for (id, toggle) in &self.project_toggles {
            nodes.push((format!("auto-project-{id}"), toggle.stable_id()));
        }
        for (id, toggle) in &self.event_toggles {
            nodes.push((format!("auto-scope-event-kind-{id}"), toggle.stable_id()));
        }
        nodes.extend(self.editor.debug_nodes());
        nodes
    }

    #[cfg(debug_assertions)]
    pub(crate) fn graph_is_mounted(&self, context: &AppContext) -> bool {
        context
            .world()
            .node(self.body.stable_id())
            .is_some_and(|body| body.children.contains(&self.canvas.stable_id()))
    }

    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        snapshot: &AutomationViewSnapshot,
        visible: bool,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        let target = Arc::new(Mutex::new(snapshot.target.clone()));
        let page =
            context.create_detached_component(document, Stack::fill_column(12.0).padding(16.0))?;
        let content_scroll = context.create_detached_component(
            document,
            ScrollView::new(ScrollAxes::Vertical)
                .style(Stack::fill_column(0.0).node_style()),
        )?;
        let body = context.create_detached_component(document, Stack::fill_row(16.0))?;
        let editor =
            super::editor_view::NodeEditorView::mount(context, document, Arc::clone(&sink))?;
        let toolbar = context.create_detached_component(document, Stack::bar(8.0).wrap(true))?;
        let node_tools = context.create_detached_component(document, Stack::bar(8.0).wrap(true))?;
        let name =
            context.create_detached_component(document, TextInput::new(snapshot.name.clone()))?;
        let name_field = context.create_detached_component(
            document,
            FormField::new("工作流名称").control_child(name.stable_id()),
        )?;
        context.append_child(name_field, name)?;
        let binding = Arc::clone(&target);
        let callback = Arc::clone(&sink);
        context.on(name, move |_, event: &TextChanged, _| {
            let selected = binding.lock().unwrap().clone();
            if let Some(target) = selected {
                emit(
                    &callback,
                    ShellIntent::Automation {
                        target,
                        action: AutomationAction::Rename(event.value.clone()),
                    },
                );
            }
        })?;
        let save = context.create_detached_component(
            document,
            Button::new("保存草稿").kind(ButtonKind::Subtle),
        )?;
        let publish = context.create_detached_component(
            document,
            Button::new("保存并发布").kind(ButtonKind::Primary),
        )?;
        let run = context.create_detached_component(
            document,
            Button::new("运行已发布版本").kind(ButtonKind::Primary),
        )?;
        let toggle = context
            .create_detached_component(document, Button::new("启用").kind(ButtonKind::Subtle))?;
        for (button, action) in [
            (save, AutomationAction::Save),
            (publish, AutomationAction::Publish),
            (run, AutomationAction::Run),
            (toggle, AutomationAction::ToggleEnabled),
        ] {
            bind_action(
                context,
                button,
                Arc::clone(&sink),
                Arc::clone(&target),
                action,
            )?;
            context.append_child(toolbar, button)?;
        }
        let inspector = context.create_detached_component(
            document,
            SearchDropdown::new(Some(snapshot.inspector_panel.clone())).placeholder("检查器"),
        )?;
        let binding = Arc::clone(&target);
        let callback = Arc::clone(&sink);
        context.on(inspector, move |_, event: &SearchDropdownEvent, _| {
            if let SearchDropdownEvent::Select(panel) = event {
                if let Some(target) = binding.lock().unwrap().clone() {
                    emit(
                        &callback,
                        ShellIntent::Automation {
                            target,
                            action: AutomationAction::SetInspector(panel.to_string()),
                        },
                    );
                }
            }
        })?;
        context.append_child(toolbar, inspector)?;
        let mut add_human = None;
        for (kind, label) in [
            ("agent", "添加 Agent"),
            ("tool", "添加工具"),
            ("logic", "添加条件"),
            ("human", "添加人工确认"),
        ] {
            let button = context
                .create_detached_component(document, Button::new(label).kind(ButtonKind::Subtle))?;
            bind_action(
                context,
                button,
                Arc::clone(&sink),
                Arc::clone(&target),
                AutomationAction::AddNode(kind.into()),
            )?;
            context.append_child(node_tools, button)?;
            if kind == "human" {
                add_human = Some(button);
            }
        }
        let add_human = add_human.expect("human node tool");
        let status = context.create_detached_component(document, Text::new(""))?;
        let error = context.create_detached_component(document, Text::new(""))?;
        let canvas = context.create_detached_component(
            document,
            GraphCanvas::new("automations", snapshot.graph.clone())
                .viewport(snapshot.viewport)
                .selection(snapshot.selection.clone()),
        )?;
        let binding = Arc::clone(&target);
        let callback = Arc::clone(&sink);
        context.on(canvas, move |_, event: &GraphCanvasEvent, _| {
            let selected = binding.lock().unwrap().clone();
            if let Some(target) = selected {
                emit(
                    &callback,
                    ShellIntent::Automation {
                        target,
                        action: AutomationAction::Graph(event.clone()),
                    },
                );
            }
        })?;
        let empty = context.create_detached_component(
            document,
            EmptyState::new("还没有自动化")
                .message("新建工作流，添加步骤后保存并发布。")
                .icon(Icon::Nodes),
        )?;
        let run_panel = context.create_detached_component(document, Stack::column(8.0))?;
        let run_picker = context.create_detached_component(
            document,
            SearchDropdown::new(None::<String>).placeholder("运行记录"),
        )?;
        let binding = Arc::clone(&target);
        let callback = Arc::clone(&sink);
        context.on(run_picker, move |_, event: &SearchDropdownEvent, _| {
            if let SearchDropdownEvent::Select(id) = event {
                if let Some(target) = binding.lock().unwrap().clone() {
                    emit(
                        &callback,
                        ShellIntent::Automation {
                            target,
                            action: AutomationAction::SelectRun(id.to_string()),
                        },
                    );
                }
            }
        })?;
        let run_detail = context.create_detached_component(document, Text::new("尚无运行记录"))?;
        let response = context.create_detached_component(
            document,
            TextArea::new("")
                .placeholder("补充说明（可选）")
                .height(72.0),
        )?;
        let run_actions =
            context.create_detached_component(document, Stack::bar(8.0).wrap(true))?;
        let resume = context.create_detached_component(
            document,
            Button::new("确认并继续").kind(ButtonKind::Primary),
        )?;
        let cancel = context.create_detached_component(
            document,
            Button::new("取消运行").kind(ButtonKind::Subtle),
        )?;
        let run_target = Arc::new(Mutex::new(
            None::<(AutomationTarget, String, Option<String>, bool)>,
        ));
        for (button, is_resume) in [(resume, true), (cancel, false)] {
            let binding = Arc::clone(&run_target);
            let callback = Arc::clone(&sink);
            context.on(button, move |_, _: &Activate, _| {
                if let Some((target, run_id, waiting, cancellable)) =
                    binding.lock().unwrap().clone()
                {
                    if (is_resume && waiting.is_some()) || (!is_resume && cancellable) {
                        let action = if is_resume {
                            AutomationAction::Resume {
                                run_id,
                                node_id: waiting.unwrap(),
                            }
                        } else {
                            AutomationAction::Cancel { run_id }
                        };
                        emit(&callback, ShellIntent::Automation { target, action });
                    }
                }
            })?;
        }
        let binding = Arc::clone(&run_target);
        let callback = Arc::clone(&sink);
        context.on(response, move |_, event: &TextChanged, _| {
            if let Some((target, run_id, Some(node_id), _)) = binding.lock().unwrap().clone() {
                emit(
                    &callback,
                    ShellIntent::Automation {
                        target,
                        action: AutomationAction::Respond {
                            run_id,
                            node_id,
                            value: event.value.clone(),
                        },
                    },
                );
            }
        })?;
        let scope_panel = context.create_detached_component(document, Stack::column(8.0))?;
        let inbox = context.create_detached_component(
            document,
            Switch::new("包括收件箱", snapshot.include_inbox),
        )?;
        let binding = Arc::clone(&target);
        let callback = Arc::clone(&sink);
        context.on(inbox, move |_, _: &ToggleChanged, _| {
            if let Some(target) = binding.lock().unwrap().clone() {
                emit(
                    &callback,
                    ShellIntent::Automation {
                        target,
                        action: AutomationAction::ToggleInbox,
                    },
                );
            }
        })?;
        let mut event_toggles = HashMap::new();
        for (value, label) in EVENT_KIND_OPTIONS {
            let toggle =
                context.create_detached_component(document, Switch::new(*label, false))?;
            let binding = Arc::clone(&target);
            let callback = Arc::clone(&sink);
            let field_value = (*value).to_owned();
            context.on(toggle, move |_, _: &ToggleChanged, _| {
                if let Some(target) = binding.lock().unwrap().clone() {
                    emit(
                        &callback,
                        ShellIntent::Automation {
                            target,
                            action: AutomationAction::ToggleScope {
                                field: "event-kind".into(),
                                value: field_value.clone(),
                            },
                        },
                    );
                }
            })?;
            event_toggles.insert((*value).to_owned(), toggle);
        }
        let back = context.create_detached_component(document, SidebarRow::new("返回"))?;
        bind_activate(
            context,
            back,
            Arc::clone(&sink),
            ShellIntent::CloseAutomations,
        )?;
        let list = context.create_detached_component(document, Stack::column(4.0))?;
        let scroll =
            context.create_detached_component(document, SidebarFrame::vertical_body_scroll())?;
        let section = context.create_detached_component(
            document,
            SidebarSection::new("自动化").count(snapshot.rows.len()),
        )?;
        context.append_child(section, list)?;
        context.append_child(scroll, section)?;
        let footer = context.create_detached_component(document, SidebarFooter::new())?;
        let mut create = None;
        for (label, icon, intent) in [
            ("刷新", Icon::Activity, ShellIntent::RefreshAutomations),
            ("新建", Icon::Add, ShellIntent::CreateAutomation),
        ] {
            let button = context
                .create_detached_component(document, SidebarFooterButton::new(label, icon))?;
            bind_activate(context, button, Arc::clone(&sink), intent)?;
            context.append_child(footer, button)?;
            if label == "新建" {
                create = Some(button);
            }
        }
        let create = create.expect("create automation");
        let sidebar = context.create_detached_component(
            document,
            SidebarFrame::new()
                .top(back.stable_id())
                .body(scroll.stable_id())
                .footer(footer.stable_id()),
        )?;
        context.append_child(sidebar, back)?;
        context.append_child(sidebar, scroll)?;
        context.append_child(sidebar, footer)?;
        let mut view = Self {
            page,
            sidebar,
            canvas,
            body,
            editor,
            section,
            list,
            empty,
            toolbar,
            node_tools,
            name,
            name_field,
            status,
            error,
            publish,
            run,
            toggle,
            add_human,
            create,
            back,
            content_scroll,
            run_panel,
            run_picker,
            run_detail,
            response,
            run_actions,
            resume,
            cancel,
            inspector,
            scope_panel,
            inbox,
            project_toggles: HashMap::new(),
            event_toggles,
            run_target,
            rows: HashMap::new(),
            target,
            sink,
        };
        view.sync(context, document, snapshot, visible)?;
        Ok(view)
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        snapshot: &AutomationViewSnapshot,
        visible: bool,
    ) -> Result<(), FrameworkError> {
        *self.target.lock().unwrap() = visible.then(|| snapshot.target.clone()).flatten();
        self.editor.sync(
            context,
            document,
            snapshot.target.as_ref().filter(|_| visible),
            snapshot.editor.as_ref(),
        )?;
        let selected_run = snapshot
            .runs
            .iter()
            .find(|run| snapshot.selected_run.as_deref() == Some(run.id.as_str()));
        *self.run_target.lock().unwrap() = if visible {
            snapshot
                .target
                .clone()
                .zip(selected_run)
                .map(|(target, run)| {
                    (
                        target,
                        run.id.clone(),
                        run.waiting_node.clone(),
                        run.can_cancel,
                    )
                })
        } else {
            None
        };
        if !visible {
            context.update_component(self.run_picker, |picker, _| picker.close())?;
            context.update_component(self.inspector, |picker, _| picker.close())?;
            return Ok(());
        }
        context.update_component(self.editor.root, |root, _| {
            *root = if snapshot.compact {
                Stack::fill_column(8.0)
            } else {
                Stack::fill_column(8.0)
                    .width(nana_ui::runtime::LengthSpec::Px(320.0))
                    .grow(0.0)
                    .shrink(0.0)
            };
        })?;
        let body = if snapshot.editor.is_some() {
            if snapshot.compact {
                vec![self.editor.root.stable_id()]
            } else {
                vec![self.canvas.stable_id(), self.editor.root.stable_id()]
            }
        } else {
            vec![self.canvas.stable_id()]
        };
        reconcile_children(context, self.body.stable_id(), &body)?;
        context.update_component(self.inspector, |picker, _| {
            picker.options = INSPECTOR_PANELS
                .iter()
                .map(|(value, label)| SearchDropdownOption::new(*value, *label))
                .collect();
            picker.value = Some(Arc::from(snapshot.inspector_panel.as_str()));
        })?;
        context.update_component(self.run_picker, |picker, _| {
            picker.options = snapshot
                .runs
                .iter()
                .map(|run| SearchDropdownOption::new(run.id.clone(), run.label.clone()))
                .collect();
            picker.value = selected_run.map(|run| Arc::from(run.id.as_str()));
        })?;
        context.update_component(self.run_detail, |text, _| {
            *text = Text::new(
                selected_run
                    .map(|run| {
                        [run.error.as_deref(), run.prompt.as_deref()]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_else(|| "尚无运行记录".into()),
            );
        })?;
        context.update_component(self.response, |field, _| {
            if field.state.value != snapshot.response {
                field.state.replace_value(snapshot.response.clone());
            }
        })?;
        let waiting = selected_run.is_some_and(|run| run.waiting_node.is_some());
        let cancellable = selected_run.is_some_and(|run| run.can_cancel);
        context.update_component(self.resume, |button, _| {
            button.disabled = snapshot.operation_pending
        })?;
        context.update_component(self.cancel, |button, _| {
            button.disabled = snapshot.cancel_pending
        })?;
        let mut actions = Vec::new();
        if waiting {
            actions.push(self.resume.stable_id());
        }
        if cancellable {
            actions.push(self.cancel.stable_id());
        }
        reconcile_children(context, self.run_actions.stable_id(), &actions)?;
        let mut children = if snapshot.runs.is_empty() {
            Vec::new()
        } else {
            vec![self.run_picker.stable_id()]
        };
        if selected_run.is_none_or(|run| run.error.is_some() || run.prompt.is_some()) {
            children.push(self.run_detail.stable_id());
        }
        if waiting {
            children.push(self.response.stable_id());
        }
        if !actions.is_empty() {
            children.push(self.run_actions.stable_id());
        }
        reconcile_children(context, self.run_panel.stable_id(), &children)?;
        context.update_component(self.inbox, |toggle, _| {
            *toggle = Switch::new("包括收件箱", snapshot.include_inbox);
        })?;
        for (value, label) in EVENT_KIND_OPTIONS {
            if let Some(toggle) = self.event_toggles.get(*value).copied() {
                context.update_component(toggle, |view, _| {
                    *view = Switch::new(*label, snapshot.event_kinds.iter().any(|kind| kind == value))
                })?;
            }
        }
        let mut scope_keep = HashSet::new();
        let mut scope_order = vec![self.inbox.stable_id()];
        for (id, label, checked) in &snapshot.projects {
            scope_keep.insert(id.clone());
            let toggle = if let Some(toggle) = self.project_toggles.get(id).copied() {
                context.update_component(toggle, |view, _| {
                    *view = Switch::new(label.clone(), *checked)
                })?;
                toggle
            } else {
                let toggle = context
                    .create_detached_component(document, Switch::new(label.clone(), *checked))?;
                let binding = Arc::clone(&self.target);
                let callback = Arc::clone(&self.sink);
                let value = id.clone();
                context.on(toggle, move |_, _: &ToggleChanged, _| {
                    if let Some(target) = binding.lock().unwrap().clone() {
                        emit(
                            &callback,
                            ShellIntent::Automation {
                                target,
                                action: AutomationAction::ToggleScope {
                                    field: "project".into(),
                                    value: value.clone(),
                                },
                            },
                        );
                    }
                })?;
                self.project_toggles.insert(id.clone(), toggle);
                toggle
            };
            scope_order.push(toggle.stable_id());
        }
        let stale_projects: Vec<_> = self
            .project_toggles
            .keys()
            .filter(|id| !scope_keep.contains(*id))
            .cloned()
            .collect();
        for id in stale_projects {
            if let Some(toggle) = self.project_toggles.remove(&id) {
                context.remove_view(toggle)?;
            }
        }
        for (value, _) in EVENT_KIND_OPTIONS {
            if let Some(toggle) = self.event_toggles.get(*value) {
                scope_order.push(toggle.stable_id());
            }
        }
        reconcile_children(context, self.scope_panel.stable_id(), &scope_order)?;
        context.update_component(self.canvas, |canvas, _| {
            canvas.model = snapshot.graph.clone();
            canvas.viewport = snapshot.viewport;
            canvas.selection = snapshot.selection.clone();
        })?;
        context.update_component(self.name, |field, _| {
            if field.state.value != snapshot.name {
                field.state.replace_value(snapshot.name.clone());
            }
        })?;
        context.update_component(self.section, |section, _| {
            *section = SidebarSection::new("自动化").count(snapshot.rows.len())
        })?;
        context.update_component(self.run, |button, _| {
            button.disabled = !snapshot.published
                || snapshot.operation_pending
                || snapshot.runs.iter().any(|run| run.can_cancel)
        })?;
        context.update_component(self.toggle, |button, _| {
            *button =
                Button::new(if snapshot.enabled { "停用" } else { "启用" }).kind(ButtonKind::Subtle)
        })?;
        context.update_component(self.status, |text, _| {
            *text = Text::new(if snapshot.operation_pending {
                "正在处理…"
            } else if snapshot.published {
                if snapshot.enabled {
                    "已发布 · 已启用"
                } else {
                    "已发布 · 已停用"
                }
            } else {
                "草稿 · 发布后可运行"
            })
        })?;
        context.update_component(self.error, |text, _| {
            *text = Text::new(snapshot.error.clone().unwrap_or_default())
        })?;
        let mut keep = HashSet::new();
        let mut order = Vec::new();
        for row in &snapshot.rows {
            keep.insert(row.id.clone());
            let item = SidebarRow::new(row.label.clone()).state(if row.selected {
                SidebarRowState::Active
            } else {
                SidebarRowState::Idle
            });
            let control = if let Some(control) = self.rows.get(&row.id).copied() {
                context.update_component(control, |view, _| *view = item)?;
                control
            } else {
                let control = context.create_detached_component(document, item)?;
                bind_activate(
                    context,
                    control,
                    Arc::clone(&self.sink),
                    ShellIntent::SelectAutomation(row.id.clone()),
                )?;
                self.rows.insert(row.id.clone(), control);
                control
            };
            order.push(control.stable_id());
        }
        let stale: Vec<_> = self
            .rows
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect();
        for id in stale {
            if let Some(row) = self.rows.remove(&id) {
                context.remove_view(row)?;
            }
        }
        reconcile_children(context, self.list.stable_id(), &order)?;
        let mut page = if snapshot.target.is_some() {
            let mut page = vec![
                self.name_field.stable_id(),
                self.status.stable_id(),
                self.toolbar.stable_id(),
                self.node_tools.stable_id(),
                self.body.stable_id(),
                self.run_panel.stable_id(),
            ];
            if snapshot.inspector_panel == "scope" {
                page.push(self.scope_panel.stable_id());
            }
            page
        } else {
            vec![self.empty.stable_id()]
        };
        if snapshot.error.is_some() {
            page.insert(0, self.error.stable_id());
        }
        reconcile_children(context, self.content_scroll.stable_id(), &page)?;
        reconcile_children(
            context,
            self.page.stable_id(),
            &[self.content_scroll.stable_id()],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workflow_refresh_keeps_footer_visible_when_the_list_overflows() {
        let mut context = AppContext::new();
        let document = DocumentId::new(335).unwrap();
        let root = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut state = snapshot("first");
        let mut view =
            AutomationView::mount(&mut context, document, &state, true, Arc::new(|_| {})).unwrap();
        context.append_child(root, view.sidebar).unwrap();
        let footer = context
            .read(view.sidebar, |sidebar| sidebar.footer.unwrap())
            .unwrap();
        for count in [1, 60, 2] {
            state.rows = (0..count)
                .map(|index| AutomationRow {
                    id: format!("workflow-{index}"),
                    label: format!("工作流 {index}"),
                    selected: index == 0,
                })
                .collect();
            view.sync(&mut context, document, &state, true).unwrap();
            context
                .layout_document(
                    document,
                    nana_ui::runtime::LayoutViewport::new(260.0, 320.0),
                )
                .unwrap();
            let bounds = context.world().layout_box(footer).unwrap();
            assert!(bounds.height > 0.0);
            assert!(
                bounds.y >= 0.0 && bounds.y + bounds.height <= 320.5,
                "footer outside viewport: {bounds:?}"
            );
        }
    }
    #[test]
    fn run_actions_follow_selection_and_stop_after_completion_or_hiding() {
        let mut context = AppContext::new();
        let document = DocumentId::new(334).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut state = snapshot("workflow");
        state.runs = vec![AutomationRunView {
            id: "run-a".into(),
            label: "等待确认".into(),
            prompt: Some("继续？".into()),
            waiting_node: Some("approval-a".into()),
            can_cancel: true,
            error: None,
        }];
        state.selected_run = Some("run-a".into());
        let mut view = AutomationView::mount(
            &mut context,
            document,
            &state,
            true,
            Arc::new(move |event| received.lock().unwrap().push(event)),
        )
        .unwrap();
        context
            .update_component(view.resume, |_, cx| cx.emit(Activate))
            .unwrap();
        state.runs[0].id = "run-b".into();
        state.selected_run = Some("run-b".into());
        state.response = "已检查".into();
        view.sync(&mut context, document, &state, true).unwrap();
        context
            .update_component(view.cancel, |_, cx| cx.emit(Activate))
            .unwrap();
        assert_eq!(
            context
                .read(view.response, |field| field.state.value.clone())
                .unwrap(),
            "已检查"
        );
        state.runs[0].prompt = None;
        state.runs[0].waiting_node = None;
        state.runs[0].can_cancel = false;
        view.sync(&mut context, document, &state, true).unwrap();
        for button in [view.resume, view.cancel] {
            context
                .update_component(button, |_, cx| cx.emit(Activate))
                .unwrap();
        }
        assert!(
            !context
                .world()
                .node(view.run_panel.stable_id())
                .unwrap()
                .children
                .contains(&view.response.stable_id())
        );
        state.runs[0].prompt = Some("继续？".into());
        state.runs[0].waiting_node = Some("approval-b".into());
        state.runs[0].can_cancel = true;
        view.sync(&mut context, document, &state, false).unwrap();
        context
            .update_component(view.resume, |_, cx| cx.emit(Activate))
            .unwrap();
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], ShellIntent::Automation { action: AutomationAction::Resume { run_id, node_id }, .. } if run_id == "run-a" && node_id == "approval-a")
        );
        assert!(
            matches!(&events[1], ShellIntent::Automation { action: AutomationAction::Cancel { run_id }, .. } if run_id == "run-b")
        );
    }
    fn snapshot(id: &str) -> AutomationViewSnapshot {
        AutomationViewSnapshot {
            rows: vec![AutomationRow {
                id: id.into(),
                label: format!("Workflow {id}"),
                selected: true,
            }],
            target: Some(AutomationTarget {
                window_id: HostedWindowId::PRIMARY,
                workflow_id: id.into(),
                modified_at: 1,
            }),
            name: format!("Workflow {id}"),
            published: true,
            ..Default::default()
        }
    }
    #[test]
    fn queued_actions_keep_their_workflow_identity_and_hidden_views_stop_emitting() {
        let mut context = AppContext::new();
        let document = DocumentId::new(331).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut view = AutomationView::mount(
            &mut context,
            document,
            &snapshot("first"),
            true,
            Arc::new(move |event| received.lock().unwrap().push(event)),
        )
        .unwrap();
        context
            .update_component(view.run, |_, cx| cx.emit(Activate))
            .unwrap();
        let mut next = snapshot("second");
        next.target.as_mut().unwrap().modified_at = 2;
        view.sync(&mut context, document, &next, true).unwrap();
        context
            .update_component(view.run, |_, cx| cx.emit(Activate))
            .unwrap();
        view.sync(&mut context, document, &next, false).unwrap();
        context
            .update_component(view.run, |_, cx| cx.emit(Activate))
            .unwrap();
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], ShellIntent::Automation { target, action: AutomationAction::Run } if target.workflow_id == "first" && target.modified_at == 1)
        );
        assert!(
            matches!(&events[1], ShellIntent::Automation { target, action: AutomationAction::Run } if target.workflow_id == "second" && target.modified_at == 2)
        );
    }
    #[test]
    fn list_refresh_updates_count_and_selection_and_removes_old_workflow_controls() {
        let mut context = AppContext::new();
        let document = DocumentId::new(332).unwrap();
        let mut view = AutomationView::mount(
            &mut context,
            document,
            &snapshot("first"),
            true,
            Arc::new(|_| {}),
        )
        .unwrap();
        let old = view.rows["first"];
        assert_eq!(
            context.read(old, |row| row.state).unwrap(),
            SidebarRowState::Active
        );
        let empty = AutomationViewSnapshot {
            error: Some("读取失败".into()),
            ..Default::default()
        };
        view.sync(&mut context, document, &empty, true).unwrap();
        assert!(context.world().node(old.stable_id()).is_none());
        assert_eq!(
            context.read(view.section, |section| section.count).unwrap(),
            Some(0)
        );
        assert_eq!(
            context
                .world()
                .node(view.page.stable_id())
                .unwrap()
                .children,
            vec![view.content_scroll.stable_id()]
        );
        let children = &context
            .world()
            .node(view.content_scroll.stable_id())
            .unwrap()
            .children;
        assert_eq!(children, &[view.error.stable_id(), view.empty.stable_id()]);
        view.sync(&mut context, document, &snapshot("restored"), true)
            .unwrap();
        assert_eq!(
            context.read(view.section, |section| section.count).unwrap(),
            Some(1)
        );
        assert!(
            context
                .world()
                .node(view.content_scroll.stable_id())
                .unwrap()
                .children
                .contains(&view.body.stable_id())
        );
    }
    #[test]
    fn graph_and_name_edits_carry_the_view_target_and_unpublished_workflows_disable_run() {
        let mut context = AppContext::new();
        let document = DocumentId::new(333).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut state = snapshot("first");
        state.published = false;
        state.target.as_mut().unwrap().window_id = nana_ui_platform::WindowId(23);
        let view = AutomationView::mount(
            &mut context,
            document,
            &state,
            true,
            Arc::new(move |event| received.lock().unwrap().push(event)),
        )
        .unwrap();
        assert!(context.read(view.run, |button| button.disabled).unwrap());
        context
            .update_component(view.canvas, |_, cx| {
                cx.emit(GraphCanvasEvent::SelectionChanged(None))
            })
            .unwrap();
        context
            .update_component(view.name, |_, cx| {
                cx.emit(TextChanged {
                    value: "renamed".into(),
                    selection: nana_ui::runtime::TextSelection::default(),
                })
            })
            .unwrap();
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        for event in events.iter() {
            assert!(
                matches!(event, ShellIntent::Automation { target, .. } if target.window_id == nana_ui_platform::WindowId(23) && target.workflow_id == "first")
            );
        }
        assert!(
            matches!(&events[1], ShellIntent::Automation { action: AutomationAction::Rename(value), .. } if value == "renamed")
        );
    }
}
