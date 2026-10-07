//! Prepared-turn execution sequence.
//!
//! AgentKit, Jobs and product I/O stay behind [`AgentTurnHost`]. This module
//! owns the order: auto-select, guide, compaction, session bind/fork, hooks,
//! observed submit and page hand-off.

use lilia_contracts::{LiliaAgentWorkflow, ProjectId, TaskId};
use thiserror::Error;

use crate::runtime::DesktopAgentRuntime;
use crate::turn::{DesktopSessionBranchMode, DesktopTurnRequest};
use crate::turn_page::{handle_observed_page_with_claim, TurnPageHost};
use crate::DesktopTurnQueueError;

#[derive(Debug, Error)]
pub enum AgentTurnError {
    #[error("task `{0}` has no active Native Agent turn")]
    NoActiveTurn(TaskId),
    #[error("desktop {0} state is unavailable")]
    StateUnavailable(&'static str),
    #[error("{0}")]
    Agent(String),
    #[error("invalid desktop input `{field}`: {message}")]
    InvalidInput {
        field: &'static str,
        message: String,
    },
    #[error(transparent)]
    Queue(#[from] DesktopTurnQueueError),
    #[error(transparent)]
    Product(#[from] lilia_contracts::ProductError),
}

impl AgentTurnError {
    /// A worker can outlive its durable claim across recovery. Such a result
    /// must be dropped instead of turning the replacement owner into a local
    /// failure or showing a stale pending error.
    pub fn is_stale_claim(&self) -> bool {
        matches!(
            self,
            Self::NoActiveTurn(_) | Self::Queue(DesktopTurnQueueError::ClaimOwnership { .. })
        )
    }
}

#[derive(Clone, Debug)]
pub struct ObservedTurnOutcome {
    pub session_id: String,
    pub session_version: u64,
    pub waiting_approval: bool,
    pub waiting_interaction: bool,
    pub completed: bool,
    pub cancelled_by_user: bool,
}

#[derive(Clone, Debug)]
pub struct TurnSubmitSpec {
    pub task_id: TaskId,
    pub turn_id: String,
    pub session_id: String,
    pub request: DesktopTurnRequest,
    pub claim_token: Option<String>,
}

/// Host I/O for one prepared turn. Does not hold Jobs.
pub trait AgentTurnHost: TurnPageHost {
    fn apply_automatic_selection(
        &self,
        request: DesktopTurnRequest,
    ) -> Result<DesktopTurnRequest, AgentTurnError>;
    fn persist_request(
        &self,
        turn_id: &str,
        request: &DesktopTurnRequest,
        claim_token: Option<&str>,
    ) -> Result<(), AgentTurnError>;
    fn mark_guide_sent(&self, guide_id: &str) -> Result<(), AgentTurnError>;
    fn run_compaction(
        &self,
        task_id: &TaskId,
        turn_id: &str,
        request: &DesktopTurnRequest,
        claim_token: Option<&str>,
    ) -> Result<(), AgentTurnError>;
    fn load_task(&self, task_id: &TaskId) -> Result<(String, Option<ProjectId>), AgentTurnError>;
    fn refresh_profile(&self) -> Result<String, AgentTurnError>;
    fn existing_session(&self, task_id: &TaskId) -> Result<Option<String>, AgentTurnError>;
    fn fork_through_turn(
        &self,
        source: &str,
        target: &str,
        source_turn_id: &str,
    ) -> Result<String, AgentTurnError>;
    fn fork_session(&self, source: &str, target: &str) -> Result<String, AgentTurnError>;
    fn open_session(
        &self,
        task_id: &TaskId,
        existing: Option<&str>,
        profile_id: &str,
        title: Option<&str>,
    ) -> Result<String, AgentTurnError>;
    fn persist_binding(
        &self,
        task_id: &TaskId,
        session_id: &str,
        profile_id: &str,
        replace: bool,
    ) -> Result<(), AgentTurnError>;
    fn bind_forked_session(
        &self,
        task_id: &TaskId,
        session_id: &str,
        profile_id: &str,
    ) -> Result<(), AgentTurnError>;
    fn cancel_session_turn(&self, session_id: &str, turn_id: &str) -> Result<(), AgentTurnError>;
    fn emit_running(&self, task_id: &TaskId, turn_id: &str);
    fn execute_prompt_hooks(
        &self,
        task_id: &TaskId,
        turn_id: &str,
        workspace: Option<&str>,
        content: &str,
    ) -> Result<(), AgentTurnError>;
    fn submit_observed(&self, spec: TurnSubmitSpec) -> Result<ObservedTurnOutcome, AgentTurnError>;
}

pub fn run_prepared_turn(
    runtime: &DesktopAgentRuntime,
    host: &dyn AgentTurnHost,
    task_id: &TaskId,
    turn_id: &str,
) -> Result<(), AgentTurnError> {
    run_prepared_turn_with_claim(runtime, host, task_id, turn_id, None)
}

pub fn run_prepared_turn_with_claim(
    runtime: &DesktopAgentRuntime,
    host: &dyn AgentTurnHost,
    task_id: &TaskId,
    turn_id: &str,
    expected_claim_token: Option<&str>,
) -> Result<(), AgentTurnError> {
    let mut active = runtime
        .active(task_id, turn_id)
        .ok_or_else(|| AgentTurnError::NoActiveTurn(task_id.clone()))?;
    if let Some(expected) = expected_claim_token {
        if active.claim_token.as_deref() != Some(expected) {
            return Err(AgentTurnError::Queue(
                DesktopTurnQueueError::ClaimOwnership {
                    turn_id: turn_id.to_owned(),
                },
            ));
        }
    }
    let claim_token = expected_claim_token.or(active.claim_token.as_deref());
    let prepared_request = host.apply_automatic_selection(active.request.clone())?;
    if prepared_request != active.request {
        host.persist_request(turn_id, &prepared_request, claim_token)?;
        if !runtime.replace_active_request_with_claim(
            task_id,
            turn_id,
            prepared_request.clone(),
            expected_claim_token,
        ) {
            return Err(AgentTurnError::NoActiveTurn(task_id.clone()));
        }
        active.request = prepared_request;
    }
    if let Some(guide_id) = active.request.guide_id.as_deref() {
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        host.mark_guide_sent(guide_id)?;
    }
    if matches!(
        active.request.workflow.as_ref(),
        Some(LiliaAgentWorkflow::LiliaCompact)
    ) {
        return host.run_compaction(task_id, turn_id, &active.request, claim_token);
    }
    let (title, _project_id) = host.load_task(task_id)?;
    let profile_id = host.refresh_profile()?;
    let existing_binding = host.existing_session(task_id)?;
    let (session_id, forked) = if let Some(branch) = active.request.session_branch.as_ref() {
        let source = existing_binding
            .as_ref()
            .ok_or_else(|| AgentTurnError::InvalidInput {
                field: "agent_session",
                message: "task has no Agent session to branch".to_owned(),
            })?;
        let target_session_id = format!(
            "native-{}-{}-{}",
            task_id.as_str(),
            match branch.mode {
                DesktopSessionBranchMode::Continue => "continue",
                DesktopSessionBranchMode::Fork => "fork",
            },
            uuid::Uuid::new_v4()
        );
        let session_id =
            host.fork_through_turn(source, &target_session_id, &branch.source_turn_id)?;
        (session_id, true)
    } else if active.request.session_fork {
        if let Some(source) = existing_binding.as_ref() {
            let target_session_id =
                format!("native-{}-fork-{}", task_id.as_str(), uuid::Uuid::new_v4());
            let session_id = host.fork_session(source, &target_session_id)?;
            (session_id, true)
        } else {
            (
                host.open_session(task_id, None, &profile_id, Some(&title))?,
                false,
            )
        }
    } else {
        (
            host.open_session(
                task_id,
                existing_binding.as_deref(),
                &profile_id,
                Some(&title),
            )?,
            false,
        )
    };
    if forked {
        host.bind_forked_session(task_id, &session_id, &profile_id)?;
    } else {
        host.persist_binding(
            task_id,
            &session_id,
            &profile_id,
            active.request.session_fork,
        )?;
    }
    let cancel_requested = runtime.attach_session_with_claim(
        task_id,
        turn_id,
        session_id.clone(),
        expected_claim_token,
    );
    if expected_claim_token.is_some()
        && runtime
            .active(task_id, turn_id)
            .is_none_or(|active| active.claim_token.as_deref() != expected_claim_token)
    {
        return Err(AgentTurnError::Queue(
            DesktopTurnQueueError::ClaimOwnership {
                turn_id: turn_id.to_owned(),
            },
        ));
    }
    if cancel_requested || active.cancellation_mode.is_some() {
        host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
        host.cancel_session_turn(&session_id, turn_id)?;
    }
    host.ensure_turn_claim(task_id, turn_id, expected_claim_token)?;
    host.emit_running(task_id, turn_id);
    host.execute_prompt_hooks(
        task_id,
        turn_id,
        active.request.workspace_path.as_deref(),
        &active.request.content,
    )?;
    let page = host.submit_observed(TurnSubmitSpec {
        task_id: task_id.clone(),
        turn_id: turn_id.to_owned(),
        session_id,
        request: active.request,
        claim_token: claim_token.map(str::to_owned),
    })?;
    handle_observed_page_with_claim(runtime, host, task_id, turn_id, expected_claim_token, page)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lilia_contracts::{ExecutionPermission, PendingProjection, ProjectId, TaskId};

    use super::*;
    use crate::runtime::DesktopAgentRuntime;
    use crate::turn::{DesktopSessionBranchAnchor, DesktopSessionBranchMode, DesktopTurnRequest};
    use crate::turn_page::{TurnFinishKind, TurnPageHost};

    struct ForkHost {
        fail_bind: bool,
        bound: Mutex<String>,
        steps: Mutex<Vec<&'static str>>,
    }

    impl ForkHost {
        fn new(fail_bind: bool) -> Self {
            Self {
                fail_bind,
                bound: Mutex::new("parent-session".to_owned()),
                steps: Mutex::new(Vec::new()),
            }
        }

        fn step(&self, step: &'static str) {
            self.steps.lock().expect("steps").push(step);
        }
    }

    impl TurnPageHost for ForkHost {
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
        }

        fn request_title_update(&self, _task_id: TaskId, _turn_id: String) {}
    }

    impl AgentTurnHost for ForkHost {
        fn apply_automatic_selection(
            &self,
            request: DesktopTurnRequest,
        ) -> Result<DesktopTurnRequest, AgentTurnError> {
            Ok(request)
        }

        fn persist_request(
            &self,
            _turn_id: &str,
            _request: &DesktopTurnRequest,
            _claim_token: Option<&str>,
        ) -> Result<(), AgentTurnError> {
            Ok(())
        }

        fn mark_guide_sent(&self, _guide_id: &str) -> Result<(), AgentTurnError> {
            Ok(())
        }

        fn run_compaction(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
            _request: &DesktopTurnRequest,
            _claim_token: Option<&str>,
        ) -> Result<(), AgentTurnError> {
            Ok(())
        }

        fn load_task(
            &self,
            _task_id: &TaskId,
        ) -> Result<(String, Option<ProjectId>), AgentTurnError> {
            Ok(("Fork".to_owned(), None))
        }

        fn refresh_profile(&self) -> Result<String, AgentTurnError> {
            Ok("profile".to_owned())
        }

        fn existing_session(&self, _task_id: &TaskId) -> Result<Option<String>, AgentTurnError> {
            Ok(Some(self.bound.lock().expect("bound").clone()))
        }

        fn fork_through_turn(
            &self,
            source: &str,
            _target: &str,
            _source_turn_id: &str,
        ) -> Result<String, AgentTurnError> {
            assert_eq!(source, "parent-session");
            self.step("fork");
            Ok("target-session".to_owned())
        }

        fn fork_session(&self, source: &str, _target: &str) -> Result<String, AgentTurnError> {
            assert_eq!(source, "parent-session");
            self.step("fork");
            Ok("target-session".to_owned())
        }

        fn open_session(
            &self,
            _task_id: &TaskId,
            _existing: Option<&str>,
            _profile_id: &str,
            _title: Option<&str>,
        ) -> Result<String, AgentTurnError> {
            self.step("open");
            Ok("opened-session".to_owned())
        }

        fn persist_binding(
            &self,
            _task_id: &TaskId,
            _session_id: &str,
            _profile_id: &str,
            _replace: bool,
        ) -> Result<(), AgentTurnError> {
            self.step("persist");
            Ok(())
        }

        fn bind_forked_session(
            &self,
            _task_id: &TaskId,
            session_id: &str,
            _profile_id: &str,
        ) -> Result<(), AgentTurnError> {
            self.step("bind");
            if self.fail_bind {
                return Err(AgentTurnError::Agent(
                    "forked session is not ready to bind".to_owned(),
                ));
            }
            *self.bound.lock().expect("bound") = session_id.to_owned();
            Ok(())
        }

        fn cancel_session_turn(
            &self,
            _session_id: &str,
            _turn_id: &str,
        ) -> Result<(), AgentTurnError> {
            Ok(())
        }

        fn emit_running(&self, _task_id: &TaskId, _turn_id: &str) {}

        fn execute_prompt_hooks(
            &self,
            _task_id: &TaskId,
            _turn_id: &str,
            _workspace: Option<&str>,
            _content: &str,
        ) -> Result<(), AgentTurnError> {
            Ok(())
        }

        fn submit_observed(
            &self,
            spec: TurnSubmitSpec,
        ) -> Result<ObservedTurnOutcome, AgentTurnError> {
            self.step("submit");
            Ok(ObservedTurnOutcome {
                session_id: spec.session_id,
                session_version: 1,
                waiting_approval: false,
                waiting_interaction: false,
                completed: true,
                cancelled_by_user: false,
            })
        }
    }

    fn run_fork(branch: bool, fail_bind: bool) -> (DesktopAgentRuntime, ForkHost, TaskId) {
        let host = ForkHost::new(fail_bind);
        let runtime = DesktopAgentRuntime::default();
        let task_id = TaskId::new("task-fork-bind").unwrap();
        let mut request = DesktopTurnRequest::new(task_id.clone(), "continue from here");
        if branch {
            request.session_branch = Some(DesktopSessionBranchAnchor {
                source_turn_id: "source-turn".to_owned(),
                mode: DesktopSessionBranchMode::Fork,
            });
        } else {
            request.session_fork = true;
        }
        runtime.enqueue_with_turn_id(request, "turn-prepared".to_owned());
        let result = run_prepared_turn(&runtime, &host, &task_id, "turn-prepared");
        if fail_bind {
            assert!(result.is_err());
        } else {
            result.expect("forked turn binds and submits");
        }
        (runtime, host, task_id)
    }

    #[test]
    fn prepared_fork_binds_the_task_to_the_target_session_before_submit() {
        for branch in [true, false] {
            let (runtime, host, task_id) = run_fork(branch, false);
            assert_eq!(host.bound.lock().expect("bound").as_str(), "target-session");
            assert_eq!(
                runtime.snapshot(&task_id).session_id.as_deref(),
                Some("target-session")
            );
            assert_eq!(
                host.steps.lock().expect("steps").as_slice(),
                ["fork", "bind", "submit"]
            );
        }
    }

    #[test]
    fn prepared_fork_binding_failure_leaves_the_task_on_the_original_session() {
        for branch in [true, false] {
            let (runtime, host, task_id) = run_fork(branch, true);
            assert_eq!(host.bound.lock().expect("bound").as_str(), "parent-session");
            assert!(runtime.snapshot(&task_id).session_id.is_none());
            assert_eq!(
                host.steps.lock().expect("steps").as_slice(),
                ["fork", "bind"]
            );
        }
    }
}
