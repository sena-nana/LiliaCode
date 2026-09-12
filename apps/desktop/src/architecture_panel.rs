use std::sync::Arc;

use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, Entity, FrameworkError, LengthSpec, ScrollAxes,
    ScrollView, Stack, Text,
};
use nana_ui::{GraphCanvasEvent, GraphSelection};

use crate::application::{ProjectArchitectureChangeRecord, ProjectArchitectureGraph};
use crate::runtime_shell::ShellIntent;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArchitecturePanelSnapshot {
    pub graph: Option<ProjectArchitectureGraph>,
    pub records: Vec<ProjectArchitectureChangeRecord>,
    pub selection: Option<GraphSelection>,
    pub selected_history: Option<String>,
    pub error: Option<String>,
}

pub fn record_key(record: &ProjectArchitectureChangeRecord) -> String {
    record.event.id.clone().unwrap_or_else(|| {
        format!(
            "{}-{}",
            record.event.before_version,
            record.event.created_at.unwrap_or_default()
        )
    })
}

pub struct ArchitecturePanel {
    pub root: Entity<ScrollView>,
    content: Entity<Stack>,
    children: Vec<nana_ui::runtime::StableNodeId>,
    texts: Vec<Entity<Text>>,
    pub controls: Vec<(String, Entity<Button>)>,
    rendered: Option<ArchitecturePanelSnapshot>,
    sink: Arc<dyn Fn(ShellIntent) + Send + Sync>,
}

impl ArchitecturePanel {
    pub fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Arc<dyn Fn(ShellIntent) + Send + Sync>,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(
            document,
            ScrollView::new(ScrollAxes::Vertical).style(Stack::fill_column(0.0).node_style()),
        )?;
        let content = context.create_detached_component(document, Stack::column(8.0))?;
        context.append_child(root, content)?;
        Ok(Self {
            root,
            content,
            children: Vec::new(),
            texts: Vec::new(),
            controls: Vec::new(),
            rendered: None,
            sink,
        })
    }

    fn text(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        value: impl Into<String>,
    ) -> Result<(), FrameworkError> {
        let text = context.create_detached_component(document, Text::new(value))?;
        self.children.push(text.stable_id());
        self.texts.push(text);
        Ok(())
    }

    fn select(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        key: String,
        label: String,
        active: bool,
        intent: ShellIntent,
    ) -> Result<(), FrameworkError> {
        let button = context.create_detached_component(
            document,
            Button::new(if active {
                format!("✓ {label}")
            } else {
                label
            })
            .layout(
                Stack::bar(0.0)
                    .height(LengthSpec::Px(30.0))
                    .node_style()
                    .layout,
            ),
        )?;
        let sink = self.sink.clone();
        context.on(button, move |_, _: &Activate, _| sink(intent.clone()))?;
        self.children.push(button.stable_id());
        self.controls.push((key, button));
        Ok(())
    }

    pub fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        snapshot: &ArchitecturePanelSnapshot,
    ) -> Result<(), FrameworkError> {
        if self.rendered.as_ref() == Some(snapshot) {
            return Ok(());
        }
        for text in self.texts.drain(..) {
            context.remove_view(text)?;
        }
        for (_, button) in self.controls.drain(..) {
            context.remove_view(button)?;
        }
        self.children.clear();
        if let Some(error) = &snapshot.error {
            self.text(context, document, error)?;
        }
        if let Some(graph) = &snapshot.graph {
            self.text(
                context,
                document,
                format!(
                    "版本 {} · {} 个节点 · {} 个关系",
                    graph.version,
                    graph.nodes.len(),
                    graph.edges.len()
                ),
            )?;
            if !graph.summary.is_empty() {
                self.text(context, document, &graph.summary)?;
            }
            if graph.nodes.is_empty() && graph.edges.is_empty() {
                self.text(
                    context,
                    document,
                    "暂无架构图，后续对话涉及架构时会逐步补全",
                )?;
            }
            if !graph.nodes.is_empty() {
                self.text(context, document, "节点")?;
            }
            for node in &graph.nodes {
                let selected = Some(GraphSelection::Node(node.id.clone().into()));
                self.select(
                    context,
                    document,
                    format!("node-{}", node.id),
                    format!(
                        "{} · {}",
                        if node.label.is_empty() {
                            &node.id
                        } else {
                            &node.label
                        },
                        node.node_type
                    ),
                    snapshot.selection == selected,
                    ShellIntent::ArchitectureGraph(GraphCanvasEvent::SelectionChanged(selected)),
                )?;
                if !node.summary.is_empty() {
                    self.text(context, document, &node.summary)?;
                }
                if !node.paths.is_empty() {
                    self.text(context, document, node.paths.join(" · "))?;
                }
                if !node.tags.is_empty() {
                    self.text(context, document, node.tags.join(" · "))?;
                }
            }
            if !graph.edges.is_empty() {
                self.text(context, document, "关系")?;
            }
            for edge in &graph.edges {
                let selected = Some(GraphSelection::Edge(edge.id.clone().into()));
                self.select(
                    context,
                    document,
                    format!("edge-{}", edge.id),
                    format!(
                        "{} · {}",
                        if edge.label.is_empty() {
                            &edge.id
                        } else {
                            &edge.label
                        },
                        edge.edge_type
                    ),
                    snapshot.selection == selected,
                    ShellIntent::ArchitectureGraph(GraphCanvasEvent::SelectionChanged(selected)),
                )?;
                self.text(context, document, format!("{} → {}", edge.from, edge.to))?;
                if !edge.summary.is_empty() {
                    self.text(context, document, &edge.summary)?;
                }
            }
        }
        if !snapshot.records.is_empty() {
            self.text(context, document, "最近变更")?;
        }
        for record in &snapshot.records {
            let event = &record.event;
            let key = record_key(record);
            let selected = snapshot.selected_history.as_deref() == Some(key.as_str());
            let status = crate::desktop::architecture_status_label(event.status);
            let version = event
                .after_version
                .map(|version| format!("v{version}"))
                .unwrap_or_else(|| "无新版本".into());
            self.select(
                context,
                document,
                format!("history-{key}"),
                format!("{status} · v{} → {version}", event.before_version),
                selected,
                ShellIntent::ArchitectureHistory(key),
            )?;
            if !event.reason.is_empty() {
                self.text(context, document, &event.reason)?;
            }
            let time = event
                .created_at
                .map(|time| {
                    let minutes = time.div_euclid(60_000).rem_euclid(24 * 60);
                    format!(
                        "{} {:02}:{:02} UTC",
                        crate::desktop::format_civil_date(time),
                        minutes / 60,
                        minutes % 60
                    )
                })
                .unwrap_or_else(|| "时间未知".into());
            self.text(context, document, time)?;
            if selected {
                for change in &event.changes {
                    self.text(
                        context,
                        document,
                        crate::desktop::architecture_change_label(change),
                    )?;
                }
                if let (Some(before), Some(after)) = (&record.before_graph, &record.after_graph) {
                    self.text(
                        context,
                        document,
                        format!(
                            "节点 {} → {} · 关系 {} → {}",
                            before.nodes.len(),
                            after.nodes.len(),
                            before.edges.len(),
                            after.edges.len()
                        ),
                    )?;
                }
            }
        }
        context.reconcile_children(self.content.stable_id(), &self.children)?;
        self.rendered = Some(snapshot.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        ArchitectureBackend, ArchitectureChangeStatus, ArchitecturePermission,
        ProjectArchitectureChange, ProjectArchitectureChangeEvent, ProjectArchitectureNode,
    };
    use std::sync::Mutex;

    #[test]
    fn node_and_history_controls_dispatch_normal_selection_and_reconcile_removed_nodes() {
        let id = DocumentId::new(920).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(id);
        let context = document.context_mut();
        let root = context
            .create_component(id, Stack::fill_column(0.0))
            .unwrap();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        let mut panel = ArchitecturePanel::mount(
            context,
            id,
            Arc::new(move |intent| sink.lock().unwrap().push(intent)),
        )
        .unwrap();
        context.append_child(root, panel.root).unwrap();
        let before = ProjectArchitectureGraph::empty("project");
        let mut graph = before.clone();
        graph.version = 1;
        graph.nodes.push(ProjectArchitectureNode {
            id: "service".into(),
            label: "服务".into(),
            node_type: "module".into(),
            summary: "提供项目服务".into(),
            paths: vec!["src/service.rs".into()],
            tags: Vec::new(),
        });
        let record = ProjectArchitectureChangeRecord {
            event: ProjectArchitectureChangeEvent {
                id: Some("change".into()),
                project_id: "project".into(),
                task_id: "task".into(),
                turn_id: None,
                backend: ArchitectureBackend::NativeAgentkit,
                permission: ArchitecturePermission::Full,
                status: ArchitectureChangeStatus::Applied,
                reason: "提取服务".into(),
                changes: vec![ProjectArchitectureChange::UpsertNode {
                    node: graph.nodes[0].clone(),
                }],
                before_version: 0,
                after_version: Some(1),
                created_at: Some(1),
                resolved_at: Some(1),
            },
            before_graph: Some(before),
            after_graph: Some(graph.clone()),
        };
        let mut snapshot = ArchitecturePanelSnapshot {
            graph: Some(graph),
            records: vec![record],
            ..Default::default()
        };
        panel.sync(context, id, &snapshot).unwrap();
        let node = panel
            .controls
            .iter()
            .find(|(key, _)| key == "node-service")
            .unwrap()
            .1
            .stable_id();
        let history = panel
            .controls
            .iter()
            .find(|(key, _)| key == "history-change")
            .unwrap()
            .1
            .stable_id();
        assert!(context.activate_node(node).unwrap());
        assert!(context.activate_node(history).unwrap());
        let events = messages.lock().unwrap();
        assert!(
            matches!(&events[0], ShellIntent::ArchitectureGraph(GraphCanvasEvent::SelectionChanged(Some(GraphSelection::Node(id)))) if id.as_str() == "service")
        );
        assert!(matches!(&events[1], ShellIntent::ArchitectureHistory(id) if id == "change"));
        drop(events);
        snapshot.selected_history = Some("change".into());
        panel.sync(context, id, &snapshot).unwrap();
        assert_eq!(
            panel.rendered.as_ref().unwrap().selected_history,
            snapshot.selected_history
        );
        snapshot.graph.as_mut().unwrap().nodes.clear();
        panel.sync(context, id, &snapshot).unwrap();
        assert!(!context.world().contains(node));
        assert!(!panel.controls.iter().any(|(key, _)| key == "node-service"));
        assert!(panel
            .controls
            .iter()
            .any(|(key, _)| key == "history-change"));
    }
}
