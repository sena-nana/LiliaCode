use super::MemoryMessage;
use crate::runtime_layout::reconcile_children;
use nana_ui::{ButtonKind, PopoverPlacement};
use nana_ui::runtime::{
    ActionMenu, ActionMenuItem, Activate, AppContext, Button, DocumentId, Entity, FormField,
    FrameworkError, MutationQueue, PopoverToggled, ScrollAxes, ScrollView, StableNodeId, Stack,
    Switch, Text, TextArea, TextChanged, TextInput, ToggleChanged,
};
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
    scope: Entity<Button>,
    save: Entity<Button>,
    delete: Entity<Button>,
    reset: Entity<Button>,
    task_menu: Entity<ActionMenu>,
    task_items: HashMap<String, Entity<ActionMenuItem>>,
    list: Entity<Stack>,
    rows: HashMap<String, MemoryRow>,
    sink: Sink,
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
            (
                "composer.memory-task-reset".into(),
                self.reset.stable_id(),
            ),
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
        let root = context.create_detached_component(document, Stack::fill_column(12.0))?;
        let header = context.create_detached_component(document, Stack::bar(8.0))?;
        let title = context.create_detached_component(document, Text::new("记忆"))?;
        let create = button(
            context,
            document,
            "新建",
            ButtonKind::Primary,
            MemoryMessage::New,
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
                MemoryMessage::TitleChanged as fn(String) -> MemoryMessage,
            ),
            (
                "正文",
                144.0,
                MemoryMessage::BodyReplaced as fn(String) -> MemoryMessage,
            ),
            (
                "标签",
                40.0,
                MemoryMessage::TagsChanged as fn(String) -> MemoryMessage,
            ),
        ] {
            let mut area = TextArea::new("").height(height);
            if label == "正文" {
                area = area.resize_vertical(true);
            }
            let input = context.create_detached_component(document, area)?;
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
        let scope = button(
            context,
            document,
            "项目",
            ButtonKind::Subtle,
            MemoryMessage::ToggleScope,
            &sink,
        )?;
        let save = button(
            context,
            document,
            "保存",
            ButtonKind::Primary,
            MemoryMessage::Save,
            &sink,
        )?;
        let delete = button(
            context,
            document,
            "删除",
            ButtonKind::Danger,
            MemoryMessage::Delete,
            &sink,
        )?;
        let reset = button(
            context,
            document,
            "重置冷却",
            ButtonKind::Subtle,
            MemoryMessage::ResetTaskCooldown,
            &sink,
        )?;
        let task_menu = context.create_detached_component(
            document,
            ActionMenu::new()
                .trigger("注入任务".to_owned())
                .placement(PopoverPlacement::Bottom)
                .open(false),
        )?;
        let menu_sink = Arc::clone(&sink);
        context.on(task_menu, move |_, _: &PopoverToggled, _| {
            menu_sink(MemoryMessage::ToggleTaskMenu)
        })?;
        let cooldown = context.create_detached_component(document, TextInput::new(""))?;
        let cooldown_field = context.create_detached_component(
            document,
            FormField::new("冷却轮数").control_child(cooldown.stable_id()),
        )?;
        context.append_child(cooldown_field, cooldown)?;
        let cooldown_sink = Arc::clone(&sink);
        context.on(cooldown, move |_, event: &TextChanged, _| {
            cooldown_sink(MemoryMessage::CooldownChanged(event.value.clone()))
        })?;
        let enabled = context.create_detached_component(document, Switch::new("启用记忆", true))?;
        let enabled_sink = Arc::clone(&sink);
        context.on(enabled, move |_, _: &ToggleChanged, _| {
            enabled_sink(MemoryMessage::ToggleEnabled)
        })?;
        let global = context.create_detached_component(document, Switch::new("全局注入", true))?;
        let global_sink = Arc::clone(&sink);
        context.on(global, move |_, _: &ToggleChanged, _| {
            global_sink(MemoryMessage::ToggleGlobal)
        })?;
        let baseline = context.create_detached_component(document, Switch::new("基线注入", true))?;
        let baseline_sink = Arc::clone(&sink);
        context.on(baseline, move |_, _: &ToggleChanged, _| {
            baseline_sink(MemoryMessage::ToggleBaseline)
        })?;
        let task_injection =
            context.create_detached_component(document, Switch::new("为此会话注入记忆", true))?;
        let task_sink = Arc::clone(&sink);
        context.on(task_injection, move |_, _: &ToggleChanged, _| {
            task_sink(MemoryMessage::ToggleTaskInjection)
        })?;
        for button in [scope, save, delete, reset] {
            context.append_child(actions, button)?;
        }
        context.append_child(actions, task_menu)?;
        context.append_child(root, actions)?;
        let settings = context.create_detached_component(document, Stack::bar(8.0).wrap(true))?;
        context.append_child(settings, enabled)?;
        context.append_child(settings, global)?;
        context.append_child(settings, baseline)?;
        context.append_child(settings, task_injection)?;
        context.append_child(settings, cooldown_field)?;
        context.append_child(root, settings)?;
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
            cooldown,
            enabled,
            global,
            baseline,
            task_injection,
            scope,
            save,
            delete,
            reset,
            task_menu,
            task_items: HashMap::new(),
            list,
            rows: HashMap::new(),
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
        snapshot: &MemoryViewSnapshot,
    ) -> Result<(), FrameworkError> {
        let changed_selection = self.selected != snapshot.selected;
        for (input, value) in
            self.fields
                .into_iter()
                .zip([&snapshot.title, &snapshot.body, &snapshot.tags])
        {
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
        context.update_component(self.scope, |button, _| {
            button.label = snapshot.scope_label.clone()
        })?;
        context.update_component(self.cooldown, |input, _| {
            if changed_selection || input.state.value != snapshot.cooldown {
                input.state.replace_value(snapshot.cooldown.clone());
            }
        })?;
        context.update_component(self.enabled, |toggle, _| {
            *toggle = Switch::new("启用记忆", snapshot.enabled);
            toggle.disabled = snapshot.selected.is_none();
        })?;
        context.update_component(self.global, |toggle, _| {
            *toggle = Switch::new("全局注入", snapshot.global_enabled);
        })?;
        context.update_component(self.baseline, |toggle, _| {
            *toggle = Switch::new("基线注入", snapshot.baseline_enabled);
            toggle.disabled = !snapshot.global_enabled;
        })?;
        context.update_component(self.task_injection, |toggle, _| {
            *toggle = Switch::new(
                "为此会话注入记忆",
                snapshot.task_injection.unwrap_or(false),
            );
            toggle.disabled = snapshot.task_injection.is_none();
        })?;
        context.update_component(self.reset, |button, _| {
            button.disabled = snapshot.task_injection.is_none()
        })?;
        context.update_component(self.save, |button, _| {
            button.disabled = snapshot.title.trim().is_empty() || snapshot.body.trim().is_empty()
        })?;
        context.update_component(self.delete, |button, _| {
            button.disabled = snapshot.selected.is_none()
        })?;
        context.update_component(self.task_menu, |menu, _| {
            *menu = ActionMenu::new()
                .trigger("注入任务".to_owned())
                .placement(PopoverPlacement::Bottom)
                .open(snapshot.task_menu_open);
        })?;
        let mut menu_order = Vec::new();
        if snapshot.task_menu_open {
            for (id, label) in &snapshot.tasks {
                let item = if let Some(item) = self.task_items.get(id).copied() {
                    context.update_component(item, |item, _| {
                        *item = ActionMenuItem::new(label.clone());
                    })?;
                    item
                } else {
                    let item = context.create_detached_component(
                        document,
                        ActionMenuItem::new(label.clone()),
                    )?;
                    let sink = Arc::clone(&self.sink);
                    let task_id = id.clone();
                    context.on(item, move |_, _: &Activate, _| {
                        sink(MemoryMessage::SelectInjectionTask(task_id.clone()))
                    })?;
                    self.task_items.insert(id.clone(), item);
                    item
                };
                menu_order.push(item.stable_id());
            }
        }
        let keep_tasks = snapshot
            .tasks
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<HashSet<_>>();
        let stale_tasks = self
            .task_items
            .keys()
            .filter(|id| !snapshot.task_menu_open || !keep_tasks.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for id in stale_tasks {
            if let Some(item) = self.task_items.remove(&id) {
                context.remove_view(item)?;
            }
        }
        reconcile_children(context, self.task_menu.stable_id(), &menu_order)?;
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
                    sink(MemoryMessage::Select(id.clone()))
                })?;
                self.rows.insert(card.id.clone(), MemoryRow { root });
            }
            let row = &self.rows[&card.id];
            context.update_component(row.root, |button, _| {
                button.label = format!("{} · {}", card.title, card.subtitle);
            })?;
            order.push(row.root.stable_id());
        }
        reconcile_children(context, self.list.stable_id(), &order)?;
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

fn button(
    context: &mut AppContext,
    document: DocumentId,
    label: &str,
    kind: ButtonKind,
    message: MemoryMessage,
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
            [MemoryMessage::ToggleBaseline, MemoryMessage::ToggleTaskInjection]
        ));
        let nodes = view.debug_nodes();
        assert!(nodes.iter().any(|(id, _)| id == "switch.memory-baseline"));
        assert!(nodes
            .iter()
            .any(|(id, _)| id == "switch.memory-task-enabled"));
    }
}
