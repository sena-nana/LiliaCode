use lilia_contracts::{ProjectId, TaskId};

use crate::application::{
    ArchitectureBackend, DesktopApplication, DesktopApplicationError, DesktopArchitectureService,
    DesktopMemory, DesktopMemoryService, DesktopRoadmapService, MemoryInjectionState,
    MemorySettings, MemoryUpsertInput, Milestone, MilestoneUpdatePatch,
    ProjectArchitectureApplyInput, ProjectArchitectureApplyResult, ProjectArchitectureChangeEvent,
    ProjectArchitectureChangeRecord, ProjectArchitectureGraph, ProjectArchitectureQuarantineRecord,
    ProjectArchitectureRejectInput, ProjectArchitectureRollbackResult, ProjectRoadmap,
    TaskMilestoneLink,
};

impl DesktopApplication {
    pub fn architecture_service(&self) -> DesktopArchitectureService {
        self.inner.architecture.clone()
    }

    pub fn project_architecture(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectArchitectureGraph, DesktopApplicationError> {
        Ok(self.inner.architecture.graph(project_id.as_str())?)
    }

    pub fn project_architecture_changes(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> Result<Vec<ProjectArchitectureChangeRecord>, DesktopApplicationError> {
        Ok(self
            .inner
            .architecture
            .list_changes(project_id.as_str(), limit)?)
    }

    pub fn project_architecture_quarantine(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<ProjectArchitectureQuarantineRecord>, DesktopApplicationError> {
        Ok(self
            .inner
            .architecture
            .list_quarantine(project_id.as_str())?)
    }

    pub fn apply_project_architecture(
        &self,
        input: ProjectArchitectureApplyInput,
    ) -> Result<ProjectArchitectureApplyResult, DesktopApplicationError> {
        Ok(self.inner.architecture.apply(input)?)
    }

    pub fn reject_project_architecture(
        &self,
        input: ProjectArchitectureRejectInput,
    ) -> Result<ProjectArchitectureChangeEvent, DesktopApplicationError> {
        Ok(self.inner.architecture.reject(input)?)
    }

    pub fn rollback_project_architecture(
        &self,
        project_id: &ProjectId,
        task_id: &TaskId,
        backend: ArchitectureBackend,
    ) -> Result<ProjectArchitectureRollbackResult, DesktopApplicationError> {
        Ok(self
            .inner
            .architecture
            .rollback(project_id.as_str(), task_id.as_str(), backend)?)
    }

    pub fn memory_service(&self) -> DesktopMemoryService {
        self.inner.memory.clone()
    }

    pub fn list_memories(
        &self,
        project_id: Option<&ProjectId>,
    ) -> Result<Vec<DesktopMemory>, DesktopApplicationError> {
        Ok(self.inner.memory.list(project_id.map(ProjectId::as_str))?)
    }

    pub fn memory(
        &self,
        memory_id: &str,
    ) -> Result<Option<DesktopMemory>, DesktopApplicationError> {
        Ok(self.inner.memory.memory(memory_id)?)
    }

    pub fn save_memory(
        &self,
        input: MemoryUpsertInput,
    ) -> Result<DesktopMemory, DesktopApplicationError> {
        Ok(self.inner.memory.save(input)?)
    }

    pub fn set_memory_enabled(
        &self,
        memory_id: &str,
        enabled: bool,
        expected_updated_at: Option<i64>,
    ) -> Result<DesktopMemory, DesktopApplicationError> {
        Ok(self
            .inner
            .memory
            .set_enabled_if_unmodified(memory_id, enabled, expected_updated_at)?)
    }

    pub fn delete_memory(
        &self,
        memory_id: &str,
        expected_updated_at: Option<i64>,
    ) -> Result<bool, DesktopApplicationError> {
        Ok(self
            .inner
            .memory
            .delete_if_unmodified(memory_id, expected_updated_at)?)
    }

    pub fn memory_settings(&self) -> Result<MemorySettings, DesktopApplicationError> {
        Ok(self.inner.memory.settings()?)
    }

    pub fn save_memory_settings(
        &self,
        settings: MemorySettings,
    ) -> Result<MemorySettings, DesktopApplicationError> {
        Ok(self.inner.memory.save_settings(settings)?)
    }

    pub fn memory_injection_state(
        &self,
        task_id: &TaskId,
    ) -> Result<MemoryInjectionState, DesktopApplicationError> {
        Ok(self.inner.memory.injection_state(task_id.as_str())?)
    }

    pub fn set_task_memory_enabled(
        &self,
        task_id: &TaskId,
        enabled: bool,
        expected_updated_at: Option<i64>,
    ) -> Result<MemoryInjectionState, DesktopApplicationError> {
        Ok(self.inner.memory.set_task_enabled_if_unmodified(
            task_id.as_str(),
            enabled,
            expected_updated_at,
        )?)
    }

    pub fn reset_task_memory_cooldown(
        &self,
        task_id: &TaskId,
        expected_updated_at: Option<i64>,
    ) -> Result<MemoryInjectionState, DesktopApplicationError> {
        Ok(self
            .inner
            .memory
            .reset_task_cooldown_if_unmodified(task_id.as_str(), expected_updated_at)?)
    }

    pub fn roadmap_service(&self) -> DesktopRoadmapService {
        self.inner.roadmap.clone()
    }

    pub fn project_roadmap(
        &self,
        project_id: &ProjectId,
    ) -> Result<ProjectRoadmap, DesktopApplicationError> {
        Ok(self.inner.roadmap.list(project_id)?)
    }

    pub fn create_milestone(
        &self,
        project_id: &ProjectId,
        title: &str,
    ) -> Result<Milestone, DesktopApplicationError> {
        Ok(self.inner.roadmap.create(project_id, title)?)
    }

    pub fn update_milestone(
        &self,
        project_id: &ProjectId,
        milestone_id: &str,
        patch: MilestoneUpdatePatch,
    ) -> Result<Milestone, DesktopApplicationError> {
        Ok(self.inner.roadmap.update(project_id, milestone_id, patch)?)
    }

    pub fn delete_milestone(
        &self,
        project_id: &ProjectId,
        milestone_id: &str,
    ) -> Result<bool, DesktopApplicationError> {
        Ok(self.inner.roadmap.delete(project_id, milestone_id)?)
    }

    pub fn reorder_milestones(
        &self,
        project_id: &ProjectId,
        ordered_ids: Vec<String>,
    ) -> Result<Vec<Milestone>, DesktopApplicationError> {
        Ok(self.inner.roadmap.reorder(project_id, ordered_ids)?)
    }

    pub fn set_milestone_tasks(
        &self,
        project_id: &ProjectId,
        milestone_id: &str,
        task_ids: Vec<String>,
    ) -> Result<Vec<TaskMilestoneLink>, DesktopApplicationError> {
        Ok(self
            .inner
            .roadmap
            .set_tasks(project_id, milestone_id, task_ids)?)
    }
}
