use std::io::Write;
use std::process::{Command, Stdio};

use tauri::AppHandle;

use super::types::ClaudeHistoryUtilityOutput;
use crate::chat::runner::locate_agent_runner;

fn locate_claude_history_utility(app: &AppHandle) -> std::path::PathBuf {
    let runner = locate_agent_runner(app);
    runner
        .parent()
        .map(|dir| dir.join("claude-history.mjs"))
        .unwrap_or_else(|| std::path::PathBuf::from("claude-history.mjs"))
}

pub(super) fn run_claude_history_utility(
    app: &AppHandle,
    payload: serde_json::Value,
) -> Result<ClaudeHistoryUtilityOutput, String> {
    let script = locate_claude_history_utility(app);
    let node = std::env::var("LILIA_NODE_BIN").unwrap_or_else(|_| "node".to_string());
    let mut child = Command::new(node)
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动 Claude history utility：{e}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        let line = serde_json::to_string(&payload)
            .map_err(|e| format!("Claude history payload 序列化失败：{e}"))?;
        stdin
            .write_all(format!("{line}\n").as_bytes())
            .map_err(|e| format!("写入 Claude history utility 失败：{e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("等待 Claude history utility 失败：{e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| {
            let detail = stderr.trim();
            if detail.is_empty() {
                "Claude history utility 没有返回数据".to_string()
            } else {
                format!("Claude history utility 没有返回数据：{detail}")
            }
        })?;
    let result: ClaudeHistoryUtilityOutput = serde_json::from_str(line)
        .map_err(|e| format!("解析 Claude history utility 输出失败：{e}"))?;
    if let Some(error) = result.error.clone() {
        return Err(error);
    }
    if !output.status.success() {
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            "Claude history utility 异常退出".to_string()
        } else {
            format!("Claude history utility 异常退出：{detail}")
        });
    }
    Ok(result)
}
