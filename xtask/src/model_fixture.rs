use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{json, Value};

use crate::{Result, XtaskError};

/// A loopback-only provider boundary for the real desktop request pipeline.
/// Only JSON bodies are retained; authentication headers never enter artifacts.
pub(crate) struct ModelFixture {
    endpoint: String,
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    held: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ModelFixture {
    pub fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| XtaskError::io("model_fixture_bind_failed", "bind fixture", error))?;
        let address = listener.local_addr().map_err(|error| {
            XtaskError::io(
                "model_fixture_address_failed",
                "read fixture address",
                error,
            )
        })?;
        listener.set_nonblocking(true).map_err(|error| {
            XtaskError::io(
                "model_fixture_nonblocking_failed",
                "configure fixture",
                error,
            )
        })?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let held = Arc::new(AtomicBool::new(false));
        let response_held = held.clone();
        let captured = requests.clone();
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if let Ok(request) = read_request(&mut stream) {
                            let (content_type, response) = match request {
                                FixtureRequest::Page(path) => {
                                    let page = if path.ends_with("two") { "二" } else { "一" };
                                    (
                                        "text/html; charset=utf-8",
                                        format!(
                                            "<!doctype html><meta charset=utf-8><title>浏览验收{page}</title><body style='font:20px system-ui;padding:32px;background:#eef6ff;color:#173c62'><h1>浏览验收{page}</h1><p>网页、导航与截图上下文</p><a href='/page/two'>第二页</a><div style='margin-top:24px;height:180px;background:#4b96df;border-radius:16px'></div></body>"
                                        ),
                                    )
                                }
                                FixtureRequest::Model(body) => {
                                    let approval = turn_decision_response(&body)
                                        .or_else(|| extensions_execution_response(&body))
                                        .or_else(|| approval_response(&body))
                                        .or_else(|| fork_replay_response(&body));
                                    captured.lock().expect("fixture requests").push(body);
                                    while response_held.load(Ordering::Acquire)
                                        && !stopping.load(Ordering::Acquire)
                                    {
                                        thread::sleep(Duration::from_millis(10));
                                    }
                                    ("application/json", approval.unwrap_or_else(|| json!({
                                "choices": [{"finish_reason": "stop", "message": {
                                    "role": "assistant", "content": concat!(
                                        "本地验收回复：模型与引用请求已收到。\n\n",
                                        "![验收图片](data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI3MiIgaGVpZ2h0PSI0MCI+PHJlY3Qgd2lkdGg9IjcyIiBoZWlnaHQ9IjQwIiByeD0iNSIgZmlsbD0iIzI4N2VjMiIvPjxjaXJjbGUgY3g9IjM2IiBjeT0iMjAiIHI9IjEyIiBmaWxsPSIjZmZkMTY2Ii8+PC9zdmc+)\n\n",
                                        "行内公式 $E=mc^2$。\n\n$$\\frac{1}{2}+\\sqrt{x^2+y^2}$$\n\n",
                                        "```mermaid\nflowchart LR\n A[输入] --> B[执行] --> C[完成]\n```"
                                    )
                                }}],
                                "usage": {"prompt_tokens": 32, "completion_tokens": 12, "total_tokens": 44}
                                    })).to_string())
                                }
                            };
                            let _ = write!(
                                stream,
                                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                                response.len()
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
            endpoint: format!("http://{address}/v1/chat/completions"),
            requests,
            stop,
            held,
            worker: Some(worker),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn page_url(&self, page: &str) -> String {
        format!(
            "{}/page/{page}",
            self.endpoint.trim_end_matches("/v1/chat/completions")
        )
    }

    pub fn requests(&self) -> Vec<Value> {
        self.requests.lock().expect("fixture requests").clone()
    }

    pub fn hold_responses(&self, held: bool) {
        self.held.store(held, Ordering::Release);
    }
}

fn turn_decision_response(request: &Value) -> Option<Value> {
    let user = request["messages"]
        .as_array()?
        .iter()
        .rev()
        .find(|message| message["role"] == "user")?["content"]
        .as_str()?;
    let input: Value = serde_json::from_str(user).ok()?;
    if input["tierPolicy"].is_null()
        || input["promptPreview"].is_null()
        || input["current"].is_null()
    {
        return None;
    }
    let decision = json!({"tier":"normal", "reasoningEffort":"medium",
        "planMode":false, "goalMode":false, "sessionFork":false,
        "summary":"本地验收回合策略", "signals":["固定本地夹具"]});
    Some(json!({"choices":[{"finish_reason":"stop","message":{
        "role":"assistant","content":decision.to_string()}}],
        "usage":{"prompt_tokens":32,"completion_tokens":12,"total_tokens":44}}))
}

fn fork_replay_response(request: &Value) -> Option<Value> {
    let user = request["messages"]
        .as_array()?
        .iter()
        .rev()
        .find(|message| message["role"] == "user")?["content"]
        .as_str()?;
    let response = [
        (
            "NATIVE_PARITY_FORK_A_20260903",
            "NATIVE_PARITY_REPLY_A_20260903",
        ),
        (
            "NATIVE_PARITY_FORK_B_20260903",
            "NATIVE_PARITY_REPLY_B_20260903",
        ),
        (
            "NATIVE_PARITY_FORK_C_20260903",
            "NATIVE_PARITY_REPLY_C_20260903",
        ),
    ]
    .into_iter()
    .find(|(marker, _)| user.contains(marker))?
    .1;
    Some(
        json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":response}}],
        "usage":{"prompt_tokens":32,"completion_tokens":12,"total_tokens":44}}),
    )
}

fn extensions_execution_response(request: &Value) -> Option<Value> {
    let messages = request["messages"].as_array()?;
    let (index, user) = messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| message["role"] == "user")?;
    let content = user["content"].as_str()?;
    if !content.contains("Native MCP 执行验收") {
        return None;
    }
    if messages[index + 1..].iter().any(|message| {
        message["role"] == "tool" && message["tool_call_id"] == "native-mcp-execution-call"
    }) {
        return Some(
            json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"本地 MCP 工具结果已接收。"}}]}),
        );
    }
    let name = request["tools"]
        .as_array()?
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .find(|name| name.contains("native") && name.contains("exec") && name.ends_with("echo"))?;
    Some(
        json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[{
            "id":"native-mcp-execution-call","type":"function","function":{"name":name,"arguments":json!({"message":"native-ui-mcp-payload"}).to_string()}
        }]}}]}),
    )
}

fn approval_response(request: &Value) -> Option<Value> {
    let messages = request["messages"].as_array()?;
    let (index, user) = messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| message["role"] == "user")?;
    let content = user["content"].as_str()?;
    let call_id = if content.contains("验证修改计划与草稿恢复") {
        "plan-revise-fixture-call"
    } else if content.contains("验证审批与草稿恢复") {
        "approval-fixture-call"
    } else {
        return None;
    };
    let finished = messages[index + 1..]
        .iter()
        .any(|message| message["role"] == "tool");
    Some(if finished {
        json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"审批已处理，继续完成。"}}]})
    } else {
        json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[{
            "id":call_id, "type":"function", "function":{"name":"confirm_plan","arguments":json!({
                "title":"确认验收计划", "plan":"检查界面与草稿。\n确认后继续当前任务。", "question":"是否执行此计划？"
            }).to_string()}
        }]}}]})
    })
}

impl Drop for ModelFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

enum FixtureRequest {
    Page(String),
    Model(Value),
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<FixtureRequest> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = BufReader::new(stream);
    let mut length = None;
    let mut header_bytes = 0;
    let mut page = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        header_bytes += read;
        if read == 0 || header_bytes > 65_536 {
            return Err(std::io::Error::other("invalid fixture request headers"));
        }
        if header_bytes == read {
            page = line
                .strip_prefix("GET ")
                .and_then(|line| line.split_whitespace().next())
                .map(str::to_owned);
        }
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
        }
    }
    if let Some(page) = page {
        return Ok(FixtureRequest::Page(page));
    }
    let length = length
        .filter(|length| *length <= 16 * 1024 * 1024)
        .ok_or_else(|| std::io::Error::other("missing or excessive fixture body length"))?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .map(FixtureRequest::Model)
        .map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_receives_delayed_request_bodies_on_successive_connections() {
        let fixture = ModelFixture::start().unwrap();
        let address = fixture
            .endpoint()
            .strip_prefix("http://")
            .unwrap()
            .split('/')
            .next()
            .unwrap();
        for index in 0..2 {
            let request = json!({"model":"local-fixture", "messages":[{
                "role":"user", "content":format!("delayed-body-{index}")
            }]});
            let body = serde_json::to_vec(&request).unwrap();
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            write!(stream, "POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.flush().unwrap();
            thread::sleep(Duration::from_millis(100));
            let split = body.len() / 2;
            stream.write_all(&body[..split]).unwrap();
            stream.flush().unwrap();
            thread::sleep(Duration::from_millis(100));
            stream.write_all(&body[split..]).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            let (status, response) = response.split_once("\r\n\r\n").unwrap();
            assert!(status.starts_with("HTTP/1.1 200 OK"));
            let response: Value = serde_json::from_str(response).unwrap();
            assert!(response["choices"][0]["message"]["content"]
                .as_str()
                .is_some());
            assert_eq!(fixture.requests().len(), index + 1);
            assert_eq!(fixture.requests()[index], request);
        }
    }
}
