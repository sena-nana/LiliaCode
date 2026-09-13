use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AutomationSwitchCases {
    Values(Vec<String>),
    LegacyLines(String),
}

impl AutomationSwitchCases {
    pub fn values(&self) -> Vec<String> {
        let values: Vec<&str> = match self {
            Self::Values(values) => values.iter().map(String::as_str).collect(),
            Self::LegacyLines(value) => value.lines().collect(),
        };
        values
            .into_iter()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn old_begin_requests_keep_their_wire_shape() {
        let value = json!({"workflowId":"workflow","trigger":{"id":"signal","kind":"manual","createdAt":1}});
        let request: AutomationBeginRunInput = serde_json::from_value(value).unwrap();
        assert!(request.expected_version_id.is_none());
        assert!(serde_json::to_value(request)
            .unwrap()
            .get("expectedVersionId")
            .is_none());
    }

    #[test]
    fn legacy_cases_migrate_to_values_without_changing_order() {
        let cases: AutomationSwitchCases =
            serde_json::from_value(json!("first\n second \n")).unwrap();
        let canonical = AutomationSwitchCases::Values(cases.values());
        assert_eq!(
            serde_json::to_value(canonical).unwrap(),
            json!(["first", "second"])
        );
        assert!(serde_json::from_value::<AutomationSwitchCases>(json!(["first", 2])).is_err());
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationSignalEnvelope {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub event_kind: Option<String>,
    #[serde(default)]
    pub automation_run_id: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AutomationOperationRequest {
    Start {
        workflow_id: String,
        expected_version_id: String,
        trigger: AutomationSignalEnvelope,
    },
    Resume {
        workflow_id: String,
        run_id: String,
        node_id: String,
        payload: Option<serde_json::Value>,
    },
    Cancel {
        workflow_id: String,
        run_id: String,
    },
}

impl AutomationOperationRequest {
    pub fn workflow_id(&self) -> &str {
        match self {
            Self::Start { workflow_id, .. }
            | Self::Resume { workflow_id, .. }
            | Self::Cancel { workflow_id, .. } => workflow_id,
        }
    }
    pub fn run_id(&self) -> Option<&str> {
        match self {
            Self::Start { .. } => None,
            Self::Resume { run_id, .. } | Self::Cancel { run_id, .. } => Some(run_id),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationOperationResult {
    pub workflow_id: String,
    pub run_id: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationBeginRunInput {
    pub workflow_id: String,
    pub trigger: AutomationSignalEnvelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version_id: Option<String>,
}
