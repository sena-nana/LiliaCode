use lilia_contracts::{
    AgentSessionRef, ArtifactId, ArtifactMaterializationStatus, AssignmentId, BindingId,
    ConversationId, ExpectedRevision, IdempotencyKey, MilestoneId, ProductArtifact,
    ProductAssignment, ProductCommandMeta, ProductConversation, ProductEntity, ProductEntityKind,
    ProductMilestone, ProductResult, ProductWorkflow, ProductWorkflowRun, ProjectAsset,
    ProjectAssetId, ProjectAssetKind, ProjectId, TaskId, WorkflowId, WorkflowRunId,
};

use super::ProductServices;

pub struct BrowserScreenshotInput {
    pub task_id: TaskId,
    pub project_id: lilia_contracts::ProjectId,
    pub agent_session: AgentSessionRef,
    pub artifact_id: ArtifactId,
    pub artifact_ref: String,
    pub resource_ref: String,
    pub content_hash: String,
    pub size_bytes: u64,
    pub command_id: String,
}

impl ProductServices {
    pub fn create_conversation(
        &self,
        id: ConversationId,
        project_id: Option<ProjectId>,
        task_id: Option<TaskId>,
        title: impl Into<String>,
    ) -> ProductResult<ProductConversation> {
        if let Some(project_id) = &project_id {
            self.get_project(project_id)?;
        }
        if let Some(task_id) = &task_id {
            self.get_task(task_id)?;
        }
        let conversation = ProductConversation::new(id, project_id, task_id, title)?;
        entity_conversation(self.create_entity(ProductEntity::Conversation(conversation))?)
    }

    pub fn fork_conversation(
        &self,
        source_id: &ConversationId,
        id: ConversationId,
        title: impl Into<String>,
    ) -> ProductResult<ProductConversation> {
        let source = self.get_conversation(source_id)?;
        let conversation = ProductConversation::fork(id, &source, title)?;
        entity_conversation(self.create_entity(ProductEntity::Conversation(conversation))?)
    }

    pub fn get_conversation(&self, id: &ConversationId) -> ProductResult<ProductConversation> {
        entity_conversation(self.get_entity(ProductEntityKind::Conversation, id.as_str())?)
    }

    pub fn list_conversations(&self) -> ProductResult<Vec<ProductConversation>> {
        self.list_entities(ProductEntityKind::Conversation)?
            .into_iter()
            .map(entity_conversation)
            .collect()
    }

    pub fn bind_conversation_session(
        &self,
        conversation_id: &ConversationId,
        binding_id: BindingId,
        expected: ExpectedRevision,
    ) -> ProductResult<ProductConversation> {
        let mut conversation = self.get_conversation(conversation_id)?;
        conversation.bind_session(binding_id);
        entity_conversation(
            self.update_entity(ProductEntity::Conversation(conversation), expected)?,
        )
    }

    pub fn advance_conversation_timeline(
        &self,
        conversation_id: &ConversationId,
        cursor: u64,
        expected: ExpectedRevision,
    ) -> ProductResult<ProductConversation> {
        let mut conversation = self.get_conversation(conversation_id)?;
        conversation.advance_timeline_cursor(cursor)?;
        entity_conversation(
            self.update_entity(ProductEntity::Conversation(conversation), expected)?,
        )
    }

    pub fn create_milestone(
        &self,
        id: MilestoneId,
        project_id: ProjectId,
        title: impl Into<String>,
    ) -> ProductResult<ProductMilestone> {
        self.get_project(&project_id)?;
        let milestone = ProductMilestone::new(id, project_id, title)?;
        entity_milestone(self.create_entity(ProductEntity::Milestone(milestone))?)
    }

    pub fn create_workflow(
        &self,
        id: WorkflowId,
        name: impl Into<String>,
    ) -> ProductResult<ProductWorkflow> {
        let workflow = ProductWorkflow::new(id, name)?;
        entity_workflow(self.create_entity(ProductEntity::Workflow(workflow))?)
    }

    pub fn publish_workflow(
        &self,
        workflow_id: &WorkflowId,
        expected: ExpectedRevision,
    ) -> ProductResult<ProductWorkflow> {
        let mut workflow =
            entity_workflow(self.get_entity(ProductEntityKind::Workflow, workflow_id.as_str())?)?;
        workflow.publish();
        entity_workflow(self.update_entity(ProductEntity::Workflow(workflow), expected)?)
    }

    pub fn start_workflow_run(
        &self,
        id: WorkflowRunId,
        workflow_id: &WorkflowId,
        task_id: Option<TaskId>,
    ) -> ProductResult<ProductWorkflowRun> {
        let workflow =
            entity_workflow(self.get_entity(ProductEntityKind::Workflow, workflow_id.as_str())?)?;
        if let Some(task_id) = &task_id {
            self.get_task(task_id)?;
        }
        let run = ProductWorkflowRun::new(id, &workflow, task_id)?;
        entity_workflow_run(self.create_entity(ProductEntity::WorkflowRun(run))?)
    }

    pub fn create_assignment(
        &self,
        id: AssignmentId,
        task_id: TaskId,
        role: impl Into<String>,
        assignee: impl Into<String>,
    ) -> ProductResult<ProductAssignment> {
        self.get_task(&task_id)?;
        let assignment = ProductAssignment::new(id, task_id, role, assignee)?;
        entity_assignment(self.create_entity(ProductEntity::Assignment(assignment))?)
    }

    pub fn attach_artifact(
        &self,
        id: ArtifactId,
        task_id: TaskId,
        agent_session: AgentSessionRef,
        artifact_ref: impl Into<String>,
        media_type: impl Into<String>,
    ) -> ProductResult<ProductArtifact> {
        self.get_task(&task_id)?;
        let artifact = ProductArtifact::new(id, task_id, agent_session, artifact_ref, media_type)?;
        entity_artifact(self.create_entity(ProductEntity::Artifact(artifact))?)
    }

    pub fn materialize_artifact(
        &self,
        artifact_id: &ArtifactId,
        resource_ref: String,
        expected: ExpectedRevision,
    ) -> ProductResult<ProductArtifact> {
        let mut artifact =
            entity_artifact(self.get_entity(ProductEntityKind::Artifact, artifact_id.as_str())?)?;
        artifact.set_materialization(
            ArtifactMaterializationStatus::Materialized,
            Some(resource_ref),
        )?;
        entity_artifact(self.update_entity(ProductEntity::Artifact(artifact), expected)?)
    }

    /// Record a fully materialized browser screenshot as one idempotent product event.
    pub fn record_browser_screenshot(
        &self,
        input: BrowserScreenshotInput,
    ) -> ProductResult<lilia_contracts::ProductCommandResult<ProductEntity>> {
        let BrowserScreenshotInput {
            task_id,
            project_id,
            agent_session,
            artifact_id,
            artifact_ref,
            resource_ref,
            content_hash,
            size_bytes,
            command_id,
        } = input;
        let task = self.get_task(&task_id)?;
        if task.project_id.as_ref() != Some(&project_id) {
            return Err(lilia_contracts::ProductError::InvalidState {
                message: "browser artifact task is outside the target project".into(),
            });
        }
        let project = self.get_project(&project_id)?;
        if project.archive != lilia_contracts::ProjectArchiveState::Active {
            return Err(lilia_contracts::ProductError::InvalidState {
                message: "archived projects cannot receive browser artifacts".into(),
            });
        }
        if task.archived {
            return Err(lilia_contracts::ProductError::InvalidState {
                message: "archived tasks cannot receive browser artifacts".into(),
            });
        }
        if content_hash.len() != 64
            || !content_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(lilia_contracts::ProductError::InvalidInput {
                field: "content_hash".into(),
                message: "browser screenshot content_hash must be a SHA-256 hex digest".into(),
            });
        }
        if !lilia_contracts::is_opaque_browser_resource_ref(&resource_ref) {
            return Err(lilia_contracts::ProductError::InvalidInput {
                field: "resource_ref".into(),
                message: "browser screenshot resource_ref must be opaque".into(),
            });
        }
        let mut artifact = ProductArtifact::new(
            artifact_id,
            task_id,
            agent_session,
            artifact_ref,
            "image/png",
        )?;
        artifact.content_hash = Some(content_hash);
        artifact.source_event_id = Some(command_id.clone());
        artifact.provenance = Some("browser.screenshot".into());
        artifact.set_materialization(
            ArtifactMaterializationStatus::Materialized,
            Some(resource_ref),
        )?;
        artifact.size_bytes = Some(size_bytes);
        let meta =
            ProductCommandMeta::create(command_id.clone(), IdempotencyKey::new(command_id)?)?;
        self.create_entity_command(
            &meta,
            ProductEntity::Artifact(artifact),
            "browser_screenshot_captured",
        )
    }

    pub fn create_project_asset(
        &self,
        id: ProjectAssetId,
        project_id: ProjectId,
        kind: ProjectAssetKind,
        title: impl Into<String>,
        content_ref: impl Into<String>,
    ) -> ProductResult<ProjectAsset> {
        self.get_project(&project_id)?;
        let asset = ProjectAsset::new(id, project_id, kind, title, content_ref)?;
        entity_project_asset(self.create_entity(ProductEntity::ProjectAsset(asset))?)
    }
}

macro_rules! entity_decoder {
    ($name:ident, $variant:ident, $ty:ty) => {
        fn $name(entity: ProductEntity) -> ProductResult<$ty> {
            match entity {
                ProductEntity::$variant(value) => Ok(value),
                other => Err(lilia_contracts::ProductError::InvalidState {
                    message: format!(
                        "expected {}, received {}",
                        ProductEntityKind::$variant.as_str(),
                        other.kind().as_str()
                    ),
                }),
            }
        }
    };
}

entity_decoder!(entity_conversation, Conversation, ProductConversation);
entity_decoder!(entity_milestone, Milestone, ProductMilestone);
entity_decoder!(entity_workflow, Workflow, ProductWorkflow);
entity_decoder!(entity_workflow_run, WorkflowRun, ProductWorkflowRun);
entity_decoder!(entity_assignment, Assignment, ProductAssignment);
entity_decoder!(entity_artifact, Artifact, ProductArtifact);
entity_decoder!(entity_project_asset, ProjectAsset, ProjectAsset);

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use lilia_contracts::{ProductRevision, ProjectId, TaskId};

    use super::*;
    use crate::application::InMemoryProductStore;

    #[test]
    fn core_use_cases_run_without_agentkit_or_host_dependencies() {
        let products = ProductServices::new(Arc::new(Mutex::new(InMemoryProductStore::new())));
        let project = products
            .create_project(ProjectId::new("project-1").unwrap(), "Product")
            .unwrap();
        let task = products
            .create_task(
                TaskId::new("task-1").unwrap(),
                Some(project.id.clone()),
                "Implement",
            )
            .unwrap();
        let conversation = products
            .create_conversation(
                ConversationId::new("conversation-1").unwrap(),
                Some(project.id),
                Some(task.id.clone()),
                "Implementation",
            )
            .unwrap();
        let workflow = products
            .create_workflow(WorkflowId::new("workflow-1").unwrap(), "Review")
            .unwrap();
        let workflow = products
            .publish_workflow(
                &workflow.id,
                ExpectedRevision::new(workflow.revision.get()).unwrap(),
            )
            .unwrap();
        let run = products
            .start_workflow_run(
                WorkflowRunId::new("run-1").unwrap(),
                &workflow.id,
                Some(task.id),
            )
            .unwrap();

        assert_eq!(conversation.revision, ProductRevision::INITIAL);
        assert!(run.agent_session.is_none());
    }

    #[test]
    fn stale_revision_is_rejected_for_generic_product_entities() {
        let products = ProductServices::new(Arc::new(Mutex::new(InMemoryProductStore::new())));
        let workflow = products
            .create_workflow(WorkflowId::new("workflow-1").unwrap(), "Review")
            .unwrap();
        let expected = ExpectedRevision::new(workflow.revision.get()).unwrap();
        products.publish_workflow(&workflow.id, expected).unwrap();
        assert!(matches!(
            products.publish_workflow(&workflow.id, expected),
            Err(lilia_contracts::ProductError::Conflict {
                conflict: lilia_contracts::ConflictKind::StaleRevision,
                ..
            })
        ));
    }

    #[test]
    fn browser_screenshot_is_task_project_bound_and_command_idempotent() {
        let products = ProductServices::new(Arc::new(Mutex::new(InMemoryProductStore::new())));
        let project = products
            .create_project(ProjectId::new("browser-project").unwrap(), "Browser")
            .unwrap();
        let task = products
            .create_task(
                TaskId::new("browser-task").unwrap(),
                Some(project.id.clone()),
                "Capture",
            )
            .unwrap();
        let ordinary = products
            .attach_artifact(
                ArtifactId::new("ordinary-artifact").unwrap(),
                task.id.clone(),
                lilia_contracts::AgentSessionRef::new("session").unwrap(),
                "ordinary-ref",
                "text/plain",
            )
            .unwrap();
        let mut ordinary = ordinary;
        ordinary.content_hash = Some("a".repeat(64));
        products
            .update_entity(
                ProductEntity::Artifact(ordinary),
                ExpectedRevision::new(1).unwrap(),
            )
            .unwrap();
        let input = || {
            products.record_browser_screenshot(BrowserScreenshotInput {
                task_id: task.id.clone(),
                project_id: project.id.clone(),
                agent_session: lilia_contracts::AgentSessionRef::new("session").unwrap(),
                artifact_id: ArtifactId::new("browser-artifact").unwrap(),
                artifact_ref: "browser.screenshot:hash".into(),
                resource_ref: "resource://browser/project/task/hash.png".into(),
                content_hash: "a".repeat(64),
                size_bytes: 4,
                command_id: "browser-command".into(),
            })
        };
        let first = input().unwrap();
        let duplicate = input().unwrap();
        assert!(!first.duplicate);
        assert!(duplicate.duplicate);
        assert_eq!(
            products
                .list_entities(ProductEntityKind::Artifact)
                .unwrap()
                .len(),
            2
        );
        let events = products
            .product_events(&lilia_contracts::PageRequest::default())
            .unwrap();
        let browser_events = events
            .items
            .iter()
            .filter(|event| event.action == "browser_screenshot_captured")
            .collect::<Vec<_>>();
        assert_eq!(browser_events.len(), 1);
        assert_eq!(browser_events[0].command_id, "browser-command");
        let other = products
            .create_task(
                TaskId::new("other-task").unwrap(),
                Some(project.id),
                "Other",
            )
            .unwrap();
        assert!(products
            .record_browser_screenshot(BrowserScreenshotInput {
                task_id: other.id,
                project_id: ProjectId::new("missing-project").unwrap(),
                agent_session: lilia_contracts::AgentSessionRef::new("session").unwrap(),
                artifact_id: ArtifactId::new("other-artifact").unwrap(),
                artifact_ref: "ref".into(),
                resource_ref: "resource://browser/project/other".into(),
                content_hash: "a".repeat(64),
                size_bytes: 4,
                command_id: "other-command".into(),
            })
            .is_err());
    }
}
