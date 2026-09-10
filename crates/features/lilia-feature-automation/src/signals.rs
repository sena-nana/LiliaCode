use crate::{AutomationDraft, AutomationSignalEnvelope};

pub fn automation_signal_matches(
    snapshot: &AutomationDraft,
    signal: &AutomationSignalEnvelope,
) -> bool {
    if signal.automation_run_id.is_some() {
        return false;
    }
    let scope = &snapshot.scope;
    let project = signal.project_id.as_deref().filter(|id| !id.is_empty());
    if match project {
        None => !scope.include_inbox,
        Some(id) => {
            !scope.project_ids.is_empty()
                && !scope.project_ids.iter().any(|candidate| candidate == id)
        }
    } {
        return false;
    }
    for (allowed, actual) in [
        (&scope.backends, signal.backend.as_deref()),
        (&scope.event_kinds, signal.event_kind.as_deref()),
        (
            &scope.task_statuses,
            signal
                .payload
                .get("taskStatus")
                .and_then(serde_json::Value::as_str),
        ),
    ] {
        if !allowed.is_empty()
            && actual.is_none_or(|actual| !allowed.iter().any(|value| value == actual))
        {
            return false;
        }
    }
    snapshot.nodes.iter().any(|node| {
        node.kind == "trigger"
            && node
                .config
                .get("triggerKind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("manual")
                == signal.kind
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AutomationNode, AutomationNodePosition, AutomationScopeFilter};
    use serde_json::json;

    #[test]
    fn inbox_scope_still_checks_backend_status_event_and_recursion() {
        let snapshot = AutomationDraft {
            nodes: vec![AutomationNode {
                id: "trigger".into(),
                kind: "trigger".into(),
                title: "事件".into(),
                position: AutomationNodePosition { x: 0.0, y: 0.0 },
                config: json!({ "triggerKind": "task_changed" }),
            }],
            edges: vec![],
            scope: AutomationScopeFilter {
                include_inbox: true,
                project_ids: vec!["project-a".into()],
                backends: vec!["native-agentkit".into()],
                event_kinds: vec!["task_status_changed".into()],
                task_statuses: vec!["done".into()],
            },
        };
        let mut signal = AutomationSignalEnvelope {
            id: "event".into(),
            kind: "task_changed".into(),
            project_id: None,
            task_id: None,
            backend: Some("native-agentkit".into()),
            event_kind: Some("task_status_changed".into()),
            automation_run_id: None,
            payload: json!({ "taskStatus": "done" }),
            created_at: 1,
        };
        assert!(automation_signal_matches(&snapshot, &signal));
        signal.backend = Some("other".into());
        assert!(!automation_signal_matches(&snapshot, &signal));
        signal.backend = Some("native-agentkit".into());
        signal.payload["taskStatus"] = json!("running");
        assert!(!automation_signal_matches(&snapshot, &signal));
        signal.payload["taskStatus"] = json!("done");
        signal.automation_run_id = Some("run".into());
        assert!(!automation_signal_matches(&snapshot, &signal));
    }
}
