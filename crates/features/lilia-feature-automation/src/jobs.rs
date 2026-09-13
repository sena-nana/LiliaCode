use std::sync::Arc;

use lilia_contracts::{AutomationOperationRequest, AutomationOperationResult};
use lilia_kernel::{JobContext, JobProtocol};

pub const AUTOMATION_OPERATE_PROTOCOL: &str = "lilia.automation/operate@1";

pub trait AutomationOperationPort: Send + Sync + 'static {
    fn operate(
        &self,
        request: AutomationOperationRequest,
        context: &JobContext,
    ) -> Result<AutomationOperationResult, String>;
}

pub(crate) fn operation_protocol(port: Arc<dyn AutomationOperationPort>) -> JobProtocol {
    JobProtocol::new(
        AUTOMATION_OPERATE_PROTOCOL,
        Arc::new(move |payload, context| {
            let request: AutomationOperationRequest = serde_json::from_value(payload)
                .map_err(|error| format!("invalid automation operation: {error}"))?;
            if request.workflow_id().trim().is_empty()
                || request.run_id().is_some_and(|id| id.trim().is_empty())
            {
                return Err("automation operation identity is missing".into());
            }
            if matches!(&request, AutomationOperationRequest::Start { expected_version_id, .. } if expected_version_id.trim().is_empty())
            {
                return Err("published version identity is missing".into());
            }
            if let AutomationOperationRequest::Resume { node_id, .. } = &request {
                if node_id.trim().is_empty() {
                    return Err("confirmation node identity is missing".into());
                }
            }
            if context.is_cancelled() {
                return Err("automation operation cancelled before execution".into());
            }
            serde_json::to_value(port.operate(request, context)?).map_err(|error| error.to_string())
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;
    #[derive(Default)]
    struct Port(Mutex<Vec<AutomationOperationRequest>>);
    impl AutomationOperationPort for Port {
        fn operate(
            &self,
            request: AutomationOperationRequest,
            _: &JobContext,
        ) -> Result<AutomationOperationResult, String> {
            let result = AutomationOperationResult {
                workflow_id: request.workflow_id().into(),
                run_id: request.run_id().unwrap_or("created").into(),
                error: None,
            };
            self.0.lock().unwrap().push(request);
            Ok(result)
        }
    }
    #[test]
    fn cancellation_and_invalid_identity_prevent_side_effects() {
        let port = Arc::new(Port::default());
        let protocol = operation_protocol(port.clone());
        let context = JobContext::new();
        context.request_cancel();
        assert!((protocol.handler)(
            json!({"operation":"cancel","workflowId":"workflow","runId":"run"}),
            &context
        )
        .is_err());
        assert!((protocol.handler)(json!({"operation":"resume","workflowId":"workflow","runId":"run","nodeId":"","payload":null}), &JobContext::new()).is_err());
        assert!(port.0.lock().unwrap().is_empty());
        let result = (protocol.handler)(json!({"operation":"resume","workflowId":"workflow","runId":"run","nodeId":"human","payload":{"response":"confirmed"}}), &JobContext::new()).unwrap();
        assert_eq!(
            serde_json::from_value::<AutomationOperationResult>(result)
                .unwrap()
                .run_id,
            "run"
        );
        let requests = port.0.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(
            matches!(&requests[0], AutomationOperationRequest::Resume { node_id, payload: Some(value), .. } if node_id == "human" && value["response"] == "confirmed")
        );
    }
}
