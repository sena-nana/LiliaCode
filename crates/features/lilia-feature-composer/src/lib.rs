//! Composer domain feature.
//!
//! Owns the durable per-task draft: its reducer, its optimistic revision and
//! its SQLite home. Turn dispatch lives in the agent-session domain; this crate
//! only guarantees that a draft mutation is revision-safe and durable.

mod prompt;
mod state;
mod store;

use std::sync::Arc;

use lilia_contracts::TaskId;
use lilia_kernel::{
    Event, EventBus, Feature, FeatureContext, FeatureId, JobContext, JobProtocol, KernelError,
    ServiceKey, ServiceRef,
};
use serde_json::Value;

pub use prompt::{
    optimize_prompt_slot, PromptOptimizeInput, PromptOptimizePort, PromptOptimizeResult,
    PromptRoute, OPTIMIZE_PROMPT_PROTOCOL,
};
pub use state::{
    ensure_expected_revision, ComposerCommand, ComposerState, ContentAtomKind, ContentAtomSpan,
};
pub use store::ComposerStore;

#[derive(Debug, thiserror::Error)]
pub enum ComposerError {
    #[error("composer task {task_id} is unavailable: {message}")]
    TaskUnavailable { task_id: TaskId, message: String },
    #[error("composer content changed before paste")]
    ContentConflict,
    #[error("composer revision overflowed")]
    RevisionOverflow,
    #[error("composer revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("composer serialization failed for {field}: {message}")]
    Serialization {
        field: &'static str,
        message: String,
    },
    #[error("composer storage failed during {operation}: {message}")]
    Storage {
        operation: &'static str,
        message: String,
    },
}

/// Published whenever a draft reaches a new revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerChanged {
    pub task_id: TaskId,
    pub revision: u64,
}

impl Event for ComposerChanged {
    const NAME: &'static str = "lilia.composer.changed";
}

/// Product authority required before reading or mutating a durable draft.
pub trait ComposerTaskAuthority: Send + Sync {
    fn ensure_task(&self, task_id: &TaskId) -> Result<(), ComposerError>;
}

/// Authority over composer drafts.
pub struct ComposerService {
    store: Arc<ComposerStore>,
    authority: Arc<dyn ComposerTaskAuthority>,
    events: EventBus,
}

impl ComposerService {
    pub fn new(
        store: Arc<ComposerStore>,
        authority: Arc<dyn ComposerTaskAuthority>,
        events: EventBus,
    ) -> Self {
        Self {
            store,
            authority,
            events,
        }
    }

    pub fn snapshot(&self, task_id: &TaskId) -> Result<ComposerState, ComposerError> {
        self.authority.ensure_task(task_id)?;
        self.store.snapshot(task_id)
    }

    pub fn execute(
        &self,
        task_id: &TaskId,
        command: ComposerCommand,
    ) -> Result<(ComposerState, bool), ComposerError> {
        self.authority.ensure_task(task_id)?;
        let (state, changed) = self.store.execute(task_id, command)?;
        if changed {
            self.publish(&state);
        }
        Ok((state, changed))
    }

    fn publish(&self, state: &ComposerState) {
        self.events.publish(ComposerChanged {
            task_id: state.task_id.clone(),
            revision: state.revision,
        });
    }
}

/// Service slot for [`ComposerService`].
pub enum ComposerServiceKey {}

impl ServiceKey for ComposerServiceKey {
    type Value = Arc<ComposerService>;

    const NAME: &'static str = "lilia.composer";
}

pub struct ComposerFeature {
    service: Arc<ComposerService>,
    prompt_optimize: Arc<dyn PromptOptimizePort>,
}

impl ComposerFeature {
    pub fn new(
        service: Arc<ComposerService>,
        prompt_optimize: Arc<dyn PromptOptimizePort>,
    ) -> Self {
        Self {
            service,
            prompt_optimize,
        }
    }
}

impl Feature for ComposerFeature {
    fn id(&self) -> FeatureId {
        FeatureId::new("lilia.feature.composer").expect("the composer feature id is not blank")
    }

    fn provides(&self) -> Vec<ServiceRef> {
        vec![ServiceRef::of::<ComposerServiceKey>()]
    }

    fn protocols(&self) -> Vec<JobProtocol> {
        let port = Arc::clone(&self.prompt_optimize);
        vec![JobProtocol::new(
            OPTIMIZE_PROMPT_PROTOCOL,
            Arc::new(move |payload, _context: &JobContext| {
                run_optimize_prompt_job(payload, port.as_ref())
            }),
        )]
    }

    fn mount(&self, cx: &mut FeatureContext<'_>) -> Result<(), KernelError> {
        cx.provide::<ComposerServiceKey>(Arc::clone(&self.service))
    }
}

fn run_optimize_prompt_job(payload: Value, port: &dyn PromptOptimizePort) -> Result<Value, String> {
    let input: PromptOptimizeInput = serde_json::from_value(payload)
        .map_err(|error| format!("invalid prompt optimization request: {error}"))?;
    let result = port.optimize(input)?;
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[cfg(test)]
mod prompt_job_tests {
    use super::*;

    struct FailingPort;

    impl PromptOptimizePort for FailingPort {
        fn optimize(&self, _input: PromptOptimizeInput) -> Result<PromptOptimizeResult, String> {
            Err("the auxiliary model is not configured".to_owned())
        }
    }

    #[test]
    fn an_unreadable_payload_fails_the_job_instead_of_panicking() {
        let error = run_optimize_prompt_job(serde_json::json!({ "prompt": 7 }), &FailingPort)
            .expect_err("a malformed request cannot be optimized");

        assert!(
            error.contains("invalid prompt optimization request"),
            "{error}"
        );
    }

    #[test]
    fn a_failing_port_fails_the_job_with_the_hosts_message() {
        let error =
            run_optimize_prompt_job(serde_json::json!({ "prompt": "ship it" }), &FailingPort)
                .expect_err("a failing auxiliary model fails the job");

        assert_eq!(error, "the auxiliary model is not configured");
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use std::sync::{mpsc, Barrier};

    struct TaskAuthority;
    impl ComposerTaskAuthority for TaskAuthority {
        fn ensure_task(&self, task_id: &TaskId) -> Result<(), ComposerError> {
            if task_id.as_str().starts_with("task-") {
                Ok(())
            } else {
                Err(ComposerError::TaskUnavailable {
                    task_id: task_id.clone(),
                    message: "task does not exist".into(),
                })
            }
        }
    }
    struct NoPrompt;
    impl PromptOptimizePort for NoPrompt {
        fn optimize(&self, _: PromptOptimizeInput) -> Result<PromptOptimizeResult, String> {
            Err("unused".into())
        }
    }
    fn setup() -> (
        Arc<ComposerService>,
        lilia_storage::Db,
        lilia_kernel::Kernel,
    ) {
        let db = lilia_storage::Db::in_memory().unwrap();
        let kernel = lilia_kernel::Kernel::new();
        let service = Arc::new(ComposerService::new(
            Arc::new(ComposerStore::new(db.clone()).unwrap()),
            Arc::new(TaskAuthority),
            kernel.events().clone(),
        ));
        kernel
            .mount(Arc::new(ComposerFeature::new(
                Arc::clone(&service),
                Arc::new(NoPrompt),
            )))
            .unwrap();
        (service, db, kernel)
    }
    #[test]
    fn unknown_tasks_cannot_create_drafts_or_emit_events() {
        let (service, db, kernel) = setup();
        let (tx, rx) = mpsc::channel();
        let subscription = kernel
            .events()
            .on::<ComposerChanged, _>(None, move |event| {
                tx.send(event.clone()).unwrap();
            });
        let missing = TaskId::new("missing").unwrap();
        assert!(matches!(
            service.snapshot(&missing),
            Err(ComposerError::TaskUnavailable { .. })
        ));
        assert!(matches!(
            service.execute(&missing, ComposerCommand::SetContent("orphan".into())),
            Err(ComposerError::TaskUnavailable { .. })
        ));
        assert_eq!(
            db.lock()
                .query_row("SELECT COUNT(*) FROM desktop_composer_drafts", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
        assert!(rx.try_recv().is_err());
        kernel.events().unsubscribe(subscription);
    }
    #[test]
    fn registry_and_direct_writes_emit_once_after_commit_but_conflicts_and_noops_do_not() {
        let (service, _, kernel) = setup();
        let mounted = kernel.service::<ComposerServiceKey>().unwrap();
        assert!(Arc::ptr_eq(&mounted, &service));
        let reader = Arc::clone(&service);
        let (tx, rx) = mpsc::channel();
        let subscription = kernel
            .events()
            .on::<ComposerChanged, _>(None, move |event| {
                tx.send((event.clone(), reader.snapshot(&event.task_id).unwrap()))
                    .unwrap();
            });
        let task = TaskId::new("task-a").unwrap();
        let (first, changed) = service
            .execute(&task, ComposerCommand::SetContent("first".into()))
            .unwrap();
        assert!(changed);
        let (event, observed) = rx.try_recv().unwrap();
        assert_eq!(observed, first);
        assert_eq!(event.revision, first.revision);
        assert!(rx.try_recv().is_err());
        assert!(
            !mounted
                .execute(&task, ComposerCommand::SetContent("first".into()))
                .unwrap()
                .1
        );
        assert!(mounted
            .execute(
                &task,
                ComposerCommand::ApplyPaste {
                    expected_revision: 0,
                    expected_content: String::new(),
                    content: "stale".into(),
                    attachments: vec![]
                }
            )
            .is_err());
        assert!(rx.try_recv().is_err());
        let second = mounted
            .execute(&task, ComposerCommand::SetContent("second".into()))
            .unwrap()
            .0;
        assert_eq!(rx.try_recv().unwrap().1, second);
        assert!(rx.try_recv().is_err());
        kernel.events().unsubscribe(subscription);
    }
    #[test]
    fn concurrent_expected_revision_writers_have_exactly_one_winner() {
        let (service, _, _) = setup();
        for attempt in 0..16 {
            let task = TaskId::new(format!("task-race-{attempt}")).unwrap();
            let barrier = Arc::new(Barrier::new(2));
            let workers = ["left", "right"].map(|text| {
                let service = Arc::clone(&service);
                let barrier = Arc::clone(&barrier);
                let task = task.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    service.execute(
                        &task,
                        ComposerCommand::ApplyPaste {
                            expected_revision: 0,
                            expected_content: String::new(),
                            content: text.into(),
                            attachments: vec![],
                        },
                    )
                })
            });
            let results = workers.map(|worker| worker.join().unwrap());
            assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
            assert_eq!(
                results
                    .iter()
                    .filter(|result| matches!(
                        result,
                        Err(ComposerError::RevisionConflict {
                            expected: 0,
                            actual: 1
                        })
                    ))
                    .count(),
                1
            );
            let winner = results.into_iter().find_map(Result::ok).unwrap().0;
            assert_eq!(service.snapshot(&task).unwrap(), winner);
        }
    }
}
