use super::RoadmapMessage;
use crate::runtime_layout::reconcile_children;
use nana_ui::ButtonKind;
use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, Entity, FormField, FrameworkError, MutationQueue,
    ScrollAxes, ScrollView, StableNodeId, Stack, Switch, Text, TextArea, TextChanged,
    ToggleChanged,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoadmapViewSnapshot {
    pub project_id: Option<String>,
    pub selected: Option<String>,
    pub title: String,
    pub description: String,
    pub due_date: String,
    pub status_label: String,
    pub error: Option<String>,
    pub cards: Vec<RoadmapCard>,
    pub tasks: Vec<RoadmapTask>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoadmapCard {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoadmapTask {
    pub id: String,
    pub title: String,
    pub linked: bool,
}

type Sink = Arc<dyn Fn(RoadmapMessage) + Send + Sync>;

struct RoadmapRow {
    root: Entity<Button>,
}

pub(crate) struct RoadmapView {
    pub(crate) root: Entity<Stack>,
    error: Entity<Text>,
    fields: [Entity<TextArea>; 3],
    create: Entity<Button>,
    status: Entity<Button>,
    up: Entity<Button>,
    down: Entity<Button>,
    save: Entity<Button>,
    delete: Entity<Button>,
    list: Entity<Stack>,
    rows: HashMap<String, RoadmapRow>,
    task_toggles: HashMap<String, Entity<Switch>>,
    sink: Sink,
    selected: Option<String>,
    project_id: Option<String>,
    suspended: bool,
    restore_focus: bool,
}

impl RoadmapView {
    pub(crate) fn debug_nodes(&self) -> Vec<(String, StableNodeId)> {
        let mut nodes = vec![
            (
                "composer.project-milestone-create".into(),
                self.create.stable_id(),
            ),
            (
                "composer.project-milestone-save".into(),
                self.save.stable_id(),
            ),
            (
                "field.project-milestone-title".into(),
                self.fields[0].stable_id(),
            ),
            (
                "field.project-milestone-description".into(),
                self.fields[1].stable_id(),
            ),
        ];
        for (id, toggle) in &self.task_toggles {
            nodes.push((format!("switch.milestone-task-{id}"), toggle.stable_id()));
        }
        nodes
    }

    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Sink,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, Stack::fill_column(12.0))?;
        let header = context.create_detached_component(document, Stack::bar(8.0))?;
        let title = context.create_detached_component(document, Text::new("路线图"))?;
        let create = button(
            context,
            document,
            "新建里程碑",
            ButtonKind::Primary,
            RoadmapMessage::Create,
            &sink,
        )?;
        context.append_child(header, title)?;
        context.append_child(header, create)?;
        context.append_child(root, header)?;
        let error = context.create_detached_component(document, Text::new(""))?;
        context.append_child(root, error)?;
        let mut inputs = Vec::new();
        for (label, height, message) in [
            (
                "标题",
                40.0,
                RoadmapMessage::TitleChanged as fn(String) -> RoadmapMessage,
            ),
            (
                "描述",
                144.0,
                RoadmapMessage::DescriptionChanged as fn(String) -> RoadmapMessage,
            ),
            (
                "截止日期",
                40.0,
                RoadmapMessage::DueDateChanged as fn(String) -> RoadmapMessage,
            ),
        ] {
            let input =
                context.create_detached_component(document, TextArea::new("").height(height))?;
            let field = context.create_detached_component(
                document,
                FormField::new(label).control_child(input.stable_id()),
            )?;
            context.append_child(field, input)?;
            context.append_child(root, field)?;
            let sink = Arc::clone(&sink);
            context.on(input, move |_, event: &TextChanged, _| {
                sink(message(event.value.clone()))
            })?;
            inputs.push(input);
        }
        let actions = context.create_detached_component(document, Stack::row(8.0))?;
        let status = button(
            context,
            document,
            "状态",
            ButtonKind::Subtle,
            RoadmapMessage::CycleStatus,
            &sink,
        )?;
        let up = button(
            context,
            document,
            "上移",
            ButtonKind::Subtle,
            RoadmapMessage::Move(-1),
            &sink,
        )?;
        let down = button(
            context,
            document,
            "下移",
            ButtonKind::Subtle,
            RoadmapMessage::Move(1),
            &sink,
        )?;
        let save = button(
            context,
            document,
            "保存",
            ButtonKind::Primary,
            RoadmapMessage::Save,
            &sink,
        )?;
        let delete = button(
            context,
            document,
            "删除",
            ButtonKind::Danger,
            RoadmapMessage::Delete,
            &sink,
        )?;
        for button in [save, status, up, down, delete] {
            context.append_child(actions, button)?;
        }
        context.append_child(root, actions)?;
        let scroll =
            context.create_detached_component(document, ScrollView::new(ScrollAxes::Vertical))?;
        let list = context.create_detached_component(document, Stack::column(6.0))?;
        context.append_child(scroll, list)?;
        context.append_child(root, scroll)?;
        context.world_mut().register_focus_scope(root.stable_id())?;
        Ok(Self {
            root,
            error,
            fields: inputs.try_into().ok().unwrap(),
            create,
            status,
            up,
            down,
            save,
            delete,
            list,
            rows: HashMap::new(),
            task_toggles: HashMap::new(),
            sink,
            selected: None,
            project_id: None,
            suspended: false,
            restore_focus: false,
        })
    }

    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        snapshot: &RoadmapViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let changed_selection = self.selected != snapshot.selected;
        for (input, value) in self.fields.into_iter().zip([
            &snapshot.title,
            &snapshot.description,
            &snapshot.due_date,
        ]) {
            context.update_component(input, |input, _| {
                if changed_selection || input.state.value != *value {
                    input.state.replace_value(value.clone());
                }
            })?;
        }
        self.selected = snapshot.selected.clone();
        self.project_id = snapshot.project_id.clone();
        context.update_component(self.error, |text, _| {
            *text = Text::new(snapshot.error.clone().unwrap_or_default())
        })?;
        let mut children = context
            .world()
            .node(self.root.stable_id())
            .unwrap()
            .children
            .clone();
        children.retain(|id| *id != self.error.stable_id());
        if snapshot
            .error
            .as_ref()
            .is_some_and(|error| !error.is_empty())
        {
            children.insert(1, self.error.stable_id());
        }
        reconcile_children(context, self.root.stable_id(), &children)?;
        context.update_component(self.create, |button, _| {
            button.disabled = snapshot.project_id.is_none();
        })?;
        context.update_component(self.status, |button, _| {
            button.label = if snapshot.status_label.is_empty() {
                "状态".into()
            } else {
                snapshot.status_label.clone()
            };
            button.disabled = snapshot.selected.is_none();
        })?;
        context.update_component(self.save, |button, _| {
            button.disabled = snapshot.title.trim().is_empty();
        })?;
        context.update_component(self.delete, |button, _| {
            button.disabled = snapshot.selected.is_none()
        })?;
        let position = snapshot
            .cards
            .iter()
            .position(|card| Some(&card.id) == snapshot.selected.as_ref());
        context.update_component(self.up, |button, _| {
            button.disabled = position.is_none_or(|index| index == 0)
        })?;
        context.update_component(self.down, |button, _| {
            button.disabled = position.is_none_or(|index| index + 1 == snapshot.cards.len())
        })?;
        let keep = snapshot
            .cards
            .iter()
            .map(|card| card.id.as_str())
            .collect::<HashSet<_>>();
        let stale = self
            .rows
            .keys()
            .filter(|id| !keep.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale {
            context.remove_view(self.rows.remove(&id).unwrap().root)?;
        }
        let mut order = Vec::new();
        for card in &snapshot.cards {
            if !self.rows.contains_key(&card.id) {
                let root = context.create_detached_component(
                    document,
                    Button::new(format!("{} · {}", card.title, card.subtitle))
                        .kind(ButtonKind::Subtle),
                )?;
                let sink = Arc::clone(&self.sink);
                let id = card.id.clone();
                context.on(root, move |_, _: &Activate, _| {
                    sink(RoadmapMessage::Select(id.clone()))
                })?;
                self.rows.insert(card.id.clone(), RoadmapRow { root });
            }
            let row = &self.rows[&card.id];
            context.update_component(row.root, |button, _| {
                button.label = format!("{} · {}", card.title, card.subtitle);
            })?;
            order.push(row.root.stable_id());
        }
        reconcile_children(context, self.list.stable_id(), &order)?;
        let keep_tasks = snapshot
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<HashSet<_>>();
        let stale_tasks = self
            .task_toggles
            .keys()
            .filter(|id| !keep_tasks.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale_tasks {
            if let Some(toggle) = self.task_toggles.remove(&id) {
                context.remove_view(toggle)?;
            }
        }
        let mut task_order = Vec::new();
        for task in &snapshot.tasks {
            let toggle = if let Some(toggle) = self.task_toggles.get(&task.id).copied() {
                context.update_component(toggle, |view, _| {
                    *view = Switch::new(task.title.clone(), task.linked);
                })?;
                toggle
            } else {
                let toggle = context.create_detached_component(
                    document,
                    Switch::new(task.title.clone(), task.linked),
                )?;
                let sink = Arc::clone(&self.sink);
                let id = task.id.clone();
                context.on(toggle, move |_, _: &ToggleChanged, _| {
                    sink(RoadmapMessage::ToggleTask(id.clone()))
                })?;
                self.task_toggles.insert(task.id.clone(), toggle);
                toggle
            };
            task_order.push(toggle.stable_id());
        }
        let mut children = context
            .world()
            .node(self.root.stable_id())
            .unwrap()
            .children
            .clone();
        children.retain(|id| {
            !self
                .task_toggles
                .values()
                .any(|toggle| toggle.stable_id() == *id)
        });
        children.extend(task_order);
        reconcile_children(context, self.root.stable_id(), &children)?;
        self.restore_focus |= self.suspended;
        self.suspended = false;
        Ok(())
    }

    pub(crate) fn belongs_to(&self, snapshot: &RoadmapViewSnapshot) -> bool {
        self.project_id == snapshot.project_id
    }

    pub(crate) fn suspend(&mut self, context: &mut AppContext) -> Result<(), FrameworkError> {
        if !self.suspended {
            let mut changes = MutationQueue::new();
            changes.park_subtree(self.root.stable_id());
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
        if context.world().contains(self.error.stable_id()) {
            context.remove_view(self.error)?;
        }
        Ok(())
    }
}

fn button(
    context: &mut AppContext,
    document: DocumentId,
    label: &str,
    kind: ButtonKind,
    message: RoadmapMessage,
    sink: &Sink,
) -> Result<Entity<Button>, FrameworkError> {
    let button = context.create_detached_component(document, Button::new(label).kind(kind))?;
    let sink = Arc::clone(sink);
    context.on(button, move |_, _: &Activate, _| sink(message.clone()))?;
    Ok(button)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::TextSelection;
    use std::sync::Mutex;

    #[test]
    fn roadmap_view_updates_owned_rows_and_routes_actions() {
        let mut context = AppContext::new();
        let document = DocumentId::new(611).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink_events = Arc::clone(&events);
        let mut view = RoadmapView::mount(
            &mut context,
            document,
            Arc::new(move |event| sink_events.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = RoadmapViewSnapshot {
            selected: Some("one".into()),
            title: "Title".into(),
            description: "Body".into(),
            cards: vec![RoadmapCard {
                id: "one".into(),
                title: "Old".into(),
                subtitle: "Enabled".into(),
            }],
            ..Default::default()
        };
        view.sync(&mut context, document, &snapshot).unwrap();
        let row = view.rows["one"].root;
        snapshot.cards[0].title = "Edited".into();
        snapshot.cards[0].subtitle = "Disabled".into();
        view.sync(&mut context, document, &snapshot).unwrap();
        assert_eq!(view.rows["one"].root, row);
        assert_eq!(
            context
                .read(view.rows["one"].root, |button| button.label.clone())
                .unwrap(),
            "Edited · Disabled"
        );
        context
            .update_component(row, |_, cx| cx.emit(Activate))
            .unwrap();
        context
            .update_component(view.save, |_, cx| cx.emit(Activate))
            .unwrap();
        let received = events.lock().unwrap();
        assert!(
            matches!(&received[..], [RoadmapMessage::Select(id), RoadmapMessage::Save] if id == "one")
        );
        drop(received);
        snapshot.cards.push(RoadmapCard {
            id: "two".into(),
            title: "Second".into(),
            subtitle: "Pending".into(),
        });
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(context.read(view.up, |button| button.disabled).unwrap());
        assert!(!context.read(view.down, |button| button.disabled).unwrap());
        context
            .update_component(view.down, |_, cx| cx.emit(Activate))
            .unwrap();
        context
            .update_component(view.status, |_, cx| cx.emit(Activate))
            .unwrap();
        assert!(matches!(
            &events.lock().unwrap()[2..],
            [RoadmapMessage::Move(1), RoadmapMessage::CycleStatus]
        ));
        snapshot.cards.swap(0, 1);
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(!context.read(view.up, |button| button.disabled).unwrap());
        assert!(context.read(view.down, |button| button.disabled).unwrap());
        assert_eq!(
            context
                .world()
                .node(view.list.stable_id())
                .unwrap()
                .children,
            vec![view.rows["two"].root.stable_id(), row.stable_id()]
        );
        snapshot.cards.clear();
        snapshot.selected = None;
        snapshot.title.clear();
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(!context.world().contains(row.stable_id()));
        assert!(context.read(view.save, |button| button.disabled).unwrap());
        assert!(context.read(view.delete, |button| button.disabled).unwrap());
    }

    #[test]
    fn roadmap_navigation_restores_selection_focus_and_disposes_parked_nodes() {
        let mut context = AppContext::new();
        let document = DocumentId::new(612).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut view = RoadmapView::mount(&mut context, document, Arc::new(|_| {})).unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = RoadmapViewSnapshot {
            project_id: Some("project-one".into()),
            title: "Title".into(),
            description: "Roadmap body".into(),
            ..Default::default()
        };
        view.sync(&mut context, document, &snapshot).unwrap();
        let body = view.fields[1];
        let selection = TextSelection {
            anchor: 2,
            focus: 7,
        };
        context
            .update_component(body, |input, _| input.state.selection = selection)
            .unwrap();
        context.focus_node(document, body.stable_id()).unwrap();
        view.suspend(&mut context).unwrap();
        assert_eq!(context.world().focused(document), None);
        view.sync(&mut context, document, &snapshot).unwrap();
        context.append_child(host, view.root).unwrap();
        view.restore_focus(&mut context).unwrap();
        assert_eq!(context.world().focused(document), Some(body.stable_id()));
        assert_eq!(
            context.read(body, |input| input.state.selection).unwrap(),
            selection
        );
        assert!(view.belongs_to(&snapshot));
        snapshot.project_id = Some("project-two".into());
        assert!(!view.belongs_to(&snapshot));
        let nodes = [
            view.root.stable_id(),
            view.error.stable_id(),
            view.fields[0].stable_id(),
            body.stable_id(),
            view.fields[2].stable_id(),
            view.list.stable_id(),
        ];
        view.suspend(&mut context).unwrap();
        view.dispose(&mut context).unwrap();
        for node in nodes {
            assert!(!context.world().contains(node));
        }
    }
}
