use std::sync::Arc;

use nana_ui::runtime::{
    Activate, AppContext, Button, DocumentId, Entity, FrameworkError, LengthSpec, ScrollAxes,
    ScrollView, Stack, Text, TextArea, TextChanged,
};

use crate::application::{
    DesktopGoalSnapshot, DesktopGoalStatus, DesktopTaskTodo, DesktopTodoGuideStatus,
    DesktopTodoPriority, DesktopTodoSource,
};
use crate::runtime_compat::HostedWindowId;
use crate::runtime_shell::ShellIntent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoEditor {
    Guide,
    Goal,
}

#[derive(Debug, Clone, Default)]
pub struct TodoEditState {
    pub task_id: Option<lilia_contracts::TaskId>,
    pub editor: Option<TodoEditor>,
    pub draft: String,
    pub editing: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TodoAction {
    NewGuide,
    Edit(String),
    TextChanged(String),
    Save,
    Cancel,
    Priority(String),
    Delete(String),
    Dispatch(String),
    EditGoal,
    RefreshGoal,
    ClearGoal,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TodoPanelSnapshot {
    pub visible: bool,
    pub locked: bool,
    pub todos: Vec<DesktopTaskTodo>,
    pub goal: Option<DesktopGoalSnapshot>,
    pub editor: Option<TodoEditor>,
    pub draft: String,
    pub error: Option<String>,
}

pub fn visible_todo(todo: &DesktopTaskTodo) -> bool {
    match todo.source {
        DesktopTodoSource::Agent => !todo.done,
        DesktopTodoSource::Lilia => {
            todo.guide_status.is_some() && todo.guide_status != Some(DesktopTodoGuideStatus::Sent)
        }
    }
}

pub fn editable_guide(todo: &DesktopTaskTodo) -> bool {
    todo.source == DesktopTodoSource::Lilia
        && todo.guide_status == Some(DesktopTodoGuideStatus::Pending)
}

pub struct TodoPanel {
    pub root: Entity<Stack>,
    toolbar: Entity<Stack>,
    rows: Entity<Stack>,
    scroll: Entity<ScrollView>,
    editor: Entity<Stack>,
    pub input: Entity<TextArea>,
    error: Entity<Text>,
    pub controls: Vec<(String, Entity<Button>)>,
    row_nodes: Vec<Entity<Stack>>,
    row_snapshot: Option<(Vec<DesktopTaskTodo>, Option<DesktopGoalSnapshot>, bool)>,
    sink: Arc<dyn Fn(ShellIntent) + Send + Sync>,
    window_id: HostedWindowId,
}

impl TodoPanel {
    pub fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Arc<dyn Fn(ShellIntent) + Send + Sync>,
        window_id: HostedWindowId,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, Stack::column(4.0))?;
        let toolbar = context.create_detached_component(document, Stack::bar(6.0))?;
        let rows = context.create_detached_component(document, Stack::column(4.0))?;
        let scroll = context.create_detached_component(
            document,
            ScrollView::new(ScrollAxes::Vertical).style(
                Stack::column(0.0)
                    .with_layout(|layout| layout.max_height = Some(LengthSpec::Px(144.0)))
                    .shrink(1.0)
                    .node_style(),
            ),
        )?;
        context.append_child(scroll, rows)?;
        let editor = context.create_detached_component(document, Stack::column(4.0))?;
        let input = context.create_detached_component(document, TextArea::new("").height(64.0))?;
        let input_sink = sink.clone();
        context.on(input, move |_, event: &TextChanged, _| {
            input_sink(ShellIntent::Todo {
                window_id,
                action: TodoAction::TextChanged(event.value.clone()),
            })
        })?;
        context.append_child(editor, input)?;
        let actions = context.create_detached_component(document, Stack::bar(6.0))?;
        context.append_child(editor, actions)?;
        let error = context.create_detached_component(document, Text::new(""))?;
        let mut panel = Self {
            root,
            toolbar,
            rows,
            scroll,
            editor,
            input,
            error,
            controls: Vec::new(),
            row_nodes: Vec::new(),
            row_snapshot: None,
            sink,
            window_id,
        };
        panel.button(
            context,
            document,
            toolbar,
            "new",
            "添加引导",
            TodoAction::NewGuide,
            true,
        )?;
        panel.button(
            context,
            document,
            toolbar,
            "goal-edit",
            "设置目标",
            TodoAction::EditGoal,
            true,
        )?;
        panel.button(
            context,
            document,
            actions,
            "save",
            "保存",
            TodoAction::Save,
            false,
        )?;
        panel.button(
            context,
            document,
            actions,
            "cancel",
            "取消",
            TodoAction::Cancel,
            true,
        )?;
        Ok(panel)
    }

    fn button(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        parent: Entity<Stack>,
        id: &str,
        label: &str,
        action: TodoAction,
        enabled: bool,
    ) -> Result<(), FrameworkError> {
        let button = context.create_detached_component(
            document,
            Button::new(label).disabled(!enabled).layout(
                Stack::row(0.0)
                    .width(LengthSpec::Px(if label.chars().count() > 3 {
                        76.0
                    } else {
                        52.0
                    }))
                    .height(LengthSpec::Px(28.0))
                    .node_style()
                    .layout,
            ),
        )?;
        context.append_child(parent, button)?;
        let sink = self.sink.clone();
        let window_id = self.window_id;
        context.on(button, move |_, _: &Activate, _| {
            sink(ShellIntent::Todo {
                window_id,
                action: action.clone(),
            })
        })?;
        self.controls.push((id.to_owned(), button));
        Ok(())
    }

    pub fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        state: &TodoPanelSnapshot,
    ) -> Result<(), FrameworkError> {
        let projection = (state.todos.clone(), state.goal.clone(), state.locked);
        if self.row_snapshot.as_ref() != Some(&projection) {
            for row in self.row_nodes.drain(..) {
                context.remove_view(row)?;
            }
            self.controls
                .retain(|(id, _)| matches!(id.as_str(), "new" | "goal-edit" | "save" | "cancel"));
            if let Some(goal) = &state.goal {
                let row = context.create_detached_component(document, Stack::column(3.0))?;
                let label = context.create_detached_component(
                    document,
                    Text::new(format!("目标 · {}", goal.objective)),
                )?;
                context.append_child(row, label)?;
                let status = match goal.status {
                    DesktopGoalStatus::Active => "进行中",
                    DesktopGoalStatus::Paused => "已暂停",
                    DesktopGoalStatus::Blocked => "受阻",
                    DesktopGoalStatus::UsageLimited => "用量受限",
                    DesktopGoalStatus::BudgetLimited => "预算已用完",
                    DesktopGoalStatus::Complete => "已完成",
                };
                let budget = goal
                    .token_budget
                    .map(|value| format!("/{value}"))
                    .unwrap_or_default();
                let meta = context.create_detached_component(
                    document,
                    Text::new(format!("{status} · {}{budget} tokens", goal.tokens_used)),
                )?;
                context.append_child(row, meta)?;
                let actions = context.create_detached_component(document, Stack::bar(6.0))?;
                context.append_child(row, actions)?;
                self.button(
                    context,
                    document,
                    actions,
                    "goal-refresh",
                    "刷新",
                    TodoAction::RefreshGoal,
                    !state.locked,
                )?;
                self.button(
                    context,
                    document,
                    actions,
                    "goal-clear",
                    "清除",
                    TodoAction::ClearGoal,
                    !state.locked,
                )?;
                self.row_nodes.push(row);
            }
            for todo in state.todos.iter().filter(|todo| visible_todo(todo)) {
                let row = context.create_detached_component(document, Stack::column(2.0))?;
                let priority = match todo.priority {
                    DesktopTodoPriority::High => "高优先级",
                    DesktopTodoPriority::Normal => "普通",
                    DesktopTodoPriority::Low => "低优先级",
                };
                let source = if todo.source == DesktopTodoSource::Agent {
                    "待办"
                } else if todo.guide_status == Some(DesktopTodoGuideStatus::Queued) {
                    "已排队"
                } else {
                    "待发送引导"
                };
                let label = context.create_detached_component(
                    document,
                    Text::new(format!("{source} · {priority} · {}", todo.text)),
                )?;
                context.append_child(row, label)?;
                if todo.source == DesktopTodoSource::Lilia {
                    let actions = context.create_detached_component(document, Stack::bar(4.0))?;
                    context.append_child(row, actions)?;
                    let enabled = editable_guide(todo) && !state.locked;
                    for (key, label, action) in [
                        (
                            "dispatch",
                            "立即插入",
                            TodoAction::Dispatch(todo.id.clone()),
                        ),
                        ("edit", "编辑", TodoAction::Edit(todo.id.clone())),
                        ("priority", "优先级", TodoAction::Priority(todo.id.clone())),
                        ("delete", "删除", TodoAction::Delete(todo.id.clone())),
                    ] {
                        self.button(
                            context,
                            document,
                            actions,
                            &format!("{}-{key}", todo.id),
                            label,
                            action,
                            enabled,
                        )?;
                    }
                }
                self.row_nodes.push(row);
            }
            let order = self
                .row_nodes
                .iter()
                .map(|row| row.stable_id())
                .collect::<Vec<_>>();
            context.reconcile_children(self.rows.stable_id(), &order)?;
            self.row_snapshot = Some(projection);
        }
        context.update_component(self.input, |input, _| {
            if input.state.value != state.draft {
                input.state.replace_value(state.draft.clone());
            }
            input.disabled = state.locked;
        })?;
        for (id, button) in self.controls.iter().take(4) {
            context.update_component(*button, |button, _| {
                button.disabled = state.locked || (id == "save" && state.draft.trim().is_empty())
            })?;
        }
        context.update_component(self.error, |text, _| {
            text.value = state.error.clone().unwrap_or_default()
        })?;
        let mut order = vec![self.toolbar.stable_id()];
        if !self.row_nodes.is_empty() {
            order.push(self.scroll.stable_id());
        }
        if state.editor.is_some() {
            order.push(self.editor.stable_id());
        }
        if state.error.is_some() {
            order.push(self.error.stable_id());
        }
        context
            .reconcile_children(self.root.stable_id(), &order)
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn guide(id: &str, status: DesktopTodoGuideStatus) -> DesktopTaskTodo {
        DesktopTaskTodo {
            id: id.into(),
            task_id: lilia_contracts::TaskId::new("task").unwrap(),
            text: "检查结果".into(),
            done: false,
            order: 0,
            source: DesktopTodoSource::Lilia,
            priority: DesktopTodoPriority::High,
            guide_status: Some(status),
            attachments: Vec::new(),
            conversation_references: Vec::new(),
            workflow: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn mounted_controls_route_to_their_window_and_queued_guides_cannot_mutate() {
        let document_id = DocumentId::new(913).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, Stack::column(4.0))
            .unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let events = observed.clone();
        let mut panel = TodoPanel::mount(
            context,
            document_id,
            Arc::new(move |event| events.lock().unwrap().push(event)),
            nana_ui_platform::WindowId(71),
        )
        .unwrap();
        context.append_child(root, panel.root).unwrap();
        let mut snapshot = TodoPanelSnapshot {
            visible: true,
            todos: vec![
                guide("pending", DesktopTodoGuideStatus::Pending),
                guide("queued", DesktopTodoGuideStatus::Queued),
            ],
            ..Default::default()
        };
        panel.sync(context, document_id, &snapshot).unwrap();
        let button = |panel: &TodoPanel, id: &str| {
            panel
                .controls
                .iter()
                .find(|(key, _)| key == id)
                .unwrap()
                .1
                .stable_id()
        };
        assert!(context
            .activate_node(button(&panel, "pending-dispatch"))
            .unwrap());
        assert!(!context
            .activate_node(button(&panel, "queued-delete"))
            .unwrap());
        assert!(
            matches!(&observed.lock().unwrap()[0], ShellIntent::Todo { window_id: nana_ui_platform::WindowId(71), action: TodoAction::Dispatch(id) } if id == "pending")
        );
        let editor_node = panel.input.stable_id();
        snapshot.editor = Some(TodoEditor::Guide);
        snapshot.draft = "用户的新草稿\n第二行".into();
        snapshot.todos[0].text = "更新后的引导".into();
        panel.sync(context, document_id, &snapshot).unwrap();
        assert_eq!(panel.input.stable_id(), editor_node);
        assert_eq!(
            context
                .read(panel.input, |input| input.state.value.clone())
                .unwrap(),
            snapshot.draft
        );
        snapshot.todos[0].guide_status = Some(DesktopTodoGuideStatus::Sent);
        panel.sync(context, document_id, &snapshot).unwrap();
        assert!(!panel
            .controls
            .iter()
            .any(|(id, _)| id == "pending-dispatch"));
    }
}
