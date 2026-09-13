use super::*;
use std::collections::BTreeMap;
use std::io::Read;
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

const INPUT: &str = "Native browser approval verified";
const PREFIX: &str = "browser-agent-";
const ACTIONS: [&str; 5] = ["observe", "type", "click", "scroll", "screenshot"];

#[derive(Default)]
struct Evidence {
    results: BTreeMap<String, Value>,
    page_report: Value,
    finished: bool,
    error: Option<String>,
    model_requests: usize,
}

struct ModelFixture {
    origin: String,
    state: Arc<Mutex<Evidence>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl ModelFixture {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| XtaskError::io("browser_model_bind", "bind local model", e))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| XtaskError::io("browser_model_socket", "configure local model", e))?;
        let origin = format!(
            "http://{}",
            listener.local_addr().map_err(|e| XtaskError::io(
                "browser_model_address",
                "read model address",
                e
            ))?
        );
        let state = Arc::new(Mutex::new(Evidence::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = state.clone();
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(900);
            let mut requests = 0;
            while !stopping.load(Ordering::Acquire) && Instant::now() < deadline && requests < 256 {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                requests += 1;
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                let Ok((path, body)) = read_http(&mut stream) else {
                    continue;
                };
                let response: std::result::Result<(&str, String), &str> = (|| {
                    if path == "/page" {
                        return Ok(("text/html", page_html()));
                    }
                    if path == "/report" {
                        let report: Value =
                            serde_json::from_slice(&body).map_err(|_| "invalid page report")?;
                        shared.lock().unwrap().page_report = report;
                        return Ok(("application/json", "{}".into()));
                    }
                    if path == "/v1/chat/completions" {
                        shared.lock().unwrap().model_requests += 1;
                        let request: Value =
                            serde_json::from_slice(&body).map_err(|_| "invalid model request")?;
                        let reply = model_reply(&request, &mut shared.lock().unwrap())?;
                        return Ok(("application/json", reply.to_string()));
                    }
                    Ok(("text/plain", "local fixture".into()))
                })();
                let (status, content_type, body) = match response {
                    Ok((kind, body)) => ("200 OK", kind, body),
                    Err(error) => {
                        shared.lock().unwrap().error = Some(error.into());
                        (
                            "500 Internal Server Error",
                            "application/json",
                            "{\"error\":\"local browser fixture rejected request\"}".into(),
                        )
                    }
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Ok(Self {
            origin,
            state,
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for ModelFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn read_http(stream: &mut TcpStream) -> std::result::Result<(String, Vec<u8>), &'static str> {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    Read::by_ref(&mut reader)
        .take(16385)
        .read_line(&mut first)
        .map_err(|_| "HTTP request timeout")?;
    let path = first
        .split_whitespace()
        .nth(1)
        .ok_or("missing HTTP path")?
        .to_owned();
    reader
        .get_ref()
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| "configure HTTP timeout")?;
    let mut size = 0usize;
    let mut headers = first.len();
    loop {
        let mut line = String::new();
        if Read::by_ref(&mut reader)
            .take(16385)
            .read_line(&mut line)
            .map_err(|_| "HTTP headers timeout")?
            == 0
        {
            return Err("incomplete HTTP headers");
        }
        headers += line.len();
        if headers > 16384 {
            return Err("HTTP headers exceed fixture bound");
        }
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                size = value.trim().parse().map_err(|_| "invalid content length")?;
            }
        }
    }
    if size > 4 * 1024 * 1024 {
        return Err("model request exceeds fixture bound");
    }
    let mut bytes = vec![0; size];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| "incomplete HTTP body")?;
    Ok((path, bytes))
}

fn page_html() -> String {
    r#"<!doctype html><html><meta charset="utf-8"><title>Native Agent browser acceptance</title><style>body{font:20px system-ui;margin:32px;background:#f6f8fb;color:#182030;min-height:2400px}header{position:sticky;top:0;background:#f6f8fb;padding:12px}input,button{font:inherit;padding:8px;margin:8px}</style><header><h1>Native Agent browser acceptance</h1><label>Message <input id="message" aria-label="Message"></label><button id="apply">Apply message</button><p id="result">Waiting for approved input</p></header><p>Scroll validation area</p><script>const input=document.getElementById('message'),result=document.getElementById('result');function report(){fetch('/report',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({input:input.value,result:result.textContent,scrollY:window.scrollY})})}document.getElementById('apply').onclick=()=>{result.textContent=input.value;report()};input.addEventListener('input',report);window.addEventListener('scroll',report);window.addEventListener('load',report);</script></html>"#.into()
}

fn browser_state(value: &Value) -> Option<Value> {
    if value.get("pageVersion").is_some()
        && value.get("scope").is_some()
        && value.get("page").is_some()
    {
        return Some(value.clone());
    }
    match value {
        Value::Object(map) => map.values().find_map(browser_state),
        Value::Array(values) => values.iter().find_map(browser_state),
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|v| browser_state(&v)),
        _ => None,
    }
}

fn model_reply(
    request: &Value,
    evidence: &mut Evidence,
) -> std::result::Result<Value, &'static str> {
    let messages = request["messages"]
        .as_array()
        .ok_or("missing model messages")?;
    let prompt = messages
        .iter()
        .rev()
        .find(|m| m["role"] == "user")
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    if serde_json::from_str::<Value>(prompt)
        .ok()
        .and_then(|value| value["promptPreview"].as_str().map(str::to_owned))
        .is_some_and(|preview| preview.contains("browser-agent-"))
    {
        return Ok(text_reply(serde_json::json!({"tier":"normal","reasoningEffort":"low","planMode":false,"goalMode":false,"sessionFork":false,"summary":"Local browser acceptance","signals":["local fixture"]}).to_string()));
    }
    let mode = if prompt.contains("browser-agent-deny") {
        "deny"
    } else if prompt.contains("browser-agent-cancel") {
        "cancel"
    } else if prompt.contains("browser-agent-run") {
        "run"
    } else {
        return Err("unexpected model turn");
    };
    if !request["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| {
            tool.pointer("/function/name").and_then(Value::as_str) == Some("task_browser")
        })
    }) {
        return Err("the real model tool registry did not advertise task_browser");
    }
    for message in messages.iter().filter(|m| m["role"] == "tool") {
        let Some(id) = message["tool_call_id"]
            .as_str()
            .filter(|id| id.starts_with(PREFIX))
        else {
            continue;
        };
        let content = &message["content"];
        if id.starts_with("browser-agent-run-") {
            let state = browser_state(content)
                .ok_or("browser tool did not return authoritative page state")?;
            evidence.results.insert(id.to_owned(), state);
        } else {
            evidence
                .results
                .insert(id.to_owned(), serde_json::json!({"rejected":true}));
        }
    }
    let index = if mode == "run" {
        (0..5)
            .take_while(|n| evidence.results.contains_key(&format!("{PREFIX}run-{n}")))
            .count()
    } else {
        usize::from(evidence.results.contains_key(&format!("{PREFIX}{mode}-0")))
    };
    let done = if mode == "run" { index == 5 } else { index > 0 };
    if done {
        if mode == "run" {
            evidence.finished = true;
        }
        return Ok(text_reply("Native browser acceptance completed.".into()));
    }
    let operation = match index {
        0 => serde_json::json!({"kind":"observe"}),
        1 | 2 => {
            let previous = &evidence.results[&format!("{PREFIX}run-{}", index - 1)];
            let name = if index == 1 {
                "Message"
            } else {
                "Apply message"
            };
            let role = if index == 1 { "textbox" } else { "button" };
            let target = previous
                .pointer("/page/targets")
                .and_then(Value::as_array)
                .and_then(|targets| {
                    targets
                        .iter()
                        .find(|target| target["name"] == name && target["role"] == role)
                })
                .and_then(|target| target["id"].as_str())
                .ok_or("live browser target missing")?;
            if index == 1 {
                serde_json::json!({"kind":"type","target":target,"text":INPUT})
            } else {
                serde_json::json!({"kind":"click","target":target})
            }
        }
        3 => serde_json::json!({"kind":"scroll","x":0,"y":480}),
        4 => serde_json::json!({"kind":"screenshot"}),
        _ => return Err("fixture step out of bounds"),
    };
    let mut input = serde_json::json!({"operation":operation});
    if index > 0 {
        let previous = &evidence.results[&format!("{PREFIX}run-{}", index - 1)];
        input["lifecycle"] = previous["lifecycle"].clone();
        input["pageVersion"] = previous["pageVersion"].clone();
    }
    Ok(
        serde_json::json!({"id":format!("fixture-{mode}-{index}"),"object":"chat.completion","choices":[{"index":0,"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[{"id":format!("{PREFIX}{mode}-{index}"),"type":"function","function":{"name":"task_browser","arguments":input.to_string()}}]}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
    )
}

fn text_reply(content: String) -> Value {
    serde_json::json!({"id":"fixture-complete","object":"chat.completion","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":content}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})
}

fn click(session: &Session, target: &str) -> Result<()> {
    require_ok(
        &session.request(&serde_json::json!({"command":"click","targetId":target}))?,
        "browser Agent click",
    )
}

fn visible(state: &Value) -> impl Iterator<Item = &str> {
    state
        .pointer("/observation/visibleTargetIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

fn send(session: &Session, prompt: &str) -> Result<()> {
    require_ok(&session.request(&serde_json::json!({"command":"input","targetId":"lilia.task-session.composer.input","text":prompt}))?, "enter browser acceptance task")?;
    click(session, "lilia.task-session.composer.send")
}

fn reject_and_cancel(session: &Session) -> Result<()> {
    wait_browser(session, "agent-idle", |state| {
        !visible(state).any(|id| id == "lilia.task-session.composer.interrupt")
    })?;
    for (mode, action) in [("deny", "deny"), ("cancel", "interrupt")] {
        let requested_target = format!("lilia.task-session.approval.{PREFIX}{mode}-0.deny");
        let before = session.request(&serde_json::json!({"command":"observe"}))?;
        send(
            session,
            &format!("browser-agent-{mode}: request a browser observation"),
        )?;
        let pending = wait_browser(session, &format!("agent-{mode}-pending"), |state| {
            visible(state).any(|id| id == requested_target)
        })?;
        record_browser(session, &format!("agent-{mode}-pending"), &pending)?;
        let target = if action == "deny" {
            requested_target
        } else {
            visible(&pending)
                .find(|id| *id == "lilia.task-session.composer.interrupt")
                .ok_or_else(|| {
                    XtaskError::failure(
                        "browser_agent_cancel_missing",
                        "pending browser request has no actual interrupt action",
                    )
                })?
                .to_owned()
        };
        click(session, &target)?;
        let stopped = wait_browser(session, &format!("agent-{mode}-finished"), |state| {
            !visible(state).any(|id| {
                id.starts_with("lilia.task-session.approval.")
                    || id == "lilia.task-session.composer.interrupt"
            })
        })?;
        let before_tabs = before
            .pointer("/observation/browserStates")
            .and_then(Value::as_array);
        let after_tabs = stopped
            .pointer("/observation/browserStates")
            .and_then(Value::as_array);
        let unchanged = before_tabs.zip(after_tabs).is_some_and(|(before, after)| {
            before.len() == after.len()
                && before.iter().zip(after).all(|(left, right)| {
                    left["scope"] == right["scope"]
                        && left["url"] == right["url"]
                        && (mode == "cancel" || left["pageVersion"] == right["pageVersion"])
                })
        });
        if !unchanged {
            return Err(XtaskError::failure(
                "browser_agent_rejected_action_executed",
                "denied or cancelled approval changed the browser page",
            ));
        }
        record_browser(session, &format!("agent-{mode}-finished"), &stopped)?;
    }
    Ok(())
}

pub(super) fn run(reuse_binary: bool) -> Result<Value> {
    let fixture = ModelFixture::start()?;
    let session = Session::start_with_endpoint(
        "browser-agent",
        &format!("{}/v1/chat/completions", fixture.origin),
        reuse_binary,
    )?;
    for target in [
        "lilia.sidebar.project.native-agent-debug-project",
        "lilia.sidebar.task.native-agent-debug-task",
    ] {
        click(&session, target)?;
    }
    click(&session, "lilia.settings.open")?;
    click(&session, "lilia.settings.provider")?;
    let endpoint = format!("{}/v1/chat/completions", fixture.origin);
    require_ok(&session.request(&serde_json::json!({"command":"input","targetId":"lilia.settings.provider.runtime.openai-endpoint","text":endpoint}))?, "configure actual local model endpoint")?;
    click(&session, "lilia.settings.provider.runtime.save")?;
    let saved = wait_browser(&session, "agent-provider", |state| {
        state
            .pointer("/observation/providerOpenAiEndpoint")
            .and_then(Value::as_str)
            == Some(endpoint.as_str())
            && state.pointer("/observation/providerRuntimeDirty") == Some(&Value::Bool(false))
    })?;
    write_json(&session.run_dir.join("browser-agent-provider.json"), &saved)?;
    click(&session, "lilia.settings.back")?;
    super::capture_resized_window(
        session.pid(),
        &session
            .run_dir
            .join("browser-agent-conversation-narrow.png"),
        [780, 760],
    )?;
    write_json(
        &session
            .run_dir
            .join("browser-agent-conversation-narrow.json"),
        &session.request(&serde_json::json!({"command":"observe"}))?,
    )?;
    click(&session, "lilia.task-session.iab.open")?;
    let ready = wait_browser(&session, "agent-ready", |state| {
        browser_target(state, ".address").is_some()
            && state.pointer("/observation/iabBrowserReady") == Some(&Value::Bool(true))
    })?;
    record_browser(&session, "agent-ready-narrow", &ready)?;
    let url = format!("{}/page", fixture.origin);
    require_ok(&session.request(&serde_json::json!({"command":"input","targetId":browser_target(&ready,".address").unwrap(),"text":url}))?, "navigate Agent fixture")?;
    click(
        &session,
        &browser_target(&ready, ".open").ok_or_else(|| {
            XtaskError::failure("browser_agent_navigation", "navigation target missing")
        })?,
    )?;
    let loaded = wait_browser(&session, "agent-page", |state| {
        state.pointer("/observation/iabUrl").and_then(Value::as_str) == Some(url.as_str())
            && state
                .pointer("/observation/browserStates")
                .and_then(Value::as_array)
                .is_some_and(|tabs| {
                    tabs.iter()
                        .any(|tab| tab["url"] == url && tab["control"] == "human")
                })
    })?;
    record_browser(&session, "agent-page-narrow", &loaded)?;
    super::capture_resized_window(
        session.pid(),
        &session.run_dir.join("browser-agent-wide-restored.png"),
        [1180, 760],
    )?;
    write_json(
        &session.run_dir.join("browser-agent-wide-restored.json"),
        &session.request(&serde_json::json!({"command":"observe"}))?,
    )?;
    let resume = browser_target(&loaded, ".control").ok_or_else(|| {
        XtaskError::failure(
            "browser_agent_resume",
            "explicit Agent resume target missing",
        )
    })?;
    click(&session, &resume)?;
    let active = wait_browser(&session, "agent-resumed", |state| {
        state
            .pointer("/observation/selectedTask")
            .and_then(Value::as_str)
            == Some("native-agent-debug-task")
            && state
                .pointer("/observation/browserStates")
                .and_then(Value::as_array)
                .is_some_and(|tabs| {
                    tabs.iter().any(|tab| {
                        tab["control"] == "agent"
                            && tab["url"] == url
                            && tab.pointer("/scope/taskId").and_then(Value::as_str)
                                == Some("native-agent-debug-task")
                    })
                })
    })?;
    record_browser(&session, "agent-before", &active)?;
    send(
        &session,
        "browser-agent-run: validate the local task browser",
    )?;
    let deadline = Instant::now() + Duration::from_secs(100);
    let mut approvals = std::collections::BTreeSet::new();
    loop {
        let state = session.request(&serde_json::json!({"command":"observe"}))?;
        require_ok(&state, "observe browser Agent execution")?;
        if let Some(target) = visible(&state).find(|id| {
            id.starts_with("lilia.task-session.approval.")
                && id.ends_with(".approve")
                && !approvals.contains(*id)
        }) {
            record_browser(
                &session,
                &format!("agent-approval-{}", approvals.len()),
                &state,
            )?;
            click(&session, target)?;
            approvals.insert(target.to_owned());
        }
        let evidence = fixture.state.lock().unwrap();
        if let Some(error) = &evidence.error {
            let error = error.clone();
            drop(evidence);
            record_browser(&session, "agent-failed", &state)?;
            write_json(
                &session.run_dir.join("browser-agent-errors.json"),
                &session.request(&serde_json::json!({"command":"recent-errors"}))?,
            )?;
            return Err(XtaskError::failure("browser_agent_fixture", error));
        }
        if evidence.finished {
            break;
        }
        if Instant::now() >= deadline {
            write_json(&session.run_dir.join("browser-agent-timeout.json"), &state)?;
            write_json(
                &session.run_dir.join("browser-agent-http.json"),
                &serde_json::json!({"modelRequests":evidence.model_requests,"results":evidence.results.len()}),
            )?;
            return Err(XtaskError::failure(
                "browser_agent_timeout",
                "real Agent browser turn did not complete all five approved operations",
            ));
        }
        drop(evidence);
        thread::sleep(Duration::from_millis(100));
    }
    let evidence = fixture.state.lock().unwrap();
    if approvals.len() != ACTIONS.len()
        || evidence.page_report["input"] != INPUT
        || evidence.page_report["result"] != INPUT
        || evidence.page_report["scrollY"].as_f64().unwrap_or(0.0) <= 0.0
    {
        return Err(XtaskError::failure(
            "browser_agent_behavior",
            "approved tools did not produce the expected real input, click and scroll",
        ));
    }
    let artifact = evidence.results["browser-agent-run-4"]
        .pointer("/page/screenshotArtifact")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            XtaskError::failure(
                "browser_agent_screenshot",
                "real screenshot tool returned no artifact",
            )
        })?;
    let path = fs::canonicalize(artifact)
        .map_err(|e| XtaskError::io("browser_agent_screenshot", "open screenshot artifact", e))?;
    let home = fs::canonicalize(session.run_dir.join("home"))
        .map_err(|e| XtaskError::io("browser_agent_home", "resolve isolated home", e))?;
    if !path.starts_with(home) {
        return Err(XtaskError::failure(
            "browser_agent_screenshot_scope",
            "screenshot is outside isolated run home",
        ));
    }
    let bytes = fs::read(path)
        .map_err(|e| XtaskError::io("browser_agent_screenshot", "read screenshot", e))?;
    if bytes.len() < 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(XtaskError::failure(
            "browser_agent_screenshot",
            "screenshot artifact is not a rendered PNG",
        ));
    }
    fs::write(session.run_dir.join("browser-agent-page.png"), bytes)
        .map_err(|e| XtaskError::io("browser_agent_screenshot", "save screenshot evidence", e))?;
    write_json(
        &session.run_dir.join("browser-agent-page-state.json"),
        &evidence.page_report,
    )?;
    let mut tool_results = serde_json::to_value(&evidence.results)
        .map_err(|e| XtaskError::failure("browser_agent_evidence", e.to_string()))?;
    tool_results["browser-agent-run-4"]["page"]["screenshotArtifact"] =
        Value::String("browser-agent-page.png".into());
    write_json(
        &session.run_dir.join("browser-agent-tool-results.json"),
        &tool_results,
    )?;
    drop(evidence);
    record_browser(
        &session,
        "agent-completed",
        &session.request(&serde_json::json!({"command":"observe"}))?,
    )?;
    reject_and_cancel(&session)?;
    let final_page = fixture.state.lock().unwrap().page_report.clone();
    if final_page["input"] != INPUT || final_page["result"] != INPUT {
        return Err(XtaskError::failure(
            "browser_agent_cancel_changed_page",
            "rejected requests changed the fixture page",
        ));
    }
    for entry in fs::read_dir(&session.run_dir)
        .map_err(|e| XtaskError::io("browser_agent_artifacts", "read acceptance artifacts", e))?
    {
        let entry = entry
            .map_err(|e| XtaskError::io("browser_agent_artifacts", "read artifact entry", e))?;
        let path = entry.path();
        if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("json" | "jsonl" | "log")
        ) {
            let text = fs::read_to_string(&path).map_err(|e| {
                XtaskError::io("browser_agent_artifact", "read acceptance artifact", e)
            })?;
            if text.contains("sk-native-agent-debug-fixture") {
                return Err(XtaskError::failure(
                    "browser_agent_secret_leak",
                    "browser Agent acceptance artifacts contain the credential canary",
                ));
            }
        }
    }
    let result = serde_json::json!({"passed":true,"actions":ACTIONS,"approvals":approvals.len(),"deniedPendingAction":true,"cancelledPendingAction":true,"artifacts":session.run_dir,"notCovered":["cancel during active CDP operation","upload/download completion","remote model quality"]});
    write_json(
        &session.run_dir.join("browser-agent-acceptance.json"),
        &result,
    )?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_fixture_uses_returned_identity_and_requires_successful_tool_results() {
        let mut evidence = Evidence::default();
        let mut request = serde_json::json!({"messages":[{"role":"user","content":"browser-agent-run"}],"tools":[{"type":"function","function":{"name":"task_browser"}}]});
        let reply = model_reply(&request, &mut evidence).unwrap();
        let input: Value = serde_json::from_str(
            reply
                .pointer("/choices/0/message/tool_calls/0/function/arguments")
                .unwrap()
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(input["operation"]["kind"], "observe");
        let state = serde_json::json!({"scope":{"tabId":"actual-tab"},"lifecycle":37,"pageVersion":64,"page":{"targets":[{"id":"label","role":"StaticText","name":"Message"},{"id":"live-textbox-92","role":"textbox","name":"Message"}]}});
        request["messages"].as_array_mut().unwrap().push(serde_json::json!({"role":"tool","tool_call_id":"browser-agent-run-0","content":state.to_string()}));
        let reply = model_reply(&request, &mut evidence).unwrap();
        let input: Value = serde_json::from_str(
            reply
                .pointer("/choices/0/message/tool_calls/0/function/arguments")
                .unwrap()
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(input["operation"]["target"], "live-textbox-92");
        assert_eq!(input["lifecycle"], 37);
        assert_eq!(input["pageVersion"], 64);
        request["messages"][1]["content"] = Value::String("{\"error\":\"cancelled\"}".into());
        assert!(model_reply(&request, &mut evidence).is_err());
    }

    #[test]
    fn fixture_serves_real_http_model_and_page_report_without_remote_credentials() {
        let fixture = ModelFixture::start().unwrap();
        let _idle_preconnect =
            TcpStream::connect(fixture.origin.trim_start_matches("http://")).unwrap();
        let mut stream = TcpStream::connect(fixture.origin.trim_start_matches("http://")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        thread::sleep(Duration::from_millis(100));
        let body = serde_json::json!({"messages":[{"role":"user","content":"browser-agent-run"}],"tools":[{"type":"function","function":{"name":"task_browser"}}]})
            .to_string();
        write!(stream,"POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let payload: Value =
            serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(
            payload.pointer("/choices/0/message/tool_calls/0/function/name"),
            Some(&Value::String("task_browser".into()))
        );
        let mut stream = TcpStream::connect(fixture.origin.trim_start_matches("http://")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let body = serde_json::json!({"input":INPUT,"result":INPUT,"scrollY":480}).to_string();
        write!(stream,"POST /report HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        stream.read_to_end(&mut Vec::new()).unwrap();
        assert_eq!(fixture.state.lock().unwrap().page_report["scrollY"], 480);
    }
}
