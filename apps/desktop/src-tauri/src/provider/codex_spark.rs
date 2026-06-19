use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::chat::runner::locate_agent_runner;
use crate::BACKEND_CODEX;

use super::config::{
    load_active_backend, load_assistant_ai_config, load_router_mode, ROUTER_CODEX_ACCOUNT,
};

pub(crate) const CODEX_SPARK_MODEL: &str = "gpt-5.3-codex-spark";
pub(crate) const CODEX_SPARK_BASE_URL: &str = "codex-account://spark";

const DEFAULT_SPARK_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexSparkPromptCommand<'a> {
    kind: &'static str,
    prompt: &'a str,
    instruction: &'a str,
    timeout_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexSparkPromptOutput {
    ok: bool,
    text: Option<String>,
    error: Option<String>,
}

pub(crate) fn codex_account_spark_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    let config = load_assistant_ai_config(app);
    config.codex_account_spark_enabled
        && load_active_backend(app) == BACKEND_CODEX
        && load_router_mode(app, BACKEND_CODEX) == ROUTER_CODEX_ACCOUNT
}

pub(crate) fn is_codex_account_spark_request(
    backend: Option<&str>,
    model: &str,
    base_url: &str,
) -> bool {
    backend == Some(BACKEND_CODEX) && model == CODEX_SPARK_MODEL && base_url == CODEX_SPARK_BASE_URL
}

pub(crate) fn request_codex_account_spark<R: Runtime>(
    app: &AppHandle<R>,
    prompt: &str,
    instruction: &str,
) -> Result<String, String> {
    request_codex_account_spark_with_timeout(app, prompt, instruction, DEFAULT_SPARK_TIMEOUT)
}

fn request_codex_account_spark_with_timeout<R: Runtime>(
    app: &AppHandle<R>,
    prompt: &str,
    instruction: &str,
    timeout: Duration,
) -> Result<String, String> {
    let runner = locate_agent_runner(app);
    let payload = CodexSparkPromptCommand {
        kind: "codex_spark_prompt",
        prompt,
        instruction,
        timeout_ms: timeout.as_millis().min(u64::MAX as u128) as u64,
    };
    let mut child = Command::new("node")
        .arg(&runner)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("无法启动 Codex Spark runner：{err}"))?;

    let input =
        serde_json::to_vec(&payload).map_err(|err| format!("Codex Spark 请求序列化失败：{err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&input)
            .and_then(|_| stdin.write_all(b"\n"))
            .map_err(|err| format!("写入 Codex Spark runner stdin 失败：{err}"))?;
    }

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if started.elapsed() > timeout + Duration::from_secs(5) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Codex Spark runner 超时".to_string());
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => return Err(format!("等待 Codex Spark runner 失败：{err}")),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("读取 Codex Spark runner 输出失败：{err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let Some(line) = stdout.lines().rev().find(|line| !line.trim().is_empty()) else {
        let detail = stderr.trim();
        return if detail.is_empty() {
            Err("Codex Spark runner 没有输出".to_string())
        } else {
            Err(format!("Codex Spark runner 没有输出：{detail}"))
        };
    };
    let parsed: CodexSparkPromptOutput = serde_json::from_str(line.trim())
        .map_err(|err| format!("Codex Spark runner 输出解析失败：{err}"))?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Codex Spark 请求失败".to_string()));
    }
    parsed
        .text
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "Codex Spark 返回空文本".to_string())
}
