use std::sync::{Arc, Mutex, MutexGuard};

use crate::RoadmapChanged;
use lilia_contracts::ProjectId;
use lilia_kernel::EventBus;
use lilia_storage::Db;

use super::{
    Milestone, MilestoneUpdatePatch, ProjectRoadmap, RoadmapStore, RoadmapStoreError,
    SqliteRoadmapStore, TaskMilestoneLink,
};

#[derive(Clone)]
pub struct DesktopRoadmapService {
    inner: Arc<DesktopRoadmapServiceInner>,
    events: EventBus,
}

struct DesktopRoadmapServiceInner {
    store: Mutex<Box<dyn RoadmapStore>>,
}

impl DesktopRoadmapService {
    pub fn from_db(db: Db) -> Result<Self, RoadmapStoreError> {
        Self::from_store(SqliteRoadmapStore::from_db(db)?)
    }

    pub fn in_memory() -> Result<Self, RoadmapStoreError> {
        Self::from_store(SqliteRoadmapStore::in_memory()?)
    }

    pub fn from_store(store: impl RoadmapStore + 'static) -> Result<Self, RoadmapStoreError> {
        Ok(Self {
            events: EventBus::new(),
            inner: Arc::new(DesktopRoadmapServiceInner {
                store: Mutex::new(Box::new(store)),
            }),
        })
    }

    pub fn with_events(mut self, events: EventBus) -> Self {
        self.events = events;
        self
    }

    pub fn list(&self, project_id: &ProjectId) -> Result<ProjectRoadmap, RoadmapStoreError> {
        self.store()?.list(project_id.as_str())
    }

    pub fn create(
        &self,
        project_id: &ProjectId,
        title: &str,
    ) -> Result<Milestone, RoadmapStoreError> {
        let milestone = self.store()?.create(project_id.as_str(), title)?;
        self.changed(project_id, Some(milestone.id.clone()));
        Ok(milestone)
    }

    pub fn update(
        &self,
        project_id: &ProjectId,
        milestone_id: &str,
        patch: MilestoneUpdatePatch,
    ) -> Result<Milestone, RoadmapStoreError> {
        let milestone = {
            let mut store = self.store()?;
            ensure_milestone(store.as_ref(), project_id, milestone_id)?;
            store.update(milestone_id, patch)?
        };
        self.changed(project_id, Some(milestone.id.clone()));
        Ok(milestone)
    }

    pub fn delete(
        &self,
        project_id: &ProjectId,
        milestone_id: &str,
    ) -> Result<bool, RoadmapStoreError> {
        let deleted = {
            let mut store = self.store()?;
            if !has_milestone(store.as_ref(), project_id, milestone_id)? {
                return Ok(false);
            }
            store.delete(milestone_id)?
        };
        if deleted {
            self.changed(project_id, Some(milestone_id.to_owned()));
        }
        Ok(deleted)
    }

    pub fn reorder(
        &self,
        project_id: &ProjectId,
        ordered_ids: Vec<String>,
    ) -> Result<Vec<Milestone>, RoadmapStoreError> {
        let milestones = self.store()?.reorder(project_id.as_str(), ordered_ids)?;
        self.changed(project_id, None);
        Ok(milestones)
    }

    pub fn set_tasks(
        &self,
        project_id: &ProjectId,
        milestone_id: &str,
        task_ids: Vec<String>,
    ) -> Result<Vec<TaskMilestoneLink>, RoadmapStoreError> {
        let links = {
            let mut store = self.store()?;
            ensure_milestone(store.as_ref(), project_id, milestone_id)?;
            store.set_tasks(milestone_id, task_ids)?
        };
        self.changed(project_id, Some(milestone_id.to_owned()));
        Ok(links)
    }

    fn changed(&self, project_id: &ProjectId, milestone_id: Option<String>) {
        self.events.publish(RoadmapChanged {
            project_id: project_id.clone(),
            milestone_id,
        });
    }

    fn store(&self) -> Result<MutexGuard<'_, Box<dyn RoadmapStore>>, RoadmapStoreError> {
        self.inner
            .store
            .lock()
            .map_err(|_| RoadmapStoreError::StateUnavailable)
    }
}

fn has_milestone(
    store: &dyn RoadmapStore,
    project_id: &ProjectId,
    milestone_id: &str,
) -> Result<bool, RoadmapStoreError> {
    Ok(store
        .list(project_id.as_str())?
        .milestones
        .iter()
        .any(|milestone| milestone.id == milestone_id))
}

fn ensure_milestone(
    store: &dyn RoadmapStore,
    project_id: &ProjectId,
    milestone_id: &str,
) -> Result<(), RoadmapStoreError> {
    if has_milestone(store, project_id, milestone_id)? {
        Ok(())
    } else {
        Err(RoadmapStoreError::MilestoneNotFound {
            milestone_id: milestone_id.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (
        DesktopRoadmapService,
        lilia_kernel::Kernel,
        ProjectId,
        ProjectId,
    ) {
        let store = SqliteRoadmapStore::in_memory().unwrap();
        store.db().lock().execute_batch("INSERT INTO projects (id, name, created_at) VALUES ('project-a', 'A', 1), ('project-b', 'B', 1); INSERT INTO tasks (id, project_id, session_id, title, created_at) VALUES ('task-a', 'project-a', 'session-a', 'A', 1), ('task-b', 'project-b', 'session-b', 'B', 1);").unwrap();
        let kernel = lilia_kernel::Kernel::new();
        let service = DesktopRoadmapService::from_store(store)
            .unwrap()
            .with_events(kernel.events().clone());
        kernel
            .mount(Arc::new(crate::RoadmapFeature::new(service.clone())))
            .unwrap();
        (
            service,
            kernel,
            ProjectId::new("project-a").unwrap(),
            ProjectId::new("project-b").unwrap(),
        )
    }

    #[test]
    fn direct_and_mounted_mutations_publish_once_after_commit() {
        let (service, kernel, project, _) = setup();
        let mounted = kernel.service::<crate::RoadmapServiceKey>().unwrap();
        let reader = service.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let subscription = kernel.events().on::<RoadmapChanged, _>(None, move |event| {
            tx.send((event.clone(), reader.list(&event.project_id).unwrap()))
                .unwrap();
        });
        let first = service.create(&project, "First").unwrap();
        let (event, stored) = rx.try_recv().unwrap();
        assert_eq!(event.project_id, project);
        assert_eq!(event.milestone_id.as_deref(), Some(first.id.as_str()));
        assert_eq!(stored.milestones, vec![first.clone()]);
        assert!(rx.try_recv().is_err());
        let updated = mounted
            .update(
                &project,
                &first.id,
                MilestoneUpdatePatch {
                    title: Some("Updated".to_owned()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(rx.try_recv().unwrap().1.milestones, vec![updated.clone()]);
        assert!(rx.try_recv().is_err());
        let links = mounted
            .set_tasks(&project, &first.id, vec!["task-a".to_owned()])
            .unwrap();
        assert_eq!(rx.try_recv().unwrap().1.links, links);
        assert!(rx.try_recv().is_err());
        mounted.reorder(&project, vec![first.id.clone()]).unwrap();
        assert_eq!(rx.try_recv().unwrap().0.milestone_id, None);
        assert!(rx.try_recv().is_err());
        assert!(mounted.delete(&project, &first.id).unwrap());
        assert!(rx.try_recv().unwrap().1.milestones.is_empty());
        assert!(!service.delete(&project, &first.id).unwrap());
        assert!(rx.try_recv().is_err());
        kernel.events().unsubscribe(subscription);
    }

    #[test]
    fn wrong_project_and_invalid_writes_preserve_records_and_publish_nothing() {
        let (service, kernel, project, other) = setup();
        let milestone = service.create(&project, "Keep").unwrap();
        service
            .set_tasks(&project, &milestone.id, vec!["task-a".to_owned()])
            .unwrap();
        let before = service.list(&project).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        kernel.events().on::<RoadmapChanged, _>(None, move |event| {
            tx.send(event.clone()).unwrap();
        });
        assert!(matches!(
            service.update(
                &other,
                &milestone.id,
                MilestoneUpdatePatch {
                    title: Some("Wrong".to_owned()),
                    ..Default::default()
                }
            ),
            Err(RoadmapStoreError::MilestoneNotFound { .. })
        ));
        assert!(!service.delete(&other, &milestone.id).unwrap());
        assert!(matches!(
            service.set_tasks(&other, &milestone.id, vec![]),
            Err(RoadmapStoreError::MilestoneNotFound { .. })
        ));
        assert!(service.reorder(&other, vec![milestone.id.clone()]).is_err());
        assert!(service
            .set_tasks(&project, &milestone.id, vec!["task-b".to_owned()])
            .is_err());
        assert!(service.create(&project, "   ").is_err());
        assert!(service
            .update(
                &project,
                &milestone.id,
                MilestoneUpdatePatch {
                    title: Some("   ".to_owned()),
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(service.list(&project).unwrap(), before);
        assert!(service.list(&other).unwrap().milestones.is_empty());
        assert!(rx.try_recv().is_err());
    }
}
