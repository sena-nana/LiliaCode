use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use mutsuki_agent_adapter_api::{CredentialBroker, ModelProtocolAdapter};
use mutsuki_agent_adapter_openai::OpenAiCompatibleAdapter;
use mutsuki_agent_bundle::{
    native_coding_tool_plugin, AdapterBackedModelProvider, AgentLoop, AgentRuntimeRunner,
    ModelGateway, NativeCodingAgentBundle, ToolRegistry,
};
use mutsuki_agent_contracts::{
    AgentError, AgentMessage, AgentPermissionMode, AgentResult, AgentRunRequest, AgentRunResult,
    AgentRunStatus, AgentSessionCreateRequest, AgentSessionGetRequest, AgentToolDescriptor,
    AgentToolExecuteRequest, AgentToolExecution, InteractionKind, ToolSideEffect,
    ToolTargetPayloadMode, AGENT_RUN_PROTOCOL, AGENT_SESSION_CREATE_PROTOCOL,
    AGENT_SESSION_GET_PROTOCOL,
};
use mutsuki_runtime_contracts::{
    PluginDeploymentKind, RuntimeProfile, RuntimeProfileMode, Task, TaskBatch, TaskHandle,
    TaskOutcome, TaskStatus,
};
use mutsuki_runtime_host::{
    HostRuntime, HostRuntimeConfig, RuntimeBootstrapper, TokioAsyncExecutor,
};
use mutsuki_runtime_sdk::{
    contracts::RunnerResult, HostRuntime as _, PluginBuilder, ProtocolSpec, RuntimeClient,
    RuntimeClientRef, RuntimeFailure, RuntimeResult, SdkProtocol, TaskAwaitRunnerAdapter,
    TaskSubmitterRuntimeClient,
};
use serde_json::{json, Value};

use crate::anthropic_adapter::AnthropicMessagesAdapter;
use crate::model_turn::{openai_adapter_descriptor, LiveModelDriver, LiveModelTurnPlan};
use crate::subagent::NativeSubagentDefinition;

const HOST_PROFILE_ID: &str = "lilia.native-coding.agentkit-host";
const SUBAGENT_TOOL_NAME: &str = "delegate_agent";
const SUBAGENT_TOOL_PLUGIN_ID: &str = "lilia.plugin.agent.custom-subagent";
const SUBAGENT_TOOL_RUNNER_ID: &str = "lilia.agent.custom-subagent.runner";
const SUBAGENT_TOOL_PROTOCOL: &str = "lilia.agent.custom-subagent.tool@1";
const SUBAGENT_TASK_TIMEOUT: Duration = Duration::from_secs(90);
const MCP_TOOL_PLUGIN_ID: &str = "lilia.plugin.agent.shared-mcp";
const MCP_TOOL_PROTOCOL: &str = "lilia.agent.shared-mcp.tool@1";

#[derive(Clone, Debug)]
struct SharedMcpToolProtocol;
impl SdkProtocol for SharedMcpToolProtocol {
    const PROTOCOL_ID: &'static str = MCP_TOOL_PROTOCOL;
}
impl ProtocolSpec for SharedMcpToolProtocol {}
const PROJECT_ARCHITECTURE_TOOL_NAME: &str = "update_project_architecture";
const PROJECT_ARCHITECTURE_CONTRACT_JSON: &str =
    include_str!("../../lilia-contracts/contracts/architecture-contract.json");

#[derive(Clone, Debug)]
struct NativeSubagentToolProtocol;

impl SdkProtocol for NativeSubagentToolProtocol {
    const PROTOCOL_ID: &'static str = SUBAGENT_TOOL_PROTOCOL;
}

impl ProtocolSpec for NativeSubagentToolProtocol {}

#[derive(Default)]
struct DeferredRuntimeClient {
    client: OnceLock<RuntimeClientRef>,
}

impl DeferredRuntimeClient {
    fn bind(&self, runtime: &HostRuntime) -> AgentResult<()> {
        let submitter = runtime.host_context().task_submitter_ref();
        self.client
            .set(TaskSubmitterRuntimeClient::new(submitter).into_runtime_client())
            .map_err(|_| {
                AgentError::new("agent.host.already_bound", "runtime client already bound")
            })
    }

    fn client(&self) -> RuntimeResult<RuntimeClientRef> {
        self.client.get().cloned().ok_or_else(|| {
            RuntimeFailure::new(mutsuki_runtime_contracts::RuntimeError::new(
                "agent.host.not_bound",
                HOST_PROFILE_ID,
                "runtime_client.not_bound",
            ))
        })
    }
}

impl RuntimeClient for DeferredRuntimeClient {
    fn submit_batch(&self, batch: TaskBatch) -> RuntimeResult<Vec<TaskHandle>> {
        self.client()?.submit_batch(batch)
    }

    fn task_outcome(&self, handle: &TaskHandle) -> RuntimeResult<Option<TaskOutcome>> {
        self.client()?.task_outcome(handle)
    }
}

/// Product Host lifecycle wrapper. Agent/session/model/tool facts stay in the
/// AgentKit runners registered in this Host; Lilia only submits typed tasks.
pub(crate) struct AgentKitHost {
    runtime: Arc<HostRuntime>,
    next_task: AtomicU64,
    subagents: Option<Arc<LiveSubagentToolRuntime>>,
    mcp_calls: SharedMcpCalls,
    browser: Option<Arc<crate::BrowserSessions>>,
}

type SharedMcpCalls = Arc<Mutex<SharedMcpCallState>>;

#[derive(Default)]
struct SharedMcpCallState {
    active: BTreeMap<String, (String, String, mutsuki_agent_plugin_mcp::McpCancellation)>,
    turns: BTreeMap<(String, String), (u64, bool)>,
}

pub(crate) struct SharedMcpTurnGuard {
    calls: SharedMcpCalls,
    key: (String, String),
    generation: u64,
}
impl SharedMcpTurnGuard {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}
impl Drop for SharedMcpTurnGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.calls.lock() {
            if state
                .turns
                .get(&self.key)
                .map(|(generation, _)| *generation)
                != Some(self.generation)
            {
                return;
            }
            state.turns.remove(&self.key);
            for (session, turn, cancellation) in state.active.values() {
                if session == &self.key.0 && turn == &self.key.1 {
                    cancellation.cancel();
                }
            }
        }
    }
}

#[cfg(test)]
fn mcp_registration_hooks() -> &'static Mutex<BTreeMap<String, Box<dyn FnOnce() + Send>>> {
    static HOOKS: OnceLock<Mutex<BTreeMap<String, Box<dyn FnOnce() + Send>>>> = OnceLock::new();
    HOOKS.get_or_init(Mutex::default)
}

struct SharedMcpCallGuard {
    calls: SharedMcpCalls,
    task_id: String,
}
impl Drop for SharedMcpCallGuard {
    fn drop(&mut self) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.active.remove(&self.task_id);
        }
    }
}

impl AgentKitHost {
    pub(crate) fn build(
        bundle: NativeCodingAgentBundle,
        plan: Option<&LiveModelTurnPlan>,
        credentials: Arc<dyn CredentialBroker>,
        enable_workspace_tools: bool,
        subagents: &[NativeSubagentDefinition],
        browser: Option<Arc<crate::BrowserSessions>>,
    ) -> AgentResult<Self> {
        let enabled_subagents = subagents
            .iter()
            .filter(|subagent| subagent.enabled)
            .cloned()
            .collect::<Vec<_>>();
        let subagent_runtime = if enabled_subagents.is_empty() || plan.is_none() {
            None
        } else {
            let child_host = Arc::new(Self::build_host(
                bundle.clone(),
                plan,
                Arc::clone(&credentials),
                enable_workspace_tools,
                ToolAccess::ReadOnly,
                None,
                None,
            )?);
            Some(Arc::new(LiveSubagentToolRuntime::new(
                child_host,
                enabled_subagents,
            )))
        };
        Self::build_host(
            bundle,
            plan,
            credentials,
            enable_workspace_tools,
            ToolAccess::Full,
            subagent_runtime,
            browser,
        )
    }

    fn build_host(
        mut bundle: NativeCodingAgentBundle,
        plan: Option<&LiveModelTurnPlan>,
        credentials: Arc<dyn CredentialBroker>,
        enable_workspace_tools: bool,
        tool_access: ToolAccess,
        subagents: Option<Arc<LiveSubagentToolRuntime>>,
        browser: Option<Arc<crate::BrowserSessions>>,
    ) -> AgentResult<Self> {
        let mcp_names = bundle
            .mcp
            .catalog(None, None)?
            .tools
            .into_iter()
            .map(|tool| tool.namespaced_name)
            .collect::<std::collections::BTreeSet<_>>();
        let mut product_tools = bundle.routed_model_tools();
        for descriptor in &mut product_tools {
            if mcp_names.contains(&descriptor.name) {
                descriptor.target_protocol_id = MCP_TOOL_PROTOCOL.into();
                descriptor.target_payload_mode = ToolTargetPayloadMode::ExecutionRequest;
            }
        }
        if browser.is_some() {
            product_tools.push(crate::browser_tool::descriptor());
        }
        if !product_tools
            .iter()
            .any(|descriptor| descriptor.name == PROJECT_ARCHITECTURE_TOOL_NAME)
        {
            product_tools.push(project_architecture_tool_descriptor());
        }
        let routed_tools = product_tools
            .into_iter()
            .filter(|descriptor| {
                tool_access.allows(descriptor)
                    && (enable_workspace_tools
                        || mcp_names.contains(&descriptor.name)
                        || descriptor.name == crate::browser_tool::TOOL
                        || matches!(
                            &descriptor.execution,
                            AgentToolExecution::Interaction { .. }
                        ))
            })
            .chain(subagents.iter().map(|runtime| runtime.tool_descriptor()))
            .collect::<Vec<_>>();
        let tools = ToolRegistry::default();
        for descriptor in routed_tools.iter().cloned() {
            tools.register(descriptor)?;
        }
        bundle.core.tools = tools;
        bundle.core.context.set_tools(routed_tools.clone());

        if let Some(plan) = plan {
            let adapter: Arc<dyn ModelProtocolAdapter> = match plan.driver {
                LiveModelDriver::OpenAiCompatible => Arc::new(
                    OpenAiCompatibleAdapter::new(openai_adapter_descriptor(), credentials)
                        .map_err(protocol_error)?,
                ),
                LiveModelDriver::AnthropicMessages => Arc::new(
                    AnthropicMessagesAdapter::new(
                        AnthropicMessagesAdapter::default_descriptor(),
                        credentials,
                    )
                    .map_err(protocol_error)?,
                ),
            };
            let adapter = if let Some(sessions) = &browser {
                Arc::new(crate::browser_tool::PrivateBrowserAdapter::new(
                    adapter,
                    sessions.clone(),
                )) as Arc<dyn ModelProtocolAdapter>
            } else {
                adapter
            };
            let model = ModelGateway::with_default_provider(plan.provider.provider_id.clone());
            model.register(Arc::new(AdapterBackedModelProvider::new(
                plan.provider.clone(),
                adapter,
                routed_tools,
            )?));
            bundle.core.model = model;
            bundle.core.agent_loop = AgentLoop::default().with_default_model(plan.model.clone());
        }

        let deferred = Arc::new(DeferredRuntimeClient::default());
        let client: RuntimeClientRef = deferred.clone();
        let mut manifests = bundle.core.manifests();
        let mut native_tools = native_coding_tool_plugin(client.clone(), bundle.clone()).build();
        manifests.push(native_tools.manifest.clone());
        let mcp_calls = SharedMcpCalls::default();
        if let Some(runtime) = subagents.as_ref() {
            let _ = runtime.parent_calls.set(mcp_calls.clone());
        }
        let mut mcp_tools =
            shared_mcp_tool_plugin(client.clone(), bundle.mcp.clone(), mcp_calls.clone()).build();
        manifests.push(mcp_tools.manifest.clone());
        let mut browser_tools = browser
            .clone()
            .map(|sessions| crate::browser_tool::plugin(client.clone(), sessions).build());
        if let Some(plugin) = browser_tools.as_ref() {
            manifests.push(plugin.manifest.clone());
        }
        let mut subagent_tools = subagents.as_ref().map(|runtime| {
            native_subagent_tool_plugin(client.clone(), Arc::clone(runtime)).build()
        });
        if let Some(plugin) = subagent_tools.as_ref() {
            manifests.push(plugin.manifest.clone());
        }

        let mut bootstrapper = RuntimeBootstrapper::new();
        for manifest in &manifests {
            bootstrapper.register_manifest(manifest.clone());
        }
        for kind in AgentRuntimeRunner::ALL {
            bootstrapper.register_builtin_runner(bundle.core.runtime_runner(kind, client.clone()));
        }
        bootstrapper.register_async_handler(bundle.core.model_async_handler());
        for runner in native_tools.runners.drain(..) {
            bootstrapper.register_builtin_runner(runner);
        }
        for runner in mcp_tools.runners.drain(..) {
            bootstrapper.register_builtin_runner(runner);
        }
        if let Some(plugin) = browser_tools.as_mut() {
            for runner in plugin.runners.drain(..) {
                bootstrapper.register_builtin_runner(runner);
            }
        }
        if let Some(plugin) = subagent_tools.as_mut() {
            for runner in plugin.runners.drain(..) {
                bootstrapper.register_builtin_runner(runner);
            }
        }

        let enabled_plugins = manifests
            .iter()
            .map(|manifest| manifest.plugin_id.clone())
            .collect::<Vec<_>>();
        let profile = RuntimeProfile {
            profile_id: HOST_PROFILE_ID.into(),
            mode: RuntimeProfileMode::FullDev,
            enabled_plugins: enabled_plugins.clone(),
            bindings: BTreeMap::new(),
            surface_bindings: BTreeMap::new(),
            supported_extensions: Vec::new(),
            plugin_deployments: enabled_plugins
                .into_iter()
                .map(|plugin_id| (plugin_id, PluginDeploymentKind::Builtin))
                .collect(),
            observability: Default::default(),
            allow_dynamic_registration: false,
            allow_hot_reload: false,
        };
        let runtime = Arc::new(
            bootstrapper
                .into_host_runtime_with_config(
                    profile,
                    HostRuntimeConfig {
                        event_driven: true,
                        async_executor: Some(Arc::new(
                            TokioAsyncExecutor::new(2, 64, 64, 4 * 1024 * 1024)
                                .map_err(runtime_error)?,
                        )),
                        ..HostRuntimeConfig::default()
                    },
                )
                .map_err(runtime_error)?,
        );
        deferred.bind(&runtime)?;
        Ok(Self {
            runtime,
            next_task: AtomicU64::new(1),
            subagents,
            mcp_calls,
            browser,
        })
    }

    pub(crate) fn submit(
        &self,
        label: &str,
        protocol_id: &str,
        payload: serde_json::Value,
    ) -> AgentResult<TaskHandle> {
        if protocol_id == AGENT_RUN_PROTOCOL {
            if let (Some(browser), Ok(request)) = (
                &self.browser,
                serde_json::from_value::<AgentRunRequest>(payload.clone()),
            ) {
                for decision in request.permission_decisions {
                    if decision.decision
                        != mutsuki_agent_contracts::PermissionDecisionKind::Approved
                    {
                        browser
                            .private_inputs
                            .lock()
                            .expect("browser private inputs")
                            .discard_call(&decision.session_id, &decision.action_id);
                    }
                }
            }
        }
        let id = self.next_task.fetch_add(1, Ordering::Relaxed);
        self.runtime
            .submit_task(Task::new(
                format!("lilia:{label}:{id}"),
                protocol_id,
                payload,
            ))
            .map_err(runtime_error)
    }

    pub(crate) fn wait(
        &self,
        handle: &TaskHandle,
        timeout: Duration,
    ) -> AgentResult<serde_json::Value> {
        let states = self
            .runtime
            .wait_task_states(vec![handle.clone()], timeout)
            .map_err(runtime_error)?;
        if !states.first().is_some_and(|state| {
            matches!(
                state.status,
                Some(
                    TaskStatus::Completed
                        | TaskStatus::Failed
                        | TaskStatus::Cancelled
                        | TaskStatus::Expired
                        | TaskStatus::DeadLetter
                )
            )
        }) {
            return Err(AgentError::new(
                "agent.host.timeout",
                "AgentKit task did not reach a terminal state before the deadline",
            ));
        }
        let outcome = self
            .runtime
            .task_outcome(handle)
            .map_err(runtime_error)?
            .ok_or_else(|| {
                AgentError::new("agent.host.timeout", "AgentKit task outcome is unavailable")
            })?;
        task_output(outcome)
    }

    pub(crate) fn try_output(&self, handle: &TaskHandle) -> AgentResult<Option<serde_json::Value>> {
        self.runtime
            .task_outcome(handle)
            .map_err(runtime_error)?
            .map(task_output)
            .transpose()
    }

    pub(crate) fn cancel(&self, handle: &TaskHandle) -> AgentResult<()> {
        self.runtime.cancel_task(handle).map_err(runtime_error)
    }

    pub(crate) fn begin_mcp_turn(
        &self,
        session: &str,
        turn: &str,
    ) -> AgentResult<SharedMcpTurnGuard> {
        let key = (session.to_owned(), turn.to_owned());
        let mut state = self
            .mcp_calls
            .lock()
            .map_err(|_| AgentError::provider_unavailable("MCP active calls unavailable"))?;
        if state.turns.contains_key(&key) {
            return Err(AgentError::invalid_input("MCP turn is already active"));
        }
        static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        state.turns.insert(key.clone(), (generation, true));
        Ok(SharedMcpTurnGuard {
            calls: self.mcp_calls.clone(),
            key,
            generation,
        })
    }

    pub(crate) fn cancel_mcp(&self, session_id: &str, turn_id: &str) -> AgentResult<()> {
        let mut calls = self
            .mcp_calls
            .lock()
            .map_err(|_| AgentError::provider_unavailable("MCP active calls unavailable"))?;
        if let Some(open) = calls
            .turns
            .get_mut(&(session_id.to_owned(), turn_id.to_owned()))
        {
            open.1 = false;
        }
        for (session, turn, cancellation) in calls.active.values() {
            if session == session_id && turn == turn_id {
                cancellation.cancel();
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn before_mcp_registration(session: &str, hook: impl FnOnce() + Send + 'static) {
        mcp_registration_hooks()
            .lock()
            .unwrap()
            .insert(session.into(), Box::new(hook));
    }

    #[cfg(test)]
    pub(crate) fn mcp_turn_cancelled(&self, session: &str, turn: &str) -> bool {
        self.mcp_calls
            .lock()
            .unwrap()
            .turns
            .get(&(session.into(), turn.into()))
            .is_none_or(|(_, open)| !open)
    }

    #[cfg(test)]
    pub(crate) fn remove_mcp_turn_admission(&self, session: &str, turn: &str) {
        let key = (session.into(), turn.into());
        let generation = self
            .mcp_calls
            .lock()
            .unwrap()
            .turns
            .get(&key)
            .map(|entry| entry.0);
        let Some(generation) = generation else {
            return;
        };
        drop(SharedMcpTurnGuard {
            calls: self.mcp_calls.clone(),
            key,
            generation,
        });
    }

    #[cfg(test)]
    pub(crate) fn mcp_turn_count(&self) -> usize {
        self.mcp_calls.lock().unwrap().turns.len()
    }

    pub(crate) fn cancel_subagents(&self, parent_session_id: &str) -> AgentResult<usize> {
        self.subagents
            .as_ref()
            .map_or(Ok(0), |runtime| runtime.cancel_parent(parent_session_id))
    }
}

fn project_architecture_tool_descriptor() -> AgentToolDescriptor {
    let contract: Value = serde_json::from_str(PROJECT_ARCHITECTURE_CONTRACT_JSON)
        .expect("architecture-contract.json must be valid JSON");
    let mut descriptor = AgentToolDescriptor::new(
        PROJECT_ARCHITECTURE_TOOL_NAME,
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
enum ToolAccess {
    Full,
    ReadOnly,
}

impl ToolAccess {
    fn allows(self, descriptor: &AgentToolDescriptor) -> bool {
        match self {
            Self::Full => true,
            Self::ReadOnly => {
                matches!(&descriptor.execution, AgentToolExecution::Routed)
                    && !descriptor.requires_approval
                    && matches!(
                        descriptor.side_effect,
                        ToolSideEffect::None
                            | ToolSideEffect::WorkspaceRead
                            | ToolSideEffect::ExternalRead
                    )
            }
        }
    }
}

struct ActiveSubagent {
    handle: TaskHandle,
    session_id: String,
    turn_id: String,
}

struct LiveSubagentToolRuntime {
    child_host: Arc<AgentKitHost>,
    definitions: BTreeMap<String, NativeSubagentDefinition>,
    results: Mutex<BTreeMap<String, Value>>,
    active: Mutex<BTreeMap<String, Vec<ActiveSubagent>>>,
    parent_calls: OnceLock<SharedMcpCalls>,
    gates: Mutex<BTreeMap<String, Arc<Mutex<()>>>>,
}

impl LiveSubagentToolRuntime {
    fn new(child_host: Arc<AgentKitHost>, definitions: Vec<NativeSubagentDefinition>) -> Self {
        Self {
            child_host,
            definitions: definitions
                .into_iter()
                .map(|definition| (definition.id.clone(), definition))
                .collect(),
            results: Mutex::new(BTreeMap::new()),
            active: Mutex::new(BTreeMap::new()),
            parent_calls: OnceLock::new(),
            gates: Mutex::new(BTreeMap::new()),
        }
    }

    fn tool_descriptor(&self) -> AgentToolDescriptor {
        let agent_ids = self.definitions.keys().cloned().collect::<Vec<_>>();
        let catalog = self
            .definitions
            .values()
            .map(|definition| {
                let summary = if definition.description.is_empty() {
                    definition.name.as_str()
                } else {
                    definition.description.as_str()
                };
                format!("{} ({}): {}", definition.name, definition.id, summary)
            })
            .collect::<Vec<_>>()
            .join("; ");
        let mut descriptor = AgentToolDescriptor::new(
            SUBAGENT_TOOL_NAME,
            SUBAGENT_TOOL_PROTOCOL,
            format!(
                "Delegate a bounded read-only research or review task to one configured Agent. Available Agents: {catalog}"
            ),
        );
        descriptor.input_schema = json!({
            "type": "object",
            "required": ["agentId", "task"],
            "properties": {
                "agentId": {"type": "string", "enum": agent_ids},
                "task": {"type": "string", "minLength": 1, "maxLength": 16000}
            },
            "additionalProperties": false
        });
        descriptor.output_schema = json!({
            "type": "object",
            "required": ["agentId", "agentName", "status", "summary"],
            "properties": {
                "agentId": {"type": "string"},
                "agentName": {"type": "string"},
                "status": {"type": "string"},
                "summary": {"type": "string"}
            }
        });
        descriptor.side_effect = ToolSideEffect::WorkspaceRead;
        descriptor.target_payload_mode = ToolTargetPayloadMode::ExecutionRequest;
        descriptor
    }

    fn execute(&self, request: AgentToolExecuteRequest) -> AgentResult<Value> {
        if request.name != SUBAGENT_TOOL_NAME {
            return Err(AgentError::not_found(format!(
                "custom subagent tool `{}` is not registered",
                request.name
            )));
        }
        let parent_session_id = request
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AgentError::invalid_input("subagent parent session_id is required"))?
            .to_owned();
        if parent_session_id.contains(":subagent:") {
            return Err(AgentError::new(
                "agent.subagent.depth_exceeded",
                "custom subagents cannot recursively delegate",
            ));
        }
        let context = request.context.as_ref().and_then(Value::as_object);
        let fallback_turn_id = request
            .call_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("turn");
        let parent_turn_id = context
            .and_then(|value| value.get("turn_id").or_else(|| value.get("turnId")))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(fallback_turn_id)
            .to_owned();
        let input = request
            .input
            .as_object()
            .ok_or_else(|| AgentError::invalid_input("subagent tool input must be an object"))?;
        let agent_id = required_input_string(input, "agentId")?;
        let task = required_input_string(input, "task")?;
        if task.chars().count() > 16_000 {
            return Err(AgentError::invalid_input(
                "subagent task exceeds 16000 characters",
            ));
        }
        let definition = self.definitions.get(&agent_id).cloned().ok_or_else(|| {
            AgentError::not_found(format!("custom subagent `{agent_id}` is not enabled"))
        })?;
        let call_id = request
            .call_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}:{}:{}", parent_turn_id, definition.id, task));
        let idempotency_key = format!("{parent_session_id}:{parent_turn_id}:{call_id}");
        let gate = {
            let mut gates = self
                .gates
                .lock()
                .map_err(|_| AgentError::provider_unavailable("subagent gate state unavailable"))?;
            Arc::clone(
                gates
                    .entry(idempotency_key.clone())
                    .or_insert_with(|| Arc::new(Mutex::new(()))),
            )
        };
        let _guard = gate
            .lock()
            .map_err(|_| AgentError::provider_unavailable("subagent call gate unavailable"))?;
        if let Some(result) = self
            .results
            .lock()
            .map_err(|_| AgentError::provider_unavailable("subagent result state unavailable"))?
            .get(&idempotency_key)
            .cloned()
        {
            return Ok(result);
        }

        let child_session_id = format!(
            "{}:subagent:{}:{}",
            parent_session_id,
            definition.id,
            compact_identifier(&call_id)
        );
        let child_turn_id = format!("{parent_turn_id}:subagent:{}", compact_identifier(&call_id));
        let system_prompt = format!(
            "You are the configured Lilia Agent named `{}`.\n\n{}\n\nWork as a bounded read-only subagent. Use only the read tools exposed to you. Do not ask the user, modify files, or delegate again. Return concise findings and concrete evidence to the parent Agent.",
            definition.name, definition.instruction
        );
        let mut child_request = AgentRunRequest::new(
            format!("lilia.custom-subagent.{}", definition.id),
            vec![
                AgentMessage::system(system_prompt),
                AgentMessage::user(task),
            ],
        );
        self.ensure_child_session(
            &child_session_id,
            &child_request.profile_id,
            &definition.name,
        )?;
        let admission = self
            .child_host
            .begin_mcp_turn(&child_session_id, &child_turn_id)?;
        child_request.session_id = Some(child_session_id.clone());
        child_request.turn_id = Some(child_turn_id.clone());
        child_request.permission_mode = AgentPermissionMode::ReadOnly;
        child_request.max_steps = 8;
        child_request.metadata = Some(json!({
            "parentSessionId": parent_session_id,
            "parentTurnId": parent_turn_id,
            "turn_id": child_turn_id,
            "mcpAdmission": admission.generation(),
            "subagentId": definition.id,
            "subagentName": definition.name,
            "callId": call_id,
        }));
        let handle = {
            let mut active = self.active.lock().map_err(|_| {
                AgentError::provider_unavailable("subagent active state unavailable")
            })?;
            let parent_calls = self.parent_calls.get().ok_or_else(|| {
                AgentError::provider_unavailable("subagent parent lifecycle unavailable")
            })?;
            let generation = context
                .and_then(|metadata| metadata.get("mcpAdmission"))
                .and_then(Value::as_u64);
            let admitted = parent_calls
                .lock()
                .map_err(|_| AgentError::provider_unavailable("parent turn state unavailable"))?
                .turns
                .get(&(parent_session_id.clone(), parent_turn_id.clone()))
                .is_some_and(|(current, open)| *open && Some(*current) == generation);
            if !admitted {
                return Err(AgentError::new(
                    "agent.subagent.cancelled",
                    "Parent turn is no longer active",
                ));
            }
            let handle = self.child_host.submit(
                "custom-subagent",
                AGENT_RUN_PROTOCOL,
                serde_json::to_value(child_request)
                    .map_err(|error| AgentError::invalid_input(error.to_string()))?,
            )?;
            active
                .entry(parent_session_id.clone())
                .or_default()
                .push(ActiveSubagent {
                    handle: handle.clone(),
                    session_id: child_session_id,
                    turn_id: child_turn_id,
                });
            handle
        };
        let output = self.child_host.wait(&handle, SUBAGENT_TASK_TIMEOUT);
        self.remove_active(&parent_session_id, &handle)?;
        let run: AgentRunResult = serde_json::from_value(output?)
            .map_err(|error| AgentError::new("agent.subagent.result_invalid", error.to_string()))?;
        let summary = run
            .messages
            .iter()
            .rev()
            .find(|message| message.role == mutsuki_agent_contracts::AgentRole::Assistant)
            .map(|message| message.content.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{} finished without a text result", definition.name));
        let status = match run.status {
            AgentRunStatus::Completed => "completed",
            AgentRunStatus::Cancelled => "cancelled",
            AgentRunStatus::BudgetExceeded => "budget_exceeded",
            AgentRunStatus::Failed => "failed",
            AgentRunStatus::WaitingApproval | AgentRunStatus::WaitingInteraction => {
                return Err(AgentError::new(
                    "agent.subagent.unexpected_interaction",
                    "read-only custom subagent requested an unsupported interaction",
                ));
            }
        };
        let result = json!({
            "agentId": definition.id,
            "agentName": definition.name,
            "status": status,
            "summary": summary,
            "usage": run.usage,
        });
        self.results
            .lock()
            .map_err(|_| AgentError::provider_unavailable("subagent result state unavailable"))?
            .insert(idempotency_key, result.clone());
        Ok(result)
    }

    fn ensure_child_session(
        &self,
        session_id: &str,
        profile_id: &str,
        title: &str,
    ) -> AgentResult<()> {
        let get = self.child_host.submit(
            "custom-subagent-session-get",
            AGENT_SESSION_GET_PROTOCOL,
            serde_json::to_value(AgentSessionGetRequest {
                session_id: session_id.to_owned(),
            })
            .map_err(|error| AgentError::invalid_input(error.to_string()))?,
        )?;
        match self.child_host.wait(&get, SUBAGENT_TASK_TIMEOUT) {
            Ok(_) => return Ok(()),
            Err(error) if error.code.contains("not_found") => {}
            Err(error) => return Err(error),
        }
        let create = self.child_host.submit(
            "custom-subagent-session-create",
            AGENT_SESSION_CREATE_PROTOCOL,
            serde_json::to_value(AgentSessionCreateRequest {
                session_id: Some(session_id.to_owned()),
                profile_id: profile_id.to_owned(),
                title: Some(title.to_owned()),
            })
            .map_err(|error| AgentError::invalid_input(error.to_string()))?,
        )?;
        self.child_host
            .wait(&create, SUBAGENT_TASK_TIMEOUT)
            .map(|_| ())
    }

    fn remove_active(&self, parent_session_id: &str, handle: &TaskHandle) -> AgentResult<()> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| AgentError::provider_unavailable("subagent active state unavailable"))?;
        if let Some(handles) = active.get_mut(parent_session_id) {
            handles.retain(|candidate| candidate.handle.task_id != handle.task_id);
            if handles.is_empty() {
                active.remove(parent_session_id);
            }
        }
        Ok(())
    }

    fn cancel_parent(&self, parent_session_id: &str) -> AgentResult<usize> {
        let handles = self
            .active
            .lock()
            .map_err(|_| AgentError::provider_unavailable("subagent active state unavailable"))?
            .remove(parent_session_id)
            .unwrap_or_default();
        for child in &handles {
            self.child_host
                .cancel_mcp(&child.session_id, &child.turn_id)?;
            self.child_host.cancel(&child.handle)?;
        }
        Ok(handles.len())
    }
}

fn shared_mcp_tool_plugin(
    client: RuntimeClientRef,
    service: Arc<mutsuki_agent_plugin_mcp::SharedMcpService>,
    calls: SharedMcpCalls,
) -> PluginBuilder {
    let descriptor = mutsuki_agent_sdk::orchestration_runner(
        "lilia.agent.shared-mcp.runner",
        MCP_TOOL_PLUGIN_ID,
    )
    .accepts::<SharedMcpToolProtocol>()
    .build();
    PluginBuilder::new(MCP_TOOL_PLUGIN_ID)
        .protocol::<SharedMcpToolProtocol>()
        .runner(Box::new(TaskAwaitRunnerAdapter::new(
            descriptor,
            client,
            Box::new(move |_context, task| {
                let service = service.clone();
                let calls = calls.clone();
                Box::pin(async move {
                    let result = (|| -> AgentResult<Value> {
                        let request: AgentToolExecuteRequest =
                            serde_json::from_value(task.payload.clone().into())
                                .map_err(|error| AgentError::invalid_input(error.to_string()))?;
                        if request.approval.as_ref().is_some_and(|approval| {
                            approval.decision.decision
                                != mutsuki_agent_contracts::PermissionDecisionKind::Approved
                        }) {
                            return Err(AgentError::new(
                                "agent.permission.denied",
                                "MCP tool approval was not granted",
                            ));
                        }
                        let control = mutsuki_agent_plugin_mcp::McpRequestControl::default();
                        let session = request.session_id.as_deref().ok_or_else(|| {
                            AgentError::invalid_input("MCP call session is required")
                        })?;
                        let turn = request
                            .context
                            .as_ref()
                            .and_then(|context| context.get("turn_id"))
                            .and_then(Value::as_str)
                            .filter(|turn| !turn.is_empty())
                            .ok_or_else(|| {
                                AgentError::invalid_input("MCP call turn is required")
                            })?;
                        #[cfg(test)]
                        {
                            let hook = mcp_registration_hooks().lock().unwrap().remove(session);
                            if let Some(hook) = hook {
                                hook();
                            }
                        }
                        {
                            let mut state = calls.lock().map_err(|_| {
                                AgentError::provider_unavailable("MCP active calls unavailable")
                            })?;
                            let generation = request
                                .context
                                .as_ref()
                                .and_then(|context| context.get("mcpAdmission"))
                                .and_then(Value::as_u64);
                            if !state
                                .turns
                                .get(&(session.to_owned(), turn.to_owned()))
                                .is_some_and(|(admitted, open)| {
                                    *open && Some(*admitted) == generation
                                })
                            {
                                return Err(AgentError::new(
                                    "agent.mcp.cancelled",
                                    "MCP turn was cancelled",
                                ));
                            }
                            state.active.insert(
                                task.task_id.clone(),
                                (
                                    session.to_owned(),
                                    turn.to_owned(),
                                    control.cancellation.clone(),
                                ),
                            );
                        }
                        let _guard = SharedMcpCallGuard {
                            calls,
                            task_id: task.task_id.clone(),
                        };
                        serde_json::to_value(service.call_tool(
                            &request.name,
                            request.input,
                            &control,
                        )?)
                        .map_err(|error| AgentError::invalid_input(error.to_string()))
                    })()
                    .map_err(|error| {
                        mutsuki_agent_sdk::runtime_failure(MCP_TOOL_PLUGIN_ID, &task.task_id, error)
                    })?;
                    let mut completed = RunnerResult::completed(task.task_id);
                    completed.output = Some(result);
                    Ok(completed)
                })
            }),
        )))
}

fn native_subagent_tool_plugin(
    client: RuntimeClientRef,
    runtime: Arc<LiveSubagentToolRuntime>,
) -> PluginBuilder {
    let descriptor =
        mutsuki_agent_sdk::orchestration_runner(SUBAGENT_TOOL_RUNNER_ID, SUBAGENT_TOOL_PLUGIN_ID)
            .accepts::<NativeSubagentToolProtocol>()
            .build();
    PluginBuilder::new(SUBAGENT_TOOL_PLUGIN_ID)
        .protocol::<NativeSubagentToolProtocol>()
        .runner(Box::new(TaskAwaitRunnerAdapter::new(
            descriptor,
            client,
            Box::new(move |_context, task| {
                let runtime = Arc::clone(&runtime);
                Box::pin(async move { run_native_subagent_tool(runtime, task) })
            }),
        )))
}

fn run_native_subagent_tool(
    runtime: Arc<LiveSubagentToolRuntime>,
    task: Task,
) -> RuntimeResult<RunnerResult> {
    let request: AgentToolExecuteRequest = serde_json::from_value(task.payload.clone().into())
        .map_err(|error| {
            mutsuki_agent_sdk::runtime_failure(
                SUBAGENT_TOOL_PLUGIN_ID,
                &task.task_id,
                AgentError::invalid_input(error.to_string()),
            )
        })?;
    let output = runtime.execute(request).map_err(|error| {
        mutsuki_agent_sdk::runtime_failure(SUBAGENT_TOOL_PLUGIN_ID, &task.task_id, error)
    })?;
    let mut result = RunnerResult::completed(task.task_id);
    result.output = Some(output);
    Ok(result)
}

fn required_input_string(
    input: &serde_json::Map<String, Value>,
    field: &'static str,
) -> AgentResult<String> {
    input
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| AgentError::invalid_input(format!("subagent tool requires `{field}`")))
}

fn compact_identifier(value: &str) -> String {
    let compact = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(64)
        .collect::<String>();
    if compact.is_empty() {
        "call".to_owned()
    } else {
        compact
    }
}

fn task_output(outcome: TaskOutcome) -> AgentResult<serde_json::Value> {
    match outcome {
        TaskOutcome::Completed {
            output: Some(output),
            ..
        } => Ok(output),
        TaskOutcome::Completed { .. } => Err(AgentError::new(
            "agent.result_missing",
            "AgentKit task completed without a typed result",
        )),
        TaskOutcome::Failed { error, .. } => Err(AgentError::new(
            error.code,
            error
                .evidence
                .get("message")
                .and_then(|value| match value {
                    mutsuki_runtime_contracts::ScalarValue::String(message) => {
                        Some(message.clone())
                    }
                    _ => None,
                })
                .unwrap_or(error.route),
        )),
        other => Err(AgentError::new(
            "agent.host.task_failed",
            format!("AgentKit task did not complete: {other:?}"),
        )),
    }
}

fn protocol_error(error: mutsuki_agent_contracts::ProtocolError) -> AgentError {
    AgentError::new(error.code, error.message)
}

fn runtime_error(error: RuntimeFailure) -> AgentError {
    let runtime = error.error();
    AgentError::new(
        runtime.code.clone(),
        runtime
            .evidence
            .get("message")
            .and_then(|value| match value {
                mutsuki_runtime_contracts::ScalarValue::String(message) => Some(message.clone()),
                _ => None,
            })
            .unwrap_or_else(|| runtime.route.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_without_local_workspace_is_registered_through_the_real_tool_runner() {
        struct Host;
        impl crate::TaskBrowserHost for Host {
            fn execute(
                &self,
                _: &lilia_contracts::BrowserRequest,
                _: &crate::BrowserCancellation,
            ) -> Result<lilia_contracts::BrowserPage, crate::BrowserError> {
                Err(crate::BrowserError::Unavailable)
            }
            fn cancel(&self, _: &lilia_contracts::BrowserScope) {}
        }
        for (access, expected) in [(ToolAccess::Full, true), (ToolAccess::ReadOnly, false)] {
            let bootstrap = crate::NativeRuntimeBootstrap::embedded_reference().unwrap();
            let host = AgentKitHost::build_host(
                bootstrap.bundle().clone(),
                None,
                crate::model_turn::adapter_credential_broker(
                    bootstrap.credentials().broker().clone(),
                ),
                false,
                access,
                None,
                Some(Arc::new(crate::BrowserSessions::for_test(Arc::new(Host)))),
            )
            .unwrap();
            let handle = host
                .submit(
                    "list-browser",
                    mutsuki_agent_contracts::AGENT_TOOL_LIST_PROTOCOL,
                    json!({}),
                )
                .unwrap();
            let output = host.wait(&handle, Duration::from_secs(5)).unwrap();
            let listed: mutsuki_agent_contracts::AgentToolListResult =
                serde_json::from_value(output).unwrap();
            let browser = listed
                .tools
                .iter()
                .find(|tool| tool.name == crate::browser_tool::TOOL);
            assert_eq!(browser.is_some(), expected);
            if let Some(browser) = browser {
                assert!(browser.requires_approval);
                assert_eq!(browser.side_effect, ToolSideEffect::ExternalWrite);
            }
            assert!(listed
                .tools
                .iter()
                .all(|tool| tool.name == crate::browser_tool::TOOL
                    || matches!(tool.execution, AgentToolExecution::Interaction { .. })));
        }
    }

    #[test]
    fn architecture_tool_is_a_typed_model_visible_interaction() {
        let descriptor = project_architecture_tool_descriptor();
        assert_eq!(descriptor.name, PROJECT_ARCHITECTURE_TOOL_NAME);
        assert_eq!(descriptor.target_protocol_id, AGENT_RUN_PROTOCOL);
        assert!(matches!(
            descriptor.execution,
            AgentToolExecution::Interaction {
                interaction_kind: InteractionKind::Custom
            }
        ));
        assert_eq!(descriptor.input_schema["type"], "object");
        assert!(descriptor.input_schema["properties"]["changes"].is_object());
    }
}
