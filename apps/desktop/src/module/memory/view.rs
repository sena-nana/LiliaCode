use super::MemoryMessage;
use crate::runtime_layout::{view_bar, view_column, view_fill_column, view_row, Bound};
use nana_ui::runtime::view::{entity_ref, signal, text, widget, with_refs, EachExt, WhenExt};
use nana_ui::runtime::{
    ActionMenu, ActionMenuItem, Activate, AppContext, Button, DocumentId, Entity, FormField,
    FrameworkError, LengthSpec, MutationQueue, PopoverToggled, ScrollAxes, ScrollView,
    StableNodeId, Stack, Switch, Text, TextArea, TextInput,
};
use nana_ui::{ButtonKind, PopoverPlacement};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryViewSnapshot {
    pub project_id: Option<String>,
    pub selected: Option<String>,
    pub title: String,
    pub body: String,
    pub tags: String,
    pub scope_label: String,
    pub error: Option<String>,
    pub cards: Vec<MemoryCard>,
    pub tasks: Vec<(String, String)>,
    pub task_menu_open: bool,
    pub enabled: bool,
    pub global_enabled: bool,
    pub baseline_enabled: bool,
    pub task_injection: Option<bool>,
    pub cooldown: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryCard {
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

type Sink = Arc<dyn Fn(MemoryMessage) + Send + Sync>;

struct MemoryRow {
    root: Entity<Button>,
}

pub(crate) struct MemoryView {
    pub(crate) root: Entity<Stack>,
    error: Entity<Text>,
    fields: [Entity<TextArea>; 3],
    create: Entity<Button>,
    cooldown: Entity<TextInput>,
    enabled: Entity<Switch>,
    global: Entity<Switch>,
    baseline: Entity<Switch>,
    task_injection: Entity<Switch>,
    save: Entity<Button>,
    delete: Entity<Button>,
    reset: Entity<Button>,
    task_menu: Entity<ActionMenu>,
    task_items: HashMap<String, Entity<ActionMenuItem>>,
    list: Entity<Stack>,
    rows: HashMap<String, MemoryRow>,
    title_text: Bound<String>,
    body_text: Bound<String>,
    tags_text: Bound<String>,
    cooldown_text: Bound<String>,
    error_text: Bound<String>,
    show_error: Bound<bool>,
    scope_label: Bound<String>,
    enabled_on: Bound<bool>,
    enabled_off: Bound<bool>,
    global_on: Bound<bool>,
    baseline_on: Bound<bool>,
    baseline_off: Bound<bool>,
    task_on: Bound<bool>,
    task_off: Bound<bool>,
    save_off: Bound<bool>,
    delete_off: Bound<bool>,
    reset_off: Bound<bool>,
    menu_open: Bound<bool>,
    cards: Bound<Vec<MemoryCard>>,
    tasks: Bound<Vec<(String, String)>>,
    selected: Option<String>,
    project_id: Option<String>,
    suspended: bool,
    restore_focus: bool,
}

impl MemoryView {
    pub(crate) fn debug_nodes(&self) -> Vec<(String, StableNodeId)> {
        let mut nodes = vec![
            ("composer.memory-new".into(), self.create.stable_id()),
            ("field.memory-title".into(), self.fields[0].stable_id()),
            ("field.memory-body".into(), self.fields[1].stable_id()),
            ("field.memory-tags".into(), self.fields[2].stable_id()),
            ("field.memory-cooldown".into(), self.cooldown.stable_id()),
            ("switch.memory-enabled".into(), self.enabled.stable_id()),
            ("switch.memory-global".into(), self.global.stable_id()),
            ("switch.memory-baseline".into(), self.baseline.stable_id()),
            (
                "switch.memory-task-enabled".into(),
                self.task_injection.stable_id(),
            ),
            ("composer.memory-save".into(), self.save.stable_id()),
            ("composer.memory-task-reset".into(), self.reset.stable_id()),
            ("memory.task-menu".into(), self.task_menu.stable_id()),
        ];
        for (id, row) in &self.rows {
            nodes.push((format!("project-card.memory-{id}"), row.root.stable_id()));
        }
        for (id, item) in &self.task_items {
            nodes.push((format!("memory.task.{id}"), item.stable_id()));
        }
        nodes
    }

    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Sink,
    ) -> Result<Self, FrameworkError> {
        let title_text = Bound::new();
        let body_text = Bound::new();
        let tags_text = Bound::new();
        let cooldown_text = Bound::new();
        let error_text = Bound::new();
        let show_error = Bound::new();
        let scope_label = Bound::new();
        let enabled_on = Bound::new();
        let enabled_off = Bound::new();
        let global_on = Bound::new();
        let baseline_on = Bound::new();
        let baseline_off = Bound::new();
        let task_on = Bound::new();
        let task_off = Bound::new();
        let save_off = Bound::new();
        let delete_off = Bound::new();
        let reset_off = Bound::new();
        let menu_open = Bound::new();
        let cards: Bound<Vec<MemoryCard>> = Bound::new();
        let tasks: Bound<Vec<(String, String)>> = Bound::new();
        let title_slot = title_text.clone();
        let body_slot = body_text.clone();
        let tags_slot = tags_text.clone();
        let cooldown_slot = cooldown_text.clone();
        let error_slot = error_text.clone();
        let show_error_slot = show_error.clone();
        let scope_slot = scope_label.clone();
        let enabled_on_slot = enabled_on.clone();
        let enabled_off_slot = enabled_off.clone();
        let global_on_slot = global_on.clone();
        let baseline_on_slot = baseline_on.clone();
        let baseline_off_slot = baseline_off.clone();
        let task_on_slot = task_on.clone();
        let task_off_slot = task_off.clone();
        let save_off_slot = save_off.clone();
        let delete_off_slot = delete_off.clone();
        let reset_off_slot = reset_off.clone();
        let menu_open_slot = menu_open.clone();
        let cards_slot = cards.clone();
        let tasks_slot = tasks.clone();
        let (
            _,
            (
                root,
                error,
                fields,
                [create, _scope, save, delete, reset],
                cooldown,
                [enabled, global, baseline, task_injection],
                task_menu,
                list,
            ),
        ) = context.mount_view_detached(document, move || {
            let title_text = title_slot.install(signal(String::new()));
            let body_text = body_slot.install(signal(String::new()));
            let tags_text = tags_slot.install(signal(String::new()));
            let cooldown_text = cooldown_slot.install(signal(String::new()));
            let error_text = error_slot.install(signal(String::new()));
            let show_error = show_error_slot.install(signal(false));
            let scope_label = scope_slot.install(signal(String::new()));
            let enabled_on = enabled_on_slot.install(signal(true));
            let enabled_off = enabled_off_slot.install(signal(false));
            let global_on = global_on_slot.install(signal(true));
            let baseline_on = baseline_on_slot.install(signal(true));
            let baseline_off = baseline_off_slot.install(signal(false));
            let task_on = task_on_slot.install(signal(false));
            let task_off = task_off_slot.install(signal(true));
            let save_off = save_off_slot.install(signal(false));
            let delete_off = delete_off_slot.install(signal(false));
            let reset_off = reset_off_slot.install(signal(false));
            let menu_open = menu_open_slot.install(signal(false));
            let cards = cards_slot.install(signal(Vec::new()));
            let tasks = tasks_slot.install(signal(Vec::new()));
            let root = entity_ref::<Stack>();
            let error = entity_ref::<Text>();
            let title = entity_ref::<TextArea>();
            let body = entity_ref::<TextArea>();
            let tags = entity_ref::<TextArea>();
            let create = entity_ref::<Button>();
            let scope = entity_ref::<Button>();
            let save = entity_ref::<Button>();
            let delete = entity_ref::<Button>();
            let reset = entity_ref::<Button>();
            let cooldown = entity_ref::<TextInput>();
            let enabled = entity_ref::<Switch>();
            let global = entity_ref::<Switch>();
            let baseline = entity_ref::<Switch>();
            let task_injection = entity_ref::<Switch>();
            let task_menu = entity_ref::<ActionMenu>();
            let list = entity_ref::<Stack>();
            let new_sink = Arc::clone(&sink);
            let scope_sink = Arc::clone(&sink);
            let save_sink = Arc::clone(&sink);
            let delete_sink = Arc::clone(&sink);
            let reset_sink = Arc::clone(&sink);
            let title_sink = Arc::clone(&sink);
            let body_sink = Arc::clone(&sink);
            let tags_sink = Arc::clone(&sink);
            let cooldown_sink = Arc::clone(&sink);
            let enabled_sink = Arc::clone(&sink);
            let global_sink = Arc::clone(&sink);
            let baseline_sink = Arc::clone(&sink);
            let task_sink = Arc::clone(&sink);
            let menu_sink = Arc::clone(&sink);
            let card_sink = Arc::clone(&sink);
            let item_sink = sink;
            let page = view_fill_column(12.0).entity_ref(root).children((
                view_bar(8.0).children((
                    text("记忆"),
                    widget(Button::new("新建").kind(ButtonKind::Primary))
                        .entity_ref(create)
                        .on_activate(move || new_sink(MemoryMessage::New)),
                )),
                widget(
                    ScrollView::new(ScrollAxes::Vertical)
                        .style(Stack::fill_column(0.0).node_style()),
                )
                .children(
                    widget(Stack::column(12.0).with_layout(|layout| {
                        // Keep the save row and the switches off the scrollport edge.
                        layout.padding_bottom = Some(LengthSpec::Px(16.0));
                    }))
                    .children((
                        text(error_text).entity_ref(error).visible(show_error),
                        field(
                            "标题",
                            widget(TextArea::new("").height(40.0))
                                .entity_ref(title)
                                .value(title_text)
                                .on_input(move |event| {
                                    title_sink(MemoryMessage::TitleChanged(event.value.to_string()))
                                }),
                        ),
                        field(
                            "正文",
                            widget(TextArea::new("").height(112.0).resize_vertical(true))
                                .entity_ref(body)
                                .value(body_text)
                                .on_input(move |event| {
                                    body_sink(MemoryMessage::BodyReplaced(event.value.to_string()))
                                }),
                        ),
                        field(
                            "标签",
                            widget(TextArea::new("").height(40.0))
                                .entity_ref(tags)
                                .value(tags_text)
                                .on_input(move |event| {
                                    tags_sink(MemoryMessage::TagsChanged(event.value.to_string()))
                                }),
                        ),
                        view_row(8.0).children((
                            widget(Button::new("项目").kind(ButtonKind::Subtle))
                                .entity_ref(scope)
                                .label(scope_label)
                                .on_activate(move || scope_sink(MemoryMessage::ToggleScope)),
                            widget(Button::new("保存").kind(ButtonKind::Primary))
                                .entity_ref(save)
                                .disabled(save_off)
                                .on_activate(move || save_sink(MemoryMessage::Save)),
                            widget(Button::new("删除").kind(ButtonKind::Danger))
                                .entity_ref(delete)
                                .disabled(delete_off)
                                .on_activate(move || delete_sink(MemoryMessage::Delete)),
                            widget(Button::new("重置冷却").kind(ButtonKind::Subtle))
                                .entity_ref(reset)
                                .disabled(reset_off)
                                .on_activate(move || reset_sink(MemoryMessage::ResetTaskCooldown)),
                            widget(
                                ActionMenu::new()
                                    .trigger("注入任务".to_owned())
                                    .placement(PopoverPlacement::Bottom)
                                    .open(false),
                            )
                            .entity_ref(task_menu)
                            .bind(move |menu| menu.popover.open = menu_open.get())
                            .on(move |_event: &PopoverToggled| {
                                menu_sink(MemoryMessage::ToggleTaskMenu)
                            })
                            .children({
                                let item_sink = Arc::clone(&item_sink);
                                let tasks = tasks;
                                menu_open.then_show(move || {
                                    let item_sink = Arc::clone(&item_sink);
                                    let tasks = tasks;
                                    tasks.each(
                                        |task| task.0.clone(),
                                        move |task| {
                                            let id = task.0.clone();
                                            let label_id = id.clone();
                                            let tasks = tasks;
                                            let sink = Arc::clone(&item_sink);
                                            widget(ActionMenuItem::new(task.1.clone()))
                                                .bind(move |item| {
                                                    if let Some(label) = tasks.with(|tasks| {
                                                        tasks
                                                            .iter()
                                                            .find(|(task_id, _)| {
                                                                task_id == &label_id
                                                            })
                                                            .map(|(_, label)| label.clone())
                                                    }) {
                                                        item.label = Arc::from(label);
                                                    }
                                                })
                                                .on(move |_event: &Activate| {
                                                    sink(MemoryMessage::SelectInjectionTask(
                                                        id.clone(),
                                                    ))
                                                })
                                        },
                                    )
                                })
                            }),
                        )),
                        widget(Stack::bar(8.0).wrap(true)).children((
                            widget(Switch::new("启用记忆", true))
                                .entity_ref(enabled)
                                .label("启用记忆")
                                .checked(enabled_on)
                                .disabled(enabled_off)
                                .on_change(move |_| enabled_sink(MemoryMessage::ToggleEnabled)),
                            widget(Switch::new("全局注入", true))
                                .entity_ref(global)
                                .label("全局注入")
                                .checked(global_on)
                                .on_change(move |_| global_sink(MemoryMessage::ToggleGlobal)),
                            widget(Switch::new("基线注入", true))
                                .entity_ref(baseline)
                                .label("基线注入")
                                .checked(baseline_on)
                                .disabled(baseline_off)
                                .on_change(move |_| baseline_sink(MemoryMessage::ToggleBaseline)),
                            widget(Switch::new("为此会话注入记忆", true))
                                .entity_ref(task_injection)
                                .label("为此会话注入记忆")
                                .checked(task_on)
                                .disabled(task_off)
                                .on_change(move |_| task_sink(MemoryMessage::ToggleTaskInjection)),
                            field(
                                "冷却轮数",
                                widget(TextInput::new(""))
                                    .entity_ref(cooldown)
                                    .value(cooldown_text)
                                    .on_input(move |event| {
                                        cooldown_sink(MemoryMessage::CooldownChanged(
                                            event.value.to_string(),
                                        ))
                                    }),
                            ),
                        )),
                        view_column(6.0).entity_ref(list).children(
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
                                                sink(MemoryMessage::Select(id.clone()))
                                            })
                                    },
                                )
                                .gap(6.0),
                        ),
                    )),
                ),
            ));
            with_refs(
                page,
                (
                    root,
                    error,
                    [title, body, tags],
                    [create, scope, save, delete, reset],
                    cooldown,
                    [enabled, global, baseline, task_injection],
                    task_menu,
                    list,
                ),
            )
        })?;
        context
            .compat_world_mut()
            .register_focus_scope(root.stable_id())?;
        Ok(Self {
            root,
            error,
            fields,
            create,
            cooldown,
            enabled,
            global,
            baseline,
            task_injection,
            save,
            delete,
            reset,
            task_menu,
            task_items: HashMap::new(),
            list,
            rows: HashMap::new(),
            title_text,
            body_text,
            tags_text,
            cooldown_text,
            error_text,
            show_error,
            scope_label,
            enabled_on,
            enabled_off,
            global_on,
            baseline_on,
            baseline_off,
            task_on,
            task_off,
            save_off,
            delete_off,
            reset_off,
            menu_open,
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
        document: DocumentId,
        snapshot: &MemoryViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let changed_selection = self.selected != snapshot.selected;
        self.selected = snapshot.selected.clone();
        self.project_id = snapshot.project_id.clone();
        // Switching records replaces the draft outright. A focused edit is
        // otherwise ahead of this snapshot until its change message lands;
        // pushing the stale text would replace the buffer and drop undo.
        if changed_selection {
            for (input, value) in self.fields.into_iter().zip([
                snapshot.title.as_str(),
                snapshot.body.as_str(),
                snapshot.tags.as_str(),
            ]) {
                replace_area(context, input, value)?;
            }
            replace_input(context, self.cooldown, &snapshot.cooldown)?;
        }
        adopt_area(
            context,
            document,
            self.fields[0],
            &self.title_text,
            &snapshot.title,
        )?;
        adopt_area(
            context,
            document,
            self.fields[1],
            &self.body_text,
            &snapshot.body,
        )?;
        adopt_area(
            context,
            document,
            self.fields[2],
            &self.tags_text,
            &snapshot.tags,
        )?;
        adopt_input(
            context,
            document,
            self.cooldown,
            &self.cooldown_text,
            &snapshot.cooldown,
        )?;
        self.error_text
            .set(snapshot.error.clone().unwrap_or_default());
        self.show_error.set(
            snapshot
                .error
                .as_ref()
                .is_some_and(|error| !error.is_empty()),
        );
        self.scope_label.set(snapshot.scope_label.clone());
        self.enabled_on.set(snapshot.enabled);
        self.enabled_off.set(snapshot.selected.is_none());
        self.global_on.set(snapshot.global_enabled);
        self.baseline_on.set(snapshot.baseline_enabled);
        self.baseline_off.set(!snapshot.global_enabled);
        self.task_on.set(snapshot.task_injection.unwrap_or(false));
        self.task_off.set(snapshot.task_injection.is_none());
        self.save_off
            .set(snapshot.title.trim().is_empty() || snapshot.body.trim().is_empty());
        self.delete_off.set(snapshot.selected.is_none());
        self.reset_off.set(snapshot.task_injection.is_none());
        self.menu_open.set(snapshot.task_menu_open);
        self.cards.set(snapshot.cards.clone());
        self.tasks.set(if snapshot.task_menu_open {
            snapshot.tasks.clone()
        } else {
            Vec::new()
        });
        context.flush_reactive()?;
        self.rows = zip_buttons(context, self.list, &snapshot.cards, |card| &card.id)
            .into_iter()
            .map(|(id, root)| (id, MemoryRow { root }))
            .collect();
        self.task_items = zip_menu_items(context, self.task_menu, &snapshot.tasks);
        self.restore_focus |= self.suspended;
        self.suspended = false;
        Ok(())
    }

    pub(crate) fn belongs_to(&self, snapshot: &MemoryViewSnapshot) -> bool {
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

fn card_label(cards: &nana_ui::runtime::view::Signal<Vec<MemoryCard>>, id: &str) -> String {
    cards.with(|cards| {
        cards
            .iter()
            .find(|card| card.id == id)
            .map(|card| format!("{} · {}", card.title, card.subtitle))
            .unwrap_or_default()
    })
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

fn adopt_area(
    context: &AppContext,
    document: DocumentId,
    area: Entity<TextArea>,
    bound: &Bound<String>,
    value: &str,
) -> Result<(), FrameworkError> {
    let current = context.read(area, |area| area.state.value.clone())?;
    adopt_editor(context, document, area.stable_id(), &current, bound, value);
    Ok(())
}

fn adopt_input(
    context: &AppContext,
    document: DocumentId,
    input: Entity<TextInput>,
    bound: &Bound<String>,
    value: &str,
) -> Result<(), FrameworkError> {
    let current = context.read(input, |input| input.state.value.clone())?;
    adopt_editor(context, document, input.stable_id(), &current, bound, value);
    Ok(())
}

fn adopt_editor(
    context: &AppContext,
    document: DocumentId,
    node: StableNodeId,
    current: &nana_ui::runtime::TextValue,
    bound: &Bound<String>,
    value: &str,
) {
    let focused = context.world().focused(document) == Some(node);
    if focused && current.as_ref() != value {
        return;
    }
    bound.set(value.to_owned());
}

fn zip_buttons<T>(
    context: &AppContext,
    list: Entity<Stack>,
    items: &[T],
    id_of: impl Fn(&T) -> &str,
) -> HashMap<String, Entity<Button>> {
    let mut cursor = row_buttons(context, list.stable_id()).into_iter();
    let mut rows = HashMap::new();
    let mut seen = HashSet::new();
    for item in items {
        let id = id_of(item);
        if !seen.insert(id.to_owned()) {
            continue;
        }
        let Some(child) = cursor.next() else {
            break;
        };
        rows.insert(id.to_owned(), child);
    }
    rows
}

fn row_buttons(context: &AppContext, list: StableNodeId) -> Vec<Entity<Button>> {
    let children = context
        .world()
        .node(list)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut buttons = Vec::new();
    for child in children {
        if let Some(button) = as_button(context, child) {
            buttons.push(button);
            continue;
        }
        let nested = context
            .world()
            .node(child)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for grandchild in nested {
            if let Some(button) = as_button(context, grandchild) {
                buttons.push(button);
            }
        }
    }
    buttons
}

fn as_button(context: &AppContext, id: StableNodeId) -> Option<Entity<Button>> {
    let button = Entity::from_stable_id(id);
    context.read(button, |_| ()).is_ok().then_some(button)
}

fn zip_menu_items(
    context: &AppContext,
    menu: Entity<ActionMenu>,
    tasks: &[(String, String)],
) -> HashMap<String, Entity<ActionMenuItem>> {
    let Some(branch) = context
        .world()
        .node(menu.stable_id())
        .and_then(|node| node.children.first().copied())
    else {
        return HashMap::new();
    };
    let Some(list) = context
        .world()
        .node(branch)
        .and_then(|node| node.children.first().copied())
    else {
        return HashMap::new();
    };
    let children = context
        .world()
        .node(list)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    let mut items = HashMap::new();
    let mut cursor = children.into_iter();
    let mut seen = HashSet::new();
    for (id, _) in tasks {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(child) = cursor.next() else {
            break;
        };
        items.insert(id.clone(), Entity::from_stable_id(child));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::{TextSelection, ToggleChanged};
    use std::sync::Mutex;

    #[test]
    fn memory_view_updates_owned_rows_and_routes_actions() {
        let mut context = AppContext::new();
        let document = DocumentId::new(601).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink_events = Arc::clone(&events);
        let mut view = MemoryView::mount(
            &mut context,
            document,
            Arc::new(move |event| sink_events.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = MemoryViewSnapshot {
            selected: Some("one".into()),
            title: "Title".into(),
            body: "Body".into(),
            cards: vec![MemoryCard {
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
            matches!(&received[..], [MemoryMessage::Select(id), MemoryMessage::Save] if id == "one")
        );
        drop(received);
        snapshot.cards.clear();
        snapshot.selected = None;
        snapshot.body.clear();
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(!context.world().contains(row.stable_id()));
        assert!(context.read(view.save, |button| button.disabled).unwrap());
        assert!(context.read(view.delete, |button| button.disabled).unwrap());
    }

    #[test]
    fn memory_navigation_restores_selection_focus_and_disposes_parked_nodes() {
        let mut context = AppContext::new();
        let document = DocumentId::new(602).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let mut view = MemoryView::mount(&mut context, document, Arc::new(|_| {})).unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = MemoryViewSnapshot {
            project_id: Some("project-one".into()),
            title: "Title".into(),
            body: "Memory body".into(),
            ..Default::default()
        };
        view.sync(&mut context, document, &snapshot).unwrap();
        let body = view.fields[1];
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
            view.fields[2].stable_id(),
            view.list.stable_id(),
        ];
        view.suspend(&mut context).unwrap();
        view.dispose(&mut context).unwrap();
        for node in nodes {
            assert!(!context.world().contains(node));
        }
    }

    #[test]
    fn memory_view_routes_baseline_and_task_injection_switches() {
        let mut context = AppContext::new();
        let document = DocumentId::new(603).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink_events = Arc::clone(&events);
        let mut view = MemoryView::mount(
            &mut context,
            document,
            Arc::new(move |event| sink_events.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = MemoryViewSnapshot {
            global_enabled: true,
            baseline_enabled: true,
            task_injection: Some(true),
            ..Default::default()
        };
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(!context
            .read(view.baseline, |toggle| toggle.disabled)
            .unwrap());
        assert!(!context
            .read(view.task_injection, |toggle| toggle.disabled)
            .unwrap());
        assert!(!context.read(view.reset, |button| button.disabled).unwrap());
        context
            .update_component(view.baseline, |_, cx| {
                cx.emit(ToggleChanged { checked: false })
            })
            .unwrap();
        context
            .update_component(view.task_injection, |_, cx| {
                cx.emit(ToggleChanged { checked: false })
            })
            .unwrap();
        snapshot.global_enabled = false;
        snapshot.task_injection = None;
        view.sync(&mut context, document, &snapshot).unwrap();
        assert!(context
            .read(view.baseline, |toggle| toggle.disabled)
            .unwrap());
        assert!(context
            .read(view.task_injection, |toggle| toggle.disabled)
            .unwrap());
        assert!(context.read(view.reset, |button| button.disabled).unwrap());
        let received = events.lock().unwrap();
        assert!(matches!(
            &received[..],
            [
                MemoryMessage::ToggleBaseline,
                MemoryMessage::ToggleTaskInjection
            ]
        ));
        let nodes = view.debug_nodes();
        assert!(nodes.iter().any(|(id, _)| id == "switch.memory-baseline"));
        assert!(nodes
            .iter()
            .any(|(id, _)| id == "switch.memory-task-enabled"));
    }

    #[test]
    fn memory_title_keeps_normal_undo_across_a_stale_snapshot() {
        let mut context = AppContext::new();
        let document = DocumentId::new(604).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink_events = Arc::clone(&events);
        let mut view = MemoryView::mount(
            &mut context,
            document,
            Arc::new(move |event| sink_events.lock().unwrap().push(event)),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = MemoryViewSnapshot {
            title: "正常入口多行记忆".into(),
            body: "正文".into(),
            ..Default::default()
        };
        view.sync(&mut context, document, &snapshot).unwrap();
        let title = view.fields[0];
        context.focus_node(document, title.stable_id()).unwrap();
        context.select_all_focused_text(document).unwrap();
        assert!(context
            .replace_focused_text(document, "撤销重做临时标题")
            .unwrap());
        view.sync(&mut context, document, &snapshot).unwrap();
        let mut input = crate::runtime_input::ScriptedInput::bind(&mut context, document);
        let outcome = input
            .press_key(
                &mut context,
                crate::agent_debug::retained_key_event("Control+z"),
            )
            .unwrap();
        assert!(outcome.prevent_default);
        assert_eq!(
            context.read(title, |area| area.state.value.to_string()).unwrap(),
            "正常入口多行记忆"
        );
        assert!(events.lock().unwrap().iter().any(|event| {
            matches!(event, MemoryMessage::TitleChanged(value) if value == "正常入口多行记忆")
        }));

        snapshot.selected = Some("other".into());
        snapshot.title = "另一条记忆".into();
        view.sync(&mut context, document, &snapshot).unwrap();
        let mut input = crate::runtime_input::ScriptedInput::bind(&mut context, document);
        let outcome = input
            .press_key(
                &mut context,
                crate::agent_debug::retained_key_event("Control+z"),
            )
            .unwrap();
        assert!(!outcome.prevent_default);
        assert_eq!(
            context.read(title, |area| area.state.value.to_string()).unwrap(),
            "另一条记忆"
        );
    }
}
