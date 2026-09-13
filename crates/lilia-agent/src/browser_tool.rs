use std::sync::Arc;

use lilia_contracts::{BrowserOperation, BrowserRequest};
use mutsuki_agent_contracts::{
    AgentError, AgentToolDescriptor, AgentToolExecuteRequest, PermissionDecisionKind,
    ToolSideEffect, ToolTargetPayloadMode,
};
use mutsuki_runtime_contracts::Task;
use mutsuki_runtime_sdk::{
    contracts::RunnerResult, PluginBuilder, ProtocolSpec, RuntimeClientRef, RuntimeResult,
    SdkProtocol, TaskAwaitRunnerAdapter,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::BrowserSessions;

const PLUGIN: &str = "lilia.plugin.task-browser";
const PROTOCOL: &str = "lilia.agent.task-browser@1";
pub(crate) const TOOL: &str = "task_browser";

const PRIVATE_INPUT_LIMIT: usize = 128;
const PRIVATE_INPUT_BYTES: usize = 4 * 1024 * 1024;
const PRIVATE_INPUT_MAX_BYTES: usize = 1024 * 1024;
const PRIVATE_INPUT_TTL: std::time::Duration = std::time::Duration::from_secs(300);

#[derive(Default)]
pub(crate) struct BrowserPrivateInputs {
    entries: std::collections::BTreeMap<String, PrivateInput>,
    expiry_started: bool,
}
struct PrivateInput {
    session: String,
    call: String,
    scope: lilia_contracts::BrowserScope,
    lifecycle: u64,
    expected: Value,
    operation: BrowserOperation,
    created: std::time::Instant,
    size_bytes: usize,
}
impl BrowserPrivateInputs {
    pub(crate) fn discard_scope(&mut self, scope: &lilia_contracts::BrowserScope) {
        self.entries.retain(|_, entry| &entry.scope != scope);
    }
    pub(crate) fn discard_session(&mut self, session: &str) {
        self.entries.retain(|_, entry| entry.session != session);
    }
    pub(crate) fn discard_call(&mut self, session: &str, call: &str) {
        self.entries
            .retain(|_, entry| entry.session != session || entry.call != call);
    }
    fn start_expiry(inputs: &Arc<std::sync::Mutex<Self>>) -> Result<(), AgentError> {
        let mut guard = inputs.lock().expect("browser private inputs");
        if guard.expiry_started {
            return Ok(());
        }
        let weak = Arc::downgrade(inputs);
        std::thread::Builder::new()
            .name("browser-private-expiry".into())
            .spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let Some(inputs) = weak.upgrade() else {
                    break;
                };
                inputs.lock().expect("browser private inputs").sweep();
            })
            .map_err(|_| {
                AgentError::new(
                    "lilia.browser.private_expiry",
                    "private input expiry is unavailable",
                )
            })?;
        guard.expiry_started = true;
        Ok(())
    }
    fn sweep(&mut self) {
        self.entries
            .retain(|_, entry| entry.created.elapsed() < PRIVATE_INPUT_TTL);
    }
    fn take(&mut self, key: &str) -> Option<PrivateInput> {
        self.sweep();
        self.entries.remove(key)
    }
}

fn safe_browser_url(value: &str) -> String {
    if value == "about:blank" {
        return value.into();
    }
    reqwest::Url::parse(value)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .map(|mut url| {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        })
        .unwrap_or_default()
}

fn private_strings(operation: &BrowserOperation) -> Vec<String> {
    match operation {
        BrowserOperation::Type { text, .. } => vec![text.clone()],
        BrowserOperation::Navigate { url } => {
            let mut values = vec![url.clone()];
            if let Ok(url) = reqwest::Url::parse(url) {
                values.push(url.username().into());
                if let Some(value) = url.password() {
                    values.push(value.into());
                }
                if let Some(value) = url.fragment() {
                    values.push(value.into());
                }
                values.extend(url.query_pairs().map(|(_, value)| value.into_owned()));
            }
            values
        }
        _ => vec![],
    }
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect()
}

fn protect_model_result(
    sessions: &BrowserSessions,
    session: Option<&str>,
    result: &mut mutsuki_agent_contracts::ModelGenerateResult,
) -> Result<(), AgentError> {
    let mut inserted = Vec::new();
    let protected = protect_model_result_inner(sessions, session, result, &mut inserted);
    if protected.is_err() {
        let mut inputs = sessions
            .private_inputs
            .lock()
            .expect("browser private inputs");
        for key in inserted {
            inputs.entries.remove(&key);
        }
    }
    protected
}

fn protect_model_result_inner(
    sessions: &BrowserSessions,
    session: Option<&str>,
    result: &mut mutsuki_agent_contracts::ModelGenerateResult,
    inserted: &mut Vec<String>,
) -> Result<(), AgentError> {
    let mut changed = false;
    for call in &mut result.tool_calls {
        if call.name != TOOL {
            continue;
        }
        if call.input.pointer("/operation/privateInput").is_some() {
            return Err(AgentError::invalid_input(
                "private browser references cannot be replayed by the model",
            ));
        }
        let parsed: Input = serde_json::from_value(call.input.clone())
            .map_err(|_| AgentError::invalid_input("invalid browser operation"))?;
        let operation = parsed.operation;
        if !matches!(
            operation,
            BrowserOperation::Type { .. } | BrowserOperation::Navigate { .. }
        ) {
            continue;
        }
        let session =
            session.ok_or_else(|| AgentError::invalid_input("browser session is required"))?;
        let scope = sessions.agent_scope(session).map_err(browser_error)?;
        sessions.validate_scope(&scope).map_err(browser_error)?;
        let state = sessions.state(&scope).map_err(browser_error)?;
        BrowserPrivateInputs::start_expiry(&sessions.private_inputs)?;
        let key = uuid::Uuid::new_v4().to_string();
        let mut expected = json!({"operation": &operation});
        if let Some(value) = parsed.lifecycle {
            expected["lifecycle"] = json!(value);
        }
        if let Some(value) = parsed.page_version {
            expected["pageVersion"] = json!(value);
        }
        let object = expected
            .get_mut("operation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| AgentError::invalid_input("invalid browser operation"))?;
        let redactions = private_strings(&operation);
        let (sensitive, site) = match &operation {
            BrowserOperation::Type { text, .. } => {
                object.insert("text".into(), Value::String("[private input]".into()));
                (text.clone(), safe_browser_url(&state.page.url))
            }
            BrowserOperation::Navigate { url } => {
                let site = safe_browser_url(url);
                object.insert("url".into(), Value::String(site.clone()));
                (url.clone(), site)
            }
            _ => unreachable!(),
        };
        object.insert("privateInput".into(), Value::String(key.clone()));
        object.insert("site".into(), Value::String(site));
        let mut inputs = sessions
            .private_inputs
            .lock()
            .expect("browser private inputs");
        inputs.sweep();
        let size_bytes = sensitive.len().saturating_add(expected.to_string().len());
        if size_bytes > PRIVATE_INPUT_MAX_BYTES
            || inputs.entries.len() >= PRIVATE_INPUT_LIMIT
            || inputs
                .entries
                .values()
                .map(|entry| entry.size_bytes)
                .sum::<usize>()
                .saturating_add(size_bytes)
                > PRIVATE_INPUT_BYTES
        {
            return Err(AgentError::new(
                "lilia.browser.private_capacity",
                "too many pending private browser actions",
            ));
        }
        inserted.push(key.clone());
        inputs.entries.insert(
            key,
            PrivateInput {
                session: session.into(),
                call: call.call_id.clone(),
                scope,
                lifecycle: state.lifecycle,
                expected: expected.clone(),
                operation,
                created: std::time::Instant::now(),
                size_bytes,
            },
        );
        drop(inputs);
        if sessions
            .state(&state.scope)
            .map_err(browser_error)?
            .lifecycle
            != state.lifecycle
        {
            return Err(AgentError::new(
                "lilia.browser.private_stale",
                "browser control changed while preparing input",
            ));
        }
        call.input = expected;
        for sensitive in redactions {
            result.message.content = result
                .message
                .content
                .replace(&sensitive, "[private input]");
        }
        changed = true;
    }
    if changed {
        result.raw = None;
        result.message.parts.clear();
        result.message.metadata = Some(json!({"tool_calls": result.tool_calls}));
    }
    Ok(())
}

pub(crate) struct PrivateBrowserAdapter {
    inner: Arc<dyn mutsuki_agent_adapter_api::ModelProtocolAdapter>,
    sessions: Arc<BrowserSessions>,
}
impl PrivateBrowserAdapter {
    pub(crate) fn new(
        inner: Arc<dyn mutsuki_agent_adapter_api::ModelProtocolAdapter>,
        sessions: Arc<BrowserSessions>,
    ) -> Self {
        Self { inner, sessions }
    }
}
impl mutsuki_agent_adapter_api::ModelProtocolAdapter for PrivateBrowserAdapter {
    fn descriptor(&self) -> &mutsuki_agent_adapter_api::ModelProtocolAdapterDescriptor {
        self.inner.descriptor()
    }
    fn generate(
        &self,
        provider: mutsuki_agent_adapter_api::ProviderInstanceDescriptor,
        request: mutsuki_agent_adapter_api::ModelGenerateRequest,
    ) -> mutsuki_agent_adapter_api::ModelAdapterFuture {
        let session = request.request.session_id.clone();
        let future = self.inner.generate(provider, request);
        let sessions = self.sessions.clone();
        Box::pin(async move {
            let mut result = future.await?;
            protect_model_result(&sessions, session.as_deref(), &mut result).map_err(|_| {
                mutsuki_agent_contracts::ProtocolError {
                    code: "lilia.browser.private_input".into(),
                    class: mutsuki_agent_contracts::ProtocolErrorClass::NonRetryable,
                    message: "private browser input could not be prepared; retry the action".into(),
                    retry_after_ms: None,
                }
            })?;
            Ok(result)
        })
    }
}

#[derive(Clone, Debug)]
struct BrowserProtocol;
impl SdkProtocol for BrowserProtocol {
    const PROTOCOL_ID: &'static str = PROTOCOL;
}
impl ProtocolSpec for BrowserProtocol {}

pub(crate) fn descriptor() -> AgentToolDescriptor {
    let mut tool = AgentToolDescriptor::new(
        TOOL,
        PROTOCOL,
        "Operate the task's embedded browser. Observe first and use returned lifecycle/pageVersion for subsequent actions. Human takeover requires explicit human resume.",
    );
    tool.input_schema = json!({"type":"object","properties":{
        "lifecycle":{"type":"integer","minimum":1},"pageVersion":{"type":"integer","minimum":1},
        "operation":{"type":"object","properties":{"kind":{"type":"string","enum":["observe","navigate","back","forward","reload","click","type","scroll","screenshot"]},"url":{"type":"string"},"target":{"type":"string"},"text":{"type":"string"},"privateInput":{"type":"string","description":"Host-managed private input reference"},"site":{"type":"string","description":"Host-provided approval site"},"x":{"type":"integer"},"y":{"type":"integer"}},"required":["kind"],"additionalProperties":false}
    },"required":["operation"],"additionalProperties":false});
    tool.side_effect = ToolSideEffect::ExternalWrite;
    tool.requires_approval = true;
    tool.target_payload_mode = ToolTargetPayloadMode::ExecutionRequest;
    tool
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    lifecycle: Option<u64>,
    page_version: Option<u64>,
    operation: BrowserOperation,
}

fn execute(
    sessions: &BrowserSessions,
    request: AgentToolExecuteRequest,
) -> Result<Value, AgentError> {
    let private_key = request
        .input
        .pointer("/operation/privateInput")
        .and_then(Value::as_str);
    let private = private_key.and_then(|key| {
        sessions
            .private_inputs
            .lock()
            .expect("browser private inputs")
            .take(key)
    });
    let denied = || {
        AgentError::new(
            "lilia.browser.permission",
            "browser approval is missing or stale",
        )
    };
    let session = request.session_id.as_deref().ok_or_else(denied)?;
    let approval = request.approval.as_ref().ok_or_else(denied)?;
    let grant = &approval.request;
    let decision = &approval.decision;
    if request.name != TOOL
        || grant.tool != TOOL
        || grant.session_id != session
        || decision.session_id != session
        || decision.decision != PermissionDecisionKind::Approved
        || grant.action_id != decision.action_id
        || grant.turn_id != decision.turn_id
        || grant.version != decision.version
        || request.call_id.as_deref() != Some(grant.action_id.as_str())
    {
        return Err(denied());
    }
    let scope = sessions.agent_scope(session).map_err(browser_error)?;
    let mut input: Input = serde_json::from_value(request.input.clone())
        .map_err(|_| AgentError::invalid_input("invalid browser input"))?;
    let state = sessions.state(&scope).map_err(browser_error)?;
    if matches!(
        input.operation,
        BrowserOperation::Type { .. } | BrowserOperation::Navigate { .. }
    ) {
        let private = private.ok_or_else(|| {
            AgentError::new(
                "lilia.browser.private_expired",
                "private browser input expired; request the action again",
            )
        })?;
        if private.session != session
            || request.call_id.as_deref() != Some(private.call.as_str())
            || private.scope != scope
            || private.lifecycle != state.lifecycle
            || private.expected != request.input
        {
            return Err(denied());
        }
        input.operation = private.operation;
    }
    let redactions = private_strings(&input.operation);
    let observe = matches!(input.operation, BrowserOperation::Observe);
    let lifecycle = input
        .lifecycle
        .or(observe.then_some(state.lifecycle))
        .ok_or_else(|| AgentError::invalid_input("lifecycle is required"))?;
    let page_version = input
        .page_version
        .or(observe.then_some(state.page_version))
        .ok_or_else(|| AgentError::invalid_input("pageVersion is required"))?;
    let mut result = sessions
        .execute_agent(
            session,
            &grant.turn_id,
            &BrowserRequest {
                scope,
                lifecycle,
                page_version,
                operation: input.operation,
            },
        )
        .map_err(browser_error)?;
    result.page.url = safe_browser_url(&result.page.url);
    for text in redactions {
        result.page.url = result.page.url.replace(&text, "[private input]");
        result.page.title = result.page.title.replace(&text, "[private input]");
        for target in &mut result.page.targets {
            target.name = target.name.replace(&text, "[private input]");
        }
    }
    serde_json::to_value(result)
        .map_err(|_| AgentError::invalid_input("browser result could not be encoded"))
}

fn browser_error(error: crate::BrowserError) -> AgentError {
    AgentError::new(
        "lilia.browser",
        match error {
            crate::BrowserError::Host(_) => "browser host operation failed".into(),
            other => other.to_string(),
        },
    )
}

pub(crate) fn plugin(client: RuntimeClientRef, sessions: Arc<BrowserSessions>) -> PluginBuilder {
    let runner = mutsuki_agent_sdk::orchestration_runner("lilia.task-browser.runner", PLUGIN)
        .accepts::<BrowserProtocol>()
        .build();
    PluginBuilder::new(PLUGIN)
        .protocol::<BrowserProtocol>()
        .runner(Box::new(TaskAwaitRunnerAdapter::new(
            runner,
            client,
            Box::new(move |_context, task| {
                let sessions = sessions.clone();
                Box::pin(async move { run(&sessions, task) })
            }),
        )))
}

fn run(sessions: &BrowserSessions, task: Task) -> RuntimeResult<RunnerResult> {
    let request = serde_json::from_value(task.payload.clone().into()).map_err(|_| {
        mutsuki_agent_sdk::runtime_failure(
            PLUGIN,
            &task.task_id,
            AgentError::invalid_input("invalid browser execution request"),
        )
    })?;
    let output = execute(sessions, request)
        .map_err(|error| mutsuki_agent_sdk::runtime_failure(PLUGIN, &task.task_id, error))?;
    let mut result = RunnerResult::completed(task.task_id);
    result.output = Some(output);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lilia_contracts::{BrowserPage, BrowserScope, ProjectId, TaskId};
    use mutsuki_agent_contracts::{AgentToolApproval, PermissionDecision, PermissionRequest};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Host(AtomicUsize);
    impl crate::TaskBrowserHost for Host {
        fn execute(
            &self,
            _: &BrowserRequest,
            _: &crate::BrowserCancellation,
        ) -> Result<BrowserPage, crate::BrowserError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(page())
        }
        fn cancel(&self, _: &BrowserScope) {}
    }
    fn page() -> BrowserPage {
        BrowserPage {
            url: "about:blank".into(),
            title: String::new(),
            targets: vec![],
            screenshot_artifact: None,
            screenshot_bytes: None,
        }
    }

    #[test]
    fn approved_execution_uses_host_session_scope_and_rejected_approval_never_reaches_browser() {
        let host = Arc::new(Host(AtomicUsize::new(0)));
        let sessions = BrowserSessions::for_test(host.clone());
        let scope = BrowserScope {
            project_id: ProjectId::new("project").unwrap(),
            task_id: TaskId::new("task").unwrap(),
            tab_id: "tab".into(),
        };
        sessions.register(scope.clone(), page()).unwrap();
        sessions.bind_agent("session".into(), scope).unwrap();
        let mut request = AgentToolExecuteRequest {
            call_id: Some("action".into()),
            name: TOOL.into(),
            input: json!({"operation":{"kind":"observe"}}),
            session_id: Some("session".into()),
            context: None,
            approval: Some(AgentToolApproval {
                request: PermissionRequest {
                    session_id: "session".into(),
                    turn_id: "turn".into(),
                    action_id: "action".into(),
                    tool: TOOL.into(),
                    side_effect: ToolSideEffect::ExternalWrite,
                    summary: "browser".into(),
                    version: 1,
                },
                decision: PermissionDecision {
                    session_id: "session".into(),
                    turn_id: "turn".into(),
                    action_id: "action".into(),
                    version: 1,
                    decision: PermissionDecisionKind::Rejected,
                },
            }),
        };
        assert!(execute(&sessions, request.clone()).is_err());
        assert_eq!(host.0.load(Ordering::Relaxed), 0);
        request.approval.as_mut().unwrap().decision.decision = PermissionDecisionKind::Approved;
        request.approval.as_mut().unwrap().decision.version = 2;
        assert!(execute(&sessions, request.clone()).is_err());
        assert_eq!(host.0.load(Ordering::Relaxed), 0);
        request.approval.as_mut().unwrap().decision.version = 1;
        let result = execute(&sessions, request.clone()).unwrap();
        assert_eq!(result["scope"]["taskId"], "task");
        assert_eq!(host.0.load(Ordering::Relaxed), 1);
        sessions.cancel_agent("session", "turn");
        assert!(execute(&sessions, request).is_err());
        assert_eq!(host.0.load(Ordering::Relaxed), 1);
    }
    struct RecordingHost {
        calls: std::sync::Mutex<Vec<BrowserOperation>>,
        fail: std::sync::atomic::AtomicBool,
    }
    impl crate::TaskBrowserHost for RecordingHost {
        fn execute(
            &self,
            request: &BrowserRequest,
            _: &crate::BrowserCancellation,
        ) -> Result<BrowserPage, crate::BrowserError> {
            self.calls.lock().unwrap().push(request.operation.clone());
            if self.fail.load(Ordering::Relaxed) {
                return Err(crate::BrowserError::Host(
                    "credential-canary host failure".into(),
                ));
            }
            let mut page = page();
            page.url = "https://alice:credential-canary@example.org/form?token=credential-canary#credential-canary".into();
            page.title = "credential-canary".into();
            Ok(page)
        }
        fn cancel(&self, _: &BrowserScope) {}
    }
    fn private_setup() -> (Arc<RecordingHost>, BrowserSessions, BrowserScope) {
        let host = Arc::new(RecordingHost {
            calls: Default::default(),
            fail: false.into(),
        });
        let sessions = BrowserSessions::for_test(host.clone());
        let scope = BrowserScope {
            project_id: ProjectId::new("project").unwrap(),
            task_id: TaskId::new("task").unwrap(),
            tab_id: "tab".into(),
        };
        let mut page = page();
        page.url = "https://example.org/form".into();
        page.targets.push(lilia_contracts::BrowserTarget {
            id: "field".into(),
            role: "textbox".into(),
            name: "Message".into(),
        });
        sessions.register(scope.clone(), page).unwrap();
        sessions
            .bind_agent("session".into(), scope.clone())
            .unwrap();
        (host, sessions, scope)
    }
    fn sealed(
        sessions: &BrowserSessions,
        scope: &BrowserScope,
        operation: BrowserOperation,
    ) -> mutsuki_agent_contracts::ModelGenerateResult {
        let state = sessions.state(scope).unwrap();
        let input = json!({"lifecycle":state.lifecycle,"pageVersion":state.page_version,"operation":operation});
        let mut result = mutsuki_agent_contracts::ModelGenerateResult {
            message: mutsuki_agent_contracts::AgentMessage::assistant("credential-canary"),
            stop_reason: mutsuki_agent_contracts::AgentModelStopReason::ToolCalls,
            tool_calls: vec![mutsuki_agent_contracts::AgentToolCall {
                call_id: "action".into(),
                name: TOOL.into(),
                input: input.clone(),
            }],
            usage: Default::default(),
            cost_microunits: 0,
            raw: Some(input),
            output_resource: None,
        };
        protect_model_result(sessions, Some("session"), &mut result).unwrap();
        result
    }
    fn approved(input: Value) -> AgentToolExecuteRequest {
        AgentToolExecuteRequest {
            call_id: Some("action".into()),
            name: TOOL.into(),
            input,
            session_id: Some("session".into()),
            context: None,
            approval: Some(AgentToolApproval {
                request: PermissionRequest {
                    session_id: "session".into(),
                    turn_id: "turn".into(),
                    action_id: "action".into(),
                    tool: TOOL.into(),
                    side_effect: ToolSideEffect::ExternalWrite,
                    summary: "Operate example.org".into(),
                    version: 1,
                },
                decision: PermissionDecision {
                    session_id: "session".into(),
                    turn_id: "turn".into(),
                    action_id: "action".into(),
                    version: 1,
                    decision: PermissionDecisionKind::Approved,
                },
            }),
        }
    }

    #[test]
    fn protected_input_survives_approval_without_entering_serde_session_history() {
        use mutsuki_agent_contracts::{
            AgentEvent, AgentEventEnvelope, AgentEventMeta, AgentSession,
            AgentSessionAppendRequest, AgentSessionCreateRequest,
        };
        struct Persistence(std::sync::Mutex<Vec<Value>>);
        impl mutsuki_agent_runtime::SessionPersistence for Persistence {
            fn load(&self) -> Result<Vec<AgentSession>, AgentError> {
                Ok(vec![])
            }
            fn store(&self, session: &AgentSession) -> Result<(), AgentError> {
                self.0
                    .lock()
                    .unwrap()
                    .push(serde_json::to_value(session).unwrap());
                Ok(())
            }
        }
        let (host, sessions, scope) = private_setup();
        let result = sealed(
            &sessions,
            &scope,
            BrowserOperation::Type {
                target: "field".into(),
                text: "credential-canary".into(),
            },
        );
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("credential-canary"));
        let persistence = Arc::new(Persistence(Default::default()));
        let store =
            mutsuki_agent_runtime::SessionStore::with_persistence(persistence.clone()).unwrap();
        store
            .create(AgentSessionCreateRequest {
                session_id: Some("session".into()),
                profile_id: "profile".into(),
                title: None,
            })
            .unwrap();
        store
            .append(AgentSessionAppendRequest {
                session_id: "session".into(),
                messages: vec![result.message],
                events: vec![AgentEventEnvelope {
                    session_id: "session".into(),
                    sequence: 1,
                    meta: AgentEventMeta::new("tool-start", "browser input").with_turn("turn"),
                    event: AgentEvent::ToolCallStarted {
                        turn_id: "turn".into(),
                        call_id: "action".into(),
                        name: TOOL.into(),
                        input: result.tool_calls[0].input.clone(),
                    },
                }],
                advance_turn: false,
            })
            .unwrap();
        assert!(!serde_json::to_string(&*persistence.0.lock().unwrap())
            .unwrap()
            .contains("credential-canary"));
        let request = approved(result.tool_calls[0].input.clone());
        let output = execute(&sessions, request.clone()).unwrap();
        assert!(!output.to_string().contains("credential-canary"));
        assert_eq!(
            host.calls.lock().unwrap().as_slice(),
            &[BrowserOperation::Type {
                target: "field".into(),
                text: "credential-canary".into()
            }]
        );
        assert!(execute(&sessions, request).is_err());
        assert_eq!(host.calls.lock().unwrap().len(), 1);
        assert!(sessions.private_inputs.lock().unwrap().entries.is_empty());
    }

    #[test]
    fn private_inputs_reject_changed_targets_denial_expiry_and_changed_scope() {
        for scenario in [
            "target",
            "denied",
            "expired",
            "scope",
            "cancelled",
            "failed",
        ] {
            let (host, sessions, scope) = private_setup();
            let result = sealed(
                &sessions,
                &scope,
                BrowserOperation::Type {
                    target: "field".into(),
                    text: "credential-canary".into(),
                },
            );
            let mut request = approved(result.tool_calls[0].input.clone());
            match scenario {
                "target" => request.input["operation"]["target"] = json!("another-field"),
                "denied" => {
                    request.approval.as_mut().unwrap().decision.decision =
                        PermissionDecisionKind::Rejected
                }
                "expired" => {
                    for entry in sessions.private_inputs.lock().unwrap().entries.values_mut() {
                        entry.created = std::time::Instant::now()
                            - PRIVATE_INPUT_TTL
                            - std::time::Duration::from_secs(1);
                    }
                }
                "scope" => {
                    let mut other = scope.clone();
                    other.tab_id = "other-tab".into();
                    sessions.register(other.clone(), page()).unwrap();
                    sessions.bind_agent("session".into(), other).unwrap();
                }
                "cancelled" => sessions.cancel_agent("session", "turn"),
                "failed" => host.fail.store(true, Ordering::Relaxed),
                _ => unreachable!(),
            }
            let error = execute(&sessions, request).unwrap_err();
            assert!(!error.message.contains("credential-canary"));
            assert!(sessions.private_inputs.lock().unwrap().entries.is_empty());
            assert_eq!(
                host.calls.lock().unwrap().len(),
                usize::from(scenario == "failed")
            );
        }
    }

    #[test]
    fn navigation_credentials_are_private_and_pending_capacity_is_bounded() {
        let (host, sessions, scope) = private_setup();
        let url =
            "https://alice:password-canary@example.org/path?token=query-canary#fragment-canary";
        let mut result = sealed(
            &sessions,
            &scope,
            BrowserOperation::Navigate { url: url.into() },
        );
        let encoded = serde_json::to_string(&result).unwrap();
        for secret in ["password-canary", "query-canary", "fragment-canary"] {
            assert!(!encoded.contains(secret));
        }
        execute(&sessions, approved(result.tool_calls[0].input.clone())).unwrap();
        assert_eq!(
            host.calls.lock().unwrap().as_slice(),
            &[BrowserOperation::Navigate { url: url.into() }]
        );
        assert!(protect_model_result(&sessions, Some("session"), &mut result).is_err());
        for _ in 0..PRIVATE_INPUT_LIMIT {
            sealed(
                &sessions,
                &scope,
                BrowserOperation::Navigate {
                    url: "https://example.org/".into(),
                },
            );
        }
        assert_eq!(
            sessions.private_inputs.lock().unwrap().entries.len(),
            PRIVATE_INPUT_LIMIT
        );
        let state = sessions.state(&scope).unwrap();
        result.tool_calls[0].input = json!({"lifecycle":state.lifecycle,"pageVersion":state.page_version,"operation":{"kind":"navigate","url":"https://example.org/"}});
        assert!(protect_model_result(&sessions, Some("session"), &mut result).is_err());
        assert_eq!(
            sessions.private_inputs.lock().unwrap().entries.len(),
            PRIVATE_INPUT_LIMIT
        );
        sessions.close(&scope).unwrap();
        assert!(sessions.private_inputs.lock().unwrap().entries.is_empty());
    }
}
