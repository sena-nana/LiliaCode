use std::fs;
use std::path::PathBuf;

use tauri::AppHandle;

use super::paths::home_dir;
use super::types::CodexMcpServer;

pub fn codex_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(home_dir(app)?.join(".codex").join("config.toml"))
}

/// 极简 TOML 解析：只抽 `[mcp_servers.<name>]` 节里 `command` / `args` 两个字段。
pub fn parse_codex_mcp_servers(text: &str) -> (Vec<CodexMcpServer>, Vec<String>) {
    let mut servers: Vec<CodexMcpServer> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut current: Option<CodexMcpServer> = None;

    let flush = |cur: &mut Option<CodexMcpServer>, out: &mut Vec<CodexMcpServer>| {
        if let Some(s) = cur.take() {
            out.push(s);
        }
    };

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            flush(&mut current, &mut servers);
            if let Some(name) = rest.strip_prefix("mcp_servers.") {
                let name = name.trim().trim_matches('"');
                if name.is_empty() {
                    warnings.push(format!("第 {} 行 mcp_servers 名称为空", idx + 1));
                    continue;
                }
                current = Some(CodexMcpServer {
                    name: name.to_string(),
                    command: String::new(),
                    args: Vec::new(),
                    enabled: true,
                });
            }
            continue;
        }
        let Some(server) = current.as_mut() else {
            continue;
        };
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();
        match key {
            "command" => {
                if let Some(s) = parse_toml_string(val) {
                    server.command = s;
                }
            }
            "args" => {
                if let Some(arr) = parse_toml_string_array(val) {
                    server.args = arr;
                } else {
                    warnings.push(format!("第 {} 行 args 不是字符串数组", idx + 1));
                }
            }
            _ => {}
        }
    }
    flush(&mut current, &mut servers);
    servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    (servers, warnings)
}

fn parse_toml_string(raw: &str) -> Option<String> {
    let s = raw.trim();
    let s = s.strip_prefix('"').and_then(|s| s.strip_suffix('"'))?;
    Some(s.to_string())
}

fn parse_toml_string_array(raw: &str) -> Option<Vec<String>> {
    let s = raw.trim();
    let inner = s.strip_prefix('[').and_then(|s| s.strip_suffix(']'))?;
    let mut out = Vec::new();
    for part in split_top_level_commas(inner) {
        let item = part.trim();
        if item.is_empty() {
            continue;
        }
        let unq = item.strip_prefix('"').and_then(|s| s.strip_suffix('"'))?;
        out.push(unq.to_string());
    }
    Some(out)
}

/// 按 `,` 拆分但忽略引号内的逗号。只够应付简单字符串数组。
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut prev = '\0';
    for c in s.chars() {
        if c == '"' && prev != '\\' {
            in_str = !in_str;
        }
        if c == ',' && !in_str {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
        prev = c;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn list_codex_mcp_servers(app: &AppHandle) -> (Vec<CodexMcpServer>, Vec<String>) {
    let path = match codex_config_path(app) {
        Ok(p) => p,
        Err(e) => return (Vec::new(), vec![e]),
    };
    if !path.exists() {
        return (Vec::new(), Vec::new());
    }
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            return (
                Vec::new(),
                vec![format!("读取 {} 失败：{e}", path.display())],
            );
        }
    };
    parse_codex_mcp_servers(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_toml_parses_simple_block() {
        let text = r#"
# 顶层注释
[mcp_servers.weather]
command = "node"
args = ["weather-mcp.js", "--port", "5151"]

[mcp_servers.linear]
command = "uvx"
args = ["linear-mcp"]
"#;
        let (servers, warnings) = parse_codex_mcp_servers(text);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(servers.len(), 2);
        let weather = &servers[1];
        assert_eq!(weather.name, "weather");
        assert_eq!(weather.command, "node");
        assert_eq!(weather.args, vec!["weather-mcp.js", "--port", "5151"]);
    }

    #[test]
    fn codex_toml_ignores_non_mcp_sections() {
        let text = "[other]\nfoo = 1\n[mcp_servers.x]\ncommand = \"echo\"\n";
        let (servers, _) = parse_codex_mcp_servers(text);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "x");
        assert_eq!(servers[0].command, "echo");
    }
}
