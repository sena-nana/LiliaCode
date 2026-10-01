use super::RoadmapMessage;
use crate::runtime_layout::{view_bar, view_column, view_fill_column, view_row, Bound};
use nana_ui::runtime::view::{entity_ref, signal, text, widget, with_refs, EachExt};
#[cfg(test)]
use nana_ui::runtime::Activate;
use nana_ui::runtime::{
    AppContext, Button, DocumentId, Entity, FormField, FrameworkError, MutationQueue, ScrollAxes,
    ScrollView, StableNodeId, Stack, Switch, Text, TextArea, TextInput,
};
use nana_ui::ButtonKind;
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
    fields: [Entity<TextInput>; 1],
    description: Entity<TextArea>,
    due: Entity<TextInput>,
    create: Entity<Button>,
    status: Entity<Button>,
    up: Entity<Button>,
    down: Entity<Button>,
    save: Entity<Button>,
    delete: Entity<Button>,
    list: Entity<Stack>,
    rows: HashMap<String, RoadmapRow>,
    task_host: Entity<Stack>,
    task_toggles: HashMap<String, Entity<Switch>>,
    title_text: Bound<String>,
    description_text: Bound<String>,
    due_text: Bound<String>,
    error_text: Bound<String>,
    show_error: Bound<bool>,
    status_label: Bound<String>,
    create_off: Bound<bool>,
    status_off: Bound<bool>,
    save_off: Bound<bool>,
    delete_off: Bound<bool>,
    up_off: Bound<bool>,
    down_off: Bound<bool>,
    show_tasks: Bound<bool>,
    cards: Bound<Vec<RoadmapCard>>,
    tasks: Bound<Vec<RoadmapTask>>,
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
                self.description.stable_id(),
            ),
            (
                "field.project-milestone-due-date".into(),
                self.due.stable_id(),
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
        let title_text = Bound::new();
        let description_text = Bound::new();
        let due_text = Bound::new();
        let error_text = Bound::new();
        let show_error = Bound::new();
        let status_label = Bound::new();
        let create_off = Bound::new();
        let status_off = Bound::new();
        let save_off = Bound::new();
        let delete_off = Bound::new();
        let up_off = Bound::new();
        let down_off = Bound::new();
        let show_tasks = Bound::new();
        let cards: Bound<Vec<RoadmapCard>> = Bound::new();
        let tasks: Bound<Vec<RoadmapTask>> = Bound::new();
        let title_slot = title_text.clone();
        let description_slot = description_text.clone();
        let due_slot = due_text.clone();
        let error_slot = error_text.clone();
        let show_error_slot = show_error.clone();
        let status_slot = status_label.clone();
        let create_off_slot = create_off.clone();
        let status_off_slot = status_off.clone();
        let save_off_slot = save_off.clone();
        let delete_off_slot = delete_off.clone();
        let up_off_slot = up_off.clone();
        let down_off_slot = down_off.clone();
        let show_tasks_slot = show_tasks.clone();
        let cards_slot = cards.clone();
        let tasks_slot = tasks.clone();
        let (
            _,
            (
                root,
                error,
                fields,
                description,
                due,
                [create, status, up, down, save, delete],
                scroll,
                task_host,
            ),
        ) = context.mount_view_detached(document, move || {
            let title_text = title_slot.install(signal(String::new()));
            let description_text = description_slot.install(signal(String::new()));
            let due_text = due_slot.install(signal(String::new()));
            let error_text = error_slot.install(signal(String::new()));
            let show_error = show_error_slot.install(signal(false));
            let status_label = status_slot.install(signal("状态".to_owned()));
            let create_off = create_off_slot.install(signal(false));
            let status_off = status_off_slot.install(signal(false));
            let save_off = save_off_slot.install(signal(false));
            let delete_off = delete_off_slot.install(signal(false));
            let up_off = up_off_slot.install(signal(false));
            let down_off = down_off_slot.install(signal(false));
            let show_tasks = show_tasks_slot.install(signal(false));
            let cards = cards_slot.install(signal(Vec::new()));
            let tasks = tasks_slot.install(signal(Vec::new()));
            let root = entity_ref::<Stack>();
            let error = entity_ref::<Text>();
            let title = entity_ref::<TextInput>();
            let description = entity_ref::<TextArea>();
            let due = entity_ref::<TextInput>();
            let create = entity_ref::<Button>();
            let status = entity_ref::<Button>();
            let up = entity_ref::<Button>();
            let down = entity_ref::<Button>();
            let save = entity_ref::<Button>();
            let delete = entity_ref::<Button>();
            let scroll = entity_ref::<ScrollView>();
            let task_host = entity_ref::<Stack>();
            let create_sink = Arc::clone(&sink);
            let status_sink = Arc::clone(&sink);
            let up_sink = Arc::clone(&sink);
            let down_sink = Arc::clone(&sink);
            let save_sink = Arc::clone(&sink);
            let delete_sink = Arc::clone(&sink);
            let title_sink = Arc::clone(&sink);
            let description_sink = Arc::clone(&sink);
            let due_sink = Arc::clone(&sink);
            let card_sink = Arc::clone(&sink);
            let task_sink = sink;
            let page = view_fill_column(12.0).entity_ref(root).children((
                view_bar(8.0).children((
                    text("路线图"),
                    widget(Button::new("新建里程碑").kind(ButtonKind::Primary))
                        .entity_ref(create)
                        .disabled(create_off)
                        .on_activate(move || create_sink(RoadmapMessage::Create)),
                )),
                text(error_text).entity_ref(error).visible(show_error),
                field(
                    "标题",
                    widget(TextInput::new(""))
                        .entity_ref(title)
                        .value(title_text)
                        .on_input(move |event| {
                            title_sink(RoadmapMessage::TitleChanged(event.value.to_string()))
                        }),
                ),
                field(
                    "描述",
                    widget(TextArea::new("").height(144.0))
                        .entity_ref(description)
                        .value(description_text)
                        .on_input(move |event| {
                            description_sink(RoadmapMessage::DescriptionChanged(
                                event.value.to_string(),
                            ))
                        }),
                ),
                field(
                    "截止日期",
                    widget(TextInput::new(""))
                        .entity_ref(due)
                        .value(due_text)
                        .on_input(move |event| {
                            due_sink(RoadmapMessage::DueDateChanged(event.value.to_string()))
                        }),
                ),
                view_row(8.0).children((
                    widget(Button::new("保存").kind(ButtonKind::Primary))
                        .entity_ref(save)
                        .disabled(save_off)
                        .on_activate(move || save_sink(RoadmapMessage::Save)),
                    widget(Button::new("状态").kind(ButtonKind::Subtle))
                        .entity_ref(status)
                        .label(status_label)
                        .disabled(status_off)
                        .on_activate(move || status_sink(RoadmapMessage::CycleStatus)),
                    widget(Button::new("上移").kind(ButtonKind::Subtle))
                        .entity_ref(up)
                        .disabled(up_off)
                        .on_activate(move || up_sink(RoadmapMessage::Move(-1))),
                    widget(Button::new("下移").kind(ButtonKind::Subtle))
                        .entity_ref(down)
                        .disabled(down_off)
                        .on_activate(move || down_sink(RoadmapMessage::Move(1))),
                    widget(Button::new("删除").kind(ButtonKind::Danger))
                        .entity_ref(delete)
                        .disabled(delete_off)
                        .on_activate(move || delete_sink(RoadmapMessage::Delete)),
                )),
                widget(ScrollView::new(ScrollAxes::Vertical))
                    .entity_ref(scroll)
                    .children(
                        cards
                            .each(
                                |card| card.id.clone(),
                                move |card| {
                                    let id = card.id.clone();
                                    let label_id = id.clone();
                                    let cards = cards;
                                    let sink = Arc::clone(&card_sink);
                                    widget(Button::new("").kind(ButtonKind::Subtle))
                                        .label(move || card_label(&cards, &label_id))
                                        .on_activate(move || {
                                            sink(RoadmapMessage::Select(id.clone()))
                                        })
                                },
                            )
                            .gap(6.0),
                    ),
                view_column(0.0)
                    .entity_ref(task_host)
                    .visible(show_tasks)
                    .children(
                        tasks
                            .each(
                                |task| task.id.clone(),
                                move |task| {
                                    let id = task.id.clone();
                                    let title_id = id.clone();
                                    let linked_id = id.clone();
                                    let tasks = tasks;
                                    let sink = Arc::clone(&task_sink);
                                    widget(Switch::new(task.title.clone(), task.linked))
                                        .label(move || task_title(&tasks, &title_id))
                                        .checked(move || task_linked(&tasks, &linked_id))
                                        .on_change(move |_| {
                                            sink(RoadmapMessage::ToggleTask(id.clone()))
                                        })
                                },
                            )
                            .gap(12.0),
                    ),
            ));
            with_refs(
                page,
                (
                    root,
                    error,
                    [title],
                    description,
                    due,
                    [create, status, up, down, save, delete],
                    scroll,
                    task_host,
                ),
            )
        })?;
        let list = only_child(context, scroll.stable_id())?;
        context
            .compat_world_mut()
            .register_focus_scope(root.stable_id())?;
        Ok(Self {
            root,
            error,
            fields,
            description,
            due,
            create,
            status,
            up,
            down,
            save,
            delete,
            list,
            rows: HashMap::new(),
            task_host,
            task_toggles: HashMap::new(),
            title_text,
            description_text,
            due_text,
            error_text,
            show_error,
            status_label,
            create_off,
            status_off,
            save_off,
            delete_off,
            up_off,
            down_off,
            show_tasks,
            cards,
            tasks,
            selected: None,
            project_id: None,
            suspended: false,
            restore_focus: false,
        })
    }

    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        _document: DocumentId,
        snapshot: &RoadmapViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let changed_selection = self.selected != snapshot.selected;
        self.selected = snapshot.selected.clone();
        self.project_id = snapshot.project_id.clone();
        self.title_text.set(snapshot.title.clone());
        self.description_text.set(snapshot.description.clone());
        self.due_text.set(snapshot.due_date.clone());
        self.error_text
            .set(snapshot.error.clone().unwrap_or_default());
        self.show_error.set(
            snapshot
                .error
                .as_ref()
                .is_some_and(|error| !error.is_empty()),
        );
        self.status_label.set(if snapshot.status_label.is_empty() {
            "状态".to_owned()
        } else {
            snapshot.status_label.clone()
        });
        self.create_off.set(snapshot.project_id.is_none());
        self.status_off.set(snapshot.selected.is_none());
        self.save_off.set(snapshot.title.trim().is_empty());
        self.delete_off.set(snapshot.selected.is_none());
        let position = snapshot
            .cards
            .iter()
            .position(|card| Some(&card.id) == snapshot.selected.as_ref());
        self.up_off.set(position.is_none_or(|index| index == 0));
        self.down_off
            .set(position.is_none_or(|index| index + 1 == snapshot.cards.len()));
        self.show_tasks.set(!snapshot.tasks.is_empty());
        self.cards.set(snapshot.cards.clone());
        self.tasks.set(snapshot.tasks.clone());
        if changed_selection {
            for (input, value) in self.fields.into_iter().zip([snapshot.title.as_str()]) {
                context.update_component(input, |input, _| {
                    input.state.replace_value(value.to_owned());
                })?;
            }
            context.update_component(self.due, |input, _| {
                input.state.replace_value(snapshot.due_date.clone());
            })?;
            context.update_component(self.description, |input, _| {
                input.state.replace_value(snapshot.description.clone());
            })?;
        }
        context.flush_reactive()?;
        self.rows = zip_buttons(context, self.list, &snapshot.cards, |card| card.id.as_str())
            .into_iter()
            .map(|(id, root)| (id, RoadmapRow { root }))
            .collect();
        self.task_toggles = zip_toggles(context, self.task_host, &snapshot.tasks);
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

fn field(
    label: &str,
    control: impl nana_ui::runtime::view::IntoView,
) -> impl nana_ui::runtime::view::IntoView {
    widget(FormField::new(label)).child_slot(control, |field, id| field.control_child(id))
}

fn card_label(cards: &nana_ui::runtime::view::Signal<Vec<RoadmapCard>>, id: &str) -> String {
    cards.with(|cards| {
        cards
            .iter()
            .find(|card| card.id == id)
            .map(|card| format!("{} · {}", card.title, card.subtitle))
            .unwrap_or_default()
    })
}

fn task_title(tasks: &nana_ui::runtime::view::Signal<Vec<RoadmapTask>>, id: &str) -> String {
    tasks.with(|tasks| {
        tasks
            .iter()
            .find(|task| task.id == id)
            .map(|task| task.title.clone())
            .unwrap_or_default()
    })
}

fn task_linked(tasks: &nana_ui::runtime::view::Signal<Vec<RoadmapTask>>, id: &str) -> bool {
    tasks.with(|tasks| {
        tasks
            .iter()
            .find(|task| task.id == id)
            .is_some_and(|task| task.linked)
    })
}

fn only_child(context: &AppContext, parent: StableNodeId) -> Result<Entity<Stack>, FrameworkError> {
    let id = context
        .world()
        .node(parent)
        .and_then(|node| node.children.first().copied())
        .ok_or(FrameworkError::InvalidInput)?;
    Ok(Entity::from_stable_id(id))
}

fn zip_buttons(
    context: &AppContext,
    list: Entity<Stack>,
    cards: &[RoadmapCard],
    id_of: impl Fn(&RoadmapCard) -> &str,
) -> HashMap<String, Entity<Button>> {
    let children = context
        .world()
        .node(list.stable_id())
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut rows = HashMap::new();
    let mut cursor = children.into_iter();
    let mut seen = HashSet::new();
    for card in cards {
        let id = id_of(card);
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

fn zip_toggles(
    context: &AppContext,
    host: Entity<Stack>,
    tasks: &[RoadmapTask],
) -> HashMap<String, Entity<Switch>> {
    let Some(list) = context
        .world()
        .node(host.stable_id())
        .and_then(|node| node.children.first().copied())
    else {
        return HashMap::new();
    };
    let children = context
        .world()
        .node(list)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut toggles = HashMap::new();
    let mut cursor = children.into_iter();
    let mut seen = HashSet::new();
    for task in tasks {
        if !seen.insert(task.id.clone()) {
            continue;
        }
        let Some(child) = cursor.next() else {
            break;
        };
        toggles.insert(task.id.clone(), Entity::from_stable_id(child));
    }
    toggles
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
        let body = view.description;
        let selection = TextSelection {
            anchor: 2,
            focus: 7,
            affinity: Default::default(),
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
            view.due.stable_id(),
            view.list.stable_id(),
        ];
        view.suspend(&mut context).unwrap();
        view.dispose(&mut context).unwrap();
        for node in nodes {
            assert!(!context.world().contains(node));
        }
    }
}
