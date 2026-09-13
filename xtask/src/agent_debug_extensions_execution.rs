use super::*;
use std::io::Read;
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;

const SERVER: &str = "native-ui-exec";
const HOOK: &str = "native-agentkit:user";
const TASK: &str = "native-agent-debug-task";
const MCP_RESULT: &str = "native-ui-mcp-result:native-ui-mcp-payload";

struct McpFixture {
    url: String,
    calls: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl McpFixture {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| XtaskError::io("mcp_fixture_bind", "bind local MCP", e))?;
        let url = format!(
            "http://{}/mcp",
            listener.local_addr().map_err(|e| XtaskError::io(
                "mcp_fixture_address",
                "read address",
                e
            ))?
        );
        listener
            .set_nonblocking(true)
            .map_err(|e| XtaskError::io("mcp_fixture_config", "configure listener", e))?;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let recorded = calls.clone();
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                        let request = (|| -> std::io::Result<Value> {
                            let mut reader = BufReader::new(&mut stream);
                            let mut length = 0;
                            let mut total = 0;
                            loop {
                                let mut line = String::new();
                                let n = reader.read_line(&mut line)?;
                                total += n;
                                if n == 0 || total > 65_536 {
                                    return Err(std::io::Error::other("invalid MCP request"));
                                }
                                if line == "\r\n" {
                                    break;
                                }
                                if let Some((name, value)) = line.split_once(':') {
                                    if name.eq_ignore_ascii_case("content-length") {
                                        length = value.trim().parse().unwrap_or(0);
                                    }
                                }
                            }
                            if length == 0 || length > 1_048_576 {
                                return Err(std::io::Error::other("invalid MCP body length"));
                            }
                            let mut body = vec![0; length];
                            reader.read_exact(&mut body)?;
                            serde_json::from_slice(&body).map_err(std::io::Error::other)
                        })();
                        let Ok(request) = request else {
                            let _ = write!(
                                stream,
                                "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            );
                            continue;
                        };
                        let method = request["method"].as_str().unwrap_or("");
                        if method == "tools/call" {
                            recorded.lock().unwrap().push(request["params"].clone());
                        }
                        let result = match method {
                            "initialize" => {
                                serde_json::json!({"protocolVersion":request["params"]["protocolVersion"],"capabilities":{"tools":{}},"serverInfo":{"name":"Native UI MCP Fixture","version":"1"}})
                            }
                            "tools/list" => {
                                serde_json::json!({"tools":[{"name":"echo","description":"Echo an acceptance marker without external effects","inputSchema":{"type":"object","properties":{"message":{"type":"string"}},"required":["message"]},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}]})
                            }
                            "resources/list" => serde_json::json!({"resources":[]}),
                            "prompts/list" => serde_json::json!({"prompts":[]}),
                            "tools/call" => {
                                serde_json::json!({"content":[{"type":"text","text":format!("native-ui-mcp-result:{}",request["params"]["arguments"]["message"].as_str().unwrap_or(""))}],"isError":false})
                            }
                            _ => Value::Null,
                        };
                        if request["id"].is_null() {
                            let _ = write!(
                                stream,
                                "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            );
                        } else {
                            let body=serde_json::json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string();
                            let _ = write!(
                                stream,
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            url,
            calls,
            stop,
            worker: Some(worker),
        })
    }
    fn calls(&self) -> Vec<Value> {
        self.calls.lock().unwrap().clone()
    }
}
impl Drop for McpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn settings(session: &Session, tab: &str) -> Result {
    let state = session.request(&serde_json::json!({"command":"observe"}))?;
    if state["observation"]["page"] != "settings" {
        let ui = wait_ui(session)?;
        let id = ui["snapshot"]["targets"]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .filter_map(|r| r["id"].as_str())
                    .find(|id| id.starts_with("lilia.ui.navigation.") && id.contains("settings"))
            })
            .ok_or_else(|| {
                XtaskError::failure(
                    "extension_settings_missing",
                    "normal settings navigation not exposed",
                )
            })?;
        ui_click(session, id)?;
    }
    extension_page(session, tab)
}
fn conversation(session: &Session) -> Result {
    ui_click(session, "lilia.ui.settings.back")?;
    let row = format!("lilia.ui.sidebar.row.{TASK}");
    wait_target(session, &row)?;
    ui_click(session, &row)?;
    wait_target(session, "lilia.task-session.composer.input")?;
    select_replay_model(session)
}
fn select_hook(session: &Session) -> Result {
    settings(session, "plugin-hooks")?;
    let id = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.settings.extensions-entry-hook:native-agentkit:user:")
    })?;
    ui_click(session, &id)
}
fn select_mcp(session: &Session) -> Result {
    settings(session, "plugin-mcp")?;
    let id = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.settings.extensions-entry-mcp:") && id.ends_with(":native-ui-exec")
    })?;
    ui_click(session, &id)
}
fn send(session: &Session, marker: &str, wait_for_completion: bool) -> Result<(Value, Value)> {
    let fixture = session.model_fixture.as_ref().expect("model fixture");
    let after = fixture.requests().len();
    fixture.hold_responses(true);
    let result: Result<(Value, Value)> = (|| {
        ui_input(session, "lilia.task-session.composer.input", marker)?;
        wait_observation(session, |s| {
            s["observation"]["composerContentSha256"] == composer_digest(marker)
        })?;
        require_ok(&session.request(&serde_json::json!({"command":"ui-key","targetId":"lilia.task-session.composer.input","key":"Enter"}))?,"send extension execution turn")?;
        let request = wait_model_request(session, after, |r| {
            r["messages"].as_array().is_some_and(|messages| {
                messages
                    .iter()
                    .rev()
                    .find(|m| m["role"] == "user")
                    .is_some_and(|m| m["content"].as_str().is_some_and(|c| c.contains(marker)))
            })
        })?;
        let running = wait_observation(session, |s| s["observation"]["activeTurnId"].is_string())?;
        Ok((request, running))
    })();
    fixture.hold_responses(false);
    let result = result?;
    if wait_for_completion {
        wait_completed_turn(session)?;
    }
    Ok(result)
}

pub(super) fn replay(session: &Session) -> Result {
    let mcp = McpFixture::start()?;
    let home = session.run_dir.join("home");
    let marker = home.join("native-hook-executions.jsonl");
    #[cfg(windows)]
    let command = windows_hook_append_command(&marker);
    #[cfg(not(windows))]
    let command = {
        let quoted = format!("'{}'", marker.to_string_lossy().replace('\'', "'\"'\"'"));
        format!("/bin/cat >> {quoted}; /usr/bin/printf '\\n' >> {quoted}")
    };
    select_hook(session)?;
    ui_click(session, &format!("lilia.ui.settings.hook-edit-{HOOK}"))?;
    extension_editor_input(session, &format!("hook-{HOOK}-0-command"), &command)?;
    extension_editor_input(
        session,
        &format!("hook-{HOOK}-0-matcher"),
        "Native Hook 执行验收",
    )?;
    ui_click(session, &format!("lilia.ui.settings.hook-save-{HOOK}"))?;
    wait_extension_dialog_closed(session)?;
    ui_click(session, &format!("lilia.ui.settings.hook-toggle-{HOOK}"))?;
    let hooks = home.join("config/agentkit-hooks.json");
    wait_extension_file(&hooks, |v| v["enabled"] == true)?;
    conversation(session)?;
    let (hook_request, hook_running) = send(session, "Native Hook 执行验收", true)?;
    let bytes = fs::read(&marker)
        .map_err(|e| XtaskError::io("hook_marker_missing", "read executed Hook marker", e))?;
    let records = String::from_utf8_lossy(&bytes)
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| XtaskError::failure("hook_payload_invalid", e.to_string()))?;
    if records.len() != 1
        || records[0]["taskId"] != TASK
        || records[0]["turnId"] != hook_running["observation"]["activeTurnId"]
        || records[0]["context"] != "Native Hook 执行验收"
    {
        return Err(XtaskError::failure(
            "hook_execution_mismatch",
            "normal turn did not execute the saved Hook exactly once with its task/turn/prompt",
        ));
    }
    select_hook(session)?;
    ui_click(session, &format!("lilia.ui.settings.hook-toggle-{HOOK}"))?;
    wait_extension_file(&hooks, |v| v["enabled"] == false)?;
    conversation(session)?;
    send(session, "Native Hook 执行验收", true)?;
    if fs::read(&marker).map_err(|e| XtaskError::io("hook_marker_read", "read marker", e))? != bytes
    {
        return Err(XtaskError::failure(
            "disabled_hook_executed",
            "disabled Hook still ran on a matching ordinary prompt",
        ));
    }
    settings(session, "plugin-mcp")?;
    ui_click(session, "lilia.ui.settings.mcp-new")?;
    extension_editor_input(session, "mcp-id", SERVER)?;
    extension_editor_input(session, "mcp-location", &mcp.url)?;
    let transport = "lilia.ui.settings.mcp-transport";
    reveal_scrolled_control(
        session,
        "lilia.ui.settings.extensions-editor-scroll",
        transport,
    )?;
    ui_click(session, transport)?;
    for key in ["ArrowDown", "Enter"] {
        require_ok(
            &session
                .request(&serde_json::json!({"command":"ui-key","targetId":transport,"key":key}))?,
            "select MCP HTTP transport",
        )?;
    }
    ui_click(session, "lilia.ui.settings.mcp-save")?;
    wait_extension_dialog_closed(session)?;
    let registry = home.join("config/agentkit-mcp-registry.json");
    wait_extension_file(&registry, |v| {
        v["servers"].as_array().is_some_and(|s| {
            s.iter()
                .any(|s| s["serverId"] == SERVER && s["enabled"] == true)
        })
    })?;
    wait_observation(session, |s| {
        s["observation"]["extensionsBusy"] == false
            && s["observation"]["extensionsMcpToolCount"]
                .as_u64()
                .unwrap_or(0)
                > 0
    })?;
    session.capture(&session.run_dir.join("extensions-mcp-runtime-ready.png"))?;
    conversation(session)?;
    let after = session.model_fixture.as_ref().unwrap().requests().len();
    let (mcp_request, _) = send(session, "Native MCP 执行验收", false)?;
    let approve = wait_matching_target(session, |id| {
        id.starts_with("lilia.ui.pending.pending-approve-")
    })?;
    if !mcp.calls().is_empty() {
        return Err(XtaskError::failure(
            "mcp_executed_before_approval",
            "MCP ran before normal user approval",
        ));
    }
    session.capture(&session.run_dir.join("extensions-mcp-approval.png"))?;
    ui_click(session, &approve)?;
    wait_completed_turn(session)?;
    let resumed = wait_model_request(session, after, |r| {
        r["messages"].as_array().is_some_and(|messages| {
            messages.iter().any(|m| {
                m["role"] == "tool"
                    && m["tool_call_id"] == "native-mcp-execution-call"
                    && m["content"].to_string().contains(MCP_RESULT)
            })
        })
    })?;
    let calls = mcp.calls();
    if calls.len() != 1
        || calls[0]["name"] != "echo"
        || calls[0]["arguments"]["message"] != "native-ui-mcp-payload"
    {
        return Err(XtaskError::failure(
            "mcp_execution_mismatch",
            "Agent did not call the normally configured local MCP exactly once",
        ));
    }
    write_json(
        &session.run_dir.join("extensions-mcp-executed.json"),
        &serde_json::json!({"hook":records,"hookRequest":hook_request,"hookDisabledNoExecution":true,
            "mcpRequest":mcp_request,"mcpCalls":calls,"resumedRequest":resumed,
            "completion":session.request(&serde_json::json!({"command":"observe"}))?}),
    )?;
    select_mcp(session)?;
    ui_click(session, &format!("lilia.ui.settings.mcp-toggle-{SERVER}"))?;
    wait_extension_file(&registry, |v| {
        v["servers"].as_array().is_some_and(|s| {
            s.iter()
                .any(|s| s["serverId"] == SERVER && s["enabled"] == false)
        })
    })?;
    wait_observation(session, |s| {
        s["observation"]["extensionsBusy"] == false
            && s["observation"]["extensionsMcpToolCount"] == 0
    })?;
    conversation(session)?;
    let (disabled, _) = send(session, "Native MCP 停用验收", true)?;
    if disabled["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .any(|name| name.contains("native") && name.contains("exec") && name.ends_with("echo"))
    }) || mcp.calls().len() != calls.len()
    {
        return Err(XtaskError::failure(
            "disabled_mcp_still_available",
            "disabled MCP remained in the next Agent tool catalog or executed",
        ));
    }
    write_json(
        &session.run_dir.join("extensions-runtime-verified.json"),
        &serde_json::json!({"hook":records,"hookRequest":hook_request,"hookDisabledNoExecution":true,"mcpRequest":mcp_request,"mcpCalls":calls,"resumedRequest":resumed,"disabledRequest":disabled,"mcpDisabledNoExecution":true}),
    )
}

#[cfg(windows)]
fn windows_hook_append_command(path: &Path) -> String {
    let escaped = path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$in=[Console]::OpenStandardInput();$fs=[IO.File]::Open('{escaped}',[IO.FileMode]::Append,[IO.FileAccess]::Write,[IO.FileShare]::Read);$b=New-Object byte[] 65536;do{{$n=$in.Read($b,0,$b.Length);if($n -gt 0){{$fs.Write($b,0,$n)}}}}while($n -gt 0);$fs.Write([byte[]](10),0,1);$fs.Dispose()"
    );
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!(
        "powershell -NoProfile -EncodedCommand {}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, utf16)
    )
}
