use std::fmt::Write as _;
use std::sync::Arc;

use lilia_contracts::{
    AgentSessionRef, ExpectedRevision, IdempotencyKey, ProductCommandMeta, ProductEntity,
    ProductTask, ProductTaskStatus, ProjectId, ProjectionEventId, TaskId,
    TimelineProjectionCommand, TimelineProjectionEvent,
};
use lilia_storage::ProjectionApplyResult;
use serde_json::{Value as JsonValue, json};

use super::{
    AutomationAddTodoRequest, AutomationAgentActivation, AutomationAgentDispatch,
    AutomationAgentPort, AutomationAgentTarget, AutomationCompleteAgentInput,
    AutomationCreateTaskRequest, AutomationExecutionEngine, AutomationExecutionError,
    AutomationExecutionRepository, AutomationExecutionResult, AutomationGuidePort,
    AutomationPortContext, AutomationPortError, AutomationRecordTimelineRequest,
    AutomationResumeRunInput, AutomationRunDetail, AutomationRunStatus, AutomationSendGuideRequest,
    AutomationStartAgentRequest, AutomationTaskPort, AutomationTimelinePort, AutomationTodoPort,
    AutomationUpdateTaskStatusRequest,
};
use crate::application::agent::DesktopIdempotentTurnStart;
use crate::application::{
    DesktopApplication, DesktopApplicationError, DesktopAutomationTurnCorrelation,
    DesktopExecutionPermission, DesktopTodoCreate, DesktopTodoGuideStatus, DesktopTodoPriority,
    DesktopTodoSource, DesktopTurnDispatchKind, DesktopTurnRequest, TaskQuery,
};
use crate::application::{TasksChanged, TimelineChanged};

#[derive(Clone)]
pub struct DesktopAutomationOperationService {
    automation: super::DesktopAutomationService,
    authority: lilia_service::ServiceAuthority,
    project_tasks: lilia_feature_task::ProjectTaskService,
    todos: super::DesktopTodoService,
    events: lilia_kernel::EventBus,
    agent: Arc<dyn DesktopAutomationAgentOperations>,
}

trait DesktopAutomationAgentOperations: Send + Sync {
    fn start_turn(
        &self,
        request: DesktopTurnRequest,
        idempotency_key: &str,
    ) -> Result<DesktopIdempotentTurnStart, DesktopApplicationError>;

    fn activate_turn(
        &self,
        task_id: TaskId,
        turn_id: String,
    ) -> Result<(), DesktopApplicationError>;

    fn abort_turn(&self, task_id: TaskId, turn_id: String);

    fn cancel_turn(
        &self,
        task_id: &TaskId,
        turn_id: &str,
        run_id: &str,
        node_id: &str,
    ) -> Result<(), DesktopApplicationError>;
}

struct DesktopApplicationAutomationAgentOperations {
    application: DesktopApplication,
}

impl DesktopAutomationAgentOperations for DesktopApplicationAutomationAgentOperations {
    fn start_turn(
        &self,
        request: DesktopTurnRequest,
        idempotency_key: &str,
    ) -> Result<DesktopIdempotentTurnStart, DesktopApplicationError> {
        self.application
            .start_task_turn_idempotent(request, idempotency_key)
    }

    fn activate_turn(
        &self,
        task_id: TaskId,
        turn_id: String,
    ) -> Result<(), DesktopApplicationError> {
        self.application.activate_turn_worker(task_id, turn_id)
    }

    fn abort_turn(&self, task_id: TaskId, turn_id: String) {
        self.application.abort_prepared_turn(task_id, turn_id);
    }

    fn cancel_turn(
        &self,
        task_id: &TaskId,
        turn_id: &str,
        run_id: &str,
        node_id: &str,
    ) -> Result<(), DesktopApplicationError> {
        self.application
            .cancel_automation_agent_turn(task_id, turn_id, run_id, node_id)
    }
}

impl DesktopApplication {
    pub fn automation_operation_service(&self) -> DesktopAutomationOperationService {
        DesktopAutomationOperationService {
            automation: self.automation_service(),
            authority: self.inner.authority.clone(),
            project_tasks: self.inner.project_tasks.clone(),
            todos: self.todo_service(),
            events: self.event_bus(),
            agent: Arc::new(DesktopApplicationAutomationAgentOperations {
                application: self.clone(),
            }),
        }
    }

    pub fn execute_automation_run(
        &self,
        run_id: &str,
    ) -> Result<AutomationExecutionResult, AutomationExecutionError> {
        let _execution = self.inner.automation_execution.enter(run_id);
        let _effect = super::automation_dispatch::AutomationEffectGuard::enter();
        self.automation_operation_service().execute_run(run_id)
    }

    pub fn resume_automation_run(
        &self,
        run_id: &str,
        input: AutomationResumeRunInput,
    ) -> Result<AutomationExecutionResult, AutomationExecutionError> {
        let _execution = self.inner.automation_execution.enter(run_id);
        let _effect = super::automation_dispatch::AutomationEffectGuard::enter();
        self.automation_operation_service()
            .resume_run(run_id, input)
    }

    pub fn complete_automation_agent_turn(
        &self,
        input: AutomationCompleteAgentInput,
    ) -> Result<AutomationExecutionResult, AutomationExecutionError> {
        let _execution = self.inner.automation_execution.enter(&input.run_id);
        let _effect = super::automation_dispatch::AutomationEffectGuard::enter();
        self.automation_operation_service()
            .complete_agent_turn(input)
    }

    pub fn cancel_automation_run(
        &self,
        run_id: &str,
    ) -> Result<AutomationRunDetail, AutomationExecutionError> {
        let _execution = self.inner.automation_execution.enter(run_id);
        self.automation_operation_service().cancel_run(run_id)
    }
}

impl DesktopAutomationOperationService {
    fn execute_run(
        &self,
        run_id: &str,
    ) -> Result<AutomationExecutionResult, AutomationExecutionError> {
        AutomationExecutionEngine::new(Arc::new(self.clone())).execute_run(&self.automation, run_id)
    }

    fn resume_run(
        &self,
        run_id: &str,
        input: AutomationResumeRunInput,
    ) -> Result<AutomationExecutionResult, AutomationExecutionError> {
        AutomationExecutionEngine::new(Arc::new(self.clone())).resume_human(
            &self.automation,
            run_id,
            input,
        )
    }

    fn complete_agent_turn(
        &self,
        input: AutomationCompleteAgentInput,
    ) -> Result<AutomationExecutionResult, AutomationExecutionError> {
        AutomationExecutionEngine::new(Arc::new(self.clone()))
            .complete_agent(&self.automation, input)
    }

    fn cancel_run(&self, run_id: &str) -> Result<AutomationRunDetail, AutomationExecutionError> {
        let detail = self
            .automation
            .execution_run_detail(run_id)?
            .ok_or_else(|| AutomationExecutionError::RunNotFound {
                run_id: run_id.to_owned(),
            })?;
        if !matches!(
            detail.run.status,
            AutomationRunStatus::Running | AutomationRunStatus::WaitingUser
        ) {
            return Err(AutomationExecutionError::InvalidRunState {
                run_id: run_id.to_owned(),
                expected: vec![
                    AutomationRunStatus::Running,
                    AutomationRunStatus::WaitingUser,
                ],
                actual: detail.run.status,
            });
        }
        for node in detail.nodes.iter().filter(|node| {
            node.status == AutomationRunStatus::Running
                && node
                    .output
                    .as_ref()
                    .and_then(|output| output.get("waitingAgent"))
                    .and_then(JsonValue::as_bool)
                    == Some(true)
        }) {
            let output = node
                .output
                .as_ref()
                .expect("waiting Agent output was checked");
            let task_id = output
                .get("taskId")
                .and_then(JsonValue::as_str)
                .and_then(|task_id| TaskId::new(task_id.to_owned()).ok());
            let turn_id = output.get("turnId").and_then(JsonValue::as_str);
            let (Some(task_id), Some(turn_id)) = (task_id, turn_id) else {
                return Err(AutomationExecutionError::AgentCancellation {
                    run_id: run_id.to_owned(),
                    node_id: node.node_id.clone(),
                    message: "waiting Agent output is missing a valid taskId or turnId".to_owned(),
                });
            };
            self.agent
                .cancel_turn(&task_id, turn_id, run_id, &node.node_id)
                .map_err(|error| AutomationExecutionError::AgentCancellation {
                    run_id: run_id.to_owned(),
                    node_id: node.node_id.clone(),
                    message: error.to_string(),
                })?;
        }
        AutomationExecutionEngine::new(Arc::new(self.clone())).cancel_run(&self.automation, run_id)
    }
}

impl AutomationTaskPort for DesktopAutomationOperationService {
    fn create_task(
        &self,
        request: AutomationCreateTaskRequest,
    ) -> Result<JsonValue, AutomationPortError> {
        create_product_task(self, request, "create-task")
    }

    fn update_task_status(
        &self,
        request: AutomationUpdateTaskStatusRequest,
    ) -> Result<JsonValue, AutomationPortError> {
        let task_id = TaskId::new(request.task_id).map_err(port_error)?;
        let mut task = self.project_tasks.get_task(&task_id).map_err(port_error)?;
        task.status = product_task_status(&request.status)?;
        let key = operation_key(&request.context, "update-task-status");
        let meta = ProductCommandMeta::update(
            key.clone(),
            IdempotencyKey::new(key).map_err(port_error)?,
            ExpectedRevision::new(task.revision.get()).map_err(port_error)?,
        )
        .map_err(port_error)?;
        let result = self
            .authority
            .client()
            .map_err(port_error)?
            .update_product_entity(&meta, ProductEntity::Task(task), "automation_update_status")
            .map_err(port_error)?;
        let task = product_task(result.value)?;
        if !result.duplicate {
            self.events.publish(TasksChanged {
                project_id: task.project_id.clone(),
                task_id: Some(task.id.clone()),
            });
        }
        Ok(task_output(&task))
    }
}

impl AutomationTodoPort for DesktopAutomationOperationService {
    fn add_todo(
        &self,
        request: AutomationAddTodoRequest,
    ) -> Result<JsonValue, AutomationPortError> {
        let id = stable_identity("automation-todo", &request.context);
        let task_id = TaskId::new(request.task_id).map_err(port_error)?;
        let (todo, _) = self
            .todos
            .create_task_todo_idempotent(
                &id,
                DesktopTodoCreate {
                    task_id,
                    text: request.text,
                    priority: DesktopTodoPriority::Normal,
                    attachments: Vec::new(),
                    conversation_references: Vec::new(),
                    workflow: None,
                },
                DesktopTodoSource::Agent,
                None,
            )
            .map_err(port_error)?;
        Ok(json!({
            "todoId": todo.id,
            "taskId": todo.task_id.as_str(),
        }))
    }
}

impl AutomationGuidePort for DesktopAutomationOperationService {
    fn send_guide(
        &self,
        request: AutomationSendGuideRequest,
    ) -> Result<JsonValue, AutomationPortError> {
        let id = stable_identity("automation-guide", &request.context);
        let task_id = TaskId::new(request.task_id).map_err(port_error)?;
        let priority = todo_priority(&request.priority)?;
        let (guide, _) = self
            .todos
            .create_task_todo_idempotent(
                &id,
                DesktopTodoCreate {
                    task_id,
                    text: request.text,
                    priority,
                    attachments: Vec::new(),
                    conversation_references: Vec::new(),
                    workflow: None,
                },
                DesktopTodoSource::Lilia,
                Some(DesktopTodoGuideStatus::Pending),
            )
            .map_err(port_error)?;
        Ok(json!({
            "guideId": guide.id,
            "taskId": guide.task_id.as_str(),
            "priority": request.priority,
            "guideStatus": "pending",
        }))
    }
}

impl AutomationTimelinePort for DesktopAutomationOperationService {
    fn record_timeline(
        &self,
        request: AutomationRecordTimelineRequest,
    ) -> Result<JsonValue, AutomationPortError> {
        let task_id = TaskId::new(request.task_id).map_err(port_error)?;
        self.project_tasks.get_task(&task_id).map_err(port_error)?;
        let event_id = stable_identity("automation-timeline", &request.context);
        let sequence = stable_sequence(&request.context);
        let event = TimelineProjectionEvent {
            id: ProjectionEventId::new(event_id.clone()),
            task_id: task_id.clone(),
            agent_session: AgentSessionRef::new(format!(
                "automation:{}",
                request.context.workflow_id
            ))
            .map_err(port_error)?,
            sequence,
            turn_id: None,
            kind: "automation".to_owned(),
            status: request.status,
            title: request.title,
            summary: Some(request.summary),
            payload: request.payload,
            projected: false,
        };
        let result = self
            .authority
            .apply_projection(TimelineProjectionCommand::UpsertTimelineEvent { event })
            .map_err(port_error)?;
        if result != ProjectionApplyResult::DuplicateIgnored {
            self.events.publish(TimelineChanged {
                task_id: task_id.clone(),
                cursor: Some(sequence),
            });
        }
        Ok(json!({
            "recorded": true,
            "timelineEventId": event_id,
            "automationRunId": request.context.idempotency_key.run_id,
        }))
    }
}

impl AutomationAgentPort for DesktopAutomationOperationService {
    fn start_agent(
        &self,
        request: AutomationStartAgentRequest,
    ) -> Result<AutomationAgentDispatch, AutomationPortError> {
        if request.backend != "native-agentkit" {
            return Err(AutomationPortError::new(format!(
                "unsupported automation agent backend: {}",
                request.backend
            )));
        }
        let task_id = match request.target {
            AutomationAgentTarget::ExistingTask { task_id } => {
                let task_id = TaskId::new(task_id).map_err(port_error)?;
                self.project_tasks.get_task(&task_id).map_err(port_error)?;
                task_id
            }
            AutomationAgentTarget::CreateTask { project_id, title } => {
                let output = create_product_task(
                    self,
                    AutomationCreateTaskRequest {
                        context: request.context.clone(),
                        project_id,
                        title,
                        status: "running".to_owned(),
                    },
                    "agent-create-task",
                )?;
                TaskId::new(
                    output
                        .get("taskId")
                        .and_then(JsonValue::as_str)
                        .ok_or_else(|| AutomationPortError::new("created task output has no id"))?,
                )
                .map_err(port_error)?
            }
        };
        let mut turn = DesktopTurnRequest::new(task_id.clone(), request.prompt);
        turn.model = Some(request.model);
        turn.permission = execution_permission(&request.permission);
        turn.workspace_path =
            (!request.project_cwd.trim().is_empty()).then_some(request.project_cwd);
        turn.automation = Some(DesktopAutomationTurnCorrelation {
            run_id: request.context.idempotency_key.run_id.clone(),
            node_id: request.context.idempotency_key.node_id.clone(),
        });
        let key = operation_key(&request.context, "agent-turn");
        match self.agent.start_turn(turn, &key).map_err(port_error)? {
            DesktopIdempotentTurnStart::Dispatch {
                dispatch,
                worker_start_required,
                turn_inserted,
            } => {
                let (kind, queued_count) = match dispatch.kind {
                    DesktopTurnDispatchKind::Started => ("started", 0),
                    DesktopTurnDispatchKind::Queued { position } => ("queued", position as u64),
                };
                Ok(AutomationAgentDispatch::Waiting {
                    task_id: task_id.into_inner(),
                    turn_id: dispatch.turn_id,
                    dispatch: kind.to_owned(),
                    queued_count,
                    worker_start_required,
                    turn_inserted,
                    message_id: None,
                    metadata: json!({ "idempotencyKey": key }),
                })
            }
            DesktopIdempotentTurnStart::Completed { turn_id } => {
                Ok(AutomationAgentDispatch::Completed {
                    task_id: task_id.into_inner(),
                    turn_id,
                    metadata: json!({
                        "idempotencyKey": key,
                        "terminalStatus": "completed",
                    }),
                })
            }
            DesktopIdempotentTurnStart::TerminalConflict { turn_id, status } => {
                Err(AutomationPortError::new(format!(
                    "automation agent turn {turn_id} already ended with terminal status {status}"
                )))
            }
        }
    }

    fn activate_agent(
        &self,
        activation: AutomationAgentActivation,
    ) -> Result<(), AutomationPortError> {
        let task_id = TaskId::new(activation.task_id).map_err(port_error)?;
        self.agent
            .activate_turn(task_id, activation.turn_id)
            .map_err(port_error)
    }

    fn abort_agent(
        &self,
        activation: AutomationAgentActivation,
    ) -> Result<(), AutomationPortError> {
        let task_id = TaskId::new(activation.task_id).map_err(port_error)?;
        self.agent.abort_turn(task_id, activation.turn_id);
        Ok(())
    }
}

fn create_product_task(
    service: &DesktopAutomationOperationService,
    request: AutomationCreateTaskRequest,
    operation: &str,
) -> Result<JsonValue, AutomationPortError> {
    let project_id = request
        .project_id
        .map(ProjectId::new)
        .transpose()
        .map_err(port_error)?;
    if let Some(project_id) = &project_id {
        service
            .project_tasks
            .get_project(project_id)
            .map_err(port_error)?;
    }
    let task_id =
        TaskId::new(stable_identity("automation-task", &request.context)).map_err(port_error)?;
    let mut task =
        ProductTask::new(task_id, project_id.clone(), request.title).map_err(port_error)?;
    task.status = product_task_status(&request.status)?;
    task.sort_order = service
        .project_tasks
        .query_tasks(TaskQuery::for_project_or_inbox(project_id))
        .map_err(port_error)?
        .into_iter()
        .map(|task| task.sort_order)
        .max()
        .unwrap_or(-1)
        .saturating_add(1);
    task.legacy_source = Some("automation".to_owned());
    let key = operation_key(&request.context, operation);
    let meta =
        ProductCommandMeta::create(key.clone(), IdempotencyKey::new(key).map_err(port_error)?)
            .map_err(port_error)?;
    let result = service
        .authority
        .client()
        .map_err(port_error)?
        .create_product_entity(&meta, ProductEntity::Task(task), "automation_create_task")
        .map_err(port_error)?;
    let task = product_task(result.value)?;
    if !result.duplicate {
        service.events.publish(TasksChanged {
            project_id: task.project_id.clone(),
            task_id: Some(task.id.clone()),
        });
    }
    Ok(task_output(&task))
}

fn product_task(entity: ProductEntity) -> Result<ProductTask, AutomationPortError> {
    match entity {
        ProductEntity::Task(task) => Ok(task),
        entity => Err(AutomationPortError::new(format!(
            "product command returned {}, expected task",
            entity.kind().as_str()
        ))),
    }
}

fn task_output(task: &ProductTask) -> JsonValue {
    json!({
        "taskId": task.id.as_str(),
        "projectId": task.project_id.as_ref().map(ProjectId::as_str),
        "status": product_task_status_name(task.status),
    })
}

fn product_task_status(value: &str) -> Result<ProductTaskStatus, AutomationPortError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "draft" => Ok(ProductTaskStatus::Draft),
        "waiting" => Ok(ProductTaskStatus::Waiting),
        "running" => Ok(ProductTaskStatus::Running),
        "blocked" => Ok(ProductTaskStatus::Blocked),
        "done" => Ok(ProductTaskStatus::Done),
        "cancelled" => Ok(ProductTaskStatus::Cancelled),
        other => Err(AutomationPortError::new(format!(
            "invalid automation task status: {other}"
        ))),
    }
}

fn product_task_status_name(status: ProductTaskStatus) -> &'static str {
    match status {
        ProductTaskStatus::Draft => "draft",
        ProductTaskStatus::Waiting => "waiting",
        ProductTaskStatus::Running => "running",
        ProductTaskStatus::Blocked => "blocked",
        ProductTaskStatus::Done => "done",
        ProductTaskStatus::Cancelled => "cancelled",
    }
}

fn todo_priority(value: &str) -> Result<DesktopTodoPriority, AutomationPortError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => Ok(DesktopTodoPriority::Low),
        "normal" => Ok(DesktopTodoPriority::Normal),
        "high" => Ok(DesktopTodoPriority::High),
        other => Err(AutomationPortError::new(format!(
            "invalid automation todo priority: {other}"
        ))),
    }
}

fn execution_permission(value: &str) -> DesktopExecutionPermission {
    match value.trim().to_ascii_lowercase().as_str() {
        "full" => DesktopExecutionPermission::Full,
        "readonly" => DesktopExecutionPermission::Readonly,
        _ => DesktopExecutionPermission::Ask,
    }
}

fn operation_key(context: &AutomationPortContext, operation: &str) -> String {
    format!("automation:{operation}:{}", context.idempotency_key)
}

fn stable_identity(prefix: &str, context: &AutomationPortContext) -> String {
    let raw = context.idempotency_key.to_string();
    let mut value = String::with_capacity(prefix.len() + 1 + raw.len() * 2);
    value.push_str(prefix);
    value.push('-');
    for byte in raw.bytes() {
        write!(&mut value, "{byte:02x}").expect("writing to a String cannot fail");
    }
    value
}

fn stable_sequence(context: &AutomationPortContext) -> u64 {
    context
        .idempotency_key
        .to_string()
        .bytes()
        .fold(1_u64, |hash, byte| {
            hash.wrapping_mul(1_099_511_628_211)
                .wrapping_add(u64::from(byte))
        })
        .max(1)
}

fn port_error(error: impl std::fmt::Display) -> AutomationPortError {
    AutomationPortError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use lilia_contracts::{ProductEntity, ProductTask, TaskId};
    use lilia_service::ServiceAuthority;
    use serde_json::json;

    use super::*;
    use crate::application::{
        DesktopApplicationConfig, DesktopHost, DesktopHostAction, DesktopHostContext,
        DesktopHostError, DesktopHostResult, TasksChanged, TimelineChanged, TodosChanged,
    };

    static NEXT_APPLICATION_ID: AtomicU64 = AtomicU64::new(1);

    struct NoopHost;

    impl DesktopHost for NoopHost {
        fn execute(
            &self,
            _context: &DesktopHostContext,
            _action: DesktopHostAction,
        ) -> Result<DesktopHostResult, DesktopHostError> {
            Ok(DesktopHostResult::Completed)
        }
    }

    fn application() -> DesktopApplication {
        let id = NEXT_APPLICATION_ID.fetch_add(1, Ordering::Relaxed);
        let authority = ServiceAuthority::bootstrap_in_memory_named(
            format!("test:automation-ports:{id}"),
            format!("automation-ports-test:{id}"),
        )
        .unwrap();
        DesktopApplication::from_authority(
            DesktopApplicationConfig::new(
                "C:/lilia/automation-ports",
                format!("liliacode.automation-ports-test.{id}"),
            )
            .unwrap(),
            authority,
            Arc::new(NoopHost),
        )
        .unwrap()
    }

    fn application_with_task() -> (DesktopApplication, TaskId) {
        let application = application();
        let task_id = TaskId::new("existing-automation-task").unwrap();
        application
            .authority()
            .client()
            .unwrap()
            .products()
            .create_entity(ProductEntity::Task(
                ProductTask::new(task_id.clone(), None, "Existing task").unwrap(),
            ))
            .unwrap();
        (application, task_id)
    }

    fn context(run_id: &str, node_id: &str) -> AutomationPortContext {
        AutomationPortContext {
            idempotency_key: super::super::AutomationIdempotencyKey {
                run_id: run_id.to_owned(),
                node_id: node_id.to_owned(),
            },
            workflow_id: "workflow-1".to_owned(),
            workflow_version_id: "workflow-version-1".to_owned(),
            trigger: super::super::AutomationSignalEnvelope {
                id: "signal-1".to_owned(),
                kind: "manual".to_owned(),
                project_id: None,
                task_id: None,
                backend: None,
                event_kind: None,
                automation_run_id: None,
                payload: json!({}),
                created_at: 1,
            },
        }
    }

    #[test]
    fn task_creation_replay_returns_the_original_task_without_a_second_event() {
        let application = application();
        let operations = application.automation_operation_service();
        let events = application.subscribe_events();
        let request = AutomationCreateTaskRequest {
            context: context("run:1", "node/1"),
            project_id: None,
            title: "Ship desktop release".to_owned(),
            status: "running".to_owned(),
        };

        let first = AutomationTaskPort::create_task(&operations, request.clone()).unwrap();
        let replay = AutomationTaskPort::create_task(&operations, request).unwrap();

        assert_eq!(replay, first);
        let tasks = application.query_tasks(TaskQuery::default()).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Ship desktop release");
        assert!(events.recv().unwrap().is::<TasksChanged>());
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn todo_and_guide_replays_do_not_duplicate_rows_or_events() {
        let (application, task_id) = application_with_task();
        let operations = application.automation_operation_service();
        let events = application.subscribe_events();
        let todo_request = AutomationAddTodoRequest {
            context: context("run-2", "todo-node"),
            task_id: task_id.as_str().to_owned(),
            text: "Verify installer".to_owned(),
        };
        let guide_request = AutomationSendGuideRequest {
            context: context("run-2", "guide-node"),
            task_id: task_id.as_str().to_owned(),
            text: "Keep the old database unchanged".to_owned(),
            priority: "high".to_owned(),
        };

        let todo = AutomationTodoPort::add_todo(&operations, todo_request.clone()).unwrap();
        assert_eq!(
            AutomationTodoPort::add_todo(&operations, todo_request).unwrap(),
            todo
        );
        let guide = AutomationGuidePort::send_guide(&operations, guide_request.clone()).unwrap();
        assert_eq!(
            AutomationGuidePort::send_guide(&operations, guide_request).unwrap(),
            guide
        );

        let stored = application.list_task_todos(&task_id).unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].text, "Verify installer");
        assert_eq!(
            stored[1].guide_status,
            Some(DesktopTodoGuideStatus::Pending)
        );
        assert!(events.recv().unwrap().is::<TodosChanged>());
        assert!(events.recv().unwrap().is::<TodosChanged>());
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn timeline_replay_is_a_single_projection_and_single_event() {
        let (application, task_id) = application_with_task();
        let operations = application.automation_operation_service();
        let events = application.subscribe_events();
        let request = AutomationRecordTimelineRequest {
            context: context("run-3", "timeline-node"),
            task_id: task_id.as_str().to_owned(),
            title: "Automation checkpoint".to_owned(),
            summary: "Native execution reached the gate".to_owned(),
            status: "success".to_owned(),
            backend: None,
            payload: json!({ "gate": "native" }),
        };

        let first = AutomationTimelinePort::record_timeline(&operations, request.clone()).unwrap();
        let replay = AutomationTimelinePort::record_timeline(&operations, request).unwrap();

        assert_eq!(replay, first);
        let snapshot = application.task_session_snapshot(&task_id).unwrap();
        assert_eq!(snapshot.timeline.len(), 1);
        assert_eq!(snapshot.timeline[0].title, "Automation checkpoint");
        assert!(events.recv().unwrap().is::<TimelineChanged>());
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn stable_identity_is_ascii_and_does_not_alias_normalized_input() {
        let first = context("run:a", "node/b");
        let second = context("run-a", "node-b");

        let first = stable_identity("automation-task", &first);
        let second = stable_identity("automation-task", &second);

        assert!(first.is_ascii());
        assert_ne!(first, second);
    }

    #[test]
    fn desktop_application_executes_a_published_run_through_real_ports() {
        use super::super::{
            AutomationBeginRunInput, AutomationEdge, AutomationNode, AutomationNodePosition,
            AutomationRunStatus, AutomationSaveDraftInput, AutomationScopeFilter, GraphExecution,
        };

        let application = application();
        let workflow = application
            .save_automation_draft(AutomationSaveDraftInput {
                id: Some("real-port-workflow".to_owned()),
                name: "Real ports".to_owned(),
                scope: AutomationScopeFilter::default(),
                nodes: vec![
                    AutomationNode {
                        id: "trigger".to_owned(),
                        kind: "trigger".to_owned(),
                        title: "Trigger".to_owned(),
                        position: AutomationNodePosition { x: 0.0, y: 0.0 },
                        config: json!({}),
                    },
                    AutomationNode {
                        id: "create".to_owned(),
                        kind: "tool".to_owned(),
                        title: "Create task".to_owned(),
                        position: AutomationNodePosition { x: 100.0, y: 0.0 },
                        config: json!({
                            "action": "create_task",
                            "title": "${trigger.payload.title}",
                            "status": "waiting",
                        }),
                    },
                ],
                edges: vec![AutomationEdge {
                    id: "trigger-create".to_owned(),
                    source: "trigger".to_owned(),
                    target: "create".to_owned(),
                    source_handle: None,
                    target_handle: None,
                }],
            })
            .unwrap();
        application.publish_automation(&workflow.id).unwrap();
        let begun = application
            .begin_automation_run(AutomationBeginRunInput {
                expected_version_id: None,
                workflow_id: workflow.id,
                trigger: super::super::AutomationSignalEnvelope {
                    id: "real-port-signal".to_owned(),
                    kind: "manual".to_owned(),
                    project_id: None,
                    task_id: None,
                    backend: None,
                    event_kind: None,
                    automation_run_id: None,
                    payload: json!({ "title": "Created by real ports" }),
                    created_at: 1,
                },
            })
            .unwrap();

        let result = application.execute_automation_run(&begun.run.id).unwrap();

        assert_eq!(result.execution, GraphExecution::Finished);
        assert_eq!(result.detail.run.status, AutomationRunStatus::Succeeded);
        let tasks = application.query_tasks(TaskQuery::default()).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Created by real ports");
    }

    #[test]
    fn operation_port_preserves_run_and_confirmation_ownership() {
        use lilia_contracts::{AutomationOperationRequest, AutomationSignalEnvelope};
        use lilia_feature_automation::AutomationOperationPort;
        let app = application();
        let workflow = app.save_automation_draft(serde_json::from_value(json!({
            "id":"job-workflow", "name":"Confirm", "scope":{},
            "nodes":[
                {"id":"trigger","kind":"trigger","title":"Trigger","position":{"x":0,"y":0},"config":{}},
                {"id":"human","kind":"human","title":"Confirm","position":{"x":100,"y":0},"config":{"prompt":"Review"}}
            ],
            "edges":[{"id":"next","source":"trigger","target":"human"}]
        })).unwrap()).unwrap();
        let version = app.publish_automation(&workflow.id).unwrap();
        let trigger: AutomationSignalEnvelope =
            serde_json::from_value(json!({"id":"job-signal","kind":"manual","createdAt":1}))
                .unwrap();
        let context = lilia_kernel::JobContext::new();
        let operations = app.automation_operation_service();
        let started = operations
            .operate(
                AutomationOperationRequest::Start {
                    workflow_id: workflow.id.clone(),
                    expected_version_id: version.id,
                    trigger,
                },
                &context,
            )
            .unwrap();
        assert!(started.error.is_none());
        assert_eq!(
            app.automation_run_detail(&started.run_id)
                .unwrap()
                .unwrap()
                .run
                .status,
            AutomationRunStatus::WaitingUser
        );
        assert!(
            operations
                .operate(
                    AutomationOperationRequest::Cancel {
                        workflow_id: "other".into(),
                        run_id: started.run_id.clone()
                    },
                    &context
                )
                .is_err()
        );
        let wrong = operations
            .operate(
                AutomationOperationRequest::Resume {
                    workflow_id: workflow.id.clone(),
                    run_id: started.run_id.clone(),
                    node_id: "trigger".into(),
                    payload: None,
                },
                &context,
            )
            .unwrap();
        assert!(wrong.error.is_some());
        assert_eq!(
            app.automation_run_detail(&started.run_id)
                .unwrap()
                .unwrap()
                .run
                .status,
            AutomationRunStatus::WaitingUser
        );
        let completed = operations
            .operate(
                AutomationOperationRequest::Resume {
                    workflow_id: workflow.id,
                    run_id: started.run_id.clone(),
                    node_id: "human".into(),
                    payload: Some(json!({"response":"confirmed"})),
                },
                &context,
            )
            .unwrap();
        assert!(completed.error.is_none());
        assert_eq!(completed.run_id, started.run_id);
        let detail = app
            .automation_run_detail(&completed.run_id)
            .unwrap()
            .unwrap();
        assert_eq!(detail.run.status, AutomationRunStatus::Succeeded);
        assert_eq!(
            detail
                .nodes
                .iter()
                .find(|node| node.node_id == "human")
                .unwrap()
                .output
                .as_ref()
                .unwrap()["user"]["response"],
            "confirmed"
        );
    }
}

impl lilia_feature_automation::AutomationOperationPort for DesktopAutomationOperationService {
    fn operate(
        &self,
        request: lilia_contracts::AutomationOperationRequest,
        context: &lilia_kernel::JobContext,
    ) -> Result<lilia_contracts::AutomationOperationResult, String> {
        use lilia_contracts::{AutomationOperationRequest, AutomationOperationResult};
        let workflow_id = request.workflow_id().to_owned();
        if let Some(run_id) = request.run_id() {
            let detail = self
                .automation
                .run_detail(run_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "自动化运行已不存在。".to_owned())?;
            if detail.run.workflow_id != workflow_id {
                return Err("运行不属于所选自动化。".into());
            }
        }
        if context.is_cancelled() {
            return Err("操作已取消。".into());
        }
        let (run_id, result) = match request {
            AutomationOperationRequest::Start {
                workflow_id,
                expected_version_id,
                trigger,
            } => {
                let detail = self
                    .automation
                    .try_begin_run(super::AutomationBeginRunInput {
                        workflow_id,
                        trigger,
                        expected_version_id: Some(expected_version_id),
                    })
                    .map_err(automation_start_error)?;
                let run_id = detail.run.id;
                context.report(
                    serde_json::json!({"workflowId":detail.run.workflow_id,"runId":run_id}),
                );
                let result = if context.is_cancelled() {
                    self.cancel_run(&run_id).map(|_| ())
                } else {
                    self.execute_run(&run_id).map(|_| ())
                };
                (run_id, result)
            }
            AutomationOperationRequest::Resume {
                run_id,
                node_id,
                payload,
                ..
            } => {
                let result = self
                    .resume_run(
                        &run_id,
                        AutomationResumeRunInput {
                            node_id: Some(node_id),
                            payload,
                        },
                    )
                    .map(|_| ());
                (run_id, result)
            }
            AutomationOperationRequest::Cancel { run_id, .. } => {
                let result = self.cancel_run(&run_id).map(|_| ());
                (run_id, result)
            }
        };
        Ok(AutomationOperationResult {
            workflow_id,
            run_id,
            error: result.err().and_then(automation_operation_error),
        })
    }
}

fn automation_start_error(error: lilia_feature_automation::DesktopAutomationError) -> String {
    use lilia_feature_automation::{AutomationStoreError, DesktopAutomationError};
    match error {
        DesktopAutomationError::Store(AutomationStoreError::PublishedVersionChanged { .. }) => {
            "自动化已发布新版本，请重新运行。"
        }
        DesktopAutomationError::Store(AutomationStoreError::ActiveRunExists { .. }) => {
            "此自动化已有进行中的运行，请继续或取消该运行。"
        }
        DesktopAutomationError::Store(AutomationStoreError::PublishedVersionRequired {
            ..
        }) => "请先发布自动化再运行。",
        _ => "暂时无法启动自动化，请刷新后重试。",
    }
    .into()
}

fn automation_operation_error(error: AutomationExecutionError) -> Option<String> {
    use lilia_feature_automation::AutomationStoreError;
    match error {
        AutomationExecutionError::InvalidRunState {
            actual: AutomationRunStatus::Cancelled,
            ..
        }
        | AutomationExecutionError::InvalidNodeState {
            status: AutomationRunStatus::Cancelled,
            ..
        }
        | AutomationExecutionError::Store(AutomationStoreError::InvalidStateTransition {
            actual: AutomationRunStatus::Cancelled,
            ..
        }) => None,
        AutomationExecutionError::AgentCancellation { .. } => {
            Some("未能停止 Agent，请重试取消。".into())
        }
        AutomationExecutionError::WaitingNodeNotFound { .. }
        | AutomationExecutionError::InvalidRunState { .. } => {
            Some("运行状态已变化，请检查最新进度。".into())
        }
        _ => Some("自动化操作失败，请查看运行详情后重试。".into()),
    }
}
