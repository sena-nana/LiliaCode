//! Input use cases coordinate draft authority with project and task facts.
use std::sync::Arc;

use lilia_contracts::{ProductTask, Project, ProjectId, TaskId};
use lilia_feature_composer::{
    ComposerError, ComposerService, ComposerStore, ComposerTaskAuthority,
};
use lilia_feature_task::ProjectTaskService;
use lilia_kernel::{
    EventBus, Feature, FeatureContext, FeatureId, KernelError, ServiceKey, ServiceRef,
};

use crate::application::{
    DesktopApplication, DesktopApplicationError, DesktopComposerCommand, DesktopComposerState,
    ProjectContext, ProjectQuery, TaskQuery,
};

struct ProductComposerAuthority(ProjectTaskService);
impl ComposerTaskAuthority for ProductComposerAuthority {
    fn ensure_task(&self, task_id: &TaskId) -> Result<(), ComposerError> {
        self.0
            .get_task(task_id)
            .map(|_| ())
            .map_err(|error| ComposerError::TaskUnavailable {
                task_id: task_id.clone(),
                message: error.to_string(),
            })
    }
}

#[derive(Clone)]
pub struct ComposerInputService {
    draft: Arc<ComposerService>,
    projects: ProjectTaskService,
}
impl ComposerInputService {
    pub(crate) fn new(
        store: Arc<ComposerStore>,
        projects: ProjectTaskService,
        events: EventBus,
    ) -> Self {
        let draft = Arc::new(ComposerService::new(
            store,
            Arc::new(ProductComposerAuthority(projects.clone())),
            events,
        ));
        Self { draft, projects }
    }
    pub(crate) fn draft_service(&self) -> Arc<ComposerService> {
        Arc::clone(&self.draft)
    }
    pub fn composer_state(
        &self,
        task_id: &TaskId,
    ) -> Result<DesktopComposerState, DesktopApplicationError> {
        Ok(self.draft.snapshot(task_id)?)
    }
    pub fn execute_composer_command(
        &self,
        task_id: &TaskId,
        command: DesktopComposerCommand,
    ) -> Result<DesktopComposerState, DesktopApplicationError> {
        Ok(self.draft.execute(task_id, command)?.0)
    }
    pub fn task_has_project(&self, task_id: &TaskId) -> Result<bool, DesktopApplicationError> {
        Ok(self.get_task(task_id)?.project_id.is_some())
    }
    pub(super) fn get_task(
        &self,
        task_id: &TaskId,
    ) -> Result<ProductTask, DesktopApplicationError> {
        Ok(self.projects.get_task(task_id)?)
    }
    pub(super) fn query_tasks(
        &self,
        query: TaskQuery,
    ) -> Result<Vec<ProductTask>, DesktopApplicationError> {
        Ok(self.projects.query_tasks(query)?)
    }
    pub(super) fn query_projects(
        &self,
        query: ProjectQuery,
    ) -> Result<Vec<Project>, DesktopApplicationError> {
        Ok(self.projects.query_projects(query)?)
    }
    pub(super) fn project_context(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectContext, DesktopApplicationError> {
        Ok(ProjectContext::from_project(
            &self.projects.get_project(project_id)?,
        )?)
    }
}

pub enum ComposerInputServiceKey {}
impl ServiceKey for ComposerInputServiceKey {
    type Value = ComposerInputService;
    const NAME: &'static str = "lilia.composer.input";
}
pub struct ComposerInputFeature {
    service: ComposerInputService,
}
impl ComposerInputFeature {
    pub fn new(service: ComposerInputService) -> Self {
        Self { service }
    }
}
impl Feature for ComposerInputFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.composer-input").expect("nonempty feature id")
    }
    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<ComposerInputServiceKey>()]
    }
    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<ComposerInputServiceKey>(self.service.clone())
    }
}
impl DesktopApplication {
    pub fn composer_input_service(&self) -> ComposerInputService {
        self.inner.composer_input.clone()
    }
}
