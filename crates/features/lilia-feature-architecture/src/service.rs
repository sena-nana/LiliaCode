use std::sync::{Arc, Mutex, MutexGuard};

use crate::ArchitectureChanged;
use lilia_contracts::{ProjectId, TaskId};
use lilia_kernel::EventBus;
use lilia_service::ServiceAuthority;
use lilia_storage::Db;

use super::{
    ArchitectureStore, DesktopArchitectureError, ProjectArchitectureApplyInput,
    ProjectArchitectureApplyResult, ProjectArchitectureChangeEvent,
    ProjectArchitectureChangeRecord, ProjectArchitectureGraph, ProjectArchitectureQuarantineRecord,
    ProjectArchitectureRejectInput, ProjectArchitectureRollbackResult, SqliteArchitectureStore,
};

#[derive(Clone)]
pub struct DesktopArchitectureService {
    inner: Arc<DesktopArchitectureServiceInner>,
    authority: ServiceAuthority,
    events: EventBus,
}

struct DesktopArchitectureServiceInner {
    store: Mutex<Box<dyn ArchitectureStore>>,
}

impl DesktopArchitectureService {
    pub fn from_db(db: Db, authority: ServiceAuthority) -> Result<Self, DesktopArchitectureError> {
        Self::from_store(SqliteArchitectureStore::from_db(db)?, authority)
    }

    pub fn in_memory(authority: ServiceAuthority) -> Result<Self, DesktopArchitectureError> {
        Self::from_store(SqliteArchitectureStore::in_memory()?, authority)
    }

    pub fn from_store(
        store: impl ArchitectureStore + 'static,
        authority: ServiceAuthority,
    ) -> Result<Self, DesktopArchitectureError> {
        Ok(Self {
            authority,
            events: EventBus::new(),
            inner: Arc::new(DesktopArchitectureServiceInner {
                store: Mutex::new(Box::new(store)),
            }),
        })
    }

    pub fn with_events(mut self, events: EventBus) -> Self {
        self.events = events;
        self
    }

    fn validate_project(&self, project_id: &str) -> Result<ProjectId, DesktopArchitectureError> {
        let project_id = ProjectId::new(project_id)?;
        self.authority
            .client()?
            .products()
            .get_project(&project_id)?;
        Ok(project_id)
    }

    fn validate_task(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<ProjectId, DesktopArchitectureError> {
        let project_id = self.validate_project(project_id)?;
        let task_id = TaskId::new(task_id)?;
        let task = self.authority.client()?.products().get_task(&task_id)?;
        if task.project_id.as_ref() != Some(&project_id) {
            return Err(DesktopArchitectureError::TaskProjectMismatch {
                project_id,
                task_id,
            });
        }
        Ok(project_id)
    }

    fn changed(&self, project_id: ProjectId, version: i64) {
        self.events.publish(ArchitectureChanged {
            project_id,
            version,
        });
    }

    pub fn graph(
        &self,
        project_id: &str,
    ) -> Result<ProjectArchitectureGraph, DesktopArchitectureError> {
        self.validate_project(project_id)?;
        self.store()?.graph(project_id)
    }

    pub fn list_changes(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<ProjectArchitectureChangeRecord>, DesktopArchitectureError> {
        self.validate_project(project_id)?;
        self.store()?.list_changes(project_id, limit.clamp(1, 200))
    }

    pub fn list_quarantine(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectArchitectureQuarantineRecord>, DesktopArchitectureError> {
        self.validate_project(project_id)?;
        self.store()?.list_quarantine(project_id)
    }

    pub fn apply(
        &self,
        input: ProjectArchitectureApplyInput,
    ) -> Result<ProjectArchitectureApplyResult, DesktopArchitectureError> {
        let project_id = self.validate_task(&input.project_id, &input.task_id)?;
        let result = self.store()?.apply(input)?;
        self.changed(project_id, result.graph.version);
        Ok(result)
    }

    pub fn reject(
        &self,
        input: ProjectArchitectureRejectInput,
    ) -> Result<ProjectArchitectureChangeEvent, DesktopArchitectureError> {
        let project_id = self.validate_task(&input.project_id, &input.task_id)?;
        let event = self.store()?.reject(input)?;
        self.changed(project_id, event.before_version);
        Ok(event)
    }

    pub fn rollback(
        &self,
        project_id: &str,
        task_id: &str,
        backend: super::ArchitectureBackend,
    ) -> Result<ProjectArchitectureRollbackResult, DesktopArchitectureError> {
        let id = self.validate_task(project_id, task_id)?;
        let result = self.store()?.rollback(project_id, task_id, backend)?;
        if result.event.is_some() {
            self.changed(id, result.graph.version);
        }
        Ok(result)
    }

    fn store(
        &self,
    ) -> Result<MutexGuard<'_, Box<dyn ArchitectureStore>>, DesktopArchitectureError> {
        self.inner
            .store
            .lock()
            .map_err(|_| DesktopArchitectureError::StateUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArchitectureBackend, ArchitecturePermission, ProjectArchitectureChange};

    fn setup() -> (DesktopArchitectureService, lilia_kernel::Kernel) {
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            &uuid::Uuid::new_v4().to_string(),
            "lilia-service",
        )
        .unwrap();
        let client = authority.client().unwrap();
        let project = ProjectId::new("project-a").unwrap();
        let other = ProjectId::new("project-b").unwrap();
        client
            .products()
            .create_project(project.clone(), "A")
            .unwrap();
        client
            .products()
            .create_project(other.clone(), "B")
            .unwrap();
        client
            .products()
            .create_task(TaskId::new("task-a").unwrap(), Some(project), "A")
            .unwrap();
        client
            .products()
            .create_task(TaskId::new("task-b").unwrap(), Some(other), "B")
            .unwrap();
        let kernel = lilia_kernel::Kernel::new();
        let service = DesktopArchitectureService::in_memory(authority)
            .unwrap()
            .with_events(kernel.events().clone());
        kernel
            .mount(Arc::new(crate::ArchitectureFeature::new(service.clone())))
            .unwrap();
        (service, kernel)
    }

    fn input() -> ProjectArchitectureApplyInput {
        ProjectArchitectureApplyInput {
            project_id: "project-a".to_owned(),
            task_id: "task-a".to_owned(),
            turn_id: None,
            backend: ArchitectureBackend::NativeAgentkit,
            permission: ArchitecturePermission::Full,
            reason: "Update summary".to_owned(),
            changes: vec![ProjectArchitectureChange::SetSummary {
                summary: "Working graph".to_owned(),
            }],
            request_id: Some("request-a".to_owned()),
            expected_version: Some(0),
        }
    }

    #[test]
    fn mounted_and_direct_mutations_publish_once_with_committed_graph() {
        let (service, kernel) = setup();
        let mounted = kernel.service::<crate::ArchitectureServiceKey>().unwrap();
        let reader = service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let subscription = kernel
            .events()
            .on::<ArchitectureChanged, _>(None, move |event| {
                tx.send((
                    event.clone(),
                    reader.graph(event.project_id.as_str()).unwrap(),
                ))
                .unwrap();
            });
        let applied = service.apply(input()).unwrap();
        let (event, graph) = rx.try_recv().unwrap();
        assert_eq!(event.project_id.as_str(), "project-a");
        assert_eq!(event.version, applied.graph.version);
        assert_eq!(graph, applied.graph);
        assert!(rx.try_recv().is_err());
        let rolled_back = mounted
            .rollback("project-a", "task-a", ArchitectureBackend::NativeAgentkit)
            .unwrap();
        assert!(rolled_back.event.is_some());
        let (event, graph) = rx.try_recv().unwrap();
        assert_eq!(event.version, rolled_back.graph.version);
        assert_eq!(graph, rolled_back.graph);
        assert!(rx.try_recv().is_err());
        assert!(mounted
            .rollback("project-b", "task-b", ArchitectureBackend::NativeAgentkit)
            .unwrap()
            .event
            .is_none());
        assert!(rx.try_recv().is_err());
        kernel.events().unsubscribe(subscription);
    }

    #[test]
    fn rejection_notifies_committed_history_without_advancing_graph_version() {
        let (service, kernel) = setup();
        let reader = service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let subscription = kernel
            .events()
            .on::<ArchitectureChanged, _>(None, move |event| {
                tx.send((
                    event.clone(),
                    reader.list_changes(event.project_id.as_str(), 40).unwrap(),
                ))
                .unwrap();
            });
        let request = input();
        let rejected = service
            .reject(ProjectArchitectureRejectInput {
                project_id: request.project_id,
                task_id: request.task_id,
                turn_id: None,
                backend: request.backend,
                permission: request.permission,
                reason: request.reason,
                changes: request.changes,
                request_id: request.request_id,
                expected_version: request.expected_version,
            })
            .unwrap();
        let (event, history) = rx.try_recv().unwrap();
        assert_eq!(event.version, 0);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event, rejected);
        assert_eq!(
            service.graph("project-a").unwrap(),
            ProjectArchitectureGraph::empty("project-a")
        );
        assert!(rx.try_recv().is_err());
        kernel.events().unsubscribe(subscription);
    }

    #[test]
    fn service_refuses_unknown_projects_and_foreign_tasks_without_side_effects() {
        let (service, kernel) = setup();
        let (tx, rx) = std::sync::mpsc::channel();
        kernel.events().observe(None, move |event| {
            tx.send(event.clone()).unwrap();
        });
        assert!(service.graph("missing").is_err());
        assert!(service.list_changes("missing", 40).is_err());
        assert!(service.list_quarantine("missing").is_err());
        let mut wrong = input();
        wrong.task_id = "task-b".to_owned();
        assert!(matches!(
            service.apply(wrong.clone()),
            Err(DesktopArchitectureError::TaskProjectMismatch { .. })
        ));
        assert!(matches!(
            service.reject(ProjectArchitectureRejectInput {
                project_id: wrong.project_id.clone(),
                task_id: wrong.task_id.clone(),
                turn_id: None,
                backend: wrong.backend,
                permission: wrong.permission,
                reason: wrong.reason,
                changes: wrong.changes,
                request_id: wrong.request_id,
                expected_version: wrong.expected_version,
            }),
            Err(DesktopArchitectureError::TaskProjectMismatch { .. })
        ));
        assert!(matches!(
            service.rollback("project-a", "task-b", ArchitectureBackend::NativeAgentkit),
            Err(DesktopArchitectureError::TaskProjectMismatch { .. })
        ));
        let mut stale = input();
        stale.expected_version = Some(99);
        assert!(matches!(
            service.apply(stale),
            Err(DesktopArchitectureError::VersionConflict { .. })
        ));
        assert_eq!(
            service.graph("project-a").unwrap(),
            ProjectArchitectureGraph::empty("project-a")
        );
        assert!(service.list_changes("project-a", 40).unwrap().is_empty());
        assert!(rx.try_recv().is_err());
    }
}
