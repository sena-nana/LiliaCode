use lilia_contracts::{AutomationOperationRequest, AutomationOperationResult};
use lilia_kernel::JobId;
use std::collections::HashMap;

pub(crate) struct PendingOperation {
    pub request: AutomationOperationRequest,
    pub selection: Option<String>,
    pub response: String,
}

impl PendingOperation {
    pub fn addresses(&self, result: &AutomationOperationResult) -> bool {
        self.request.workflow_id() == result.workflow_id
            && self.request.run_id().is_none_or(|id| id == result.run_id)
    }
    pub fn may_select(&self, result: &AutomationOperationResult, current: Option<&str>) -> bool {
        self.addresses(result)
            && (current == Some(result.run_id.as_str())
                || (matches!(self.request, AutomationOperationRequest::Start { .. })
                    && current == self.selection.as_deref()))
    }
}

#[derive(Default)]
pub(crate) struct AutomationOperations {
    pub pending: HashMap<JobId, PendingOperation>,
    errors: HashMap<(String, Option<String>), String>,
}
impl AutomationOperations {
    pub fn busy(&self, workflow_id: &str) -> bool {
        self.pending
            .values()
            .any(|pending| pending.request.workflow_id() == workflow_id)
    }
    pub fn cancelling(&self, run_id: &str) -> bool {
        self.pending.values().any(|pending| matches!(&pending.request, AutomationOperationRequest::Cancel { run_id: id, .. } if id == run_id))
    }
    pub fn error(&self, workflow_id: &str, run_id: Option<&str>) -> Option<String> {
        self.errors
            .get(&(workflow_id.into(), run_id.map(str::to_owned)))
            .or_else(|| self.errors.get(&(workflow_id.into(), None)))
            .cloned()
    }
    pub fn set_error(&mut self, workflow_id: &str, run_id: Option<&str>, error: Option<String>) {
        let key = (workflow_id.to_owned(), run_id.map(str::to_owned));
        if let Some(error) = error {
            self.errors.insert(key, error);
        } else {
            self.errors.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_results_cannot_select_another_run_or_workflow() {
        let pending = PendingOperation {
            request: AutomationOperationRequest::Resume {
                workflow_id: "a".into(),
                run_id: "one".into(),
                node_id: "human".into(),
                payload: None,
            },
            selection: Some("one".into()),
            response: "draft".into(),
        };
        let mut result = AutomationOperationResult {
            workflow_id: "a".into(),
            run_id: "one".into(),
            error: None,
        };
        assert!(pending.may_select(&result, Some("one")));
        assert!(!pending.may_select(&result, Some("two")));
        result.run_id = "two".into();
        assert!(!pending.addresses(&result));
        result.run_id = "one".into();
        result.workflow_id = "b".into();
        assert!(!pending.addresses(&result));
        let mut state = AutomationOperations::default();
        state.set_error("a", Some("one"), Some("failed".into()));
        assert!(state.error("a", Some("two")).is_none());
        assert!(state.error("b", Some("one")).is_none());
        assert_eq!(state.error("a", Some("one")).as_deref(), Some("failed"));
    }
}
