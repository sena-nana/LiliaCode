use super::ArchitectureMessage;
use crate::runtime_layout::reconcile_children;
use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, Entity, FrameworkError, GraphCanvas, MutationQueue,
    ScrollAxes, ScrollView, Stack, Text,
};
use nana_ui::{ButtonKind, GraphCanvasEvent, GraphModel, GraphSelection, GraphViewport};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArchitectureViewSnapshot {
    pub project_id: Option<String>,
    pub graph: GraphModel,
    pub viewport: GraphViewport,
    pub selection: Option<GraphSelection>,
    pub summary: String,
    pub records: Vec<ArchitectureRecord>,
    pub can_rollback: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArchitectureRecord {
    pub id: String,
    pub title: String,
    pub status: String,
}

type Sink = Arc<dyn Fn(ArchitectureMessage) + Send + Sync>;

pub(crate) struct ArchitectureView {
    pub(crate) root: Entity<Stack>,
    pub(crate) inspector: Entity<Stack>,
    canvas: Entity<GraphCanvas>,
    summary: Entity<Text>,
    detail: Entity<Text>,
    history: Entity<Stack>,
    records: HashMap<String, Entity<Text>>,
    rollback: Entity<Button>,
    project_id: Option<String>,
    suspended: bool,
    restore_focus: bool,
}

impl ArchitectureView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Sink,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, Stack::fill_column(12.0))?;
        let toolbar = context.create_detached_component(document, Stack::bar(8.0))?;
        let title = context.create_detached_component(document, Text::new("架构"))?;
        context.append_child(toolbar, title)?;
        let refresh = context
            .create_detached_component(document, Button::new("刷新").kind(ButtonKind::Subtle))?;
        let refresh_sink = Arc::clone(&sink);
        context.on(refresh, move |_, _: &Activate, _| {
            refresh_sink(ArchitectureMessage::Refresh)
        })?;
        let rollback = context
            .create_detached_component(document, Button::new("回滚").kind(ButtonKind::Subtle))?;
        let rollback_sink = Arc::clone(&sink);
        context.on(rollback, move |_, _: &Activate, _| {
            rollback_sink(ArchitectureMessage::Rollback)
        })?;
        context.append_child(toolbar, refresh)?;
        context.append_child(toolbar, rollback)?;
        context.append_child(root, toolbar)?;
        let summary = context.create_detached_component(document, Text::new(""))?;
        context.append_child(root, summary)?;
        let canvas = context.create_detached_component(
            document,
            GraphCanvas::new("architecture", GraphModel::empty()),
        )?;
        context.on(canvas, move |_, event: &GraphCanvasEvent, _| {
            sink(ArchitectureMessage::Graph(event.clone()))
        })?;
        context.append_child(root, canvas)?;
        let inspector = context.create_detached_component(document, Stack::fill_column(12.0))?;
        let detail = context.create_detached_component(document, Text::new(""))?;
        context.append_child(inspector, detail)?;
        let scroll =
            context.create_detached_component(document, ScrollView::new(ScrollAxes::Vertical))?;
        let history = context.create_detached_component(document, Stack::column(8.0))?;
        context.append_child(scroll, history)?;
        context.append_child(inspector, scroll)?;
        context.world_mut().register_focus_scope(root.stable_id())?;
        Ok(Self {
            root,
            inspector,
            canvas,
            summary,
            detail,
            history,
            records: HashMap::new(),
            rollback,
            project_id: None,
            suspended: false,
            restore_focus: false,
        })
    }

    pub(crate) fn belongs_to(&self, snapshot: &ArchitectureViewSnapshot) -> bool {
        self.project_id == snapshot.project_id
    }

    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        snapshot: &ArchitectureViewSnapshot,
    ) -> Result<(), FrameworkError> {
        self.project_id = snapshot.project_id.clone();
        context.update_component(self.canvas, |canvas, _| {
            canvas.model = snapshot.graph.clone();
            canvas.viewport = snapshot.viewport;
            canvas.selection = snapshot.selection.clone();
        })?;
        context.update_component(self.rollback, |button, _| {
            button.disabled = !snapshot.can_rollback
        })?;
        context.update_component(self.summary, |text, _| *text = Text::new(&snapshot.summary))?;
        let mut children = context
            .world()
            .node(self.root.stable_id())
            .unwrap()
            .children
            .clone();
        children.retain(|id| *id != self.summary.stable_id());
        if !snapshot.summary.is_empty() {
            children.insert(1, self.summary.stable_id());
        }
        reconcile_children(context, self.root.stable_id(), &children)?;
        context.update_component(self.detail, |text, _| {
            *text = Text::new(selection_description(snapshot))
        })?;
        let stale = self
            .records
            .keys()
            .filter(|id| !snapshot.records.iter().any(|record| &record.id == *id))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale {
            context.remove_view(self.records.remove(&id).unwrap())?;
        }
        let mut order = Vec::new();
        for record in &snapshot.records {
            let label = if record.status.is_empty() {
                record.title.clone()
            } else {
                format!("{} · {}", record.title, record.status)
            };
            let row = if let Some(row) = self.records.get(&record.id).copied() {
                context.update_component(row, |text, _| *text = Text::new(&label))?;
                row
            } else {
                let row = context.create_detached_component(document, Text::new(label))?;
                self.records.insert(record.id.clone(), row);
                row
            };
            order.push(row.stable_id());
        }
        reconcile_children(context, self.history.stable_id(), &order)?;
        self.restore_focus |= self.suspended;
        self.suspended = false;
        Ok(())
    }

    pub(crate) fn suspend(&mut self, context: &mut AppContext) -> Result<(), FrameworkError> {
        if !self.suspended {
            let mut changes = MutationQueue::new();
            changes.park_subtree(self.root.stable_id());
            changes.park_subtree(self.inspector.stable_id());
            context.commit_mutations(changes)?;
            self.suspended = true;
        }
        Ok(())
    }
    pub(crate) fn restore_focus(&mut self, context: &mut AppContext) -> Result<(), FrameworkError> {
        if std::mem::take(&mut self.restore_focus) {
            let mut changes = MutationQueue::new();
            changes.restore_focus_within(self.root.stable_id());
            context.commit_mutations(changes)?;
        }
        Ok(())
    }
    pub(crate) fn dispose(self, context: &mut AppContext) -> Result<(), FrameworkError> {
        context.remove_view(self.root)?;
        context.remove_view(self.inspector)?;
        if context.world().contains(self.summary.stable_id()) {
            context.remove_view(self.summary)?;
        }
        Ok(())
    }
}

fn selection_description(snapshot: &ArchitectureViewSnapshot) -> String {
    match snapshot.selection.as_ref() {
        Some(GraphSelection::Node(node_id)) => snapshot
            .graph
            .node(node_id)
            .map(|node| {
                if node.label.is_empty() {
                    node.id.to_string()
                } else {
                    node.label.clone()
                }
            })
            .unwrap_or_else(|| node_id.to_string()),
        Some(GraphSelection::Edge(edge_id)) => snapshot
            .graph
            .edge(edge_id)
            .and_then(|edge| {
                edge.label
                    .as_deref()
                    .map(str::trim)
                    .filter(|label| !label.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "关系".to_owned()),
        Some(GraphSelection::Port { node, .. }) => snapshot
            .graph
            .node(node)
            .map(|graph_node| graph_node.label.clone())
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| node.to_string()),
        None => "选择图中的节点。".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::{GraphNode, GraphPoint, GraphSize};
    use std::sync::Mutex;

    fn snapshot() -> ArchitectureViewSnapshot {
        ArchitectureViewSnapshot {
            project_id: Some("project-one".into()),
            graph: GraphModel::new(
                vec![GraphNode::new(
                    "service",
                    "Service",
                    GraphPoint::new(20.0, 30.0),
                    GraphSize::new(160.0, 80.0),
                )],
                vec![],
            )
            .unwrap(),
            viewport: GraphViewport::new(GraphPoint::new(12.0, 24.0), 1.2),
            selection: Some(GraphSelection::Node("service".into())),
            records: vec![ArchitectureRecord {
                id: "change".into(),
                title: "Update".into(),
                status: "Applied".into(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn architecture_view_forwards_graph_events_and_updates_selected_detail_and_history() {
        let mut context = AppContext::new();
        let document = DocumentId::new(621).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink_events = Arc::clone(&events);
        let mut view = ArchitectureView::mount(
            &mut context,
            document,
            Arc::new(move |event| sink_events.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        context.append_child(host, view.inspector).unwrap();
        let mut snapshot = snapshot();
        view.sync(&mut context, document, &snapshot).unwrap();
        assert_eq!(
            context
                .read(view.detail, |text| text.value.clone())
                .unwrap(),
            "Service"
        );
        assert!(
            context
                .read(view.rollback, |button| button.disabled)
                .unwrap()
        );
        let row = view.records["change"];
        let event =
            GraphCanvasEvent::ViewportChanged(GraphViewport::new(GraphPoint::new(45.0, 60.0), 1.5));
        context
            .update_component(view.canvas, |_, cx| cx.emit(event.clone()))
            .unwrap();
        assert!(
            matches!(&events.lock().unwrap()[..], [ArchitectureMessage::Graph(actual)] if actual == &event)
        );
        snapshot.records[0].status = "Rolled back".into();
        snapshot.can_rollback = true;
        snapshot.selection = None;
        view.sync(&mut context, document, &snapshot).unwrap();
        assert_eq!(view.records["change"], row);
        assert_eq!(
            context.read(row, |text| text.value.clone()).unwrap(),
            "Update · Rolled back"
        );
        assert_eq!(
            context
                .read(view.detail, |text| text.value.clone())
                .unwrap(),
            selection_description(&snapshot)
        );
        context
            .update_component(view.rollback, |_, cx| cx.emit(Activate))
            .unwrap();
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(ArchitectureMessage::Rollback)
        ));
        snapshot.records.clear();
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(!context.world().contains(row.stable_id()));
    }

    #[test]
    fn architecture_navigation_preserves_viewport_and_focus_and_cleans_both_regions() {
        let document_id = DocumentId::new(622).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let host = context
            .create_component(document_id, Stack::fill_column(0.0))
            .unwrap();
        let mut view = ArchitectureView::mount(context, document_id, Arc::new(|_| {})).unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = snapshot();
        view.sync(context, document_id, &snapshot).unwrap();
        context
            .focus_node(document_id, view.canvas.stable_id())
            .unwrap();
        view.suspend(context).unwrap();
        view.sync(context, document_id, &snapshot).unwrap();
        context.append_child(host, view.root).unwrap();
        view.restore_focus(context).unwrap();
        assert_eq!(
            context.world().focused(document_id),
            Some(view.canvas.stable_id())
        );
        assert_eq!(
            context
                .read(view.canvas, |canvas| (
                    canvas.viewport,
                    canvas.selection.clone()
                ))
                .unwrap(),
            (snapshot.viewport, snapshot.selection.clone())
        );
        document
            .flush(
                nana_ui::runtime::LayoutViewport::new(800.0, 700.0),
                &mut nana_ui::NanaTextShaper::default(),
            )
            .unwrap();
        let context = document.context_mut();
        let root = context.world().layout_box(view.root.stable_id()).unwrap();
        let canvas = context.world().layout_box(view.canvas.stable_id()).unwrap();
        assert!(
            canvas.height >= root.height * 0.8 && canvas.width >= root.width * 0.95,
            "root={root:?}, canvas={canvas:?}"
        );
        assert!(view.belongs_to(&snapshot));
        snapshot.project_id = Some("project-two".into());
        assert!(!view.belongs_to(&snapshot));
        let nodes = [
            view.root.stable_id(),
            view.inspector.stable_id(),
            view.canvas.stable_id(),
            view.summary.stable_id(),
            view.history.stable_id(),
            view.records["change"].stable_id(),
        ];
        view.suspend(context).unwrap();
        view.dispose(context).unwrap();
        for node in nodes {
            assert!(!context.world().contains(node));
        }
    }
}
