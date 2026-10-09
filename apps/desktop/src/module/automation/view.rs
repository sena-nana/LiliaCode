use crate::runtime_compat::HostedWindowId;
use crate::runtime_layout::{view_column, Bound};
use crate::runtime_shell::{emit, IntentSink, ShellIntent};
use nana_ui::runtime::view::{
    empty_state, entity_ref, signal, status_badge, switch, text_input, widget, with_refs, EachExt,
    EntityRef, Signal,
};
use nana_ui::runtime::GraphCanvasEvent;
use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, EmptyState, Entity, FormField, FrameworkError,
    GraphCanvas, LengthSpec, NodeStyle, ScrollAxes, ScrollView, SearchDropdown,
    SearchDropdownEvent, SearchDropdownOption, SidebarFooter, SidebarFooterButton, SidebarFrame,
    SidebarRow, SidebarRowState, SidebarSection, StableNodeId, Stack, StatusBadge, Switch,
    TextArea, TextChanged, TextInput, ValidationMessage, View,
};
use nana_ui::{
    ButtonKind, GraphModel, GraphSelection, GraphViewport, Icon, StatusTone, ValidationIntent,
};
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

#[derive(Clone)]
struct CanvasState {
    model: GraphModel,
    viewport: GraphViewport,
    selection: Option<GraphSelection>,
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
    status: Entity<StatusBadge>,
    error: Entity<ValidationMessage>,
    publish: Entity<Button>,
    run: Entity<Button>,
    toggle: Entity<Button>,
    add_human: Entity<Button>,
    create: Entity<SidebarFooterButton>,
    back: Entity<SidebarRow>,
    content_scroll: Entity<ScrollView>,
    run_panel: Entity<Stack>,
    run_picker: Entity<SearchDropdown>,
    run_detail: Entity<EmptyState>,
    response: Entity<TextArea>,
    run_actions: Entity<Stack>,
    resume: Entity<Button>,
    cancel: Entity<Button>,
    inspector: Entity<SearchDropdown>,
    scope_panel: Entity<Stack>,
    inbox: Entity<Switch>,
    project_list: Entity<Stack>,
    project_toggles: HashMap<String, Entity<Switch>>,
    event_toggles: HashMap<String, Entity<Switch>>,
    run_target: Arc<Mutex<Option<(AutomationTarget, String, Option<String>, bool)>>>,
    pub(crate) rows: HashMap<String, Entity<SidebarRow>>,
    target: Arc<Mutex<Option<AutomationTarget>>>,
    workflow_rows: Bound<Vec<AutomationRow>>,
    project_rows: Bound<Vec<(String, String, bool)>>,
    runs: Bound<Vec<AutomationRunView>>,
    event_kinds: Bound<Vec<String>>,
    show_error: Bound<bool>,
    error_text: Bound<String>,
    show_empty: Bound<bool>,
    show_workflow: Bound<bool>,
    show_scope: Bound<bool>,
    show_canvas: Bound<bool>,
    name_text: Bound<String>,
    status_label: Bound<Arc<str>>,
    status_tone: Bound<StatusTone>,
    run_disabled: Bound<bool>,
    toggle_label: Bound<String>,
    inspector_panel: Bound<String>,
    show_picker: Bound<bool>,
    show_detail: Bound<bool>,
    detail_title: Bound<Arc<str>>,
    detail_message: Bound<Option<Arc<str>>>,
    show_response: Bound<bool>,
    response_text: Bound<String>,
    show_resume: Bound<bool>,
    show_cancel: Bound<bool>,
    show_actions: Bound<bool>,
    resume_disabled: Bound<bool>,
    cancel_disabled: Bound<bool>,
    inbox_checked: Bound<bool>,
    selected_run: Bound<Option<String>>,
    canvas_state: Bound<CanvasState>,
    name_workflow: Option<String>,
    response_run: Option<String>,
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
            ("auto-inspector-panel".into(), self.inspector.stable_id()),
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
            .is_some_and(|body| {
                body.children.contains(&self.canvas.stable_id())
                    && !context
                        .world()
                        .node_style(self.canvas.stable_id())
                        .is_some_and(|style| style.layout.hidden)
            })
    }

    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        snapshot: &AutomationViewSnapshot,
        visible: bool,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        let editor =
            super::editor_view::NodeEditorView::mount(context, document, Arc::clone(&sink))?;
        let target = Arc::new(Mutex::new(None));
        let run_target = Arc::new(Mutex::new(None));
        let workflow_rows: Bound<Vec<AutomationRow>> = Bound::new();
        let project_rows: Bound<Vec<(String, String, bool)>> = Bound::new();
        let runs: Bound<Vec<AutomationRunView>> = Bound::new();
        let event_kinds: Bound<Vec<String>> = Bound::new();
        let show_error = Bound::new();
        let error_text = Bound::new();
        let show_empty = Bound::new();
        let show_workflow = Bound::new();
        let show_scope = Bound::new();
        let show_canvas = Bound::new();
        let name_text = Bound::new();
        let status_label = Bound::new();
        let status_tone = Bound::new();
        let run_disabled = Bound::new();
        let toggle_label = Bound::new();
        let inspector_panel = Bound::new();
        let show_picker = Bound::new();
        let show_detail = Bound::new();
        let detail_title = Bound::new();
        let detail_message = Bound::new();
        let show_response = Bound::new();
        let response_text = Bound::new();
        let show_resume = Bound::new();
        let show_cancel = Bound::new();
        let show_actions = Bound::new();
        let resume_disabled = Bound::new();
        let cancel_disabled = Bound::new();
        let inbox_checked = Bound::new();
        let selected_run = Bound::new();
        let canvas_state = Bound::new();
        let rows_slot = workflow_rows.clone();
        let projects_slot = project_rows.clone();
        let runs_slot = runs.clone();
        let kinds_slot = event_kinds.clone();
        let show_error_slot = show_error.clone();
        let error_text_slot = error_text.clone();
        let show_empty_slot = show_empty.clone();
        let show_workflow_slot = show_workflow.clone();
        let show_scope_slot = show_scope.clone();
        let show_canvas_slot = show_canvas.clone();
        let name_text_slot = name_text.clone();
        let status_label_slot = status_label.clone();
        let status_tone_slot = status_tone.clone();
        let run_disabled_slot = run_disabled.clone();
        let toggle_label_slot = toggle_label.clone();
        let inspector_panel_slot = inspector_panel.clone();
        let show_picker_slot = show_picker.clone();
        let show_detail_slot = show_detail.clone();
        let detail_title_slot = detail_title.clone();
        let detail_message_slot = detail_message.clone();
        let show_response_slot = show_response.clone();
        let response_text_slot = response_text.clone();
        let show_resume_slot = show_resume.clone();
        let show_cancel_slot = show_cancel.clone();
        let show_actions_slot = show_actions.clone();
        let resume_disabled_slot = resume_disabled.clone();
        let cancel_disabled_slot = cancel_disabled.clone();
        let inbox_checked_slot = inbox_checked.clone();
        let selected_run_slot = selected_run.clone();
        let canvas_state_slot = canvas_state.clone();
        let stored_target = Arc::clone(&target);
        let stored_run_target = Arc::clone(&run_target);
        let mounted = context.mount_view_detached(document, move || {
            let rows = rows_slot.install(signal(Vec::new()));
            let projects = projects_slot.install(signal(Vec::new()));
            let runs = runs_slot.install(signal(Vec::new()));
            let event_kinds = kinds_slot.install(signal(Vec::new()));
            let show_error = show_error_slot.install(signal(false));
            let error_text = error_text_slot.install(signal(String::new()));
            let show_empty = show_empty_slot.install(signal(false));
            let show_workflow = show_workflow_slot.install(signal(false));
            let show_scope = show_scope_slot.install(signal(false));
            let show_canvas = show_canvas_slot.install(signal(true));
            let name_text = name_text_slot.install(signal(String::new()));
            let status_label = status_label_slot.install(signal(Arc::<str>::from("")));
            let status_tone = status_tone_slot.install(signal(StatusTone::Neutral));
            let run_disabled = run_disabled_slot.install(signal(true));
            let toggle_label = toggle_label_slot.install(signal(String::new()));
            let inspector_panel = inspector_panel_slot.install(signal(String::new()));
            let show_picker = show_picker_slot.install(signal(false));
            let show_detail = show_detail_slot.install(signal(false));
            let detail_title = detail_title_slot.install(signal(Arc::<str>::from("")));
            let detail_message = detail_message_slot.install(signal(None));
            let show_response = show_response_slot.install(signal(false));
            let response_text = response_text_slot.install(signal(String::new()));
            let show_resume = show_resume_slot.install(signal(false));
            let show_cancel = show_cancel_slot.install(signal(false));
            let show_actions = show_actions_slot.install(signal(false));
            let resume_disabled = resume_disabled_slot.install(signal(false));
            let cancel_disabled = cancel_disabled_slot.install(signal(false));
            let inbox_checked = inbox_checked_slot.install(signal(false));
            let selected_run = selected_run_slot.install(signal(None));
            let canvas_state = canvas_state_slot.install(signal(CanvasState {
                model: GraphModel::empty(),
                viewport: GraphViewport::default(),
                selection: None,
            }));
            let page = entity_ref::<Stack>();
            let sidebar = entity_ref::<SidebarFrame>();
            let canvas = entity_ref::<GraphCanvas>();
            let body = entity_ref::<Stack>();
            let section = entity_ref::<SidebarSection>();
            let empty = entity_ref::<EmptyState>();
            let toolbar = entity_ref::<Stack>();
            let node_tools = entity_ref::<Stack>();
            let name = entity_ref::<TextInput>();
            let name_field = entity_ref::<FormField>();
            let status = entity_ref::<StatusBadge>();
            let error = entity_ref::<ValidationMessage>();
            let publish = entity_ref::<Button>();
            let run = entity_ref::<Button>();
            let toggle = entity_ref::<Button>();
            let add_human = entity_ref::<Button>();
            let create = entity_ref::<SidebarFooterButton>();
            let back = entity_ref::<SidebarRow>();
            let content_scroll = entity_ref::<ScrollView>();
            let run_panel = entity_ref::<Stack>();
            let run_picker = entity_ref::<SearchDropdown>();
            let run_detail = entity_ref::<EmptyState>();
            let response = entity_ref::<TextArea>();
            let run_actions = entity_ref::<Stack>();
            let resume = entity_ref::<Button>();
            let cancel = entity_ref::<Button>();
            let inspector = entity_ref::<SearchDropdown>();
            let scope_panel = entity_ref::<Stack>();
            let inbox = entity_ref::<Switch>();
            let mut event_refs = Vec::new();
            let mut event_views = Vec::new();
            for (value, label) in EVENT_KIND_OPTIONS {
                let toggle_ref = entity_ref();
                event_refs.push(toggle_ref);
                event_views.push(event_switch(
                    toggle_ref,
                    value,
                    label,
                    event_kinds,
                    &target,
                    &sink,
                ));
            }
            let name_target = Arc::clone(&target);
            let name_sink = Arc::clone(&sink);
            let name_control = widget(FormField::new("工作流名称"))
                .entity_ref(name_field)
                .visible(show_workflow)
                .control(text_input().entity_ref(name).value(name_text).on_input(
                    move |event: &TextChanged| {
                        emit_automation(
                            &name_sink,
                            &name_target,
                            AutomationAction::Rename(event.value.to_string()),
                        );
                    },
                ));
            let graph_target = Arc::clone(&target);
            let graph_sink = Arc::clone(&sink);
            let canvas_view = widget(GraphCanvas::new("automations", GraphModel::empty()))
                .entity_ref(canvas)
                .bind(move |canvas| {
                    canvas_state.with(|state| {
                        canvas.model = state.model.clone();
                        canvas.viewport = state.viewport;
                        canvas.selection = state.selection.clone();
                    });
                    set_hidden(&mut canvas.style, !show_canvas.get());
                })
                .on(move |event: &GraphCanvasEvent| {
                    emit_automation(
                        &graph_sink,
                        &graph_target,
                        AutomationAction::Graph(event.clone()),
                    );
                });
            let response_target = Arc::clone(&run_target);
            let response_sink = Arc::clone(&sink);
            let response_view = widget(
                TextArea::new("")
                    .placeholder("补充说明（可选）")
                    .height(72.0),
            )
            .entity_ref(response)
            .visible(show_response)
            .value(response_text)
            .on_input(move |event: &TextChanged| {
                if let Some((target, run_id, Some(node_id), _)) =
                    response_target.lock().unwrap().clone()
                {
                    emit(
                        &response_sink,
                        ShellIntent::Automation {
                            target,
                            action: AutomationAction::Respond {
                                run_id,
                                node_id,
                                value: event.value.to_string(),
                            },
                        },
                    );
                }
            });
            let resume_target = Arc::clone(&run_target);
            let resume_sink = Arc::clone(&sink);
            let cancel_target = Arc::clone(&run_target);
            let cancel_sink = Arc::clone(&sink);
            let picker_target = Arc::clone(&target);
            let picker_sink = Arc::clone(&sink);
            let inspector_target = Arc::clone(&target);
            let inspector_sink = Arc::clone(&sink);
            let inbox_target = Arc::clone(&target);
            let inbox_sink = Arc::clone(&sink);
            let back_sink = Arc::clone(&sink);
            let refresh_sink = Arc::clone(&sink);
            let create_sink = Arc::clone(&sink);
            let listed = rows;
            let page_view = widget(Stack::fill_column(12.0).padding(16.0))
                .entity_ref(page)
                .children(
                    widget(
                        ScrollView::new(ScrollAxes::Vertical)
                            .style(Stack::fill_column(0.0).node_style()),
                    )
                    .entity_ref(content_scroll)
                    .children((
                        widget(ValidationMessage::new("", ValidationIntent::Danger))
                            .entity_ref(error)
                            .visible(show_error)
                            .bind(move |message| {
                                error_text.with(|text| message.message = Arc::from(text.as_str()));
                            }),
                        empty_state("还没有自动化")
                            .message("新建工作流，添加步骤后保存并发布。")
                            .icon(Icon::Nodes)
                            .entity_ref(empty)
                            .visible(show_empty),
                        name_control,
                        widget(Stack::row(0.0)).visible(show_workflow).children(
                            status_badge(status_label)
                                .tone(status_tone)
                                .entity_ref(status)
                                .visible(show_workflow),
                        ),
                        widget(Stack::bar(8.0).wrap(true))
                            .entity_ref(toolbar)
                            .visible(show_workflow)
                            .children((
                                action_button(
                                    "保存草稿",
                                    ButtonKind::Subtle,
                                    &target,
                                    &sink,
                                    AutomationAction::Save,
                                ),
                                action_button(
                                    "保存并发布",
                                    ButtonKind::Primary,
                                    &target,
                                    &sink,
                                    AutomationAction::Publish,
                                )
                                .entity_ref(publish),
                                action_button(
                                    "运行已发布版本",
                                    ButtonKind::Primary,
                                    &target,
                                    &sink,
                                    AutomationAction::Run,
                                )
                                .entity_ref(run)
                                .disabled(run_disabled),
                                action_button(
                                    "启用",
                                    ButtonKind::Subtle,
                                    &target,
                                    &sink,
                                    AutomationAction::ToggleEnabled,
                                )
                                .entity_ref(toggle)
                                .label(toggle_label),
                                widget(SearchDropdown::new(None::<String>).placeholder("检查器"))
                                    .entity_ref(inspector)
                                    .bind(move |picker| {
                                        picker.options = INSPECTOR_PANELS
                                            .iter()
                                            .map(|(value, label)| {
                                                SearchDropdownOption::new(*value, *label)
                                            })
                                            .collect();
                                        picker.value = Some(
                                            inspector_panel.with(|panel| Arc::from(panel.as_str())),
                                        );
                                    })
                                    .on(move |event: &SearchDropdownEvent| {
                                        if let SearchDropdownEvent::Select(panel) = event {
                                            emit_automation(
                                                &inspector_sink,
                                                &inspector_target,
                                                AutomationAction::SetInspector(panel.to_string()),
                                            );
                                        }
                                    }),
                            )),
                        widget(Stack::bar(8.0).wrap(true))
                            .entity_ref(node_tools)
                            .visible(show_workflow)
                            .children((
                                action_button(
                                    "添加 Agent",
                                    ButtonKind::Subtle,
                                    &target,
                                    &sink,
                                    AutomationAction::AddNode("agent".into()),
                                ),
                                action_button(
                                    "添加工具",
                                    ButtonKind::Subtle,
                                    &target,
                                    &sink,
                                    AutomationAction::AddNode("tool".into()),
                                ),
                                action_button(
                                    "添加条件",
                                    ButtonKind::Subtle,
                                    &target,
                                    &sink,
                                    AutomationAction::AddNode("logic".into()),
                                ),
                                action_button(
                                    "添加人工确认",
                                    ButtonKind::Subtle,
                                    &target,
                                    &sink,
                                    AutomationAction::AddNode("human".into()),
                                )
                                .entity_ref(add_human),
                            )),
                        widget(
                            Stack::fill_row(16.0)
                                .min_height(LengthSpec::Px(inspector_row_min_height())),
                        )
                        .entity_ref(body)
                        .visible(show_workflow)
                        .children(canvas_view),
                        view_column(8.0)
                            .entity_ref(run_panel)
                            .visible(show_workflow)
                            .children((
                                widget(SearchDropdown::new(None::<String>).placeholder("运行记录"))
                                    .entity_ref(run_picker)
                                    .bind(move |picker| {
                                        set_hidden(&mut picker.style, !show_picker.get());
                                        picker.options = runs.with(|runs| {
                                            runs.iter()
                                                .map(|run| {
                                                    SearchDropdownOption::new(
                                                        run.id.clone(),
                                                        run.label.clone(),
                                                    )
                                                })
                                                .collect()
                                        });
                                        picker.value = runs.with(|runs| {
                                            let selected = selected_run.get();
                                            runs.iter()
                                                .find(|run| {
                                                    selected.as_deref() == Some(run.id.as_str())
                                                })
                                                .map(|run| Arc::from(run.id.as_str()))
                                        });
                                    })
                                    .on(move |event: &SearchDropdownEvent| {
                                        if let SearchDropdownEvent::Select(id) = event {
                                            emit_automation(
                                                &picker_sink,
                                                &picker_target,
                                                AutomationAction::SelectRun(id.to_string()),
                                            );
                                        }
                                    }),
                                empty_state(detail_title)
                                    .message(detail_message)
                                    .compact(true)
                                    .entity_ref(run_detail)
                                    .visible(show_detail),
                                response_view,
                                widget(Stack::bar(8.0).wrap(true))
                                    .entity_ref(run_actions)
                                    .visible(show_actions)
                                    .children((
                                        widget(Button::new("确认并继续").kind(ButtonKind::Primary))
                                            .entity_ref(resume)
                                            .disabled(resume_disabled)
                                            .visible(show_resume)
                                            .on_activate(move || {
                                                emit_run_action(&resume_sink, &resume_target, true);
                                            }),
                                        widget(Button::new("取消运行").kind(ButtonKind::Subtle))
                                            .entity_ref(cancel)
                                            .disabled(cancel_disabled)
                                            .visible(show_cancel)
                                            .on_activate(move || {
                                                emit_run_action(
                                                    &cancel_sink,
                                                    &cancel_target,
                                                    false,
                                                );
                                            }),
                                    )),
                            )),
                        view_column(8.0)
                            .entity_ref(scope_panel)
                            .visible(show_scope)
                            .children((
                                switch("包括收件箱")
                                    .entity_ref(inbox)
                                    .checked(inbox_checked)
                                    .on_change(move |_| {
                                        emit_automation(
                                            &inbox_sink,
                                            &inbox_target,
                                            AutomationAction::ToggleInbox,
                                        );
                                    }),
                                {
                                    let project_target = Arc::clone(&target);
                                    let project_sink = Arc::clone(&sink);
                                    let project_items = projects;
                                    projects
                                        .each(
                                            |project| project.0.clone(),
                                            move |project| {
                                                project_switch(
                                                    project,
                                                    project_items,
                                                    Arc::clone(&project_target),
                                                    Arc::clone(&project_sink),
                                                )
                                            },
                                        )
                                        .gap(8.0)
                                        .visible(move || projects.with(|items| !items.is_empty()))
                                },
                                view_column(8.0).children(event_views),
                            )),
                    )),
                );
            let sidebar_view = widget(SidebarFrame::new())
                .entity_ref(sidebar)
                .top(
                    widget(SidebarRow::new("返回"))
                        .child_slot(
                            widget(nana_ui::runtime::SidebarRowIcon::new(Icon::ArrowLeft)),
                            |mut row, icon| {
                                row.slots.leading = Some(icon);
                                row
                            },
                        )
                        .entity_ref(back)
                        .on(move |_: &Activate| emit(&back_sink, ShellIntent::CloseAutomations)),
                )
                .body(
                    widget(SidebarSection::new("自动化"))
                        .entity_ref(section)
                        .bind(move |section| {
                            section.count = Some(listed.with(|rows| rows.len()));
                        })
                        .children({
                            let row_sink = Arc::clone(&sink);
                            let row_items = rows;
                            rows.each(
                                |row| row.id.clone(),
                                move |row| workflow_row(row, row_items, Arc::clone(&row_sink)),
                            )
                            .gap(4.0)
                        }),
                )
                .footer(
                    widget(SidebarFooter::new()).children((
                        widget(SidebarFooterButton::new("刷新", Icon::Activity)).on(
                            move |_: &Activate| {
                                emit(&refresh_sink, ShellIntent::RefreshAutomations)
                            },
                        ),
                        widget(SidebarFooterButton::new("新建", Icon::Add))
                            .entity_ref(create)
                            .on(move |_: &Activate| {
                                emit(&create_sink, ShellIntent::CreateAutomation)
                            }),
                    )),
                );
            with_refs(
                (page_view, sidebar_view),
                (
                    (
                        page, sidebar, canvas, body, section, empty, toolbar, node_tools,
                    ),
                    (
                        name, name_field, status, error, publish, run, toggle, add_human,
                    ),
                    (
                        create,
                        back,
                        content_scroll,
                        run_panel,
                        run_picker,
                        run_detail,
                        response,
                        run_actions,
                    ),
                    (resume, cancel, inspector, scope_panel, inbox),
                    event_refs,
                ),
            )
        })?;
        let (
            _,
            (
                (page, sidebar, canvas, body, section, empty, toolbar, node_tools),
                (name, name_field, status, error, publish, run, toggle, add_human),
                (
                    create,
                    back,
                    content_scroll,
                    run_panel,
                    run_picker,
                    run_detail,
                    response,
                    run_actions,
                ),
                (resume, cancel, inspector, scope_panel, inbox),
                event_switches,
            ),
        ) = mounted;
        let port = context
            .read(section, |section| section.body)?
            .ok_or(FrameworkError::InvalidInput)?;
        let list = only_child(context, port)?;
        let project_list = child_at(context, scope_panel.stable_id(), 1)?;
        let mut event_toggles = HashMap::new();
        for ((value, _), toggle) in EVENT_KIND_OPTIONS.iter().zip(event_switches) {
            event_toggles.insert((*value).to_owned(), toggle);
        }
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
            project_list,
            project_toggles: HashMap::new(),
            event_toggles,
            run_target: stored_run_target,
            rows: HashMap::new(),
            target: stored_target,
            workflow_rows,
            project_rows,
            runs,
            event_kinds,
            show_error,
            error_text,
            show_empty,
            show_workflow,
            show_scope,
            show_canvas,
            name_text,
            status_label,
            status_tone,
            run_disabled,
            toggle_label,
            inspector_panel,
            show_picker,
            show_detail,
            detail_title,
            detail_message,
            show_response,
            response_text,
            show_resume,
            show_cancel,
            show_actions,
            resume_disabled,
            cancel_disabled,
            inbox_checked,
            selected_run,
            canvas_state,
            name_workflow: None,
            response_run: None,
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
            context.update_component(self.run_picker, |picker, _| {
                picker.close();
            })?;
            context.update_component(self.inspector, |picker, _| {
                picker.close();
            })?;
            return Ok(());
        }
        context.update_component(self.editor.root, |root, _| {
            *root = editor_frame(snapshot.compact, snapshot.editor.is_none());
        })?;
        let workflow_id = snapshot
            .target
            .as_ref()
            .map(|target| target.workflow_id.clone());
        if self.name_workflow != workflow_id {
            replace_input(context, self.name, &snapshot.name)?;
            self.name_workflow = workflow_id;
        }
        let run_id = selected_run.map(|run| run.id.clone());
        if self.response_run != run_id {
            replace_area(context, self.response, &snapshot.response)?;
            self.response_run = run_id;
        }
        let (label, tone) = workflow_status(snapshot);
        let (title, message) = run_detail_copy(selected_run);
        let waiting = selected_run.is_some_and(|run| run.waiting_node.is_some());
        let cancellable = selected_run.is_some_and(|run| run.can_cancel);
        self.workflow_rows.set(snapshot.rows.clone());
        self.project_rows.set(snapshot.projects.clone());
        self.runs.set(snapshot.runs.clone());
        self.event_kinds.set(snapshot.event_kinds.clone());
        self.show_error.set(snapshot.error.is_some());
        self.error_text
            .set(snapshot.error.clone().unwrap_or_default());
        self.show_empty.set(snapshot.target.is_none());
        self.show_workflow.set(snapshot.target.is_some());
        self.show_scope
            .set(snapshot.target.is_some() && snapshot.inspector_panel == "scope");
        self.show_canvas
            .set(!(snapshot.editor.is_some() && snapshot.compact));
        self.name_text.set(snapshot.name.clone());
        self.status_label.set(label);
        self.status_tone.set(tone);
        self.run_disabled.set(
            !snapshot.published
                || snapshot.operation_pending
                || snapshot.runs.iter().any(|run| run.can_cancel),
        );
        self.toggle_label
            .set(if snapshot.enabled { "停用" } else { "启用" }.to_owned());
        self.inspector_panel.set(snapshot.inspector_panel.clone());
        self.show_picker.set(!snapshot.runs.is_empty());
        self.show_detail
            .set(selected_run.is_none_or(|run| run.error.is_some() || run.prompt.is_some()));
        self.detail_title.set(title);
        self.detail_message.set(message);
        self.show_response.set(waiting);
        self.response_text.set(snapshot.response.clone());
        self.show_resume.set(waiting);
        self.show_cancel.set(cancellable);
        self.show_actions.set(waiting || cancellable);
        self.resume_disabled.set(snapshot.operation_pending);
        self.cancel_disabled.set(snapshot.cancel_pending);
        self.inbox_checked.set(snapshot.include_inbox);
        self.selected_run
            .set(selected_run.map(|run| run.id.clone()));
        self.canvas_state.set(CanvasState {
            model: snapshot.graph.clone(),
            viewport: snapshot.viewport,
            selection: snapshot.selection.clone(),
        });
        context.flush_reactive()?;
        self.rows = zip_entities(context, self.list, &snapshot.rows, |row| &row.id);
        self.project_toggles =
            zip_entities(context, self.project_list, &snapshot.projects, |project| {
                &project.0
            });
        self.ensure_editor(context)
    }

    fn ensure_editor(&self, context: &mut AppContext) -> Result<(), FrameworkError> {
        let present = context
            .world()
            .node(self.body.stable_id())
            .is_some_and(|body| body.children.contains(&self.editor.root.stable_id()));
        if !present {
            context.append_child(self.body, self.editor.root)?;
        }
        Ok(())
    }
}

fn emit_automation(
    sink: &IntentSink,
    target: &Mutex<Option<AutomationTarget>>,
    action: AutomationAction,
) {
    if let Some(target) = target.lock().unwrap().clone() {
        emit(sink, ShellIntent::Automation { target, action });
    }
}

fn emit_run_action(
    sink: &IntentSink,
    run_target: &Mutex<Option<(AutomationTarget, String, Option<String>, bool)>>,
    resume: bool,
) {
    if let Some((target, run_id, waiting, cancellable)) = run_target.lock().unwrap().clone() {
        if (resume && waiting.is_some()) || (!resume && cancellable) {
            let action = if resume {
                AutomationAction::Resume {
                    run_id,
                    node_id: waiting.unwrap(),
                }
            } else {
                AutomationAction::Cancel { run_id }
            };
            emit(sink, ShellIntent::Automation { target, action });
        }
    }
}

fn action_button(
    label: &str,
    kind: ButtonKind,
    target: &Arc<Mutex<Option<AutomationTarget>>>,
    sink: &IntentSink,
    action: AutomationAction,
) -> nana_ui::runtime::view::El<Button> {
    let target = Arc::clone(target);
    let sink = Arc::clone(sink);
    widget(Button::new(label).kind(kind)).on_activate(move || {
        emit_automation(&sink, &target, action.clone());
    })
}

fn event_switch(
    toggle_ref: EntityRef<Switch>,
    value: &str,
    label: &str,
    kinds: Signal<Vec<String>>,
    target: &Arc<Mutex<Option<AutomationTarget>>>,
    sink: &IntentSink,
) -> nana_ui::runtime::view::El<Switch> {
    let checked_value = value.to_owned();
    let action_value = value.to_owned();
    let target = Arc::clone(target);
    let sink = Arc::clone(sink);
    switch(label.to_owned())
        .entity_ref(toggle_ref)
        .checked(move || {
            let value = checked_value.clone();
            kinds.with(|kinds| kinds.iter().any(|kind| kind == &value))
        })
        .on_change(move |_| {
            emit_automation(
                &sink,
                &target,
                AutomationAction::ToggleScope {
                    field: "event-kind".into(),
                    value: action_value.clone(),
                },
            );
        })
}

fn project_switch(
    project: (String, String, bool),
    projects: Signal<Vec<(String, String, bool)>>,
    target: Arc<Mutex<Option<AutomationTarget>>>,
    sink: IntentSink,
) -> nana_ui::runtime::view::El<Switch> {
    let id = project.0;
    let label_id = id.clone();
    let checked_id = id.clone();
    switch(move || {
        let id = label_id.clone();
        projects.with(|projects| {
            projects
                .iter()
                .find(|item| item.0 == id)
                .map(|item| item.1.clone())
                .unwrap_or_default()
        })
    })
    .checked(move || {
        let id = checked_id.clone();
        projects.with(|projects| {
            projects
                .iter()
                .find(|item| item.0 == id)
                .is_some_and(|item| item.2)
        })
    })
    .on_change(move |_| {
        emit_automation(
            &sink,
            &target,
            AutomationAction::ToggleScope {
                field: "project".into(),
                value: id.clone(),
            },
        );
    })
}

fn workflow_row(
    row: AutomationRow,
    rows: Signal<Vec<AutomationRow>>,
    sink: IntentSink,
) -> nana_ui::runtime::view::El<SidebarRow> {
    let id = row.id;
    let label_id = id.clone();
    widget(SidebarRow::new(""))
        .bind(move |item| {
            let id = label_id.clone();
            let (label, selected) = rows.with(|rows| {
                rows.iter()
                    .find(|row| row.id == id)
                    .map(|row| (row.label.clone(), row.selected))
                    .unwrap_or_default()
            });
            item.label = Arc::from(label);
            item.state = if selected {
                SidebarRowState::Active
            } else {
                SidebarRowState::Idle
            };
        })
        .on(move |_: &Activate| emit(&sink, ShellIntent::SelectAutomation(id.clone())))
}

fn workflow_status(snapshot: &AutomationViewSnapshot) -> (Arc<str>, StatusTone) {
    if snapshot.operation_pending {
        (Arc::from("正在处理…"), StatusTone::Info)
    } else if snapshot.published {
        if snapshot.enabled {
            (Arc::from("已发布 · 已启用"), StatusTone::Success)
        } else {
            (Arc::from("已发布 · 已停用"), StatusTone::Warning)
        }
    } else {
        (Arc::from("草稿 · 发布后可运行"), StatusTone::Neutral)
    }
}

fn run_detail_copy(run: Option<&AutomationRunView>) -> (Arc<str>, Option<Arc<str>>) {
    if let Some(run) = run {
        let detail = [run.error.as_deref(), run.prompt.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n");
        (Arc::from("运行详情"), Some(Arc::from(detail)))
    } else {
        (Arc::from("尚无运行记录"), None)
    }
}

/// Floor for the canvas and node-editor row.
///
/// That row shrinks. The canvas and the editor both fill it with a zero
/// minimum, so a tall trigger-scope list collapses the row and the editor
/// paints over the switches. The floor matches the editor stack: the save
/// row, the title field, the prompt field (its text area is 120px), and the
/// column gaps.
fn inspector_row_min_height() -> f32 {
    let control = nana_ui::UI_METRICS.control_height;
    let label_band = nana_ui_core::type_scale::LINE + nana_ui_core::space::XS;
    let gap = 8.0;
    let prompt = 120.0;
    control + gap + (label_band + control) + gap + (label_band + prompt)
}

fn editor_frame(compact: bool, hidden: bool) -> Stack {
    let stack = if compact {
        Stack::fill_column(8.0)
    } else {
        Stack::fill_column(8.0)
            .width(LengthSpec::Px(320.0))
            .grow(0.0)
            .shrink(0.0)
    };
    stack.with_layout(|layout| {
        layout.hidden = hidden;
        layout.overflow_x = nana_ui_core::OverflowSpec::Hidden;
        layout.overflow_y = nana_ui_core::OverflowSpec::Hidden;
    })
}

fn set_hidden(style: &mut NodeStyle, hidden: bool) {
    if style.layout.hidden != hidden {
        Arc::make_mut(&mut style.layout).hidden = hidden;
    }
}

fn replace_area(
    context: &mut AppContext,
    input: Entity<TextArea>,
    value: &str,
) -> Result<(), FrameworkError> {
    context.update_component(input, |input, _| {
        input.state.replace_value(value.to_owned());
    })
}

fn replace_input(
    context: &mut AppContext,
    input: Entity<TextInput>,
    value: &str,
) -> Result<(), FrameworkError> {
    context.update_component(input, |input, _| {
        input.state.replace_value(value.to_owned());
    })
}

fn only_child<C: View>(
    context: &AppContext,
    parent: StableNodeId,
) -> Result<Entity<C>, FrameworkError> {
    child_at(context, parent, 0)
}

fn child_at<C: View>(
    context: &AppContext,
    parent: StableNodeId,
    index: usize,
) -> Result<Entity<C>, FrameworkError> {
    let id = context
        .world()
        .node(parent)
        .and_then(|node| node.children.get(index).copied())
        .ok_or(FrameworkError::InvalidInput)?;
    Ok(Entity::from_stable_id(id))
}

fn zip_entities<C: View, T>(
    context: &AppContext,
    list: Entity<Stack>,
    items: &[T],
    id_of: impl Fn(&T) -> &str,
) -> HashMap<String, Entity<C>> {
    let children = context
        .world()
        .node(list.stable_id())
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut rows = HashMap::new();
    let mut cursor = children.into_iter();
    let mut seen = HashSet::new();
    for item in items {
        let id = id_of(item);
        if !seen.insert(id.to_owned()) {
            continue;
        }
        let Some(child) = cursor.next() else {
            break;
        };
        rows.insert(id.to_owned(), Entity::from_stable_id(child));
    }
    rows
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
        assert!(!is_hidden(&context, view.response));
        state.runs[0].prompt = None;
        state.runs[0].waiting_node = None;
        state.runs[0].can_cancel = false;
        view.sync(&mut context, document, &state, true).unwrap();
        for button in [view.resume, view.cancel] {
            context
                .update_component(button, |_, cx| cx.emit(Activate))
                .unwrap();
        }
        assert!(is_hidden(&context, view.response));
        assert!(is_hidden(&context, view.run_actions));
        assert!(is_hidden(&context, view.run_detail));
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
        assert!(children.contains(&view.error.stable_id()));
        assert!(children.contains(&view.empty.stable_id()));
        assert!(children.contains(&view.body.stable_id()));
        assert!(!is_hidden(&context, view.error));
        assert!(!is_hidden(&context, view.empty));
        assert!(is_hidden(&context, view.name_field));
        assert!(is_hidden(&context, view.status));
        assert!(is_hidden(&context, view.toolbar));
        assert!(is_hidden(&context, view.node_tools));
        assert!(is_hidden(&context, view.body));
        assert!(is_hidden(&context, view.run_panel));
        assert!(is_hidden(&context, view.scope_panel));
        view.sync(&mut context, document, &snapshot("restored"), true)
            .unwrap();
        assert_eq!(
            context.read(view.section, |section| section.count).unwrap(),
            Some(1)
        );
        assert!(!is_hidden(&context, view.body));
        assert!(is_hidden(&context, view.empty));
        assert!(context
            .world()
            .node(view.body.stable_id())
            .unwrap()
            .children
            .contains(&view.canvas.stable_id()));
        assert!(!is_hidden(&context, view.canvas));
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

    #[cfg(debug_assertions)]
    #[test]
    fn compact_editor_hides_the_canvas_without_dropping_it() {
        let mut context = AppContext::new();
        let document = DocumentId::new(336).unwrap();
        let mut state = snapshot("workflow");
        state.compact = true;
        state.editor = Some(super::super::editor::NodeEditorSnapshot {
            node_id: "node".into(),
            title: "节点".into(),
            fields: Vec::new(),
        });
        let mut view =
            AutomationView::mount(&mut context, document, &state, true, Arc::new(|_| {})).unwrap();
        assert!(context
            .world()
            .node(view.body.stable_id())
            .unwrap()
            .children
            .contains(&view.canvas.stable_id()));
        assert!(is_hidden(&context, view.canvas));
        assert!(!view.graph_is_mounted(&context));
        state.compact = false;
        view.sync(&mut context, document, &state, true).unwrap();
        assert!(!is_hidden(&context, view.canvas));
        assert!(view.graph_is_mounted(&context));
    }

    fn is_hidden<V: View>(context: &AppContext, entity: Entity<V>) -> bool {
        context
            .world()
            .node_style(entity.stable_id())
            .is_some_and(|style| style.layout.hidden)
    }
}
