//! The architecture page as a UI module.
//!
//! Owns the graph it renders and reaches the authoritative records through the
//! architecture service, so nothing here can drift from the rows on disk. One
//! instance per window: two windows can hold different viewports and selections
//! over the same project.

use lilia_feature_architecture::{
    ArchitectureBackend, ProjectArchitectureChangeRecord, ProjectArchitectureGraph,
};
use lilia_kernel::FeatureId;
use nana_ui::{GraphCanvasEvent, GraphModel, GraphSelection, GraphViewport};

use crate::application::ProjectWorkspaceSurface;
use crate::runtime_shell::{PrimaryShellSnapshot, ShellArchitectureRecord, ShellProjectPage};
use crate::ui_module::{ShellEffect, UiModule, UiModuleContext, UiModuleOutcome};

/// The architecture domain's own message vocabulary.
#[derive(Debug, Clone)]
pub enum ArchitectureMessage {
    Open,
    Refresh,
    Rollback,
    SelectHistory(String),
    Graph(GraphCanvasEvent),
}

pub struct ArchitectureModule {
    graph: ProjectArchitectureGraph,
    history: Vec<ProjectArchitectureChangeRecord>,
    quarantine_count: usize,
    model: GraphModel,
    viewport: GraphViewport,
    selection: Option<GraphSelection>,
    selected_history: Option<String>,
    error: Option<String>,
}

impl Default for ArchitectureModule {
    fn default() -> Self {
        Self {
            graph: ProjectArchitectureGraph::empty(""),
            history: Vec::new(),
            quarantine_count: 0,
            model: GraphModel::empty(),
            viewport: GraphViewport::default(),
            selection: None,
            selected_history: None,
            error: None,
        }
    }
}

impl ArchitectureModule {
    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.architecture").expect("the architecture feature id is not blank")
    }

    pub fn graph(&self) -> &ProjectArchitectureGraph {
        &self.graph
    }

    pub fn history(&self) -> &[ProjectArchitectureChangeRecord] {
        &self.history
    }

    pub fn quarantine_count(&self) -> usize {
        self.quarantine_count
    }

    pub fn model(&self) -> &GraphModel {
        &self.model
    }

    pub fn viewport(&self) -> GraphViewport {
        self.viewport
    }

    pub fn selection(&self) -> Option<&GraphSelection> {
        self.selection.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Restores a persisted layout without touching the records, which are read
    /// from the service rather than saved with the window.
    pub fn restore_layout(&mut self, model: GraphModel, viewport: GraphViewport) {
        self.model = model;
        self.viewport = viewport;
    }

    /// Reloads graph, history and quarantine count for the window's project.
    fn refresh(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(project_id) = cx.selected_project() else {
            return UiModuleOutcome::clean();
        };
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match (
            application.project_architecture(&project_id),
            application.project_architecture_changes(&project_id, 40),
            application.project_architecture_quarantine(&project_id),
        ) {
            (Ok(graph), Ok(history), Ok(quarantine)) => {
                let reset_viewport =
                    self.graph.project_id != graph.project_id || self.graph.nodes.is_empty();
                if self.graph.project_id != graph.project_id {
                    self.selected_history = None;
                }
                self.graph = graph;
                self.history = history;
                if self.selected_history.as_ref().is_some_and(|id| {
                    !self
                        .history
                        .iter()
                        .any(|record| &crate::architecture_panel::record_key(record) == id)
                }) {
                    self.selected_history = None;
                }
                self.quarantine_count = quarantine.len();
                self.error = None;
                self.rebuild(reset_viewport);
                UiModuleOutcome::dirty()
            }
            (graph, history, quarantine) => {
                let detail = graph
                    .err()
                    .map(|error| error.to_string())
                    .or_else(|| history.err().map(|error| error.to_string()))
                    .or_else(|| quarantine.err().map(|error| error.to_string()))
                    .unwrap_or_else(|| "未知错误".to_owned());
                self.error = Some(format!("无法读取架构快照：{detail}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    /// Rebuilds the rendered graph, keeping hand-placed node positions unless the
    /// project changed underneath them.
    fn rebuild(&mut self, reset_viewport: bool) {
        let previous_positions = self
            .model
            .nodes()
            .iter()
            .map(|node| (node.id.clone(), node.position))
            .collect::<Vec<_>>();
        let had_previous_nodes = !previous_positions.is_empty();
        match crate::desktop::architecture_graph_model(&self.graph) {
            Ok(mut model) => {
                if !reset_viewport {
                    for (node_id, position) in previous_positions {
                        if model.node(&node_id).is_some() {
                            let _ = model.set_node_position(&node_id, position);
                        }
                    }
                }
                self.model = model;
                if !selection_exists(&self.model, self.selection.as_ref()) {
                    self.selection = None;
                }
                if reset_viewport || !had_previous_nodes {
                    self.viewport = crate::desktop::architecture_default_viewport(&self.model);
                    self.selection = None;
                }
            }
            Err(error) => {
                self.model = GraphModel::empty();
                self.selection = None;
                self.error = Some(format!("架构图无法显示：{error}"));
            }
        }
    }

    fn rollback(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(project_id) = cx.selected_project() else {
            return UiModuleOutcome::clean();
        };
        // The rollback is recorded against a task, so a project with no tasks has
        // nothing to attribute it to.
        let Some(task_id) = cx.selected_task().or_else(|| cx.first_task()) else {
            self.error = Some("当前项目没有可记录回滚来源的任务。".to_owned());
            return UiModuleOutcome::dirty();
        };
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match application.rollback_project_architecture(
            &project_id,
            &task_id,
            ArchitectureBackend::NativeAgentkit,
        ) {
            Ok(result) => {
                let rolled_back = result.event.is_some();
                let mut outcome = self.refresh(cx);
                if !rolled_back {
                    self.error = Some("当前没有可回滚的已应用版本。".to_owned());
                    outcome.dirty = true;
                }
                outcome
            }
            Err(error) => {
                self.error = Some(format!("无法回滚架构：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn apply_graph_event(&mut self, event: GraphCanvasEvent) -> UiModuleOutcome {
        match event {
            GraphCanvasEvent::SelectionChanged(selection) => {
                self.selection =
                    selection.filter(|selection| selection_exists(&self.model, Some(selection)));
                self.selected_history = None;
            }
            GraphCanvasEvent::ViewportInput(viewport)
            | GraphCanvasEvent::ViewportChanged(viewport) => {
                self.viewport = viewport;
            }
            GraphCanvasEvent::NodePositionInput { node, position }
            | GraphCanvasEvent::NodePositionChanged { node, position } => {
                let _ = self.model.set_node_position(&node, position);
            }
            GraphCanvasEvent::ConnectionRequested { .. } => return UiModuleOutcome::clean(),
        }
        UiModuleOutcome::dirty()
    }
}

impl UiModule for ArchitectureModule {
    type Message = ArchitectureMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            ArchitectureMessage::Open => UiModuleOutcome::effect(
                ShellEffect::RevealProjectSurface(ProjectWorkspaceSurface::Architecture),
            ),
            ArchitectureMessage::Refresh => self.refresh(cx),
            ArchitectureMessage::Rollback => self.rollback(cx),
            ArchitectureMessage::SelectHistory(id) => {
                if self
                    .history
                    .iter()
                    .any(|record| crate::architecture_panel::record_key(record) == id)
                {
                    self.selected_history = Some(id);
                    self.selection = None;
                    UiModuleOutcome::dirty()
                } else {
                    UiModuleOutcome::clean()
                }
            }
            ArchitectureMessage::Graph(event) => self.apply_graph_event(event),
        }
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(event) = envelope.downcast::<crate::application::ArchitectureChanged>() else {
            return UiModuleOutcome::clean();
        };
        if cx.selected_project().as_ref() != Some(&event.project_id) {
            return UiModuleOutcome::clean();
        }
        self.refresh(cx)
    }

    fn project(&self, cx: &UiModuleContext<'_>, into: &mut PrimaryShellSnapshot) {
        // The viewport travels even when the page is closed: reopening should
        // land where the user left it, and it is one `Copy` value rather than a
        // cloned graph.
        into.architecture_viewport = self.viewport;
        if !cx.shows(ShellProjectPage::Architecture) {
            return;
        }
        into.project_page_body = self
            .error
            .clone()
            .unwrap_or_else(|| self.graph.summary.clone());
        into.architecture_records = self
            .history
            .iter()
            .map(|record| ShellArchitectureRecord {
                id: record
                    .event
                    .id
                    .clone()
                    .unwrap_or_else(|| record.event.created_at.unwrap_or_default().to_string()),
                title: record
                    .event
                    .changes
                    .iter()
                    .map(crate::desktop::architecture_change_label)
                    .collect::<Vec<_>>()
                    .join(" · "),
                status: crate::desktop::architecture_status_label(record.event.status).to_owned(),
            })
            .collect();
        into.architecture_details = crate::architecture_panel::ArchitecturePanelSnapshot {
            graph: Some(self.graph.clone()),
            records: self.history.clone(),
            selection: self.selection.clone(),
            selected_history: self.selected_history.clone(),
            error: self.error.clone(),
        };
        into.architecture_can_rollback = cx.selected_task().or_else(|| cx.first_task()).is_some()
            && self.history.iter().any(|record| {
                record.event.status == lilia_feature_architecture::ArchitectureChangeStatus::Applied
                    && record.after_graph.as_ref() == Some(&self.graph)
            });
        into.architecture_graph = self.model.clone();
        into.architecture_selection = self.selection.clone();
    }
}

fn selection_exists(model: &GraphModel, selection: Option<&GraphSelection>) -> bool {
    match selection {
        Some(GraphSelection::Node(id)) | Some(GraphSelection::Port { node: id, .. }) => {
            model.node(id).is_some()
        }
        Some(GraphSelection::Edge(id)) => model.edge(id).is_some(),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ProjectArchitectureNode;

    #[test]
    fn graph_refresh_removes_deleted_selection_without_resetting_surviving_nodes() {
        let mut module = ArchitectureModule::default();
        module.graph = ProjectArchitectureGraph::empty("project");
        module.graph.nodes.push(ProjectArchitectureNode {
            id: "service".into(),
            label: "服务".into(),
            node_type: "module".into(),
            summary: String::new(),
            paths: Vec::new(),
            tags: Vec::new(),
        });
        module.rebuild(true);
        let id = "service".into();
        module.apply_graph_event(GraphCanvasEvent::SelectionChanged(Some(
            GraphSelection::Node(id),
        )));
        assert!(module.selection.is_some());
        let viewport = module.viewport;
        module.graph.version += 1;
        module.graph.nodes[0].label = "已重命名".into();
        module.rebuild(false);
        assert!(module.selection.is_some());
        assert_eq!(module.viewport, viewport);
        module.graph.nodes.clear();
        module.rebuild(false);
        assert!(module.selection.is_none());
        module.apply_graph_event(GraphCanvasEvent::SelectionChanged(Some(
            GraphSelection::Node("missing".into()),
        )));
        assert!(module.selection.is_none());
    }
}
