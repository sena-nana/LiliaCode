use mutsuki_agent_contracts::{
    AgentError, AgentToolDescriptor, AgentToolExecuteRequest, AgentToolExecution, InteractionKind,
    AGENT_RUN_PROTOCOL,
};
use mutsuki_runtime_contracts::Task;
use mutsuki_runtime_sdk::{
    contracts::RunnerResult, PluginBuilder, ProtocolSpec, RuntimeClientRef, RuntimeResult,
    SdkProtocol, TaskAwaitRunnerAdapter,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const PLUGIN: &str = "lilia.plugin.project-architecture";
pub(crate) const PROTOCOL: &str = "lilia.agent.project-architecture@1";
pub(crate) const TOOL: &str = "update_project_architecture";
const CONTRACT_JSON: &str =
    include_str!("../../lilia-contracts/contracts/architecture-contract.json");

#[derive(Clone, Debug)]
struct ArchitectureProtocol;
impl SdkProtocol for ArchitectureProtocol {
    const PROTOCOL_ID: &'static str = PROTOCOL;
}
impl ProtocolSpec for ArchitectureProtocol {}

pub(crate) fn descriptor() -> AgentToolDescriptor {
    let contract: Value =
        serde_json::from_str(CONTRACT_JSON).expect("architecture-contract.json must be valid JSON");
    let mut descriptor = AgentToolDescriptor::new(
        TOOL,
        AGENT_RUN_PROTOCOL,
        "Propose typed changes to the current Lilia project architecture graph. Use the authoritative project architecture snapshot in the turn context, explain the reason, and submit only changes supported by the schema. The host applies or rejects the proposal according to the current execution permission.",
    );
    descriptor.input_schema = contract["updateProjectArchitectureInputSchema"].clone();
    descriptor.execution = AgentToolExecution::Interaction {
        interaction_kind: InteractionKind::Custom,
    };
    descriptor
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Permission {
    Ask,
    Full,
    Readonly,
}

impl Permission {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "ask" => Some(Self::Ask),
            "full" | "free" => Some(Self::Full),
            "readonly" => Some(Self::Readonly),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Full => "full",
            Self::Readonly => "readonly",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Allow,
    Deny,
}

impl Decision {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proposal {
    #[serde(default)]
    reason: String,
    changes: Vec<Change>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Change {
    UpsertNode {
        node: Node,
    },
    RemoveNode {
        #[serde(rename = "nodeId")]
        node_id: String,
    },
    UpsertEdge {
        edge: Edge,
    },
    RemoveEdge {
        #[serde(rename = "edgeId")]
        edge_id: String,
    },
    SetSummary {
        summary: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    id: String,
    label: String,
    #[serde(rename = "type")]
    node_type: String,
    summary: String,
    paths: Vec<String>,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Edge {
    id: String,
    from: String,
    to: String,
    #[serde(rename = "type")]
    edge_type: String,
    label: String,
    summary: String,
}

pub(crate) fn plugin(client: RuntimeClientRef) -> PluginBuilder {
    let runner =
        mutsuki_agent_sdk::orchestration_runner("lilia.project-architecture.runner", PLUGIN)
            .accepts::<ArchitectureProtocol>()
            .build();
    PluginBuilder::new(PLUGIN)
        .protocol::<ArchitectureProtocol>()
        .runner(Box::new(TaskAwaitRunnerAdapter::new(
            runner,
            client,
            Box::new(move |_context, task| Box::pin(async move { run(task) })),
        )))
}

fn run(task: Task) -> RuntimeResult<RunnerResult> {
    let request = serde_json::from_value(task.payload.clone().into()).map_err(|_| {
        mutsuki_agent_sdk::runtime_failure(
            PLUGIN,
            &task.task_id,
            AgentError::invalid_input("invalid architecture execution request"),
        )
    })?;
    let output = execute(request)
        .map_err(|error| mutsuki_agent_sdk::runtime_failure(PLUGIN, &task.task_id, error))?;
    let mut result = RunnerResult::completed(task.task_id);
    result.output = Some(output);
    Ok(result)
}

pub(crate) fn execute(request: AgentToolExecuteRequest) -> Result<Value, AgentError> {
    if request.name != TOOL {
        return Err(AgentError::not_found(format!(
            "architecture tool `{}` is not registered",
            request.name
        )));
    }
    let session = request
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AgentError::invalid_input("architecture session is required"))?;
    let proposal: Proposal = serde_json::from_value(request.input.clone())
        .map_err(|_| AgentError::invalid_input("invalid architecture proposal"))?;
    if proposal.changes.is_empty() {
        return Err(AgentError::invalid_input(
            "architecture changes must not be empty",
        ));
    }
    let context = request
        .context
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(|| AgentError::invalid_input("architecture execution context is required"))?;
    if context
        .get("sessionId")
        .and_then(Value::as_str)
        .is_some_and(|value| value != session)
    {
        return Err(claim_error());
    }
    let permission = context_str(context, &["permission", "permissionMode"])
        .and_then(Permission::parse)
        .ok_or_else(|| {
            AgentError::invalid_input("architecture execution permission is required")
        })?;
    let claim = context_str(context, &["claimToken", "claim_token"]).unwrap_or_default();
    let active =
        context_str(context, &["activeClaimToken", "active_claim_token"]).unwrap_or_default();
    if claim.is_empty() || claim != active {
        return Err(claim_error());
    }
    let turn_id = context_str(context, &["turnId", "turn_id"])
        .ok_or_else(|| AgentError::invalid_input("architecture turn is required"))?;
    let project_id = context_str(context, &["productProjectId", "projectId"])
        .ok_or_else(|| AgentError::invalid_input("architecture project is required"))?;
    let task_id = context_str(context, &["productTaskId", "taskId"])
        .ok_or_else(|| AgentError::invalid_input("architecture task is required"))?;
    let expected_version = context
        .get("projectArchitectureVersion")
        .or_else(|| context.get("expectedVersion"))
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            AgentError::invalid_input(
                "architecture interaction is missing its expected graph version",
            )
        })?;
    let decision = match context_str(context, &["decision"]) {
        Some(value) => Some(Decision::parse(value).ok_or_else(|| {
            AgentError::invalid_input("architecture decision must be allow or deny")
        })?),
        None => None,
    };
    if decision == Some(Decision::Allow) && permission == Permission::Readonly {
        return Err(AgentError::new(
            "lilia.architecture.permission",
            "readonly turns cannot apply architecture changes",
        ));
    }
    let disposition = match decision {
        Some(Decision::Allow) => "apply",
        Some(Decision::Deny) => "reject",
        None => "hold",
    };
    Ok(json!({
        "interaction": "architecture_change",
        "permission": permission.as_str(),
        "decision": decision.map(Decision::as_str),
        "disposition": disposition,
        "status": if permission == Permission::Readonly { "proposed" } else { "pending" },
        "requiresConfirmation": permission != Permission::Full,
        "authorized": decision == Some(Decision::Allow),
        "applied": false,
        "reason": proposal.reason,
        "changes": proposal.changes,
        "sessionId": session,
        "turnId": turn_id,
        "projectId": project_id,
        "taskId": task_id,
        "expectedVersion": expected_version,
    }))
}

fn claim_error() -> AgentError {
    AgentError::new(
        "lilia.architecture.claim",
        "architecture execution claim is missing or stale",
    )
}

fn context_str<'a>(context: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        context
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal() -> Value {
        json!({
            "reason": "Add the application boundary",
            "changes": [{
                "type": "upsert_node",
                "node": {
                    "id": "desktop-application",
                    "label": "DesktopApplication",
                    "type": "service",
                    "summary": "Typed desktop application boundary",
                    "paths": ["apps/desktop"],
                    "tags": ["native"]
                }
            }]
        })
    }

    fn request(
        permission: &str,
        decision: Option<&str>,
        claim: &str,
        active: &str,
    ) -> AgentToolExecuteRequest {
        let mut context = json!({
            "permission": permission,
            "claimToken": claim,
            "activeClaimToken": active,
            "turnId": "turn-1",
            "productProjectId": "project-1",
            "productTaskId": "task-1",
            "projectArchitectureVersion": 3
        });
        if let Some(decision) = decision {
            context["decision"] = json!(decision);
        }
        AgentToolExecuteRequest {
            call_id: Some("architecture-1".into()),
            name: TOOL.into(),
            input: proposal(),
            session_id: Some("session-1".into()),
            context: Some(context),
            approval: None,
        }
    }

    #[test]
    fn descriptor_stays_a_custom_interaction_on_the_agent_run_protocol() {
        let descriptor = descriptor();
        assert_eq!(descriptor.name, TOOL);
        assert_eq!(descriptor.target_protocol_id, AGENT_RUN_PROTOCOL);
        assert!(matches!(
            descriptor.execution,
            AgentToolExecution::Interaction {
                interaction_kind: InteractionKind::Custom
            }
        ));
        assert!(descriptor.input_schema["properties"]["changes"].is_object());
    }

    #[test]
    fn execution_authorizes_ask_and_full_without_applying_and_blocks_readonly() {
        let ask = execute(request("ask", Some("allow"), "claim-1", "claim-1")).unwrap();
        assert_eq!(ask["permission"], "ask");
        assert_eq!(ask["requiresConfirmation"], true);
        assert_eq!(ask["authorized"], true);
        assert_eq!(ask["applied"], false);
        assert_eq!(ask["disposition"], "apply");
        assert_eq!(ask["status"], "pending");
        assert_eq!(ask["expectedVersion"], 3);
        assert_eq!(ask["changes"][0]["node"]["id"], "desktop-application");
        assert!(ask.get("graph").is_none());
        assert!(!ask.to_string().contains("claim-1"));

        let full = execute(request("free", Some("allow"), "claim-1", "claim-1")).unwrap();
        assert_eq!(full["permission"], "full");
        assert_eq!(full["requiresConfirmation"], false);
        assert_eq!(full["authorized"], true);
        assert_eq!(full["applied"], false);

        let denied = execute(request("readonly", Some("deny"), "claim-1", "claim-1")).unwrap();
        assert_eq!(denied["status"], "proposed");
        assert_eq!(denied["disposition"], "reject");
        assert_eq!(denied["authorized"], false);
        assert_eq!(denied["applied"], false);

        let readonly =
            execute(request("readonly", Some("allow"), "claim-1", "claim-1")).unwrap_err();
        assert_eq!(readonly.code, "lilia.architecture.permission");
        assert!(!readonly.message.contains("claim-1"));
    }

    #[test]
    fn execution_rejects_missing_and_stale_claims_and_holds_without_a_decision() {
        let missing = execute(request("ask", Some("allow"), "", "claim-1")).unwrap_err();
        assert_eq!(missing.code, "lilia.architecture.claim");
        let stale = execute(request("full", Some("allow"), "old-claim", "new-claim")).unwrap_err();
        assert_eq!(stale.code, "lilia.architecture.claim");
        assert!(!stale.message.contains("old-claim"));
        assert!(!stale.message.contains("new-claim"));

        let held = execute(request("ask", None, "claim-1", "claim-1")).unwrap();
        assert_eq!(held["disposition"], "hold");
        assert_eq!(held["authorized"], false);
        assert_eq!(held["applied"], false);
        assert_eq!(held["requiresConfirmation"], true);

        let mut invalid = request("ask", Some("allow"), "claim-1", "claim-1");
        invalid.input["changes"] = json!([]);
        assert_eq!(execute(invalid).unwrap_err().code, "agent.invalid_input");
    }
}
