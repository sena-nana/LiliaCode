use super::ArchitectureMessage;
use crate::runtime_layout::{view_bar, view_column, view_fill_column, Bound};
use nana_ui::runtime::view::{entity_ref, signal, text, widget, with_refs, EachExt};
#[cfg(test)]
use nana_ui::runtime::Activate;
use nana_ui::runtime::GraphCanvasEvent;
use nana_ui::runtime::{
    AppContext, Button, DocumentId, Entity, FrameworkError, GraphCanvas, MutationQueue, ScrollAxes,
    ScrollView, Stack, Text,
};
use nana_ui::{ButtonKind, GraphModel, GraphPoint, GraphSelection, GraphViewport};
use std::sync::Arc;

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

#[derive(Clone)]
struct CanvasState {
    model: GraphModel,
    viewport: GraphViewport,
    selection: Option<GraphSelection>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            model: GraphModel::empty(),
            viewport: GraphViewport::new(GraphPoint::new(0.0, 0.0), 1.0),
            selection: None,
        }
    }
}

pub(crate) struct ArchitectureView {
    pub(crate) root: Entity<Stack>,
    pub(crate) inspector: Entity<Stack>,
    canvas: Entity<GraphCanvas>,
    summary: Entity<Text>,
    detail: Entity<Text>,
    history: Entity<Stack>,
    rollback: Entity<Button>,
    summary_text: Bound<String>,
    show_summary: Bound<bool>,
    detail_text: Bound<String>,
    rollback_disabled: Bound<bool>,
    canvas_state: Bound<CanvasState>,
    records: Bound<Vec<ArchitectureRecord>>,
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
        let summary_text = Bound::new();
        let show_summary = Bound::new();
        let detail_text = Bound::new();
        let rollback_disabled = Bound::new();
        let canvas_state = Bound::new();
        let records: Bound<Vec<ArchitectureRecord>> = Bound::new();
        let summary_slot = summary_text.clone();
        let show_slot = show_summary.clone();
        let detail_slot = detail_text.clone();
        let rollback_slot = rollback_disabled.clone();
        let canvas_slot = canvas_state.clone();
        let records_slot = records.clone();
        let (_, (root, inspector, canvas, summary, detail, history, rollback)) = context
            .mount_view_detached(document, move || {
                let summary_text = summary_slot.install(signal(String::new()));
                let show_summary = show_slot.install(signal(false));
                let detail_text = detail_slot.install(signal(String::new()));
                let rollback_disabled = rollback_slot.install(signal(true));
                let canvas_state = canvas_slot.install(signal(CanvasState::default()));
                let records = records_slot.install(signal(Vec::new()));
                let root = entity_ref::<Stack>();
                let inspector = entity_ref::<Stack>();
                let canvas = entity_ref::<GraphCanvas>();
                let summary = entity_ref::<Text>();
                let detail = entity_ref::<Text>();
                let history = entity_ref::<Stack>();
                let rollback = entity_ref::<Button>();
                let refresh_sink = Arc::clone(&sink);
                let rollback_sink = Arc::clone(&sink);
                let graph_sink = sink;
                let page = view_fill_column(12.0).entity_ref(root).children((
                    view_bar(8.0).children((
                        text("架构"),
                        widget(Button::new("刷新").kind(ButtonKind::Subtle))
                            .on_activate(move || refresh_sink(ArchitectureMessage::Refresh)),
                        widget(Button::new("回滚").kind(ButtonKind::Subtle))
                            .entity_ref(rollback)
                            .disabled(rollback_disabled)
                            .on_activate(move || rollback_sink(ArchitectureMessage::Rollback)),
                    )),
                    text(summary_text).entity_ref(summary).visible(show_summary),
                    widget(GraphCanvas::new("architecture", GraphModel::empty()))
                        .entity_ref(canvas)
                        .bind(move |canvas| {
                            canvas_state.with(|state| {
                                canvas.model = state.model.clone();
                                canvas.viewport = state.viewport;
                                canvas.selection = state.selection.clone();
                            });
                        })
                        .on(move |event: &GraphCanvasEvent| {
                            graph_sink(ArchitectureMessage::Graph(event.clone()))
                        }),
                ));
                let inspector_view = view_fill_column(12.0).entity_ref(inspector).children((
                    text(detail_text).entity_ref(detail),
                    widget(ScrollView::new(ScrollAxes::Vertical)).children(
                        view_column(0.0).entity_ref(history).children(records.each(
                            |record| record.id.clone(),
                            move |record| {
                                let id = record.id.clone();
                                let records = records;
                                text(move || {
                                    records.with(|records| {
                                        records
                                            .iter()
                                            .find(|record| record.id == id)
                                            .map(record_label)
                                            .unwrap_or_default()
                                    })
                                })
                            },
                        )),
                    ),
                ));
                with_refs(
                    (page, inspector_view),
                    (root, inspector, canvas, summary, detail, history, rollback),
                )
            })?;
        context
            .compat_world_mut()
            .register_focus_scope(root.stable_id())?;
        Ok(Self {
            root,
            inspector,
            canvas,
            summary,
            detail,
            history,
            rollback,
            summary_text,
            show_summary,
            detail_text,
            rollback_disabled,
            canvas_state,
            records,
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
        _document: DocumentId,
        snapshot: &ArchitectureViewSnapshot,
    ) -> Result<(), FrameworkError> {
        self.project_id = snapshot.project_id.clone();
        self.canvas_state.set(CanvasState {
            model: snapshot.graph.clone(),
            viewport: snapshot.viewport,
            selection: snapshot.selection.clone(),
        });
        self.rollback_disabled.set(!snapshot.can_rollback);
        self.summary_text.set(snapshot.summary.clone());
        self.show_summary.set(!snapshot.summary.is_empty());
        self.detail_text.set(selection_description(snapshot));
        self.records.set(snapshot.records.clone());
        self.restore_focus |= self.suspended;
        self.suspended = false;
        context.flush_reactive()
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

fn record_label(record: &ArchitectureRecord) -> String {
    if record.status.is_empty() {
        record.title.clone()
    } else {
        format!("{} · {}", record.title, record.status)
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

    fn history_row(context: &AppContext, history: Entity<Stack>) -> Entity<Text> {
        let list = context.world().node(history.stable_id()).unwrap().children[0];
        Entity::from_stable_id(context.world().node(list).unwrap().children[0])
    }

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
        assert!(context
            .read(view.rollback, |button| button.disabled)
            .unwrap());
        let row = history_row(&context, view.history);
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
        assert_eq!(history_row(&context, view.history), row);
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
            history_row(context, view.history).stable_id(),
        ];
        view.suspend(context).unwrap();
        view.dispose(context).unwrap();
        for node in nodes {
            assert!(!context.world().contains(node));
        }
    }
}
