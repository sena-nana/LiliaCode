use lilia_contracts::{ChatAttachment, TaskId};

use crate::application::composer::DesktopComposerTurnRequest;
use crate::application::{DesktopApplication, DesktopApplicationError, TodosChanged};

pub use lilia_feature_agent_session::{
    DesktopGuideDispatchResult, DesktopGuideDispatchWindow, DesktopTaskTodo, DesktopTodoCreate,
    DesktopTodoError, DesktopTodoGuideStatus, DesktopTodoPriority, DesktopTodoSource,
    DesktopTodoStore, DesktopTodoUpdate, guide_message, merge_todos_with_latest_projection,
};

use lilia_kernel::{
    EventBus, Feature, FeatureContext, FeatureId, KernelError, ServiceKey, ServiceRef,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DesktopTodoService {
    store: Arc<DesktopTodoStore>,
    projects: lilia_feature_task::ProjectTaskService,
    authority: lilia_service::ServiceAuthority,
    events: EventBus,
    guide_dispatch: Arc<Mutex<()>>,
}
impl DesktopTodoService {
    pub(crate) fn new(
        store: DesktopTodoStore,
        projects: lilia_feature_task::ProjectTaskService,
        authority: lilia_service::ServiceAuthority,
        events: EventBus,
    ) -> Self {
        Self {
            store: Arc::new(store),
            projects,
            authority,
            events,
            guide_dispatch: Arc::new(Mutex::new(())),
        }
    }
}
pub struct TodoServiceKey;
impl ServiceKey for TodoServiceKey {
    type Value = DesktopTodoService;
    const NAME: &'static str = "lilia.todo.service";
}
pub struct TodoServiceFeature {
    service: DesktopTodoService,
}
impl TodoServiceFeature {
    pub fn new(service: DesktopTodoService) -> Self {
        Self { service }
    }
}
impl Feature for TodoServiceFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.todo-operations").expect("nonempty feature id")
    }
    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<TodoServiceKey>()]
    }
    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<TodoServiceKey>(self.service.clone())
    }
}

impl DesktopTodoService {
    pub fn list_task_todos(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<DesktopTaskTodo>, DesktopApplicationError> {
        self.projects.get_task(task_id)?;
        let stored = self.store.list(task_id)?;
        let projections = self
            .authority
            .shared_runtime()
            .inner()
            .product_todos_for_task(task_id);
        Ok(merge_todos_with_latest_projection(stored, &projections))
    }

    pub fn create_task_todo(
        &self,
        input: DesktopTodoCreate,
    ) -> Result<DesktopTaskTodo, DesktopApplicationError> {
        self.projects.get_task(&input.task_id)?;
        let todo = self.store.create(input)?;
        self.events.publish(TodosChanged {
            task_id: todo.task_id.clone(),
        });
        Ok(todo)
    }

    pub(crate) fn create_task_todo_idempotent(
        &self,
        id: &str,
        input: DesktopTodoCreate,
        source: DesktopTodoSource,
        guide_status: Option<DesktopTodoGuideStatus>,
    ) -> Result<(DesktopTaskTodo, bool), DesktopApplicationError> {
        self.projects.get_task(&input.task_id)?;
        let (todo, inserted) = self
            .store
            .create_idempotent(id, input, source, guide_status)?;
        if inserted {
            self.events.publish(TodosChanged {
                task_id: todo.task_id.clone(),
            });
        }
        Ok((todo, inserted))
    }

    pub fn update_task_todo(
        &self,
        id: &str,
        update: DesktopTodoUpdate,
    ) -> Result<Option<DesktopTaskTodo>, DesktopApplicationError> {
        let Some(task_id) = self.validate_todo_task(id)? else {
            return Ok(None);
        };
        let todo = self.store.update_for_task(id, &task_id, update)?;
        if let Some(todo) = &todo {
            self.events.publish(TodosChanged {
                task_id: todo.task_id.clone(),
            });
        }
        Ok(todo)
    }

    pub fn delete_task_todo(&self, id: &str) -> Result<bool, DesktopApplicationError> {
        let Some(expected_task) = self.validate_todo_task(id)? else {
            return Ok(false);
        };
        let task_id = self.store.delete_for_task(id, &expected_task)?;
        if let Some(task_id) = task_id {
            self.events.publish(TodosChanged { task_id });
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn validate_todo_task(&self, id: &str) -> Result<Option<TaskId>, DesktopApplicationError> {
        let todo = DesktopTodoStore::get_from(&self.store.connection(), id)?;
        if let Some(todo) = todo {
            self.projects.get_task(&todo.task_id)?;
            return Ok(Some(todo.task_id));
        }
        Ok(None)
    }
    pub(crate) fn notify_committed(&self, task_id: TaskId) {
        self.events.publish(TodosChanged { task_id });
    }
    fn dispatch_guide<R>(
        &self,
        task_id: &TaskId,
        guide_id: Option<&str>,
        window: DesktopGuideDispatchWindow,
        dispatch: impl FnOnce(DesktopTaskTodo) -> Result<R, DesktopApplicationError>,
    ) -> Result<Option<R>, DesktopApplicationError> {
        let _dispatch = self.guide_dispatch.try_lock().map_err(|_| {
            DesktopApplicationError::StateUnavailable("guide dispatch already active")
        })?;
        self.projects.get_task(task_id)?;
        let guide = match guide_id {
            Some(id) => self.store.select_pending_guide_by_id(task_id, id)?,
            None => self.store.select_pending_guide(task_id, window)?,
        };
        guide.map(dispatch).transpose()
    }
}

impl DesktopApplication {
    pub fn todo_service(&self) -> DesktopTodoService {
        self.inner.todo_service.clone()
    }
    pub fn list_task_todos(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<DesktopTaskTodo>, DesktopApplicationError> {
        self.inner.todo_service.list_task_todos(task_id)
    }
    pub fn create_task_todo(
        &self,
        input: DesktopTodoCreate,
    ) -> Result<DesktopTaskTodo, DesktopApplicationError> {
        self.inner.todo_service.create_task_todo(input)
    }
    pub fn update_task_todo(
        &self,
        id: &str,
        update: DesktopTodoUpdate,
    ) -> Result<Option<DesktopTaskTodo>, DesktopApplicationError> {
        self.inner.todo_service.update_task_todo(id, update)
    }
    pub fn delete_task_todo(&self, id: &str) -> Result<bool, DesktopApplicationError> {
        self.inner.todo_service.delete_task_todo(id)
    }
    pub(crate) fn set_task_guide_status(
        &self,
        id: &str,
        status: DesktopTodoGuideStatus,
    ) -> Result<Option<DesktopTaskTodo>, DesktopApplicationError> {
        self.update_task_todo(
            id,
            DesktopTodoUpdate {
                guide_status: Some(status),
                ..DesktopTodoUpdate::default()
            },
        )
    }

    pub fn dispatch_next_task_guide(
        &self,
        task_id: &TaskId,
        window: DesktopGuideDispatchWindow,
    ) -> Result<Option<DesktopGuideDispatchResult>, DesktopApplicationError> {
        self.inner
            .todo_service
            .dispatch_guide(task_id, None, window, |guide| {
                self.dispatch_task_guide_value(task_id, guide)
            })
    }

    /// Dispatches the exact pending Guide selected by the user.
    pub fn dispatch_task_guide(
        &self,
        task_id: &TaskId,
        guide_id: &str,
    ) -> Result<Option<DesktopGuideDispatchResult>, DesktopApplicationError> {
        self.inner.todo_service.dispatch_guide(
            task_id,
            Some(guide_id),
            DesktopGuideDispatchWindow::User,
            |guide| self.dispatch_task_guide_value(task_id, guide),
        )
    }

    fn dispatch_task_guide_value(
        &self,
        task_id: &TaskId,
        guide: DesktopTaskTodo,
    ) -> Result<DesktopGuideDispatchResult, DesktopApplicationError> {
        let mut request = self.composer_state(task_id)?.turn_request();
        request.content = guide_message(&guide);
        request.attachments = guide
            .attachments
            .iter()
            .cloned()
            .map(|value| {
                serde_json::from_value::<ChatAttachment>(value).map_err(|error| {
                    DesktopTodoError::InvalidAttachment {
                        guide_id: guide.id.clone(),
                        message: error.to_string(),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        request.conversation_references = guide.conversation_references.clone();
        request.workflow = guide.workflow.clone();
        request.workspace_path = self.task_workspace_path(task_id)?;
        request.guide_id = Some(guide.id.clone());
        let turn = self.start_task_turn(request)?;
        Ok(DesktopGuideDispatchResult { guide, turn })
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    fn service() -> (DesktopTodoService, lilia_kernel::Kernel, TaskId) {
        let id = uuid::Uuid::new_v4().to_string();
        let authority =
            lilia_service::ServiceAuthority::bootstrap_in_memory_named(&id, &id).unwrap();
        let projects = lilia_feature_task::ProjectTaskService::new(
            authority.clone(),
            Arc::new(lilia_feature_task::SilentProjectTaskEvents),
        );
        let task = projects
            .create_task(crate::application::DesktopTaskCreate::new(None, "task"))
            .unwrap()
            .id;
        let kernel = lilia_kernel::Kernel::new();
        let service = DesktopTodoService::new(
            DesktopTodoStore::from_shared(lilia_storage::Db::in_memory().unwrap()).unwrap(),
            projects,
            authority,
            kernel.events().clone(),
        );
        kernel
            .mount(Arc::new(TodoServiceFeature::new(service.clone())))
            .unwrap();
        (service, kernel, task)
    }
    fn input(task_id: TaskId) -> DesktopTodoCreate {
        DesktopTodoCreate {
            task_id,
            text: "guide".into(),
            priority: DesktopTodoPriority::default(),
            attachments: vec![],
            conversation_references: vec![],
            workflow: None,
        }
    }
    #[test]
    fn registered_todo_writes_are_authoritative_and_publish_after_commit_once() {
        let (service, kernel, task) = service();
        let mounted = kernel.service::<TodoServiceKey>().unwrap();
        assert!(Arc::ptr_eq(&service.store, &mounted.store));
        let reader = service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let _subscription = kernel.events().on::<TodosChanged, _>(None, move |event| {
            tx.send(reader.list_task_todos(&event.task_id).unwrap())
                .unwrap();
        });
        assert!(
            mounted
                .create_task_todo(input(TaskId::new("missing").unwrap()))
                .is_err()
        );
        assert!(rx.try_recv().is_err());
        let orphan = service
            .store
            .create(input(TaskId::new("deleted-task").unwrap()))
            .unwrap();
        assert!(
            mounted
                .update_task_todo(
                    &orphan.id,
                    DesktopTodoUpdate {
                        done: Some(true),
                        ..Default::default()
                    }
                )
                .is_err()
        );
        assert!(mounted.delete_task_todo(&orphan.id).is_err());
        assert_eq!(
            DesktopTodoStore::get_from(&service.store.connection(), &orphan.id).unwrap(),
            Some(orphan)
        );
        assert!(rx.try_recv().is_err());
        let request = input(task.clone());
        let (created, inserted) = mounted
            .create_task_todo_idempotent(
                "guide",
                request.clone(),
                DesktopTodoSource::Lilia,
                Some(DesktopTodoGuideStatus::Pending),
            )
            .unwrap();
        assert!(inserted);
        assert_eq!(rx.try_recv().unwrap(), vec![created.clone()]);
        assert!(
            !service
                .create_task_todo_idempotent(
                    "guide",
                    request,
                    DesktopTodoSource::Lilia,
                    Some(DesktopTodoGuideStatus::Pending)
                )
                .unwrap()
                .1
        );
        assert!(rx.try_recv().is_err());
        assert!(
            mounted
                .update_task_todo(
                    &created.id,
                    DesktopTodoUpdate {
                        text: Some(" ".into()),
                        ..Default::default()
                    }
                )
                .is_err()
        );
        assert!(rx.try_recv().is_err());
        assert_eq!(
            service.list_task_todos(&task).unwrap(),
            vec![created.clone()]
        );
        assert!(mounted.delete_task_todo(&created.id).unwrap());
        assert!(rx.try_recv().unwrap().is_empty());
        assert!(!mounted.delete_task_todo(&created.id).unwrap());
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn guide_dispatch_callback_can_update_todos_but_recursive_dispatch_is_rejected() {
        let (service, _, task) = service();
        let guide = service.create_task_todo(input(task.clone())).unwrap();
        let result = service
            .dispatch_guide(
                &task,
                Some(&guide.id),
                DesktopGuideDispatchWindow::User,
                |selected| {
                    assert!(
                        service
                            .dispatch_guide(
                                &task,
                                Some(&selected.id),
                                DesktopGuideDispatchWindow::User,
                                |_| Ok(())
                            )
                            .is_err()
                    );
                    service.update_task_todo(
                        &selected.id,
                        DesktopTodoUpdate {
                            guide_status: Some(DesktopTodoGuideStatus::Queued),
                            ..Default::default()
                        },
                    )
                },
            )
            .unwrap();
        assert!(result.unwrap().is_some());
        assert!(
            service
                .dispatch_guide(
                    &task,
                    Some(&guide.id),
                    DesktopGuideDispatchWindow::User,
                    |_| Ok(())
                )
                .unwrap()
                .is_none()
        );
    }
}
