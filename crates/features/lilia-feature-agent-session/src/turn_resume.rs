//! Approval and interaction worker sequences after the user answers.

use lilia_contracts::{ProductApprovalDecision, TaskId};
use serde_json::Value;

use crate::runtime::DesktopAgentRuntime;
use crate::turn_page::{handle_observed_page_with_claim, ObservedPageDisposition, TurnPageHost};
use crate::turn_run::{AgentTurnError, ObservedTurnOutcome};

#[derive(Clone, Debug)]
pub struct InteractionResumeSpec {
    pub session_id: String,
    pub turn_id: String,
    pub version: u64,
    pub interaction_id: String,
    pub accepted: bool,
    pub response: Value,
}

pub trait TurnResumeHost: TurnPageHost {
    fn respond_approval_observed(
        &self,
        task_id: &TaskId,
        decision: ProductApprovalDecision,
        claim_token: Option<&str>,
    ) -> Result<ObservedTurnOutcome, AgentTurnError>;
    fn respond_interaction_observed(
        &self,
        task_id: &TaskId,
        spec: InteractionResumeSpec,
        claim_token: Option<&str>,
    ) -> Result<ObservedTurnOutcome, AgentTurnError>;
    fn emit_approval_changed(&self, task_id: &TaskId, request_id: &str, approved: bool);
    fn emit_interaction_changed(&self, task_id: &TaskId, request_id: &str, accepted: bool);
    fn emit_waiting_approval_error(
        &self,
        task_id: TaskId,
        turn_id: String,
        request_id: String,
        error: String,
    );
    fn emit_waiting_interaction_error(
        &self,
        task_id: TaskId,
        turn_id: String,
        request_id: String,
        error: String,
    );
}

pub fn run_approval_resume(
    runtime: &DesktopAgentRuntime,
    host: &dyn TurnResumeHost,
    task_id: TaskId,
    decision: ProductApprovalDecision,
) {
    run_approval_resume_with_claim(runtime, host, task_id, decision, None);
}

pub fn run_approval_resume_with_claim(
    runtime: &DesktopAgentRuntime,
    host: &dyn TurnResumeHost,
    task_id: TaskId,
    decision: ProductApprovalDecision,
    expected_claim_token: Option<&str>,
) {
    let turn_id = decision.turn_id.clone();
    let request_id = decision.action_id.clone();
    let approved = decision.approved;
    match host.respond_approval_observed(&task_id, decision, expected_claim_token) {
        Ok(page) => {
            let page = ObservedTurnOutcome {
                cancelled_by_user: page.cancelled_by_user || !approved,
                ..page
            };
            let disposition = match handle_observed_page_with_claim(
                runtime,
                host,
                &task_id,
                &turn_id,
                expected_claim_token,
                page,
            ) {
                Ok(disposition) => disposition,
                Err(error) => {
                    // Recovery can replace this worker's durable owner between
                    // the AgentKit resume and page handling. Drop that result
                    // without publishing a failure for the replacement owner.
                    if !error.is_stale_claim() {
                        host.finish_turn_for_claim(
                            task_id,
                            turn_id,
                            crate::turn_page::TurnFinishKind::Failed,
                            Some(error.to_string()),
                            expected_claim_token,
                        );
                    }
                    return;
                }
            };
            // A successful terminal page means finish_turn_for_claim accepted
            // this worker's token; a waiting page revalidated it before its
            // effects. Recheck waiting pages immediately before publishing.
            if matches!(disposition, ObservedPageDisposition::Finished)
                || host
                    .ensure_turn_claim(&task_id, &turn_id, expected_claim_token)
                    .is_ok()
            {
                host.emit_approval_changed(&task_id, &request_id, approved);
            }
        }
        Err(error) => {
            // The AgentKit call can fail after recovery replaced this worker.
            // Revalidate the captured claim before exposing that failure as a
            // waiting error for the replacement owner.
            if !error.is_stale_claim()
                && host
                    .ensure_turn_claim(&task_id, &turn_id, expected_claim_token)
                    .is_ok()
            {
                host.emit_waiting_approval_error(task_id, turn_id, request_id, error.to_string());
            }
        }
    }
}

pub fn run_interaction_resume(
    runtime: &DesktopAgentRuntime,
    host: &dyn TurnResumeHost,
    task_id: TaskId,
    spec: InteractionResumeSpec,
) {
    run_interaction_resume_with_claim(runtime, host, task_id, spec, None);
}

pub fn run_interaction_resume_with_claim(
    runtime: &DesktopAgentRuntime,
    host: &dyn TurnResumeHost,
    task_id: TaskId,
    spec: InteractionResumeSpec,
    expected_claim_token: Option<&str>,
) {
    let turn_id = spec.turn_id.clone();
    let request_id = spec.interaction_id.clone();
    let accepted = spec.accepted;
    match host.respond_interaction_observed(&task_id, spec, expected_claim_token) {
        Ok(page) => {
            let page = ObservedTurnOutcome {
                cancelled_by_user: page.cancelled_by_user || !accepted,
                ..page
            };
            let disposition = match handle_observed_page_with_claim(
                runtime,
                host,
                &task_id,
                &turn_id,
                expected_claim_token,
                page,
            ) {
                Ok(disposition) => disposition,
                Err(error) => {
                    // See the approval path above: a stale worker's resumed page
                    // must not fail or update the replacement owner's turn.
                    if !error.is_stale_claim() {
                        host.finish_turn_for_claim(
                            task_id,
                            turn_id,
                            crate::turn_page::TurnFinishKind::Failed,
                            Some(error.to_string()),
                            expected_claim_token,
                        );
                    }
                    return;
                }
            };
            // See the approval path above: the page handler's successful
            // terminal return is the ownership fence. Recheck waiting pages
            // immediately before publishing their decision.
            if matches!(disposition, ObservedPageDisposition::Finished)
                || host
                    .ensure_turn_claim(&task_id, &turn_id, expected_claim_token)
                    .is_ok()
            {
                host.emit_interaction_changed(&task_id, &request_id, accepted);
            }
        }
        Err(error) => {
            // See the approval path above: a generic resume error is still
            // stale when the captured claim is no longer durable.
            if !error.is_stale_claim()
                && host
                    .ensure_turn_claim(&task_id, &turn_id, expected_claim_token)
                    .is_ok()
            {
                host.emit_waiting_interaction_error(
                    task_id,
                    turn_id,
                    request_id,
                    error.to_string(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lilia_contracts::{ExecutionPermission, PendingProjection, ProductApprovalDecision};
    use serde_json::json;

    use super::*;
    use crate::queue::DesktopTurnQueueError;
    use crate::runtime::DesktopAgentRuntime;
    use crate::turn::DesktopTurnRequest;
    use crate::turn_page::TurnFinishKind;

    #[derive(Default)]
    struct RecordingHost<'a> {
        bind_error: Mutex<Option<AgentTurnError>>,
        claim_error: Mutex<Option<AgentTurnError>>,
        effects: Mutex<Vec<&'static str>>,
        waiting_errors: Mutex<Vec<&'static str>>,
        finishes: Mutex<usize>,
        resume_error: Mutex<Option<AgentTurnError>>,
        recover_before_finish: Option<&'a DesktopAgentRuntime>,
    }

    impl TurnPageHost for RecordingHost<'_> {
        fn ensure_turn_claim(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
            _claim_token: Option<&str>,
        ) -> Result<(), AgentTurnError> {
            self.claim_error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
                .map_or(Ok(()), Err)
        }

        fn bind_session_version(
            &self,
            _turn_id: &str,
            _claim_token: Option<&str>,
            _version: u64,
        ) -> Result<(), AgentTurnError> {
            self.bind_error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
                .map_or(Ok(()), Err)
        }

        fn pending_projections(
            &self,
            _task_id: &TaskId,
        ) -> Result<Vec<PendingProjection>, AgentTurnError> {
            Ok(Vec::new())
        }

        fn emit_waiting_approval(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
            _request_id: Option<String>,
        ) {
        }

        fn emit_waiting_interaction(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
            _request_id: Option<String>,
            _kind: Option<String>,
            _error: Option<String>,
        ) {
        }

        fn dispatch_user_guide(&self, _task_id: &TaskId) {}

        fn turn_permission(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
        ) -> Option<ExecutionPermission> {
            None
        }

        fn respond_architecture(
            &self,
            _task_id: &TaskId,
            _request_id: &str,
            _allow: bool,
        ) -> Result<(), AgentTurnError> {
            Ok(())
        }

        fn finish_turn(
            &self,
            _task_id: TaskId,
            _turn_id: String,
            _kind: TurnFinishKind,
            _message: Option<String>,
        ) {
            *self
                .finishes
                .lock()
                .unwrap_or_else(|error| error.into_inner()) += 1;
            self.effects
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push("finish");
        }

        fn finish_turn_for_claim(
            &self,
            task_id: TaskId,
            turn_id: String,
            kind: TurnFinishKind,
            message: Option<String>,
            _claim_token: Option<&str>,
        ) -> bool {
            if let Some(runtime) = self.recover_before_finish {
                // Recovery can replace and complete the same turn while the
                // old worker is about to finish. An empty active slot alone
                // does not prove that this worker's finish was accepted.
                let request = runtime.active(&task_id, &turn_id).unwrap().request;
                assert!(runtime.hydrate_restored_active(
                    &task_id,
                    &turn_id,
                    request,
                    "replacement-claim".to_owned(),
                ));
                assert!(runtime
                    .begin_finish_with_claim(&task_id, &turn_id, Some("replacement-claim"))
                    .is_some());
                assert!(runtime.finish_without_next_with_claim(
                    &task_id,
                    &turn_id,
                    Some("replacement-claim"),
                ));
                return false;
            }
            self.finish_turn(task_id, turn_id, kind, message);
            true
        }

        fn request_title_update(&self, _task_id: TaskId, _turn_id: String) {}
    }

    impl TurnResumeHost for RecordingHost<'_> {
        fn respond_approval_observed(
            &self,
            _task_id: &TaskId,
            _decision: ProductApprovalDecision,
            _claim_token: Option<&str>,
        ) -> Result<ObservedTurnOutcome, AgentTurnError> {
            if let Some(error) = self
                .resume_error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                return Err(error);
            }
            Ok(ObservedTurnOutcome {
                session_id: "session-1".to_owned(),
                session_version: 1,
                waiting_approval: false,
                waiting_interaction: false,
                completed: true,
                cancelled_by_user: false,
            })
        }

        fn respond_interaction_observed(
            &self,
            _task_id: &TaskId,
            _spec: InteractionResumeSpec,
            _claim_token: Option<&str>,
        ) -> Result<ObservedTurnOutcome, AgentTurnError> {
            if let Some(error) = self
                .resume_error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                return Err(error);
            }
            Ok(ObservedTurnOutcome {
                session_id: "session-1".to_owned(),
                session_version: 1,
                waiting_approval: false,
                waiting_interaction: false,
                completed: true,
                cancelled_by_user: false,
            })
        }

        fn emit_approval_changed(&self, _task_id: &TaskId, _request_id: &str, _approved: bool) {
            self.effects
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push("approval");
        }

        fn emit_interaction_changed(&self, _task_id: &TaskId, _request_id: &str, _accepted: bool) {
            self.effects
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push("interaction");
        }

        fn emit_waiting_approval_error(
            &self,
            _task_id: TaskId,
            _turn_id: String,
            _request_id: String,
            _error: String,
        ) {
            self.waiting_errors
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push("approval");
        }

        fn emit_waiting_interaction_error(
            &self,
            _task_id: TaskId,
            _turn_id: String,
            _request_id: String,
            _error: String,
        ) {
            self.waiting_errors
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push("interaction");
        }
    }

    fn stale_error(turn_id: &str) -> AgentTurnError {
        AgentTurnError::Queue(DesktopTurnQueueError::ClaimOwnership {
            turn_id: turn_id.to_owned(),
        })
    }

    fn runtime_and_task() -> (DesktopAgentRuntime, TaskId) {
        let runtime = DesktopAgentRuntime::default();
        let task_id = TaskId::new("resume-stale-task").unwrap();
        runtime.enqueue_idempotent(
            DesktopTurnRequest::new(task_id.clone(), "resume"),
            "turn-1".to_owned(),
        );
        (runtime, task_id)
    }

    #[test]
    fn stale_approval_page_drops_result_without_product_events_or_finish() {
        let (runtime, task_id) = runtime_and_task();
        let host = RecordingHost {
            bind_error: Mutex::new(Some(stale_error("turn-1"))),
            ..RecordingHost::default()
        };

        run_approval_resume(
            &runtime,
            &host,
            task_id.clone(),
            ProductApprovalDecision {
                session_id: "session-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                action_id: "approval-1".to_owned(),
                version: 1,
                approved: true,
            },
        );

        assert!(host
            .effects
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty());
        assert_eq!(
            *host
                .finishes
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
            0
        );
    }

    #[test]
    fn stale_interaction_page_drops_result_without_product_events_or_finish() {
        let (runtime, task_id) = runtime_and_task();
        let host = RecordingHost {
            bind_error: Mutex::new(Some(stale_error("turn-1"))),
            ..RecordingHost::default()
        };

        run_interaction_resume(
            &runtime,
            &host,
            task_id.clone(),
            InteractionResumeSpec {
                session_id: "session-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                version: 1,
                interaction_id: "interaction-1".to_owned(),
                accepted: true,
                response: json!({"answer": "yes"}),
            },
        );

        assert!(host
            .effects
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty());
        assert_eq!(
            *host
                .finishes
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
            0
        );
    }

    #[test]
    fn recovered_approval_error_drops_waiting_error_when_claim_is_lost() {
        let (runtime, task_id) = runtime_and_task();
        let host = RecordingHost {
            resume_error: Mutex::new(Some(AgentTurnError::Agent("resume failed".to_owned()))),
            claim_error: Mutex::new(Some(stale_error("turn-1"))),
            ..RecordingHost::default()
        };

        run_approval_resume_with_claim(
            &runtime,
            &host,
            task_id,
            ProductApprovalDecision {
                session_id: "session-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                action_id: "approval-1".to_owned(),
                version: 1,
                approved: true,
            },
            Some("old-claim"),
        );

        assert!(host.waiting_errors.lock().unwrap().is_empty());
    }

    #[test]
    fn recovered_interaction_error_drops_waiting_error_when_claim_is_lost() {
        let (runtime, task_id) = runtime_and_task();
        let host = RecordingHost {
            resume_error: Mutex::new(Some(AgentTurnError::Agent("resume failed".to_owned()))),
            claim_error: Mutex::new(Some(stale_error("turn-1"))),
            ..RecordingHost::default()
        };

        run_interaction_resume_with_claim(
            &runtime,
            &host,
            task_id,
            InteractionResumeSpec {
                session_id: "session-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                version: 1,
                interaction_id: "interaction-1".to_owned(),
                accepted: true,
                response: json!({"answer": "yes"}),
            },
            Some("old-claim"),
        );

        assert!(host.waiting_errors.lock().unwrap().is_empty());
    }

    #[test]
    fn recovered_completed_approval_page_drops_product_decision_when_finish_is_rejected() {
        let (runtime, task_id) = runtime_and_task();
        assert!(runtime.claim_worker_start(&task_id, "turn-1", "old-claim".to_owned()));
        let host = RecordingHost {
            recover_before_finish: Some(&runtime),
            ..RecordingHost::default()
        };

        run_approval_resume_with_claim(
            &runtime,
            &host,
            task_id.clone(),
            ProductApprovalDecision {
                session_id: "session-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                action_id: "approval-1".to_owned(),
                version: 1,
                approved: true,
            },
            Some("old-claim"),
        );

        assert!(runtime.active(&task_id, "turn-1").is_none());
        assert!(host.effects.lock().unwrap().is_empty());
        assert_eq!(*host.finishes.lock().unwrap(), 0);
    }

    #[test]
    fn recovered_completed_interaction_page_drops_product_decision_when_finish_is_rejected() {
        let (runtime, task_id) = runtime_and_task();
        assert!(runtime.claim_worker_start(&task_id, "turn-1", "old-claim".to_owned()));
        let host = RecordingHost {
            recover_before_finish: Some(&runtime),
            ..RecordingHost::default()
        };

        run_interaction_resume_with_claim(
            &runtime,
            &host,
            task_id.clone(),
            InteractionResumeSpec {
                session_id: "session-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                version: 1,
                interaction_id: "interaction-1".to_owned(),
                accepted: true,
                response: json!({"answer": "yes"}),
            },
            Some("old-claim"),
        );

        assert!(runtime.active(&task_id, "turn-1").is_none());
        assert!(host.effects.lock().unwrap().is_empty());
        assert_eq!(*host.finishes.lock().unwrap(), 0);
    }
}
