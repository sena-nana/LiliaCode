//! Observed-page state machine after a turn, approval or interaction page.

use lilia_contracts::{ExecutionPermission, PendingProjection, PendingProjectionStatus, TaskId};

use crate::runtime::DesktopAgentRuntime;
use crate::turn_run::{AgentTurnError, ObservedTurnOutcome};

pub fn supported_pending_interaction_kind(kind: &str) -> bool {
    matches!(
        kind,
        "ask_user" | "plan_approval" | "tool_consent" | "mcp_elicitation" | "architecture_change"
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnFinishKind {
    Cancelled,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedPageDisposition {
    Waiting,
    Finished,
}

/// Host I/O for [`handle_observed_page`].
pub trait TurnPageHost {
    /// Revalidates the durable owner before publishing a page-derived product
    /// effect. Production hosts must reject a missing or stale claim.
    fn ensure_turn_claim(
        &self,
        _task_id: &TaskId,
        _turn_id: &str,
        _claim_token: Option<&str>,
    ) -> Result<(), AgentTurnError> {
        Ok(())
    }
    fn bind_session_version(
        &self,
        turn_id: &str,
        claim_token: Option<&str>,
        version: u64,
    ) -> Result<(), AgentTurnError>;
    fn pending_projections(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<PendingProjection>, AgentTurnError>;
    fn emit_waiting_approval(&self, task_id: &TaskId, turn_id: &str, request_id: Option<String>);
    fn emit_waiting_interaction(
        &self,
        task_id: &TaskId,
        turn_id: &str,
        request_id: Option<String>,
        kind: Option<String>,
        error: Option<String>,
    );
    fn dispatch_user_guide(&self, task_id: &TaskId);
    fn turn_permission(&self, task_id: &TaskId, turn_id: &str) -> Option<ExecutionPermission>;
    fn respond_architecture(
        &self,
        task_id: &TaskId,
        request_id: &str,
        allow: bool,
    ) -> Result<(), AgentTurnError>;
    fn finish_turn(
        &self,
        task_id: TaskId,
        turn_id: String,
        kind: TurnFinishKind,
        message: Option<String>,
    );
    /// Finishes a page only for the worker claim that observed it. Hosts that
    /// do not need an ownership fence may use the default forwarding method.
    fn finish_turn_for_claim(
        &self,
        task_id: TaskId,
        turn_id: String,
        kind: TurnFinishKind,
        message: Option<String>,
        _claim_token: Option<&str>,
    ) -> bool {
        self.finish_turn(task_id, turn_id, kind, message);
        true
    }
    fn request_title_update(&self, task_id: TaskId, turn_id: String);
}

pub fn handle_observed_page(
    runtime: &DesktopAgentRuntime,
    host: &dyn TurnPageHost,
    task_id: &TaskId,
    turn_id: &str,
    page: ObservedTurnOutcome,
) -> Result<(), AgentTurnError> {
    handle_observed_page_with_claim(runtime, host, task_id, turn_id, None, page).map(|_| ())
}

/// Handles a page while retaining the claim token captured by its worker.
/// Passing the token through this boundary prevents a worker that outlives
/// recovery from re-reading and borrowing the replacement owner's token.
pub fn handle_observed_page_with_claim(
    runtime: &DesktopAgentRuntime,
    host: &dyn TurnPageHost,
    task_id: &TaskId,
    turn_id: &str,
    expected_claim_token: Option<&str>,
    page: ObservedTurnOutcome,
) -> Result<ObservedPageDisposition, AgentTurnError> {
    let active = runtime
        .active(task_id, turn_id)
        .ok_or_else(|| AgentTurnError::NoActiveTurn(task_id.clone()))?;
    if let Some(expected) = expected_claim_token {
        if active.claim_token.as_deref() != Some(expected) {
            return Err(AgentTurnError::Queue(
                crate::DesktopTurnQueueError::ClaimOwnership {
                    turn_id: turn_id.to_owned(),
                },
            ));
        }
    }
    let claim_token = expected_claim_token.or(active.claim_token.as_deref());
    host.bind_session_version(turn_id, claim_token, page.session_version)?;
    if page.session_version != 0
        && !runtime.bind_session_version_with_claim(
            task_id,
            turn_id,
            page.session_version,
            expected_claim_token,
        )
    {
        return Err(AgentTurnError::NoActiveTurn(task_id.clone()));
    }
    host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
    if page.waiting_approval {
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        let request_id = host
            .pending_projections(task_id)?
            .into_iter()
            .rev()
            .find(|pending| {
                pending.status == PendingProjectionStatus::Open
                    && pending.kind == "permission_approval"
                    && pending.agent_session.as_str() == page.session_id
                    && pending.turn_id.as_deref() == Some(turn_id)
            })
            .map(|pending| pending.request_id);
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        host.emit_waiting_approval(task_id, turn_id, request_id);
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        host.dispatch_user_guide(task_id);
        return Ok(ObservedPageDisposition::Waiting);
    }
    if page.waiting_interaction {
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        let pending = host
            .pending_projections(task_id)?
            .into_iter()
            .rev()
            .find(|pending| {
                pending.status == PendingProjectionStatus::Open
                    && supported_pending_interaction_kind(&pending.kind)
                    && pending.agent_session.as_str() == page.session_id
                    && pending.turn_id.as_deref() == Some(turn_id)
            });
        let auto_allow = pending.as_ref().and_then(|pending| {
            (pending.kind == "architecture_change")
                .then(|| host.turn_permission(task_id, turn_id))
                .flatten()
                .and_then(|permission| match permission {
                    ExecutionPermission::Full => Some(true),
                    ExecutionPermission::Readonly => Some(false),
                    ExecutionPermission::Ask => None,
                })
        });
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        host.emit_waiting_interaction(
            task_id,
            turn_id,
            pending.as_ref().map(|pending| pending.request_id.clone()),
            pending.as_ref().map(|pending| pending.kind.clone()),
            None,
        );
        if let (Some(pending), Some(allow)) = (pending.as_ref(), auto_allow) {
            match host.respond_architecture(task_id, &pending.request_id, allow) {
                Ok(()) => return Ok(ObservedPageDisposition::Waiting),
                Err(error) => {
                    host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
                    host.emit_waiting_interaction(
                        task_id,
                        turn_id,
                        Some(pending.request_id.clone()),
                        Some(pending.kind.clone()),
                        Some(error.to_string()),
                    );
                }
            }
        }
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        host.dispatch_user_guide(task_id);
        return Ok(ObservedPageDisposition::Waiting);
    }
    let (kind, message) = if page.cancelled_by_user
        || (page.completed
            && runtime.cancel_requested_with_claim(task_id, turn_id, expected_claim_token))
    {
        (TurnFinishKind::Cancelled, None)
    } else if page.completed {
        (TurnFinishKind::Completed, None)
    } else {
        (
            TurnFinishKind::Failed,
            Some("Native Agent turn ended without completion".to_owned()),
        )
    };
    if !host.finish_turn_for_claim(
        task_id.clone(),
        turn_id.to_owned(),
        kind,
        message,
        expected_claim_token,
    ) {
        return Err(AgentTurnError::Queue(
            crate::DesktopTurnQueueError::ClaimOwnership {
                turn_id: turn_id.to_owned(),
            },
        ));
    }
    if matches!(kind, TurnFinishKind::Completed) {
        host.request_title_update(task_id.clone(), turn_id.to_owned());
    }
    Ok(ObservedPageDisposition::Finished)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lilia_contracts::{PendingProjection, TaskId};

    use super::*;
    use crate::runtime::DesktopAgentRuntime;
    use crate::turn::DesktopTurnRequest;
    use crate::turn_run::ObservedTurnOutcome;

    #[derive(Default)]
    struct RecordingHost {
        reject_finish: bool,
        pending: Vec<PendingProjection>,
        waits: Mutex<Vec<&'static str>>,
        finishes: Mutex<Vec<(String, TurnFinishKind, Option<String>)>>,
        titles: Mutex<Vec<String>>,
        architecture: Mutex<Vec<(String, bool)>>,
    }

    impl TurnPageHost for RecordingHost {
        fn bind_session_version(
            &self,
            _turn_id: &str,
            _claim_token: Option<&str>,
            _version: u64,
        ) -> Result<(), AgentTurnError> {
            Ok(())
        }

        fn pending_projections(
            &self,
            _task_id: &TaskId,
        ) -> Result<Vec<PendingProjection>, AgentTurnError> {
            Ok(self.pending.clone())
        }

        fn emit_waiting_approval(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
            _request_id: Option<String>,
        ) {
            self.waits.lock().unwrap().push("approval");
        }

        fn emit_waiting_interaction(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
            _request_id: Option<String>,
            _kind: Option<String>,
            _error: Option<String>,
        ) {
            self.waits.lock().unwrap().push("interaction");
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
            request_id: &str,
            allow: bool,
        ) -> Result<(), AgentTurnError> {
            self.architecture
                .lock()
                .unwrap()
                .push((request_id.to_owned(), allow));
            Ok(())
        }

        fn finish_turn(
            &self,
            _task_id: TaskId,
            turn_id: String,
            kind: TurnFinishKind,
            message: Option<String>,
        ) {
            self.finishes.lock().unwrap().push((turn_id, kind, message));
        }

        fn finish_turn_for_claim(
            &self,
            task_id: TaskId,
            turn_id: String,
            kind: TurnFinishKind,
            message: Option<String>,
            _claim_token: Option<&str>,
        ) -> bool {
            if self.reject_finish {
                return false;
            }
            self.finish_turn(task_id, turn_id, kind, message);
            true
        }

        fn request_title_update(&self, _task_id: TaskId, turn_id: String) {
            self.titles.lock().unwrap().push(turn_id);
        }
    }

    fn outcome(
        waiting_approval: bool,
        waiting_interaction: bool,
        completed: bool,
    ) -> ObservedTurnOutcome {
        ObservedTurnOutcome {
            session_id: "session-1".to_owned(),
            session_version: 0,
            waiting_approval,
            waiting_interaction,
            completed,
            cancelled_by_user: false,
        }
    }

    #[test]
    fn waiting_approval_page_marks_runtime_and_emits_wait() {
        let runtime = DesktopAgentRuntime::default();
        let task_id = TaskId::new("task-page").unwrap();
        runtime.enqueue_idempotent(
            DesktopTurnRequest::new(task_id.clone(), "hi"),
            "turn-1".into(),
        );
        let host = RecordingHost::default();
        handle_observed_page(
            &runtime,
            &host,
            &task_id,
            "turn-1",
            outcome(true, false, false),
        )
        .unwrap();
        assert_eq!(*host.waits.lock().unwrap(), ["approval"]);
        assert!(host.finishes.lock().unwrap().is_empty());
    }

    #[test]
    fn completed_page_finishes_and_requests_title() {
        let runtime = DesktopAgentRuntime::default();
        let task_id = TaskId::new("task-page").unwrap();
        runtime.enqueue_idempotent(
            DesktopTurnRequest::new(task_id.clone(), "hi"),
            "turn-1".into(),
        );
        let host = RecordingHost::default();
        handle_observed_page(
            &runtime,
            &host,
            &task_id,
            "turn-1",
            outcome(false, false, true),
        )
        .unwrap();
        let finishes = host.finishes.lock().unwrap().clone();
        assert_eq!(finishes.len(), 1);
        assert_eq!(finishes[0].1, TurnFinishKind::Completed);
        assert_eq!(*host.titles.lock().unwrap(), ["turn-1"]);
    }

    #[test]
    fn incomplete_page_fails_the_turn() {
        let runtime = DesktopAgentRuntime::default();
        let task_id = TaskId::new("task-page").unwrap();
        runtime.enqueue_idempotent(
            DesktopTurnRequest::new(task_id.clone(), "hi"),
            "turn-1".into(),
        );
        let host = RecordingHost::default();
        handle_observed_page(
            &runtime,
            &host,
            &task_id,
            "turn-1",
            outcome(false, false, false),
        )
        .unwrap();
        let finishes = host.finishes.lock().unwrap().clone();
        assert_eq!(finishes[0].1, TurnFinishKind::Failed);
        assert!(host.titles.lock().unwrap().is_empty());
    }

    #[test]
    fn rejected_terminal_pages_report_stale_claim_without_title_updates() {
        let runtime = DesktopAgentRuntime::default();
        let task_id = TaskId::new("task-page").unwrap();
        runtime.enqueue_idempotent(
            DesktopTurnRequest::new(task_id.clone(), "hi"),
            "turn-1".into(),
        );
        let host = RecordingHost {
            reject_finish: true,
            ..RecordingHost::default()
        };
        let mut cancelled = outcome(false, false, false);
        cancelled.cancelled_by_user = true;
        for page in [
            cancelled,
            outcome(false, false, true),
            outcome(false, false, false),
        ] {
            let error =
                handle_observed_page(&runtime, &host, &task_id, "turn-1", page).unwrap_err();
            assert!(error.is_stale_claim());
        }
        assert!(host.finishes.lock().unwrap().is_empty());
        assert!(host.titles.lock().unwrap().is_empty());
    }
}
