use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value as JsonValue;
use tauri::{AppHandle, Runtime};

use crate::chat::runner::locate_agent_runner;
use crate::provider::{
    build_codex_app_server_probe_status, resolve_connection_for, validate_backend_ready_for_send,
    ConnectionMode,
};
use crate::BACKEND_CODEX;

use super::types::CodexHistoryUtilityOutput;

fn locate_codex_history_utility<R: Runtime>(app: &AppHandle<R>) -> std::path::PathBuf {
    let runner = locate_agent_runner(app);
    runner
        .parent()
        .map(|dir| dir.join("codex-history.mjs"))
        .unwrap_or_else(|| std::path::PathBuf::from("codex-history.mjs"))
}

pub(super) fn run_codex_history_utility(
    app: &AppHandle<impl Runtime>,
    payload: JsonValue,
) -> Result<CodexHistoryUtilityOutput, String> {
    validate_backend_ready_for_send(BACKEND_CODEX)?;
    let script = locate_codex_history_utility(app);
    let connection = resolve_connection_for(app, BACKEND_CODEX);
    let codex_app_server = build_codex_app_server_probe_status();
    let codex_path = codex_app_server
        .path
        .ok_or_else(|| "未找到满足要求的 Codex CLI，无法读取 Codex 历史".to_string())?;

    let mut cmd = Command::new("node");
    cmd.arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("LILIA_CODEX_CLI_PATH", codex_path);
    if connection.mode == ConnectionMode::CodexAccount {
        cmd.env_remove("OPENAI_BASE_URL");
        cmd.env_remove("OPENAI_API_KEY");
        cmd.env_remove("CODEX_API_KEY");
    }
    if let Some(url) = connection.base_url {
        cmd.env("OPENAI_BASE_URL", url);
    }
    if let Some(key) = connection.api_key {
        cmd.env("OPENAI_API_KEY", key);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("无法启动 Codex history utility：{e}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        let mut bytes = serde_json::to_vec(&payload)
            .map_err(|e| format!("Codex history payload 序列化失败：{e}"))?;
        bytes.push(b'\n');
        stdin
            .write_all(&bytes)
            .map_err(|e| format!("写入 Codex history utility 失败：{e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待 Codex history utility 失败：{e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| {
            let detail = stderr.trim();
            if detail.is_empty() {
                "Codex history utility 没有返回数据".to_string()
            } else {
                format!("Codex history utility 没有返回数据：{detail}")
            }
        })?;
    let result: CodexHistoryUtilityOutput = serde_json::from_str(line)
        .map_err(|e| format!("解析 Codex history utility 输出失败：{e}"))?;
    if let Some(error) = result.error.as_ref().filter(|s| !s.trim().is_empty()) {
        return Err(error.clone());
    }
    if !output.status.success() {
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            "Codex history utility 异常退出".to_string()
        } else {
            format!("Codex history utility 异常退出：{detail}")
        });
    }
    Ok(result)
}
