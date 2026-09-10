//! The task list and inspector drafts as a UI module.
//!
//! Selection still belongs to the window's workspace session. This module owns
//! the search / title / drop drafts the inspector edits, and projects the task
//! list from the session rather than from a shell mirror.

use lilia_contracts::TaskId;
use lilia_kernel::FeatureId;

use crate::runtime_shell::{PrimaryShellSnapshot, ShellTaskRow};
use crate::shell::TaskMessage;
use crate::ui_module::{UiModule, UiModuleContext, UiModuleOutcome};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskMoveTarget {
    Project(lilia_contracts::ProjectId),
    Inbox,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskParentTarget {
    Root,
    Task(TaskId),
}

#[derive(Debug, Clone)]
pub enum TaskModuleMessage {
    Ui(TaskMessage),
}

pub struct TaskModule {
    task_search: String,
    session_page: usize,
    new_task_title: String,
    task_title_edit: String,
    task_drop_search: String,
    task_move_target: Option<TaskMoveTarget>,
    task_parent_target: Option<TaskParentTarget>,
    task_dependency_target: Option<TaskId>,
}

impl Default for TaskModule {
    fn default() -> Self {
        Self {
            task_search: String::new(),
            session_page: 0,
            new_task_title: String::new(),
            task_title_edit: String::new(),
            task_drop_search: String::new(),
            task_move_target: None,
            task_parent_target: None,
            task_dependency_target: None,
        }
    }
}

impl TaskModule {
    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.task").expect("the task feature id is not blank")
    }

    pub fn task_search(&self) -> &str {
        &self.task_search
    }

    pub fn new_task_title(&self) -> &str {
        &self.new_task_title
    }

    pub fn task_title_edit(&self) -> &str {
        &self.task_title_edit
    }

    pub fn task_drop_search(&self) -> &str {
        &self.task_drop_search
    }

    pub fn task_move_target(&self) -> Option<&TaskMoveTarget> {
        self.task_move_target.as_ref()
    }

    pub fn set_task_move_target(&mut self, target: Option<TaskMoveTarget>) {
        self.task_move_target = target;
    }

    pub fn task_parent_target(&self) -> Option<&TaskParentTarget> {
        self.task_parent_target.as_ref()
    }

    pub fn set_task_parent_target(&mut self, target: Option<TaskParentTarget>) {
        self.task_parent_target = target;
    }

    pub fn task_dependency_target(&self) -> Option<&TaskId> {
        self.task_dependency_target.as_ref()
    }

    pub fn set_task_dependency_target(&mut self, target: Option<TaskId>) {
        self.task_dependency_target = target;
    }

    pub fn set_task_title_edit(&mut self, title: String) {
        self.task_title_edit = title;
    }

    pub fn clear_new_task_title(&mut self) {
        self.new_task_title.clear();
    }

    pub fn clear_list_drafts(&mut self) {
        self.task_search.clear();
        self.session_page = 0;
        self.new_task_title.clear();
    }

    pub fn clear_inspector_targets(&mut self) {
        self.task_move_target = None;
        self.task_parent_target = None;
        self.task_dependency_target = None;
        self.task_drop_search.clear();
    }

    fn project_sessions(&self, into: &mut PrimaryShellSnapshot) {
        let query = self.task_search.trim().to_lowercase();
        let matches = into
            .tasks
            .iter()
            .filter(|task| query.is_empty() || task.title.to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        into.session_search = self.task_search.clone();
        into.session_page_count = matches.len().div_ceil(20).max(1);
        into.session_page = self.session_page.min(into.session_page_count - 1);
        into.project_page_body = format!(
            "{} 个会话 · 第 {} / {} 页",
            matches.len(),
            into.session_page + 1,
            into.session_page_count
        );
        into.session_cards = matches
            .into_iter()
            .skip(into.session_page * 20)
            .take(20)
            .collect();
    }

    fn reduce_ui(&mut self, message: TaskMessage) -> UiModuleOutcome {
        match message {
            TaskMessage::SessionPageChanged(offset) => {
                self.session_page = self.session_page.saturating_add_signed(offset);
                UiModuleOutcome::dirty()
            }
            TaskMessage::TaskSearchChanged(value) => {
                self.session_page = 0;
                self.task_search = value;
                UiModuleOutcome::dirty()
            }
            TaskMessage::NewTaskTitleChanged(value) => {
                self.new_task_title = value;
                UiModuleOutcome::dirty()
            }
            TaskMessage::TaskTitleChanged(value) => {
                self.task_title_edit = value;
                UiModuleOutcome::dirty()
            }
            TaskMessage::TaskDropSearchChanged(value) => {
                self.task_drop_search = value;
                UiModuleOutcome::dirty()
            }
            _ => UiModuleOutcome::clean(),
        }
    }
}

impl UiModule for TaskModule {
    type Message = TaskModuleMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        if matches!(
            &message,
            TaskModuleMessage::Ui(TaskMessage::SessionPageChanged(_))
        ) {
            let query = self.task_search.trim().to_lowercase();
            let count = cx
                .workspace()
                .and_then(|workspace| workspace.snapshot().ok())
                .map(|snapshot| {
                    snapshot
                        .tasks
                        .iter()
                        .filter(|task| {
                            query.is_empty() || task.title.to_lowercase().contains(&query)
                        })
                        .count()
                })
                .unwrap_or(0);
            self.session_page = self.session_page.min(count.div_ceil(20).max(1) - 1);
        }
        match message {
            TaskModuleMessage::Ui(message) => self.reduce_ui(message),
        }
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        _cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        if envelope.is::<crate::application::TasksChanged>()
            || envelope.is::<crate::application::ProjectsChanged>()
        {
            return UiModuleOutcome::dirty();
        }
        UiModuleOutcome::clean()
    }

    fn project(&self, cx: &UiModuleContext<'_>, into: &mut PrimaryShellSnapshot) {
        if cx.shows_surface(crate::application::ApplicationWorkspaceSurface::Settings)
            || cx.shows_surface(crate::application::ApplicationWorkspaceSurface::Automations)
            || cx.shows_surface(crate::application::ApplicationWorkspaceSurface::Projects)
        {
            return;
        }
        let Some(snapshot) = cx.workspace().and_then(|session| session.snapshot().ok()) else {
            into.tasks.clear();
            return;
        };
        let selected = snapshot.selected_task;
        into.tasks = snapshot
            .tasks
            .into_iter()
            .map(|task| ShellTaskRow {
                selected: selected.as_ref() == Some(&task.id),
                id: task.id.clone(),
                title: if task.title.trim().is_empty() {
                    "未命名会话".to_owned()
                } else {
                    task.title
                },
            })
            .collect();
        if cx.shows(crate::runtime_shell::ShellProjectPage::Sessions) {
            self.project_sessions(into);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_pagination_keeps_late_tasks_searchable_and_resets_search_page() {
        let mut module = TaskModule::default();
        let mut snapshot = crate::runtime_shell::empty_snapshot();
        snapshot.tasks = (0..105)
            .map(|index| ShellTaskRow {
                id: TaskId::new(format!("task-{index}")).unwrap(),
                title: format!("会话 {index}"),
                selected: false,
            })
            .collect();
        module.reduce_ui(TaskMessage::SessionPageChanged(4));
        module.project_sessions(&mut snapshot);
        assert_eq!(
            snapshot.session_cards.first().unwrap().id.as_str(),
            "task-80"
        );
        assert_eq!(snapshot.session_page_count, 6);
        module.reduce_ui(TaskMessage::SessionPageChanged(1));
        module.project_sessions(&mut snapshot);
        assert_eq!(snapshot.session_cards.len(), 5);
        assert_eq!(
            snapshot.session_cards.last().unwrap().id.as_str(),
            "task-104"
        );
        module.reduce_ui(TaskMessage::TaskSearchChanged("会话 104".to_owned()));
        module.project_sessions(&mut snapshot);
        assert_eq!(snapshot.session_page, 0);
        assert_eq!(snapshot.session_cards.len(), 1);
        assert_eq!(snapshot.session_cards[0].id.as_str(), "task-104");
        module.reduce_ui(TaskMessage::TaskSearchChanged("不存在".to_owned()));
        module.project_sessions(&mut snapshot);
        assert!(snapshot.session_cards.is_empty());
        assert_eq!(snapshot.session_page_count, 1);
    }
}
