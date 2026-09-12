use std::cell::Cell;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lilia_contracts::{PageRequest, PendingProjectionStatus, ProductEventSequence, TaskId};
use lilia_feature_automation::{
    automation_signal_matches, AutomationBeginRunInput, AutomationRunStatus,
    AutomationSignalEnvelope, AutomationStoreError, DesktopAutomationError,
};
use lilia_kernel::{EventBus, SubscriptionId};
use lilia_storage::SqliteAgentRuntimeStateStore;
use serde_json::{json, Value};

use super::{
    DesktopApplication, DesktopApplicationError, DesktopEvent, TimelineChanged, TodosChanged,
    TurnStateChanged,
};

const CURSOR_KEY: &str = "desktop.automation.product_cursor.v1";
thread_local! { static AUTOMATION_EFFECT: Cell<bool> = const { Cell::new(false) }; }

pub(super) struct AutomationEffectGuard(bool);
impl AutomationEffectGuard {
    pub(super) fn enter() -> Self {
        Self(AUTOMATION_EFFECT.replace(true))
    }
}
impl Drop for AutomationEffectGuard {
    fn drop(&mut self) {
        AUTOMATION_EFFECT.set(self.0);
    }
}

pub(crate) struct AutomationDispatcher {
    bus: EventBus,
    subscription: SubscriptionId,
}
impl Drop for AutomationDispatcher {
    fn drop(&mut self) {
        self.bus.unsubscribe(self.subscription);
    }
}

impl DesktopApplication {
    pub fn start_automation_dispatcher(&self) -> Result<(), DesktopApplicationError> {
        let mut slot = self
            .inner
            .automation_dispatcher
            .lock()
            .map_err(|_| DesktopApplicationError::StateUnavailable("automation_dispatcher"))?;
        if slot.is_some() {
            return Ok(());
        }
        let (sender, receiver) = mpsc::channel();
        let bus = self.event_bus();
        let subscription = bus.observe(None, move |event| {
            if !AUTOMATION_EFFECT.get()
                && (event.is::<TimelineChanged>()
                    || event.is::<TodosChanged>()
                    || event.is::<TurnStateChanged>())
            {
                let _ = sender.send(DesktopEvent::from_envelope(event.clone()));
            }
        });
        if let Err(error) = self
            .seed_automation_signal_cursor()
            .and_then(|_| self.capture_automation_sources_on_start())
        {
            bus.unsubscribe(subscription);
            return Err(error);
        }
        let weak = Arc::downgrade(&self.inner);
        let worker = std::thread::Builder::new()
            .name("lilia-automations".into())
            .spawn(move || {
                if let Some(inner) = weak.upgrade() {
                    if let Err(error) = (DesktopApplication { inner }).recover_automation_runs() {
                        eprintln!("[automation] {error}");
                    }
                }
                let mut next_poll = Instant::now();
                let mut dirty_tasks = std::collections::BTreeSet::new();
                loop {
                    let event = match receiver.recv_timeout(Duration::from_millis(250)) {
                        Ok(event) => Some(event),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let Some(inner) = weak.upgrade() else {
                        break;
                    };
                    let app = DesktopApplication { inner };
                    if let Some(event) = event {
                        if let Some(task) = event
                            .downcast::<TodosChanged>()
                            .map(|e| &e.task_id)
                            .or_else(|| event.downcast::<TimelineChanged>().map(|e| &e.task_id))
                            .or_else(|| event.downcast::<TurnStateChanged>().map(|e| &e.task_id))
                        {
                            dirty_tasks.insert(task.clone());
                        }
                    }
                    if Instant::now() >= next_poll {
                        dirty_tasks.retain(|task| {
                            match app.capture_automation_task_sources(task, false) {
                                Ok(()) => false,
                                Err(error) => {
                                    eprintln!("[automation] {error}");
                                    true
                                }
                            }
                        });
                        if let Err(error) = app.poll_automation_todo_signals() {
                            eprintln!("[automation] {error}");
                        }
                        if let Err(error) = app.poll_automation_product_signals() {
                            eprintln!("[automation] {error}");
                        }
                        if let Err(error) = app
                            .drain_automation_inbox(None)
                            .and_then(|_| app.recover_automation_runs())
                        {
                            eprintln!("[automation] {error}");
                        }
                        next_poll = Instant::now() + Duration::from_secs(1);
                    }
                }
            });
        if let Err(error) = worker {
            bus.unsubscribe(subscription);
            return Err(dispatch_error(error));
        }
        *slot = Some(AutomationDispatcher { bus, subscription });
        Ok(())
    }

    pub fn dispatch_automation_signal(
        &self,
        signal: AutomationSignalEnvelope,
    ) -> Result<Vec<String>, DesktopApplicationError> {
        let id = signal.id.clone();
        self.enqueue_automation_signal(signal)?;
        self.drain_automation_inbox(None)?;
        self.inner.automation_inbox.runs(&id)
    }

    fn enqueue_automation_signal(
        &self,
        mut signal: AutomationSignalEnvelope,
    ) -> Result<(), DesktopApplicationError> {
        let targets = self.automation_signal_targets(&mut signal)?;
        self.inner.automation_inbox.enqueue(&signal, &targets)
    }

    fn automation_signal_targets(
        &self,
        signal: &mut AutomationSignalEnvelope,
    ) -> Result<Vec<String>, DesktopApplicationError> {
        let mut targets = Vec::new();
        if signal.automation_run_id.is_none() {
            let mut eligible = true;
            if let Some(id) = signal.task_id.as_deref() {
                let task = match self.get_task(&TaskId::new(id)?) {
                    Ok(task) => task,
                    Err(DesktopApplicationError::Product(
                        lilia_contracts::ProductError::NotFound { .. },
                    )) => return Ok(targets),
                    Err(error) => return Err(error),
                };
                eligible = !task.archived;
                signal.project_id = task.project_id.map(|id| id.into_inner());
                if !signal.payload.is_object() {
                    signal.payload = json!({});
                }
                signal.payload["taskStatus"] =
                    serde_json::to_value(task.status).map_err(dispatch_error)?;
            }
            if eligible {
                let service = self.automation_service();
                for workflow in service.list_workflows()? {
                    if !workflow.enabled {
                        continue;
                    }
                    let Some(version_id) = workflow.published_version_id else {
                        continue;
                    };
                    if service.version(&version_id)?.is_some_and(|version| {
                        automation_signal_matches(&version.snapshot, &signal)
                    }) {
                        targets.push(workflow.id);
                    }
                }
            }
        }
        Ok(targets)
    }

    pub(super) fn drain_automation_inbox(
        &self,
        signal_id: Option<&str>,
    ) -> Result<(), DesktopApplicationError> {
        let _drain = self
            .inner
            .automation_delivery
            .lock()
            .map_err(|_| DesktopApplicationError::StateUnavailable("automation_delivery"))?;
        let service = self.automation_service();
        let mut blocked_workflows = std::collections::BTreeSet::new();
        for delivery in self.inner.automation_inbox.pending(signal_id)? {
            if blocked_workflows.contains(&delivery.workflow_id) {
                continue;
            }
            let detail = match service.try_begin_run(AutomationBeginRunInput {
                workflow_id: delivery.workflow_id.clone(),
                trigger: delivery.signal.clone(),
            }) {
                Ok(detail) => detail,
                Err(DesktopAutomationError::Store(AutomationStoreError::ActiveRunExists {
                    ..
                })) => {
                    blocked_workflows.insert(delivery.workflow_id.clone());
                    self.inner.automation_inbox.record(
                        &delivery.signal.id,
                        &delivery.workflow_id,
                        "pending",
                        None,
                        Some("workflow has an active run"),
                    )?;
                    continue;
                }
                Err(DesktopAutomationError::Store(
                    AutomationStoreError::SignalNotMatched { .. }
                    | AutomationStoreError::WorkflowNotFound { .. },
                )) => {
                    self.inner.automation_inbox.record(
                        &delivery.signal.id,
                        &delivery.workflow_id,
                        "skipped",
                        None,
                        Some("workflow removed, disabled, or published scope no longer matches"),
                    )?;
                    continue;
                }
                Err(error) => {
                    blocked_workflows.insert(delivery.workflow_id.clone());
                    self.inner.automation_inbox.record(
                        &delivery.signal.id,
                        &delivery.workflow_id,
                        "pending",
                        None,
                        Some(&error.to_string()),
                    )?;
                    continue;
                }
            };
            self.inner.automation_inbox.record(
                &delivery.signal.id,
                &delivery.workflow_id,
                "delivered",
                Some(&detail.run.id),
                None,
            )?;
            if detail.run.status == AutomationRunStatus::Running {
                if let Err(error) = self.execute_automation_run(&detail.run.id) {
                    eprintln!("[automation] {error}");
                }
            }
        }
        Ok(())
    }

    pub fn recover_automation_runs(&self) -> Result<(), DesktopApplicationError> {
        let mut last_error = None;
        for run in self.list_automation_runs(None)? {
            if run.status == AutomationRunStatus::Running {
                if let Err(error) = self.recover_automation_run(&run.id) {
                    last_error = Some(dispatch_error(error));
                }
            }
        }
        last_error.map_or(Ok(()), Err)
    }

    fn recover_automation_run(&self, run_id: &str) -> Result<(), DesktopApplicationError> {
        let _execution = self.inner.automation_execution.enter(run_id);
        let Some(mut detail) = self.automation_run_detail(run_id)? else {
            return Ok(());
        };
        if detail.run.status != AutomationRunStatus::Running {
            return Ok(());
        }
        if let Some(waiting) = detail.nodes.iter().find(|node| {
            node.status == AutomationRunStatus::Running
                && node
                    .output
                    .as_ref()
                    .and_then(|output| output.get("waitingAgent"))
                    .and_then(Value::as_bool)
                    == Some(true)
        }) {
            let output = waiting
                .output
                .as_ref()
                .expect("waiting Agent output checked");
            if let (Some(task_id), Some(turn_id)) = (
                output.get("taskId").and_then(Value::as_str),
                output.get("turnId").and_then(Value::as_str),
            ) {
                if let Some(status) =
                    self.persisted_turn_terminal_status(&TaskId::new(task_id)?, turn_id)?
                {
                    detail = self
                        .complete_automation_agent_turn(super::AutomationCompleteAgentInput {
                            run_id: run_id.to_owned(),
                            node_id: Some(waiting.node_id.clone()),
                            turn_id: turn_id.to_owned(),
                            success: status == "completed",
                            payload: Some(json!({"taskId": task_id, "turnId": turn_id})),
                            error: (status != "completed")
                                .then(|| format!("Native Agent turn ended with status {status}")),
                        })
                        .map_err(dispatch_error)?
                        .detail;
                }
            }
        }
        if detail.run.status == AutomationRunStatus::Running {
            self.execute_automation_run(run_id)
                .map_err(dispatch_error)?;
        }
        Ok(())
    }

    fn automation_cursor_store(
        &self,
    ) -> Result<&SqliteAgentRuntimeStateStore, DesktopApplicationError> {
        Ok(&self.inner.automation_cursor)
    }

    fn seed_automation_signal_cursor(&self) -> Result<(), DesktopApplicationError> {
        let store = self.automation_cursor_store()?;
        if store.setting(CURSOR_KEY)?.is_some() {
            return Ok(());
        }
        let mut cursor = 0;
        loop {
            let page = self.authority().client()?.product_events(&PageRequest {
                after: Some(ProductEventSequence::new(cursor)),
                limit: 200,
            })?;
            if let Some(event) = page.items.last() {
                cursor = event.sequence.get();
            }
            if page.next.is_none() || page.items.is_empty() {
                break;
            }
        }
        store.put_setting(CURSOR_KEY, &json!(cursor))?;
        Ok(())
    }

    #[cfg(debug_assertions)]
    pub fn automation_debug_state(&self) -> Result<Value, DesktopApplicationError> {
        let cursor = self
            .automation_cursor_store()?
            .setting(CURSOR_KEY)?
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mut after = 0;
        let mut events = Vec::new();
        loop {
            let page = self.authority().client()?.product_events(&PageRequest {
                after: Some(ProductEventSequence::new(after)),
                limit: 200,
            })?;
            for event in &page.items {
                after = event.sequence.get();
                if event.entity == "task" && event.action.contains("creat") {
                    events.push(
                        json!({"sequence":after,"taskId":event.entity_id,"action":event.action}),
                    );
                }
            }
            if page.next.is_none() || page.items.is_empty() {
                break;
            }
        }
        let mut runs = Vec::new();
        for run in self.list_automation_runs(None)? {
            if let Some(detail) = self.automation_run_detail(&run.id)? {
                runs.push(detail);
            }
        }
        Ok(
            json!({"productCursor":cursor,"latestProductSequence":after,"taskCreatedEvents":events,"runs":runs}),
        )
    }

    pub fn poll_automation_product_signals(&self) -> Result<(), DesktopApplicationError> {
        let store = self.automation_cursor_store()?;
        let mut cursor = store
            .setting(CURSOR_KEY)?
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        loop {
            let page = self.authority().client()?.product_events(&PageRequest {
                after: Some(ProductEventSequence::new(cursor)),
                limit: 200,
            })?;
            if page.items.is_empty() {
                break;
            }
            for event in &page.items {
                if event.entity == "task"
                    && !event.command_id.starts_with("automation:")
                    && !event.action.starts_with("automation_")
                {
                    if let Ok(task_id) = TaskId::new(&event.entity_id) {
                        {
                            let event_kind = if event.action.contains("create") {
                                "task_created"
                            } else if event.action.contains("status") {
                                "task_status_changed"
                            } else {
                                "task_updated"
                            };
                            let mut signal = signal(
                                format!("product:{}", event.sequence.get()),
                                "task_changed",
                                task_id,
                            );
                            signal.event_kind = Some(event_kind.into());
                            signal.payload =
                                json!({ "action": event.action, "revision": event.revision });
                            self.enqueue_automation_signal(signal)?;
                        }
                    }
                }
                cursor = event.sequence.get();
                store.put_setting(CURSOR_KEY, &json!(cursor))?;
            }
            if page.next.is_none() {
                break;
            }
        }
        self.drain_automation_inbox(None)
    }

    fn poll_automation_todo_signals(&self) -> Result<(), DesktopApplicationError> {
        let _capture = self
            .inner
            .automation_capture
            .lock()
            .map_err(|_| DesktopApplicationError::StateUnavailable("automation_capture"))?;
        let mut cursor = self
            .inner
            .automation_inbox
            .source("local-todo-cursor")?
            .map(|(_, revision)| revision)
            .unwrap_or(0);
        loop {
            let rows = self.inner.automation_inbox.todo_events_after(cursor)?;
            if rows.is_empty() {
                break;
            }
            for (sequence, task, todo, action, payload) in rows {
                let mut event = signal(
                    format!("local-todo:{sequence}"),
                    "todo_changed",
                    TaskId::new(task)?,
                );
                event.payload = json!({ "todoId": todo, "action": action, "todo": payload });
                let automated_creation = action == "created"
                    && (todo.starts_with("automation-todo-")
                        || todo.starts_with("automation-guide-"));
                let targets = if automated_creation {
                    Vec::new()
                } else {
                    self.automation_signal_targets(&mut event)?
                };
                self.inner.automation_inbox.enqueue_with_checkpoint(
                    &event,
                    &targets,
                    Some(("local-todo-cursor", &sequence.to_string(), sequence)),
                )?;
                cursor = sequence;
            }
        }
        Ok(())
    }

    fn capture_automation_sources_on_start(&self) -> Result<(), DesktopApplicationError> {
        let initial = self.inner.automation_inbox.source("initialized")?.is_none();
        if initial {
            let cursor = self.inner.automation_inbox.todo_cursor()?;
            self.inner.automation_inbox.checkpoint(
                "local-todo-cursor",
                &cursor.to_string(),
                cursor,
            )?;
        }
        for task in self.query_tasks(super::TaskQuery::default().including_archived())? {
            self.capture_automation_task_sources(&task.id, initial)?;
        }
        self.poll_automation_todo_signals()?;
        self.inner
            .automation_inbox
            .checkpoint("initialized", "true", 1)
    }

    fn capture_automation_task_sources(
        &self,
        task_id: &TaskId,
        seed: bool,
    ) -> Result<(), DesktopApplicationError> {
        let _capture = self
            .inner
            .automation_capture
            .lock()
            .map_err(|_| DesktopApplicationError::StateUnavailable("automation_capture"))?;
        let snapshot = match self.task_session_snapshot(task_id) {
            Ok(snapshot) => snapshot,
            Err(DesktopApplicationError::Product(lilia_contracts::ProductError::NotFound {
                ..
            })) => return Ok(()),
            Err(error) => return Err(error),
        };
        for row in &snapshot.timeline {
            let origin = self.automation_turn_origin(task_id, row.turn_id.as_deref())?;
            let automation_run = origin.or_else(|| {
                row.payload
                    .get("automationRunId")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
            let suppressed = seed
                || row.agent_session.as_str().starts_with("automation:")
                || automation_run.is_some();
            let mut event = signal(String::new(), "timeline_event", task_id.clone());
            event.automation_run_id = automation_run.clone();
            event.payload = json!({ "timelineEvent": row, "timelineEventKind": row.kind });
            self.capture_automation_source(
                &format!("timeline:{}", row.id.as_str()),
                serde_json::to_string(row).map_err(dispatch_error)?,
                event,
                suppressed,
            )?;
            if row.kind == "todo_list" {
                let mut event = signal(String::new(), "todo_changed", task_id.clone());
                event.automation_run_id = automation_run;
                event.payload = json!({ "todo": row.payload.get("todo"), "timelineEvent": row });
                self.capture_automation_source(
                    &format!("agent-todo:{}", row.id.as_str()),
                    serde_json::to_string(row).map_err(dispatch_error)?,
                    event,
                    suppressed,
                )?;
            }
        }
        for pending in snapshot
            .pending
            .iter()
            .filter(|p| p.status == PendingProjectionStatus::Open)
        {
            let origin = self.automation_turn_origin(task_id, pending.turn_id.as_deref())?;
            let mut event = signal(String::new(), "interaction_request", task_id.clone());
            event.automation_run_id = origin.clone();
            event.payload = json!({ "requestId": pending.request_id, "interactionKind": pending.kind, "prompt": pending.prompt, "payload": pending.payload });
            self.capture_automation_source(
                &format!("interaction:{}", pending.id),
                serde_json::to_string(pending).map_err(dispatch_error)?,
                event,
                seed || origin.is_some(),
            )?;
        }
        Ok(())
    }

    fn capture_automation_source(
        &self,
        key: &str,
        value: String,
        mut event: AutomationSignalEnvelope,
        seed: bool,
    ) -> Result<(), DesktopApplicationError> {
        let previous = self.inner.automation_inbox.source(key)?;
        if previous.as_ref().is_some_and(|(old, _)| old == &value) {
            return Ok(());
        }
        let revision = previous.map(|(_, revision)| revision + 1).unwrap_or(1);
        event.id = format!("source:{key}:{revision}");
        let targets = if seed {
            Vec::new()
        } else {
            self.automation_signal_targets(&mut event)?
        };
        self.inner.automation_inbox.enqueue_with_checkpoint(
            &event,
            &targets,
            Some((key, &value, revision)),
        )
    }

    fn automation_turn_origin(
        &self,
        task_id: &TaskId,
        turn_id: Option<&str>,
    ) -> Result<Option<String>, DesktopApplicationError> {
        let Some(turn_id) = turn_id else {
            return Ok(None);
        };
        let key = format!("turn:{}:{turn_id}", task_id.as_str());
        if let Some(origin) = self.inner.automation_inbox.origin(&key)? {
            return Ok((!origin.is_empty()).then_some(origin));
        }
        let pending = self
            .inner
            .pending_turns
            .lock()
            .map_err(|_| DesktopApplicationError::StateUnavailable("pending_turns"))?
            .list(task_id)?;
        if let Some(correlation) = pending
            .iter()
            .find(|turn| turn.turn_id == turn_id)
            .and_then(|turn| turn.request.automation.as_ref())
        {
            self.inner
                .automation_inbox
                .record_origin(&key, &correlation.run_id)?;
            return Ok(Some(correlation.run_id.clone()));
        }
        // Upgrade existing installations, including already completed Agent nodes.
        for run in self.list_automation_runs(None)? {
            if self.automation_run_detail(&run.id)?.is_some_and(|detail| {
                detail.nodes.iter().any(|node| {
                    node.output.as_ref().is_some_and(|output| {
                        output.get("taskId").and_then(Value::as_str) == Some(task_id.as_str())
                            && output.get("turnId").and_then(Value::as_str) == Some(turn_id)
                    })
                })
            }) {
                self.inner.automation_inbox.record_origin(&key, &run.id)?;
                return Ok(Some(run.id));
            }
        }
        self.inner.automation_inbox.record_origin(&key, "")?;
        Ok(None)
    }
}

fn signal(id: String, kind: &str, task_id: TaskId) -> AutomationSignalEnvelope {
    AutomationSignalEnvelope {
        id,
        kind: kind.into(),
        project_id: None,
        task_id: Some(task_id.into_inner()),
        backend: Some("native-agentkit".into()),
        event_kind: Some(kind.into()),
        automation_run_id: None,
        payload: json!({}),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(i64::MAX),
    }
}

fn dispatch_error(error: impl std::fmt::Display) -> DesktopApplicationError {
    DesktopApplicationError::InvalidInput {
        field: "automation_dispatch",
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        DesktopApplicationConfig, DesktopHost, DesktopHostAction, DesktopHostContext,
        DesktopHostError, DesktopHostResult, DesktopTaskCreate, DesktopTaskPatch,
    };
    use lilia_feature_automation::{
        AutomationEdge, AutomationNode, AutomationNodePosition, AutomationResumeRunInput,
        AutomationSaveDraftInput, AutomationScopeFilter,
    };

    struct Host;
    impl DesktopHost for Host {
        fn execute(
            &self,
            _: &DesktopHostContext,
            _: DesktopHostAction,
        ) -> Result<DesktopHostResult, DesktopHostError> {
            Ok(DesktopHostResult::Completed)
        }
    }

    fn app() -> DesktopApplication {
        let id = uuid::Uuid::new_v4().to_string();
        let authority = lilia_service::ServiceAuthority::bootstrap_in_memory_named(
            format!("automation:{id}"),
            format!("automation-{id}"),
        )
        .unwrap();
        DesktopApplication::from_authority(
            DesktopApplicationConfig::new(
                std::env::temp_dir().join(&id),
                format!("liliacode.automation.{id}"),
            )
            .unwrap(),
            authority,
            Arc::new(Host),
        )
        .unwrap()
    }

    fn draft(kind: &str, human: bool) -> AutomationSaveDraftInput {
        let node = |id: &str, kind: &str, config| AutomationNode {
            id: id.into(),
            kind: kind.into(),
            title: id.into(),
            position: AutomationNodePosition { x: 0.0, y: 0.0 },
            config,
        };
        let edge = |source: &str, target: &str| AutomationEdge {
            id: format!("{source}-{target}"),
            source: source.into(),
            target: target.into(),
            source_handle: Some("output".into()),
            target_handle: Some("input".into()),
        };
        let mut nodes = vec![
            node("trigger", "trigger", json!({ "triggerKind": kind })),
            node(
                "todo",
                "tool",
                json!({ "action": "add_todo", "taskId": "${trigger.taskId}", "text": "只创建一次" }),
            ),
        ];
        let mut edges = vec![edge("trigger", "todo")];
        if human {
            nodes.push(node("human", "human", json!({ "prompt": "是否继续？" })));
            edges.push(edge("todo", "human"));
        }
        AutomationSaveDraftInput {
            id: None,
            name: "事件自动化".into(),
            scope: AutomationScopeFilter {
                include_inbox: true,
                backends: vec!["native-agentkit".into()],
                ..Default::default()
            },
            nodes,
            edges,
        }
    }

    fn waiting_agent_run(app: &DesktopApplication, task_id: &TaskId) -> String {
        use lilia_feature_automation::{
            AutomationExecutionRepository, AutomationExecutionTransition,
            AutomationNodeStateUpdate, AutomationRunStateUpdate,
        };
        let mut input = draft("task_changed", false);
        input.nodes.insert(
            1,
            AutomationNode {
                id: "agent".into(),
                kind: "agent".into(),
                title: "Agent".into(),
                position: AutomationNodePosition { x: 100.0, y: 0.0 },
                config: json!({}),
            },
        );
        input.edges = vec![
            AutomationEdge {
                id: "to-agent".into(),
                source: "trigger".into(),
                target: "agent".into(),
                source_handle: None,
                target_handle: None,
            },
            AutomationEdge {
                id: "after-agent".into(),
                source: "agent".into(),
                target: "todo".into(),
                source_handle: None,
                target_handle: None,
            },
        ];
        let workflow = app.save_automation_draft(input).unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        let detail = app
            .begin_automation_run(AutomationBeginRunInput {
                workflow_id: workflow.id,
                trigger: signal(
                    "waiting-agent-source".into(),
                    "task_changed",
                    task_id.clone(),
                ),
            })
            .unwrap();
        app.automation_service().apply_execution_transition(AutomationExecutionTransition {
            run_id: detail.run.id.clone(),
            run: AutomationRunStateUpdate { expected_statuses: vec![AutomationRunStatus::Running], status: AutomationRunStatus::Running, error: None, finished: false },
            nodes: vec![
                AutomationNodeStateUpdate { node_id: "trigger".into(), expected_statuses: vec![AutomationRunStatus::Pending], status: AutomationRunStatus::Succeeded, input: json!({}), output: Some(json!({})), error: None, mark_started: true, finished: true },
                AutomationNodeStateUpdate { node_id: "agent".into(), expected_statuses: vec![AutomationRunStatus::Pending], status: AutomationRunStatus::Running, input: json!({}), output: Some(json!({"waitingAgent":true,"taskId":task_id.as_str(),"turnId":"durable-agent-turn"})), error: None, mark_started: true, finished: false },
            ],
        }).unwrap();
        detail.run.id
    }

    fn app_at(home: &std::path::Path) -> DesktopApplication {
        DesktopApplication::from_authority(
            DesktopApplicationConfig::new(home, "liliacode.automation-recovery").unwrap(),
            lilia_service::ServiceAuthority::bootstrap_with_home(home).unwrap(),
            Arc::new(Host),
        )
        .unwrap()
    }

    #[test]
    fn recovery_reconciles_durable_agent_completion_after_restart_and_write_failure_once() {
        use mutsuki_agent_contracts::{AgentEvent, AgentEventEnvelope, AgentEventMeta};
        for status in ["completed", "cancelled", "failed"] {
            let home =
                std::env::temp_dir().join(format!("automation-reconcile-{}", uuid::Uuid::new_v4()));
            let (task_id, run_id) = {
                let app = app_at(&home);
                let task = app
                    .create_task(DesktopTaskCreate::new(None, "recovery source"))
                    .unwrap();
                let run_id = waiting_agent_run(&app, &task.id);
                let mut session = app
                    .authority()
                    .open_agent_task_session(
                        &task.id,
                        Some("recovery-session"),
                        "mutsuki.reference.coding-agent",
                        None,
                    )
                    .unwrap();
                app.persist_session_binding(
                    &task.id,
                    &session.session_id,
                    "mutsuki.reference.coding-agent",
                )
                .unwrap();
                session.next_event_sequence += 1;
                session.events.push(AgentEventEnvelope {
                    session_id: session.session_id.clone(),
                    sequence: session.next_event_sequence,
                    meta: AgentEventMeta::new(
                        "terminal-recovery",
                        "terminal persisted before callback",
                    )
                    .with_turn("durable-agent-turn"),
                    event: AgentEvent::TurnState {
                        turn_id: "durable-agent-turn".into(),
                        status: status.into(),
                    },
                });
                let store = SqliteAgentRuntimeStateStore::open(
                    app.authority().data_paths().unwrap().agent_runtime_db(),
                )
                .unwrap();
                store
                    .put_session(
                        &format!("agentkit-session/{}", session.session_id),
                        &serde_json::to_value(session).unwrap(),
                    )
                    .unwrap();
                assert!(app.list_task_todos(&task.id).unwrap().is_empty());
                (task.id, run_id)
            };
            let app = app_at(&home);
            assert_eq!(
                app.authority()
                    .list_session_bindings(&task_id)
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                app.persisted_turn_terminal_status(&task_id, "durable-agent-turn")
                    .unwrap()
                    .as_deref(),
                Some(status)
            );
            app.domain_db().lock().execute_batch("CREATE TEMP TRIGGER fail_automation_completion BEFORE UPDATE ON automation_run_nodes BEGIN SELECT RAISE(ABORT, 'transient completion failure'); END;").unwrap();
            assert!(app.recover_automation_runs().is_err());
            assert_eq!(
                app.automation_run_detail(&run_id)
                    .unwrap()
                    .unwrap()
                    .run
                    .status,
                AutomationRunStatus::Running
            );
            assert!(app.list_task_todos(&task_id).unwrap().is_empty());
            app.domain_db()
                .lock()
                .execute_batch("DROP TRIGGER fail_automation_completion;")
                .unwrap();
            app.recover_automation_runs().unwrap();
            app.recover_automation_runs().unwrap();
            let expected = if status == "completed" {
                AutomationRunStatus::Succeeded
            } else {
                AutomationRunStatus::Failed
            };
            assert_eq!(
                app.automation_run_detail(&run_id)
                    .unwrap()
                    .unwrap()
                    .run
                    .status,
                expected
            );
            assert_eq!(
                app.list_task_todos(&task_id).unwrap().len(),
                usize::from(status == "completed")
            );
            assert!(app
                .inner
                .pending_turns
                .lock()
                .unwrap()
                .list(&task_id)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn recovery_keeps_unknown_agent_turn_waiting_without_starting_another_turn() {
        let app = app();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "unknown turn"))
            .unwrap();
        let run_id = waiting_agent_run(&app, &task.id);
        app.recover_automation_runs().unwrap();
        app.recover_automation_runs().unwrap();
        assert_eq!(
            app.automation_run_detail(&run_id)
                .unwrap()
                .unwrap()
                .run
                .status,
            AutomationRunStatus::Running
        );
        assert!(app.list_task_todos(&task.id).unwrap().is_empty());
        assert!(app
            .inner
            .pending_turns
            .lock()
            .unwrap()
            .list(&task.id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn published_signal_wait_resume_cancel_and_replay_preserve_authoritative_state() {
        let app = app();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "事件来源"))
            .unwrap();
        let mut input = draft("task_changed", true);
        input.scope.event_kinds = vec!["task_status_changed".into()];
        let workflow = app.save_automation_draft(input.clone()).unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        input.id = Some(workflow.id.clone());
        input.nodes[0].config = json!({ "triggerKind": "todo_changed" });
        input.scope.include_inbox = false;
        app.save_automation_draft(input).unwrap();

        let mut trigger = signal("stable-event".into(), "task_changed", task.id.clone());
        trigger.event_kind = Some("task_status_changed".into());
        let first = app.dispatch_automation_signal(trigger.clone()).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(
            app.automation_run_detail(&first[0])
                .unwrap()
                .unwrap()
                .run
                .status,
            AutomationRunStatus::WaitingUser
        );
        assert_eq!(
            app.dispatch_automation_signal(trigger.clone()).unwrap(),
            first
        );
        assert_eq!(app.list_task_todos(&task.id).unwrap().len(), 1);
        app.recover_automation_runs().unwrap();
        assert_eq!(
            app.automation_run_detail(&first[0])
                .unwrap()
                .unwrap()
                .run
                .status,
            AutomationRunStatus::WaitingUser
        );
        app.resume_automation_run(
            &first[0],
            AutomationResumeRunInput {
                node_id: Some("human".into()),
                payload: Some(json!({ "response": "同意" })),
            },
        )
        .unwrap();
        assert_eq!(
            app.automation_run_detail(&first[0])
                .unwrap()
                .unwrap()
                .run
                .status,
            AutomationRunStatus::Succeeded
        );
        assert_eq!(
            app.dispatch_automation_signal(trigger.clone()).unwrap(),
            first
        );
        assert_eq!(app.list_task_todos(&task.id).unwrap().len(), 1);
        trigger.id = "next-event".into();
        let second = app.dispatch_automation_signal(trigger.clone()).unwrap();
        app.cancel_automation_run(&second[0]).unwrap();
        assert_eq!(
            app.automation_run_detail(&second[0])
                .unwrap()
                .unwrap()
                .run
                .status,
            AutomationRunStatus::Cancelled
        );
        trigger.id = "recursive-event".into();
        trigger.automation_run_id = Some(second[0].clone());
        assert!(app.dispatch_automation_signal(trigger).unwrap().is_empty());
        assert_eq!(
            app.list_automation_runs(Some(&workflow.id)).unwrap().len(),
            2
        );
    }

    #[test]
    fn durable_task_status_events_dispatch_once_without_treating_title_changes_as_status() {
        let app = app();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "来源"))
            .unwrap();
        app.seed_automation_signal_cursor().unwrap();
        let mut input = draft("task_changed", false);
        input.scope.event_kinds = vec!["task_status_changed".into()];
        let workflow = app.save_automation_draft(input).unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        app.update_task(
            &task.id,
            DesktopTaskPatch {
                title: Some("改名".into()),
                ..Default::default()
            },
        )
        .unwrap();
        app.poll_automation_product_signals().unwrap();
        assert!(app.list_automation_runs(None).unwrap().is_empty());
        app.update_task(
            &task.id,
            DesktopTaskPatch {
                status: Some(lilia_contracts::ProductTaskStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
        app.poll_automation_product_signals().unwrap();
        app.poll_automation_product_signals().unwrap();
        assert_eq!(app.list_automation_runs(None).unwrap().len(), 1);
        assert_eq!(app.list_task_todos(&task.id).unwrap().len(), 1);
    }

    #[test]
    fn service_subscription_runs_without_an_open_automation_page() {
        let app = app();
        let historical = app
            .create_task(DesktopTaskCreate::new(None, "启用前任务"))
            .unwrap();
        let workflow = app
            .save_automation_draft(draft("task_changed", false))
            .unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        app.start_automation_dispatcher().unwrap();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "服务事件"))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.list_task_todos(&task.id).unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(app.list_task_todos(&task.id).unwrap().len(), 1);
        assert_eq!(app.list_automation_runs(None).unwrap().len(), 1);
        assert!(app.list_task_todos(&historical.id).unwrap().is_empty());
        let weak = Arc::downgrade(&app.inner);
        drop(app);
        let deadline = Instant::now() + Duration::from_secs(2);
        while weak.upgrade().is_some() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            weak.upgrade().is_none(),
            "the dispatcher must not retain its application"
        );
    }

    #[test]
    fn busy_workflow_retains_each_signal_until_the_previous_run_finishes() {
        let app = app();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "busy source"))
            .unwrap();
        let workflow = app
            .save_automation_draft(draft("task_changed", true))
            .unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        let first = app
            .dispatch_automation_signal(signal(
                "busy-first".into(),
                "task_changed",
                task.id.clone(),
            ))
            .unwrap();
        assert_eq!(first.len(), 1);
        assert!(app
            .dispatch_automation_signal(signal(
                "busy-second".into(),
                "task_changed",
                task.id.clone()
            ))
            .unwrap()
            .is_empty());
        assert_eq!(app.inner.automation_inbox.pending(None).unwrap().len(), 1);
        app.resume_automation_run(
            &first[0],
            AutomationResumeRunInput {
                node_id: Some("human".into()),
                payload: Some(json!({"response":"continue"})),
            },
        )
        .unwrap();
        app.drain_automation_inbox(None).unwrap();
        let second = app.inner.automation_inbox.runs("busy-second").unwrap();
        assert_eq!(second.len(), 1);
        assert_ne!(first, second);
        assert_eq!(app.list_task_todos(&task.id).unwrap().len(), 2);
        assert!(app.inner.automation_inbox.pending(None).unwrap().is_empty());
    }

    fn append_source_row(
        app: &DesktopApplication,
        task: &TaskId,
        turn: &str,
        sequence: u64,
        kind: &str,
    ) {
        use lilia_contracts::{
            AgentSessionRef, ProjectionEventId, TimelineProjectionCommand, TimelineProjectionEvent,
        };
        app.authority()
            .apply_projection(TimelineProjectionCommand::UpsertTimelineEvent {
                event: TimelineProjectionEvent {
                    id: ProjectionEventId::new(format!("signal-row-{turn}-{sequence}")),
                    task_id: task.clone(),
                    agent_session: AgentSessionRef::new("native-signal-session").unwrap(),
                    sequence,
                    turn_id: Some(turn.into()),
                    kind: kind.into(),
                    status: "completed".into(),
                    title: kind.into(),
                    summary: None,
                    payload: json!({"todo":{"items":[]}}),
                    projected: true,
                },
            })
            .unwrap();
    }

    #[test]
    fn startup_capture_replays_unconsumed_rows_and_maps_agent_todos_once() {
        let app = app();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "replay source"))
            .unwrap();
        let workflow = app
            .save_automation_draft(draft("todo_changed", false))
            .unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        app.capture_automation_sources_on_start().unwrap();
        // These durable facts arrive without an in-process notification.
        append_source_row(&app, &task.id, "user-turn", 1, "todo_list");
        append_source_row(&app, &task.id, "user-turn", 2, "todo_list");
        app.capture_automation_sources_on_start().unwrap();
        assert_eq!(app.inner.automation_inbox.pending(None).unwrap().len(), 2);
        app.drain_automation_inbox(None).unwrap();
        assert_eq!(
            app.list_automation_runs(Some(&workflow.id)).unwrap().len(),
            2
        );
        app.capture_automation_sources_on_start().unwrap();
        app.drain_automation_inbox(None).unwrap();
        assert_eq!(
            app.list_automation_runs(Some(&workflow.id)).unwrap().len(),
            2
        );
    }

    #[test]
    fn completed_agent_origin_survives_queue_ack_and_suppresses_late_events() {
        let app = app();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "automation source"))
            .unwrap();
        let workflow = app
            .save_automation_draft(draft("timeline_event", false))
            .unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        app.capture_automation_sources_on_start().unwrap();
        app.inner
            .automation_inbox
            .record_origin(
                &format!("turn:{}:completed-agent", task.id.as_str()),
                "original-run",
            )
            .unwrap();
        assert!(app
            .inner
            .pending_turns
            .lock()
            .unwrap()
            .list(&task.id)
            .unwrap()
            .is_empty());
        append_source_row(&app, &task.id, "completed-agent", 1, "assistant_message");
        app.capture_automation_task_sources(&task.id, false)
            .unwrap();
        app.drain_automation_inbox(None).unwrap();
        assert!(app.list_automation_runs(None).unwrap().is_empty());
        append_source_row(&app, &task.id, "user-turn", 2, "assistant_message");
        app.capture_automation_task_sources(&task.id, false)
            .unwrap();
        app.drain_automation_inbox(None).unwrap();
        assert_eq!(app.list_automation_runs(None).unwrap().len(), 1);
    }

    #[test]
    fn local_todo_journal_preserves_create_and_delete_between_notifications() {
        let app = app();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "todo journal"))
            .unwrap();
        let mut input = draft("todo_changed", false);
        input.nodes[1].config["action"] = json!("send_guide");
        let workflow = app.save_automation_draft(input).unwrap();
        app.publish_automation(&workflow.id).unwrap();
        app.set_automation_enabled(&workflow.id, true).unwrap();
        app.capture_automation_sources_on_start().unwrap();
        let todo = app
            .create_task_todo(super::super::DesktopTodoCreate {
                task_id: task.id.clone(),
                text: "transient todo".into(),
                priority: super::super::DesktopTodoPriority::Normal,
                attachments: Vec::new(),
                conversation_references: Vec::new(),
                workflow: None,
            })
            .unwrap();
        app.delete_task_todo(&todo.id).unwrap();
        app.poll_automation_todo_signals().unwrap();
        assert_eq!(app.inner.automation_inbox.pending(None).unwrap().len(), 2);
        app.drain_automation_inbox(None).unwrap();
        app.poll_automation_todo_signals().unwrap();
        app.drain_automation_inbox(None).unwrap();
        assert_eq!(
            app.list_automation_runs(Some(&workflow.id)).unwrap().len(),
            2
        );
        let generated = app.list_task_todos(&task.id).unwrap();
        assert_eq!(generated.len(), 2);
        app.update_task_todo(
            &generated[0].id,
            super::super::DesktopTodoUpdate {
                text: Some("user edited automated todo".into()),
                ..Default::default()
            },
        )
        .unwrap();
        app.poll_automation_todo_signals().unwrap();
        app.drain_automation_inbox(None).unwrap();
        assert_eq!(
            app.list_automation_runs(Some(&workflow.id)).unwrap().len(),
            3
        );
    }
}
