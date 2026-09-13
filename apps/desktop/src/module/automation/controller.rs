use super::editor::{AutomationNodeInspectorDraft, automation_node_config_fields};
use super::graph::automation_graph_model;
use crate::desktop::format_civil_date;
use crate::ui_module::{UiModule, UiModuleContext, UiModuleOutcome};
use lilia_contracts::AutomationSignalEnvelope;
use lilia_feature_automation::{
    AutomationNode, AutomationNodePosition, AutomationRunDetail, AutomationRunStatus,
    AutomationRunSummary, AutomationSaveDraftInput, AutomationScopeFilter, AutomationWorkflow,
    DesktopAutomationService,
};
use lilia_kernel::{JobEvent, JobRequest, JobState, Jobs};
use nana_ui::{
    GraphCanvasEvent, GraphEndpoint, GraphModel, GraphPoint, GraphSelection, GraphViewport,
};
use nana_ui_platform::WindowId;
use serde_json::{Value, json};

pub(crate) enum AutomationModuleMessage {
    Refresh,
    Select(String),
    Create,
    Rename(String),
    Action {
        target: super::view::AutomationTarget,
        action: super::view::AutomationAction,
    },
}

pub(crate) struct AutomationController {
    window: WindowId,
    service: DesktopAutomationService,
    jobs: Jobs,
    workflows: Vec<AutomationWorkflow>,
    selected_workflow_id: Option<String>,
    runs: Vec<AutomationRunSummary>,
    selected_run_id: Option<String>,
    run_detail: Option<AutomationRunDetail>,
    response: String,
    operations: crate::module::automation::operations::AutomationOperations,
    graph: GraphModel,
    viewport: GraphViewport,
    selection: Option<GraphSelection>,
    editor: AutomationNodeInspectorDraft,
    inspector_panel: String,
    error: Option<String>,
    loaded_name: Option<String>,
    loaded_updated_at: Option<i64>,
    loaded_node_id: Option<String>,
    loaded_title: String,
    loaded_config: String,
    #[cfg(test)]
    fail_list_runs: bool,
}

impl AutomationController {
    pub(crate) fn feature_id() -> lilia_kernel::FeatureId {
        lilia_kernel::FeatureId::new("lilia.automation").expect("automation feature identity")
    }
    #[cfg(debug_assertions)]
    pub(crate) fn edit_node_title(&mut self, title: String) {
        if self.editor.node_id.is_some() {
            self.editor.title = title;
            self.error = None;
        }
    }
    #[cfg(debug_assertions)]
    pub(crate) fn edit_node_config(&mut self, config: String) {
        if self.editor.node_id.is_some() {
            self.editor.config = config;
            self.error = None;
        }
    }
    #[cfg(debug_assertions)]
    pub(crate) fn edit_response(&mut self, response: String) {
        if self
            .run_detail
            .as_ref()
            .and_then(waiting_human_node)
            .is_some()
        {
            self.response = response;
            self.error = None;
        }
    }
    pub(crate) fn workflows(&self) -> &Vec<AutomationWorkflow> {
        &self.workflows
    }
    pub(crate) fn selected_workflow_id(&self) -> &Option<String> {
        &self.selected_workflow_id
    }
    pub(crate) fn runs(&self) -> &Vec<AutomationRunSummary> {
        &self.runs
    }
    pub(crate) fn selected_run_id(&self) -> &Option<String> {
        &self.selected_run_id
    }
    pub(crate) fn run_detail(&self) -> &Option<AutomationRunDetail> {
        &self.run_detail
    }
    pub(crate) fn operations(
        &self,
    ) -> &crate::module::automation::operations::AutomationOperations {
        &self.operations
    }
    pub(crate) fn graph(&self) -> &GraphModel {
        &self.graph
    }
    pub(crate) fn selection(&self) -> &Option<GraphSelection> {
        &self.selection
    }
    pub(crate) fn editor(&self) -> &AutomationNodeInspectorDraft {
        &self.editor
    }
    pub(crate) fn error(&self) -> &Option<String> {
        &self.error
    }

    pub(crate) fn toggle_automation_scope_value(
        &mut self,
        field: &str,
        value: &str,
        project_exists: bool,
    ) {
        let Some(selected) = self.selected_workflow_id.clone() else {
            return;
        };
        let value_allowed = match field {
            "project" => project_exists,
            "task-status" => matches!(value, "waiting" | "running" | "blocked" | "done"),
            "backend" => value == "native-agentkit",
            "event-kind" => matches!(
                value,
                "task_created"
                    | "task_status_changed"
                    | "task_updated"
                    | "timeline_event"
                    | "todo_changed"
                    | "interaction_request"
            ),
            _ => false,
        };
        if !value_allowed {
            return;
        }
        let Some(workflow) = self
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == selected)
        else {
            return;
        };
        let values = match field {
            "project" => &mut workflow.scope.project_ids,
            "task-status" => &mut workflow.scope.task_statuses,
            "backend" => &mut workflow.scope.backends,
            "event-kind" => &mut workflow.scope.event_kinds,
            _ => return,
        };
        if let Some(index) = values.iter().position(|item| item == value) {
            values.remove(index);
        } else {
            values.push(value.to_owned());
            values.sort();
            values.dedup();
        }
        workflow.draft.scope = workflow.scope.clone();
        self.persist_selected_automation_draft();
    }

    pub(crate) fn apply_action(
        &mut self,
        target: super::view::AutomationTarget,
        action: super::view::AutomationAction,
        project_id: Option<lilia_contracts::ProjectId>,
        task_id: Option<lilia_contracts::TaskId>,
    ) {
        if target.window_id != self.window
            || !self.selected_automation_workflow().is_some_and(|workflow| {
                workflow.id == target.workflow_id
                    && self.action_clock(workflow) == target.modified_at
            })
        {
            return;
        }
        use crate::module::automation::view::AutomationAction;
        match action {
            AutomationAction::Node { node_id, action } => {
                if self.editor.node_id.as_deref() != Some(node_id.as_str()) {
                    return;
                }
                use crate::module::automation::editor::NodeEditorAction;
                match action {
                    NodeEditorAction::Title(value) => {
                        self.editor.title = value;
                        self.error = None;
                    }
                    NodeEditorAction::Field { key, value } => {
                        if self.selected_automation_node().is_some_and(|node| {
                            automation_node_config_fields(&node.kind, &self.editor.config)
                                .contains(&key.as_str())
                        }) {
                            self.update_automation_node_config_field(key, value);
                        }
                    }
                    NodeEditorAction::Save => self.save_automation_node_inspector(),
                    NodeEditorAction::Close => {
                        self.update_automation_graph(GraphCanvasEvent::SelectionChanged(None))
                    }
                }
            }
            AutomationAction::Rename(value) => self.rename(value),
            AutomationAction::Save => self.persist_selected_automation_draft(),
            AutomationAction::Publish => {
                self.persist_selected_automation_draft();
                if self.error.is_none() {
                    self.publish_automation();
                }
            }
            AutomationAction::Run => self.run_automation(project_id, task_id),
            AutomationAction::ToggleEnabled => self.toggle_automation(),
            AutomationAction::AddNode(kind) => self.add_automation_node(&kind),
            AutomationAction::Graph(event) => self.update_automation_graph(event),
            AutomationAction::SelectRun(run_id) => self.select_automation_run(run_id),
            AutomationAction::Respond {
                run_id,
                node_id,
                value,
            } => {
                if self.run_detail.as_ref().is_some_and(|detail| {
                    detail.run.id == run_id
                        && detail.run.workflow_id == target.workflow_id
                        && detail.run.status == AutomationRunStatus::WaitingUser
                        && waiting_human_node(detail).is_some_and(|node| node.node_id == node_id)
                }) {
                    self.response = value;
                }
            }
            AutomationAction::Resume { run_id, node_id } => {
                if self.run_detail.as_ref().is_some_and(|detail| {
                    detail.run.id == run_id
                        && detail.run.workflow_id == target.workflow_id
                        && detail.run.status == AutomationRunStatus::WaitingUser
                        && waiting_human_node(detail).is_some_and(|node| node.node_id == node_id)
                }) {
                    self.resume_automation();
                }
            }
            AutomationAction::Cancel { run_id } => {
                if self.run_detail.as_ref().is_some_and(|detail| {
                    detail.run.id == run_id && detail.run.workflow_id == target.workflow_id
                }) {
                    self.cancel_automation();
                }
            }
            AutomationAction::SetInspector(panel) => {
                if super::view::INSPECTOR_PANELS
                    .iter()
                    .any(|(value, _)| *value == panel)
                {
                    self.inspector_panel = panel;
                }
            }
            AutomationAction::ToggleInbox => self.toggle_automation_scope_inbox(),
            AutomationAction::ToggleScope { field, value } => {
                self.toggle_automation_scope_value(&field, &value, true);
            }
        }
    }
    pub(crate) fn run_automation(
        &mut self,
        project_id: Option<lilia_contracts::ProjectId>,
        task_id: Option<lilia_contracts::TaskId>,
    ) {
        let Some(workflow) = self.selected_automation_workflow() else {
            return;
        };
        if workflow.published_version_id.is_none() {
            self.error = Some("请先发布自动化再运行。".to_owned());
            return;
        }
        let workflow_id = workflow.id.clone();
        let expected_version_id = workflow.published_version_id.clone().unwrap();
        let now = current_timestamp_millis();
        let signal = AutomationSignalEnvelope {
            id: format!("lilia-signal-{now}"),
            kind: "manual".to_owned(),
            project_id: project_id
                .as_ref()
                .map(|project_id| project_id.as_str().to_owned()),
            task_id: task_id.as_ref().map(|task_id| task_id.as_str().to_owned()),
            backend: Some("lilia".to_owned()),
            event_kind: Some("manual_run".to_owned()),
            automation_run_id: None,
            payload: json!({ "source": "lilia" }),
            created_at: now,
        };
        self.submit_automation_operation(lilia_contracts::AutomationOperationRequest::Start {
            workflow_id,
            expected_version_id,
            trigger: signal,
        });
    }

    pub(crate) fn rename(&mut self, value: String) {
        if let Some(workflow) = self
            .workflows
            .iter_mut()
            .find(|workflow| Some(workflow.id.as_str()) == self.selected_workflow_id.as_deref())
        {
            workflow.name = value;
            self.error = None;
        }
    }

    pub(crate) fn new(window: WindowId, service: DesktopAutomationService, jobs: Jobs) -> Self {
        Self {
            window,
            service,
            jobs,
            workflows: Vec::new(),
            selected_workflow_id: None,
            runs: Vec::new(),
            selected_run_id: None,
            run_detail: None,
            response: String::new(),
            operations: Default::default(),
            graph: GraphModel::empty(),
            viewport: GraphViewport::default(),
            selection: None,
            editor: AutomationNodeInspectorDraft::default(),
            inspector_panel: "node".into(),
            error: None,
            loaded_name: None,
            loaded_updated_at: None,
            loaded_node_id: None,
            loaded_title: String::new(),
            loaded_config: String::new(),
            #[cfg(test)]
            fail_list_runs: false,
        }
    }

    fn name_is_dirty(&self) -> bool {
        match (
            self.selected_automation_workflow().map(|workflow| &workflow.name),
            self.loaded_name.as_ref(),
        ) {
            (Some(name), Some(loaded)) => name != loaded,
            _ => false,
        }
    }

    fn inspector_is_dirty(&self) -> bool {
        self.editor.node_id != self.loaded_node_id
            || self.editor.title != self.loaded_title
            || self.editor.config != self.loaded_config
    }

    fn capture_loaded_inspector(&mut self) {
        self.loaded_node_id = self.editor.node_id.clone();
        self.loaded_title = self.editor.title.clone();
        self.loaded_config = self.editor.config.clone();
    }

    fn capture_loaded_workflow(&mut self) {
        let loaded = self.selected_automation_workflow().map(|workflow| {
            (workflow.name.clone(), workflow.updated_at)
        });
        if let Some((name, updated_at)) = loaded {
            self.loaded_name = Some(name);
            self.loaded_updated_at = Some(updated_at);
        } else {
            self.loaded_name = None;
            self.loaded_updated_at = None;
        }
    }

    fn capture_loaded(&mut self) {
        self.capture_loaded_workflow();
        self.capture_loaded_inspector();
    }

    fn action_clock(&self, workflow: &AutomationWorkflow) -> i64 {
        self.loaded_updated_at.unwrap_or(workflow.updated_at)
    }

    fn adopt_updated_at_if_unchanged(
        &mut self,
        store_name: Option<String>,
        store_updated_at: Option<i64>,
        store_node: Option<&AutomationNode>,
        inspect_node: bool,
    ) {
        let remote_name_unchanged = store_name == self.loaded_name;
        let remote_node_unchanged = if !inspect_node {
            true
        } else {
            match (&self.loaded_node_id, store_node) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(_), Some(node)) => {
                    node.title == self.loaded_title
                        && serde_json::to_string(&node.config).unwrap_or_default()
                            == self.loaded_config
                }
            }
        };
        if remote_name_unchanged && remote_node_unchanged {
            if let Some(updated_at) = store_updated_at {
                self.loaded_updated_at = Some(updated_at);
            }
        }
    }

    pub(crate) fn refresh_automations(&mut self) {
        match self.service.list_workflows() {
            Ok(workflows) => {
                let previous_selection = self.selected_workflow_id.clone();
                let dirty_name = self
                    .name_is_dirty()
                    .then(|| {
                        self.selected_automation_workflow()
                            .map(|workflow| workflow.name.clone())
                    })
                    .flatten();
                let name_dirty = dirty_name.is_some();
                let editor_dirty = self.inspector_is_dirty();
                let previous_editor = self.editor.clone();
                let previous_graph_selection = self.selection.clone();
                self.workflows = workflows;
                if !self.workflows.iter().any(|workflow| {
                    self.selected_workflow_id.as_deref() == Some(workflow.id.as_str())
                }) {
                    self.selected_workflow_id =
                        self.workflows.first().map(|workflow| workflow.id.clone());
                }
                let selection_changed = previous_selection != self.selected_workflow_id;
                let store_name = self
                    .selected_automation_workflow()
                    .map(|workflow| workflow.name.clone());
                let store_updated_at = self
                    .selected_automation_workflow()
                    .map(|workflow| workflow.updated_at);
                let store_node = previous_editor.node_id.as_ref().and_then(|node_id| {
                    self.selected_automation_workflow().and_then(|workflow| {
                        workflow
                            .draft
                            .nodes
                            .iter()
                            .find(|node| node.id == *node_id)
                            .cloned()
                    })
                });
                if !selection_changed {
                    if let Some(name) = dirty_name {
                        if let Some(workflow) = self.workflows.iter_mut().find(|workflow| {
                            Some(workflow.id.as_str()) == self.selected_workflow_id.as_deref()
                        }) {
                            workflow.name = name;
                        }
                    }
                }
                let node_missing = previous_editor.node_id.is_some() && store_node.is_none();
                if node_missing {
                    self.selection = None;
                    self.editor = AutomationNodeInspectorDraft::default();
                }
                self.rebuild_automation_graph(
                    selection_changed || self.graph.nodes().is_empty(),
                );
                if selection_changed {
                    self.refresh_automation_node_inspector();
                    self.capture_loaded();
                } else if node_missing || !editor_dirty {
                    self.refresh_automation_node_inspector();
                    self.capture_loaded_inspector();
                    if name_dirty {
                        self.adopt_updated_at_if_unchanged(
                            store_name,
                            store_updated_at,
                            store_node.as_ref(),
                            false,
                        );
                    } else {
                        self.capture_loaded_workflow();
                    }
                } else {
                    self.editor = previous_editor;
                    self.selection = previous_graph_selection;
                    self.adopt_updated_at_if_unchanged(
                        store_name,
                        store_updated_at,
                        store_node.as_ref(),
                        true,
                    );
                }
                self.refresh_automation_runs();
                if self.error.as_deref() != Some("自动化已更新，请确认后重试。") {
                    self.error = None;
                }
            }
            Err(error) => {
                self.error = Some(format!("无法读取自动化：{error}"));
            }
        }
    }

    pub(crate) fn select_automation(&mut self, workflow_id: String) {
        if self
            .workflows
            .iter()
            .any(|workflow| workflow.id == workflow_id)
        {
            self.selected_workflow_id = Some(workflow_id);
            self.selected_run_id = None;
            self.run_detail = None;
            self.response.clear();
            self.selection = None;
            self.editor = AutomationNodeInspectorDraft::default();
            self.rebuild_automation_graph(true);
            self.capture_loaded();
            self.refresh_automation_runs();
        }
    }

    pub(crate) fn refresh_automation_runs(&mut self) {
        let Some(workflow_id) = self.selected_workflow_id.as_deref() else {
            self.runs.clear();
            self.selected_run_id = None;
            self.run_detail = None;
            self.response.clear();
            return;
        };
        #[cfg(test)]
        if self.fail_list_runs {
            self.error = Some("无法读取运行历史：injected".into());
            return;
        }
        match self.service.list_runs(Some(workflow_id)) {
            Ok(runs) => {
                self.runs = runs;
                if !self
                    .runs
                    .iter()
                    .any(|run| self.selected_run_id.as_deref() == Some(run.id.as_str()))
                {
                    self.selected_run_id = self.runs.first().map(|run| run.id.clone());
                    self.response.clear();
                }
                self.refresh_automation_run_detail();
            }
            Err(error) => {
                self.error = Some(format!("无法读取运行历史：{error}"));
            }
        }
    }

    pub(crate) fn select_automation_run(&mut self, run_id: String) {
        if self.runs.iter().any(|run| run.id == run_id) {
            self.selected_run_id = Some(run_id);
            self.response.clear();
            self.refresh_automation_run_detail();
        }
    }

    pub(crate) fn refresh_automation_run_detail(&mut self) {
        let Some(run_id) = self.selected_run_id.as_deref() else {
            self.run_detail = None;
            return;
        };
        match self.service.run_detail(run_id) {
            Ok(detail) => self.run_detail = detail,
            Err(error) => {
                self.run_detail = None;
                self.error = Some(format!("无法读取运行详情：{error}"));
            }
        }
    }

    pub(crate) fn resume_automation(&mut self) {
        let Some(detail) = self.run_detail.as_ref() else {
            return;
        };
        let Some(node_id) = waiting_human_node(detail).map(|node| node.node_id.clone()) else {
            return;
        };
        self.submit_automation_operation(lilia_contracts::AutomationOperationRequest::Resume {
            workflow_id: detail.run.workflow_id.clone(),
            run_id: detail.run.id.clone(),
            node_id,
            payload: Some(json!({"confirmed":true,"response":self.response.trim()})),
        });
    }

    pub(crate) fn cancel_automation(&mut self) {
        let Some(detail) = self.run_detail.as_ref() else {
            return;
        };
        if !matches!(
            detail.run.status,
            AutomationRunStatus::Running | AutomationRunStatus::WaitingUser
        ) {
            return;
        }
        self.submit_automation_operation(lilia_contracts::AutomationOperationRequest::Cancel {
            workflow_id: detail.run.workflow_id.clone(),
            run_id: detail.run.id.clone(),
        });
    }

    pub(crate) fn submit_automation_operation(
        &mut self,
        request: lilia_contracts::AutomationOperationRequest,
    ) {
        use lilia_contracts::AutomationOperationRequest;
        let workflow_id = request.workflow_id().to_owned();
        let duplicate = match &request {
            AutomationOperationRequest::Cancel { run_id, .. } => self.operations.cancelling(run_id),
            _ => self.operations.busy(&workflow_id),
        };
        if duplicate {
            return;
        }
        let payload = match serde_json::to_value(&request) {
            Ok(payload) => payload,
            Err(error) => {
                self.operations
                    .set_error(&workflow_id, request.run_id(), Some(error.to_string()));
                return;
            }
        };
        match self.jobs.submit(JobRequest::new(
            lilia_feature_automation::AUTOMATION_OPERATE_PROTOCOL,
            payload,
        )) {
            Ok(handle) => {
                self.operations
                    .set_error(&workflow_id, request.run_id(), None);
                self.operations.pending.insert(
                    handle.id(),
                    crate::module::automation::operations::PendingOperation {
                        request,
                        selection: self.selected_run_id.clone(),
                        response: self.response.clone(),
                    },
                );
            }
            Err(error) => self.operations.set_error(
                &workflow_id,
                request.run_id(),
                Some(format!("无法执行自动化操作：{error}")),
            ),
        }
    }

    pub(crate) fn apply_automation_job(&mut self, event: JobEvent) {
        let Some(pending) = self.operations.pending.get(&event.job_id) else {
            return;
        };
        let workflow_id = pending.request.workflow_id().to_owned();
        if let JobState::Running { progress } = &event.state {
            if let Ok(result) = serde_json::from_value::<lilia_contracts::AutomationOperationResult>(
                progress.clone(),
            ) {
                if self.selected_workflow_id.as_deref() == Some(workflow_id.as_str())
                    && pending.may_select(&result, self.selected_run_id.as_deref())
                {
                    self.selected_run_id = Some(result.run_id);
                    self.refresh_automation_runs();
                }
            }
            return;
        }
        if !event.state.is_terminal() {
            return;
        }
        let pending = self.operations.pending.remove(&event.job_id).unwrap();
        match event.state {
            JobState::Completed { output } => {
                match serde_json::from_value::<lilia_contracts::AutomationOperationResult>(output) {
                    Ok(result) if pending.addresses(&result) => {
                        if result.error.is_some() {
                            self.operations.set_error(
                                &workflow_id,
                                Some(&result.run_id),
                                result.error.clone(),
                            );
                        }
                        if self.selected_workflow_id.as_deref() == Some(workflow_id.as_str()) {
                            if pending.may_select(&result, self.selected_run_id.as_deref()) {
                                self.selected_run_id = Some(result.run_id);
                                if result.error.is_none() && self.response == pending.response {
                                    self.response.clear();
                                }
                            }
                            self.refresh_automation_runs();
                        }
                    }
                    _ => self.operations.set_error(
                        &workflow_id,
                        pending.request.run_id(),
                        Some("自动化操作返回了不匹配的运行，请刷新后重试。".into()),
                    ),
                }
            }
            JobState::Failed { message } => self.operations.set_error(
                &workflow_id,
                pending.request.run_id(),
                Some(format!("自动化操作失败：{message}")),
            ),
            JobState::Cancelled | JobState::Superseded => self.operations.set_error(
                &workflow_id,
                pending.request.run_id(),
                Some("自动化操作已停止，请检查当前运行状态。".into()),
            ),
            _ => {}
        }
    }

    pub(crate) fn create_automation(&mut self) {
        let input = AutomationSaveDraftInput {
            id: None,
            name: format!("自动化 {}", self.workflows.len().saturating_add(1)),
            scope: AutomationScopeFilter::default(),
            nodes: vec![AutomationNode {
                id: "trigger".to_owned(),
                kind: "trigger".to_owned(),
                title: "手动触发".to_owned(),
                position: AutomationNodePosition { x: 80.0, y: 80.0 },
                config: json!({ "triggerKind": "manual" }),
            }],
            edges: Vec::new(),
        };
        match self.service.save_draft(input) {
            Ok(workflow) => {
                self.refresh_automations();
                self.select_automation(workflow.id);
            }
            Err(error) => {
                self.error = Some(format!("无法创建自动化：{error}"));
            }
        }
    }

    pub(crate) fn add_automation_node(&mut self, kind: &str) {
        if let Err(error) = self.flush_editor_into_draft() {
            self.error = Some(error);
            return;
        }
        let Some(selected) = self.selected_workflow_id.clone() else {
            return;
        };
        let Some(workflow) = self
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == selected)
        else {
            return;
        };
        let (title, config) = match kind {
            "agent" => (
                "Agent",
                json!({
                    "backend": "native-agentkit",
                    "permission": "ask",
                    "prompt": "请根据当前上下文继续推进。"
                }),
            ),
            "tool" => ("工具", json!({ "action": "record_timeline" })),
            "logic" => (
                "逻辑",
                json!({
                    "logic": "condition",
                    "path": "trigger.kind",
                    "equals": "manual"
                }),
            ),
            "human" => ("人工确认", json!({ "prompt": "确认后继续执行自动化。" })),
            _ => return,
        };
        let mut sequence = workflow.draft.nodes.len().saturating_add(1);
        let node_id = loop {
            let candidate = format!("{kind}-{sequence}");
            if workflow.draft.nodes.iter().all(|node| node.id != candidate) {
                break candidate;
            }
            sequence = sequence.saturating_add(1);
        };
        let index = workflow.draft.nodes.len();
        workflow.draft.nodes.push(AutomationNode {
            id: node_id.clone(),
            kind: kind.to_owned(),
            title: title.to_owned(),
            position: AutomationNodePosition {
                x: 80.0 + (index % 4) as f64 * 240.0,
                y: 80.0 + (index / 4) as f64 * 180.0,
            },
            config,
        });
        if workflow.draft.edges.is_empty() && index == 1 {
            if let Some(trigger_id) = workflow
                .draft
                .nodes
                .iter()
                .find(|node| node.kind == "trigger")
                .map(|node| node.id.clone())
            {
                workflow
                    .draft
                    .edges
                    .push(crate::application::AutomationEdge {
                        id: format!("edge:{trigger_id}:{node_id}:1"),
                        source: trigger_id,
                        target: node_id.clone(),
                        source_handle: None,
                        target_handle: None,
                    });
            }
        }
        self.rebuild_automation_graph(true);
        self.persist_selected_automation_draft();
        self.update_automation_graph(GraphCanvasEvent::SelectionChanged(Some(
            GraphSelection::Node(node_id.into()),
        )));
    }

    pub(crate) fn toggle_automation_scope_inbox(&mut self) {
        let Some(selected) = self.selected_workflow_id.clone() else {
            return;
        };
        let Some(workflow) = self
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == selected)
        else {
            return;
        };
        workflow.scope.include_inbox = !workflow.scope.include_inbox;
        workflow.draft.scope = workflow.scope.clone();
        self.persist_selected_automation_draft();
    }

    pub(crate) fn delete_automation_selection(&mut self) {
        let Some(selection) = self.selection.clone() else {
            return;
        };
        let Some(selected) = self.selected_workflow_id.clone() else {
            return;
        };
        let Some(workflow) = self
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == selected)
        else {
            return;
        };
        match selection {
            GraphSelection::Node(node_id) => {
                if workflow
                    .draft
                    .nodes
                    .iter()
                    .any(|node| node.id == node_id.as_str() && node.kind == "trigger")
                {
                    self.error = Some("触发节点不能删除。".to_owned());
                    return;
                }
                workflow
                    .draft
                    .nodes
                    .retain(|node| node.id != node_id.as_str());
                workflow.draft.edges.retain(|edge| {
                    edge.source != node_id.as_str() && edge.target != node_id.as_str()
                });
            }
            GraphSelection::Edge(edge_id) => {
                workflow
                    .draft
                    .edges
                    .retain(|edge| edge.id != edge_id.as_str());
            }
            GraphSelection::Port { .. } => return,
        }
        self.selection = None;
        self.editor = AutomationNodeInspectorDraft::default();
        self.rebuild_automation_graph(false);
        self.persist_selected_automation_draft();
    }

    pub(crate) fn refresh_automation_node_inspector(&mut self) {
        let Some(GraphSelection::Node(node_id)) = self.selection.as_ref() else {
            self.editor = AutomationNodeInspectorDraft::default();
            return;
        };
        let node = self
            .selected_automation_workflow()
            .and_then(|workflow| {
                workflow
                    .draft
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id.as_str())
            })
            .cloned();
        let Some(node) = node else {
            self.editor = AutomationNodeInspectorDraft::default();
            return;
        };
        self.editor = AutomationNodeInspectorDraft {
            node_id: Some(node.id),
            title: node.title,
            config: serde_json::to_string(&node.config).unwrap_or_else(|_| "{}".to_owned()),
        };
    }

    pub(crate) fn update_automation_node_config_field(&mut self, field: String, value: Value) {
        self.error = self.editor.set_field(field, value).err();
    }

    pub(crate) fn cycle_automation_node_config(&mut self, field: &str) {
        self.error = self.editor.cycle_field(field).err();
    }

    pub(crate) fn toggle_automation_node_config_boolean(&mut self, field: &str) {
        self.error = self.editor.toggle_field(field).err();
    }

    pub(crate) fn save_automation_node_inspector(&mut self) {
        let Some(node_id) = self.editor.node_id.clone() else {
            return;
        };
        let title = self.editor.title.trim();
        if title.is_empty() {
            self.error = Some("节点名称不能为空。".to_owned());
            return;
        }
        let config = match self.editor.validated_config() {
            Ok(config) => config,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let Some(selected) = self.selected_workflow_id.clone() else {
            return;
        };
        let Some(node) = self
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == selected)
            .and_then(|workflow| {
                workflow
                    .draft
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == node_id)
            })
        else {
            self.error = Some("所选节点已不存在。".to_owned());
            return;
        };
        node.title = title.to_owned();
        node.config = config;
        self.rebuild_automation_graph(false);
        self.persist_selected_automation_draft();
        self.refresh_automation_node_inspector();
    }

    pub(crate) fn publish_automation(&mut self) {
        let Some(workflow_id) = self.selected_workflow_id.clone() else {
            return;
        };
        match self.service.publish(&workflow_id) {
            Ok(_) => self.refresh_automations(),
            Err(error) => {
                self.error = Some(format!("无法发布自动化：{error}"));
            }
        }
    }

    pub(crate) fn toggle_automation(&mut self) {
        let Some(workflow) = self.selected_automation_workflow() else {
            return;
        };
        let workflow_id = workflow.id.clone();
        let enabled = !workflow.enabled;
        match self.service.set_enabled(&workflow_id, enabled) {
            Ok(_) => self.refresh_automations(),
            Err(error) => {
                self.error = Some(format!("无法更新自动化状态：{error}"));
            }
        }
    }

    pub(crate) fn delete_automation(&mut self) {
        let Some(workflow_id) = self.selected_workflow_id.clone() else {
            return;
        };
        match self.service.delete_workflow(&workflow_id) {
            Ok(()) => {
                self.selected_workflow_id = None;
                self.refresh_automations();
            }
            Err(error) => {
                self.error = Some(format!("无法删除自动化：{error}"));
            }
        }
    }

    pub(crate) fn selected_automation_workflow(&self) -> Option<&AutomationWorkflow> {
        let selected = self.selected_workflow_id.as_deref()?;
        self.workflows
            .iter()
            .find(|workflow| workflow.id == selected)
    }

    pub(crate) fn selected_automation_node(&self) -> Option<&AutomationNode> {
        let node_id = self.editor.node_id.as_deref()?;
        self.selected_automation_workflow()?
            .draft
            .nodes
            .iter()
            .find(|node| node.id == node_id)
    }

    pub(crate) fn rebuild_automation_graph(&mut self, reset_viewport: bool) {
        let Some(workflow) = self.selected_automation_workflow() else {
            self.graph = GraphModel::empty();
            self.selection = None;
            self.editor = AutomationNodeInspectorDraft::default();
            return;
        };
        match automation_graph_model(workflow) {
            Ok(graph) => {
                self.graph = graph;
                if reset_viewport {
                    self.viewport = self
                        .graph
                        .bounds()
                        .map(|bounds| crate::module::automation::graph::initial_viewport(bounds))
                        .unwrap_or_default();
                    self.selection = None;
                    self.editor = AutomationNodeInspectorDraft::default();
                }
            }
            Err(error) => {
                self.graph = GraphModel::empty();
                self.selection = None;
                self.editor = AutomationNodeInspectorDraft::default();
                self.error = Some(format!("自动化图无法显示：{error}"));
            }
        }
    }

    pub(crate) fn update_automation_graph(&mut self, event: GraphCanvasEvent) {
        match event {
            GraphCanvasEvent::SelectionChanged(selection) => {
                let inspected_missing = self.editor.node_id.as_ref().is_some_and(|node_id| {
                    self.selected_automation_workflow().is_none_or(|workflow| {
                        !workflow.draft.nodes.iter().any(|node| node.id == *node_id)
                    })
                });
                if inspected_missing {
                    self.editor = AutomationNodeInspectorDraft::default();
                } else if let Err(error) = self.flush_editor_into_draft() {
                    self.error = Some(error);
                    return;
                }
                self.selection = selection;
                self.refresh_automation_node_inspector();
                self.capture_loaded_inspector();
            }
            GraphCanvasEvent::ViewportInput(viewport)
            | GraphCanvasEvent::ViewportChanged(viewport) => {
                self.viewport = viewport;
            }
            GraphCanvasEvent::NodePositionInput { node, position } => {
                self.move_automation_node(node.as_str(), position, false);
            }
            GraphCanvasEvent::NodePositionChanged { node, position } => {
                self.move_automation_node(node.as_str(), position, true);
            }
            GraphCanvasEvent::ConnectionRequested { source, target } => {
                self.connect_automation_nodes(source, target);
            }
        }
    }

    pub(crate) fn move_automation_node(
        &mut self,
        node_id: &str,
        position: GraphPoint,
        persist: bool,
    ) {
        let Some(selected) = self.selected_workflow_id.clone() else {
            return;
        };
        let Some(workflow) = self
            .workflows
            .iter_mut()
            .find(|workflow| workflow.id == selected)
        else {
            return;
        };
        let Some(node) = workflow
            .draft
            .nodes
            .iter_mut()
            .find(|node| node.id == node_id)
        else {
            return;
        };
        node.position = AutomationNodePosition {
            x: f64::from(position.x),
            y: f64::from(position.y),
        };
        let graph_node = self
            .graph
            .nodes()
            .iter()
            .find_map(|node| (node.id.as_str() == node_id).then(|| node.id.clone()));
        if let Some(graph_node) = graph_node {
            let _ = self.graph.set_node_position(&graph_node, position);
        }
        if persist {
            self.persist_selected_automation_draft();
        }
    }

    pub(crate) fn connect_automation_nodes(
        &mut self,
        source: GraphEndpoint,
        target: GraphEndpoint,
    ) {
        let Some(workflow) = self.selected_automation_workflow() else {
            return;
        };
        match crate::module::automation::graph::new_connection(workflow, source, target) {
            Ok(Some(edge)) => {
                if let Some(workflow) = self.workflows.iter_mut().find(|workflow| {
                    Some(workflow.id.as_str()) == self.selected_workflow_id.as_deref()
                }) {
                    workflow.draft.edges.push(edge);
                }
                self.persist_selected_automation_draft();
                self.rebuild_automation_graph(false);
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }

    fn flush_editor_into_draft(&mut self) -> Result<(), String> {
        let Some(node_id) = self.editor.node_id.clone() else {
            return Ok(());
        };
        let title = self.editor.title.trim();
        if title.is_empty() {
            return Err("节点名称不能为空。".into());
        }
        let config = self.editor.validated_config()?;
        let Some(node) = self
            .workflows
            .iter_mut()
            .find(|workflow| Some(workflow.id.as_str()) == self.selected_workflow_id.as_deref())
            .and_then(|workflow| {
                workflow
                    .draft
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == node_id)
            })
        else {
            return Err("所选节点已不存在。".into());
        };
        node.title = title.to_owned();
        node.config = config;
        Ok(())
    }

    pub(crate) fn persist_selected_automation_draft(&mut self) {
        if let Some(updated_at) = self
            .selected_automation_workflow()
            .map(|workflow| workflow.updated_at)
        {
            if self
                .loaded_updated_at
                .is_some_and(|expected| updated_at != expected)
            {
                self.error = Some("自动化已更新，请确认后重试。".to_owned());
                self.loaded_updated_at = Some(updated_at);
                return;
            }
        }
        if let Err(error) = self.flush_editor_into_draft() {
            self.error = Some(error);
            return;
        }
        let Some(workflow) = self.selected_automation_workflow().cloned() else {
            return;
        };
        let input = AutomationSaveDraftInput {
            id: Some(workflow.id.clone()),
            name: workflow.name,
            scope: workflow.scope,
            nodes: workflow.draft.nodes,
            edges: workflow.draft.edges,
        };
        match self.service.save_draft(input) {
            Ok(updated) => {
                if let Some(workflow) = self
                    .workflows
                    .iter_mut()
                    .find(|workflow| workflow.id == updated.id)
                {
                    *workflow = updated;
                }
                self.capture_loaded();
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!("无法保存自动化：{error}"));
            }
        }
    }

    pub(crate) fn snapshot(&self, inline_size: f32) -> super::view::AutomationViewSnapshot {
        crate::module::automation::view::AutomationViewSnapshot {
            rows: self
                .workflows
                .iter()
                .map(|workflow| crate::module::automation::view::AutomationRow {
                    id: workflow.id.clone(),
                    label: if workflow.name.trim().is_empty() {
                        "未命名自动化".into()
                    } else {
                        workflow.name.clone()
                    },
                    selected: self.selected_workflow_id.as_deref() == Some(workflow.id.as_str()),
                })
                .collect(),
            target: self.selected_automation_workflow().map(|workflow| {
                crate::module::automation::view::AutomationTarget {
                    window_id: self.window,
                    workflow_id: workflow.id.clone(),
                    modified_at: self.action_clock(workflow),
                }
            }),
            name: self
                .selected_automation_workflow()
                .map(|workflow| workflow.name.clone())
                .unwrap_or_default(),
            enabled: self
                .selected_automation_workflow()
                .is_some_and(|workflow| workflow.enabled),
            published: self
                .selected_automation_workflow()
                .is_some_and(|workflow| workflow.published_version_id.is_some()),
            error: self.error.clone().or_else(|| {
                self.selected_workflow_id
                    .as_deref()
                    .and_then(|id| self.operations.error(id, self.selected_run_id.as_deref()))
            }),
            operation_pending: self
                .selected_workflow_id
                .as_deref()
                .is_some_and(|id| self.operations.busy(id)),
            cancel_pending: self
                .selected_run_id
                .as_deref()
                .is_some_and(|id| self.operations.cancelling(id)),
            runs: self
                .runs
                .iter()
                .map(|run| {
                    let detail = self.run_detail.as_ref().filter(|detail| {
                        detail.run.id == run.id && detail.run.workflow_id == run.workflow_id
                    });
                    crate::module::automation::view::AutomationRunView {
                        id: run.id.clone(),
                        label: format!(
                            "{} · {}",
                            match run.status {
                                AutomationRunStatus::Pending => "等待运行",
                                AutomationRunStatus::Running => "运行中",
                                AutomationRunStatus::Succeeded => "已完成",
                                AutomationRunStatus::Failed => "失败",
                                AutomationRunStatus::Skipped => "已跳过",
                                AutomationRunStatus::Cancelled => "已取消",
                                AutomationRunStatus::WaitingUser => "等待确认",
                            },
                            format!(
                                "{} {:02}:{:02}:{:02} UTC",
                                format_civil_date(run.started_at),
                                run.started_at.div_euclid(3_600_000).rem_euclid(24),
                                run.started_at.div_euclid(60_000).rem_euclid(60),
                                run.started_at.div_euclid(1_000).rem_euclid(60)
                            )
                        ),
                        error: detail
                            .and_then(|detail| detail.run.error.clone())
                            .or_else(|| run.error.clone()),
                        prompt: detail
                            .filter(|detail| detail.run.status == AutomationRunStatus::WaitingUser)
                            .and_then(waiting_human_node)
                            .map(|node| {
                                node.output
                                    .as_ref()
                                    .and_then(|output| output.get("prompt"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("请确认是否继续运行。")
                                    .to_owned()
                            }),
                        can_cancel: matches!(
                            run.status,
                            AutomationRunStatus::Running | AutomationRunStatus::WaitingUser
                        ),
                        waiting_node: detail
                            .filter(|detail| detail.run.status == AutomationRunStatus::WaitingUser)
                            .and_then(waiting_human_node)
                            .map(|node| node.node_id.clone()),
                    }
                })
                .collect(),
            selected_run: self.selected_run_id.clone(),
            response: self.response.clone(),
            editor: self
                .selected_automation_node()
                .and_then(|node| self.editor.snapshot(&node.kind)),
            compact: inline_size < 1000.0,
            graph: self.graph.clone(),
            viewport: self.viewport,
            selection: self.selection.clone(),
            inspector_panel: self.inspector_panel.clone(),
            include_inbox: self
                .selected_automation_workflow()
                .is_some_and(|workflow| workflow.scope.include_inbox),
            event_kinds: self
                .selected_automation_workflow()
                .map(|workflow| workflow.scope.event_kinds.clone())
                .unwrap_or_default(),
            projects: Vec::new(),
        }
    }
}

impl UiModule for AutomationController {
    type Message = AutomationModuleMessage;
    type Projection<'a> = crate::ui_module::projection::AutomationProjection<'a>;

    fn feature(&self) -> lilia_kernel::FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        if cx.window() != self.window {
            return UiModuleOutcome::clean();
        }
        match message {
            AutomationModuleMessage::Refresh => self.refresh_automations(),
            AutomationModuleMessage::Select(id) => self.select_automation(id),
            AutomationModuleMessage::Create => self.create_automation(),
            AutomationModuleMessage::Rename(value) => self.rename(value),
            AutomationModuleMessage::Action { target, action } => {
                self.apply_action(target, action, cx.selected_project(), cx.selected_task())
            }
        }
        UiModuleOutcome::dirty()
    }

    fn job(&mut self, event: &JobEvent, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        if cx.window() != self.window || !self.operations.pending.contains_key(&event.job_id) {
            return UiModuleOutcome::clean();
        }
        self.apply_automation_job(event.clone());
        UiModuleOutcome::dirty()
    }

    fn invalidate(
        &mut self,
        event: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        if cx.window() != self.window {
            return UiModuleOutcome::clean();
        }
        if let Some(event) = event.downcast::<lilia_feature_automation::AutomationRunChanged>() {
            if self.selected_workflow_id.as_deref() != Some(event.automation_id.as_str()) {
                return UiModuleOutcome::clean();
            }
            self.refresh_automation_runs();
        } else if event.is::<lilia_feature_automation::AutomationChanged>()
            && (self.selected_workflow_id.is_some()
                || cx.shows_surface(crate::application::ApplicationWorkspaceSurface::Automations))
        {
            self.refresh_automations();
        } else {
            return UiModuleOutcome::clean();
        }
        UiModuleOutcome::dirty()
    }

    fn project_fields(&self, cx: &UiModuleContext<'_>, into: Self::Projection<'_>) {
        if cx.window() == self.window
            && cx.shows_surface(crate::application::ApplicationWorkspaceSurface::Automations)
        {
            *into.automation = self.snapshot(cx.inline_size());
            let selected = self.selected_automation_workflow();
            into.automation.projects = cx
                .workspace_snapshot()
                .map(|snapshot| {
                    snapshot
                        .projects
                        .into_iter()
                        .map(|project| {
                            let id = project.id.as_str().to_owned();
                            let checked = selected.is_some_and(|workflow| {
                                workflow.scope.project_ids.iter().any(|item| item == &id)
                            });
                            (id, project.name, checked)
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
    }
}

pub(crate) fn waiting_human_node(
    detail: &AutomationRunDetail,
) -> Option<&crate::application::AutomationRunNodeState> {
    detail.nodes.iter().find(|node| {
        node.status == AutomationRunStatus::WaitingUser
            && node
                .output
                .as_ref()
                .and_then(|output| output.get("waitingUser"))
                .and_then(Value::as_bool)
                == Some(true)
    })
}

fn current_timestamp_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::{editor::NodeEditorAction, view::AutomationAction};
    use super::*;
    use std::sync::Arc;

    #[test]
    fn window_targets_and_editing_state_stay_with_their_controller() {
        let service = DesktopAutomationService::in_memory(Arc::new(
            lilia_feature_automation::SilentAutomationEvents,
        ))
        .unwrap();
        let jobs = Jobs::new(lilia_kernel::EventBus::new(), lilia_kernel::Journal::new());
        let mut first = AutomationController::new(WindowId(10), service.clone(), jobs.clone());
        first.create_automation();
        first.add_automation_node("human");
        let node_id = first.editor.node_id.clone().unwrap();
        let mut second = AutomationController::new(WindowId(11), service.clone(), jobs);
        second.refresh_automations();
        second.update_automation_graph(GraphCanvasEvent::SelectionChanged(Some(
            GraphSelection::Node(node_id.clone().into()),
        )));
        let target = first.snapshot(1180.0).target.unwrap();
        second.apply_action(
            target.clone(),
            AutomationAction::Rename("Wrong window".into()),
            None,
            None,
        );
        assert_ne!(
            second.selected_automation_workflow().unwrap().name,
            "Wrong window"
        );
        let mut stale = target.clone();
        stale.modified_at -= 1;
        first.apply_action(
            stale,
            AutomationAction::Rename("Stale edit".into()),
            None,
            None,
        );
        assert_ne!(
            first.selected_automation_workflow().unwrap().name,
            "Stale edit"
        );
        first.apply_action(
            target,
            AutomationAction::Node {
                node_id: node_id.clone(),
                action: NodeEditorAction::Title("Local draft".into()),
            },
            None,
            None,
        );
        second.apply_action(
            second.snapshot(780.0).target.unwrap(),
            AutomationAction::Node {
                node_id: node_id.clone(),
                action: NodeEditorAction::Title("Other window".into()),
            },
            None,
            None,
        );
        second.save_automation_node_inspector();
        first.refresh_automations();
        assert_eq!(first.editor.title, "Local draft");
        assert_eq!(second.editor.title, "Other window");
        assert_eq!(
            first.selected_automation_node().unwrap().title,
            "Other window"
        );
        let conflicted = first.snapshot(1180.0).target.unwrap();
        first.apply_action(
            conflicted.clone(),
            AutomationAction::Node {
                node_id: node_id.clone(),
                action: NodeEditorAction::Title("Still typing".into()),
            },
            None,
            None,
        );
        assert_eq!(first.editor.title, "Still typing");
        first.apply_action(conflicted, AutomationAction::Save, None, None);
        assert_eq!(
            first.error.as_deref(),
            Some("自动化已更新，请确认后重试。")
        );
        assert_eq!(
            first.selected_automation_node().unwrap().title,
            "Other window"
        );
        let retry = first.snapshot(1180.0).target.unwrap();
        first.apply_action(retry, AutomationAction::Save, None, None);
        assert!(first.error.is_none());
        assert_eq!(
            first.selected_automation_node().unwrap().title,
            "Still typing"
        );
        let original = second.viewport;
        let changed = GraphViewport {
            zoom: 1.5,
            ..first.viewport
        };
        first.update_automation_graph(GraphCanvasEvent::ViewportChanged(changed));
        assert_eq!(first.viewport, changed);
        assert_eq!(second.viewport, original);
        assert!(second.snapshot(780.0).compact);
        assert!(!first.snapshot(1180.0).compact);
    }

    #[test]
    fn persist_and_publish_keep_invalid_inspector_drafts_off_the_workflow() {
        let service = DesktopAutomationService::in_memory(Arc::new(
            lilia_feature_automation::SilentAutomationEvents,
        ))
        .unwrap();
        let jobs = Jobs::new(lilia_kernel::EventBus::new(), lilia_kernel::Journal::new());
        let mut controller = AutomationController::new(WindowId(12), service, jobs);
        controller.create_automation();
        controller.add_automation_node("human");
        let prompt = controller
            .selected_automation_node()
            .unwrap()
            .config
            .get("prompt")
            .cloned();
        controller.editor.config = "{".into();
        controller.persist_selected_automation_draft();
        assert!(
            controller
                .error
                .as_deref()
                .is_some_and(|error| error.contains("JSON"))
        );
        assert_eq!(
            controller
                .selected_automation_node()
                .and_then(|node| node.config.get("prompt").cloned()),
            prompt
        );
        controller.editor.title.clear();
        controller.editor.config = r#"{"prompt":"应被拒绝"}"#.into();
        let target = controller.snapshot(1180.0).target.unwrap();
        controller.apply_action(target, AutomationAction::Publish, None, None);
        assert_eq!(
            controller.error.as_deref(),
            Some("节点名称不能为空。")
        );
        assert!(
            controller
                .selected_automation_workflow()
                .unwrap()
                .published_version_id
                .is_none()
        );
        assert_eq!(
            controller
                .selected_automation_node()
                .and_then(|node| node.config.get("prompt").cloned()),
            prompt
        );
    }

    #[test]
    fn refresh_keeps_unsaved_name_and_drops_a_deleted_inspector_node() {
        let service = DesktopAutomationService::in_memory(Arc::new(
            lilia_feature_automation::SilentAutomationEvents,
        ))
        .unwrap();
        let jobs = Jobs::new(lilia_kernel::EventBus::new(), lilia_kernel::Journal::new());
        let mut controller = AutomationController::new(WindowId(13), service.clone(), jobs.clone());
        controller.create_automation();
        controller.rename("Draft name".into());
        controller.toggle_automation();
        assert_eq!(
            controller.selected_automation_workflow().unwrap().name,
            "Draft name"
        );

        controller.add_automation_node("human");
        let node_id = controller.editor.node_id.clone().unwrap();
        let mut other = AutomationController::new(WindowId(14), service, jobs);
        other.refresh_automations();
        other.update_automation_graph(GraphCanvasEvent::SelectionChanged(Some(
            GraphSelection::Node(node_id.clone().into()),
        )));
        let workflow = other.selected_automation_workflow().cloned().unwrap();
        let trigger = workflow
            .draft
            .nodes
            .iter()
            .find(|node| node.kind == "trigger")
            .cloned()
            .unwrap();
        other
            .service
            .save_draft(AutomationSaveDraftInput {
                id: Some(workflow.id),
                name: workflow.name,
                scope: workflow.scope,
                nodes: vec![trigger],
                edges: Vec::new(),
            })
            .unwrap();
        controller.refresh_automations();
        assert!(controller.editor.node_id.is_none());
        controller.update_automation_graph(GraphCanvasEvent::SelectionChanged(None));
        assert!(controller.error.is_none());
        assert_eq!(
            controller.selected_automation_workflow().unwrap().name,
            "Draft name"
        );
        let _ = node_id;
    }

    #[test]
    fn refresh_does_not_paint_a_dirty_rename_onto_another_workflow() {
        let service = DesktopAutomationService::in_memory(Arc::new(
            lilia_feature_automation::SilentAutomationEvents,
        ))
        .unwrap();
        let jobs = Jobs::new(lilia_kernel::EventBus::new(), lilia_kernel::Journal::new());
        let mut controller = AutomationController::new(WindowId(15), service.clone(), jobs.clone());
        controller.create_automation();
        let first_id = controller.selected_workflow_id.clone().unwrap();
        controller.create_automation();
        let kept_name = controller
            .selected_automation_workflow()
            .unwrap()
            .name
            .clone();
        controller.select_automation(first_id.clone());
        controller.rename("Stolen".into());
        service.delete_workflow(&first_id).unwrap();
        controller.refresh_automations();
        assert_ne!(controller.selected_workflow_id.as_deref(), Some(first_id.as_str()));
        assert_eq!(
            controller.selected_automation_workflow().unwrap().name,
            kept_name
        );
        assert!(!controller
            .workflows()
            .iter()
            .any(|workflow| workflow.name == "Stolen"));
    }

    #[test]
    fn list_runs_error_keeps_selected_run_and_confirmation_draft() {
        let service = DesktopAutomationService::in_memory(Arc::new(
            lilia_feature_automation::SilentAutomationEvents,
        ))
        .unwrap();
        let jobs = Jobs::new(lilia_kernel::EventBus::new(), lilia_kernel::Journal::new());
        let mut controller = AutomationController::new(WindowId(16), service, jobs);
        controller.create_automation();
        let workflow_id = controller.selected_workflow_id.clone().unwrap();
        controller.runs = vec![AutomationRunSummary {
            id: "run-waiting".into(),
            workflow_id,
            workflow_version_id: "version-1".into(),
            status: AutomationRunStatus::WaitingUser,
            trigger_kind: "manual".into(),
            project_id: None,
            task_id: None,
            backend: None,
            event_kind: None,
            started_at: 1,
            finished_at: None,
            error: None,
        }];
        controller.selected_run_id = Some("run-waiting".into());
        controller.response = "确认草稿".into();
        controller.fail_list_runs = true;
        controller.refresh_automation_runs();
        assert_eq!(controller.selected_run_id.as_deref(), Some("run-waiting"));
        assert_eq!(controller.response, "确认草稿");
        assert!(
            controller
                .error
                .as_deref()
                .is_some_and(|error| error.contains("运行历史"))
        );
    }
}
