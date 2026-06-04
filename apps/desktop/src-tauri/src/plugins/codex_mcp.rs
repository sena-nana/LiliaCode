use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tauri::AppHandle;
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, TableLike, Value};

use super::paths::{ensure_dir, home_dir, sanitize_extension_name};
use super::types::{CodexMcpServer, CodexMcpServerInput};

const CODEX_MCP_ROOT: &str = "mcp_servers";
const TRANSPORT_STDIO: &str = "stdio";
const TRANSPORT_HTTP: &str = "http";
const TRANSPORT_OAUTH: &str = "oauth";
const TRANSPORT_UNKNOWN: &str = "unknown";

pub fn codex_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(home_dir(app)?.join(".codex").join("config.toml"))
}

fn sanitize_codex_mcp_name(raw: &str) -> Result<String, String> {
    sanitize_extension_name(raw, "Codex MCP server")
}

fn normalize_command(raw: &str) -> Result<String, String> {
    let command = raw.trim();
    if command.is_empty() {
        return Err("command 不能为空".to_string());
    }
    Ok(command.to_string())
}

fn normalize_args(args: Vec<String>) -> Vec<String> {
    args.into_iter()
        .map(|arg| arg.trim().to_string())
        .filter(|arg| !arg.is_empty())
        .collect()
}

fn normalize_env(env: BTreeMap<String, String>) -> BTreeMap<String, String> {
    env.into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            if key.is_empty() {
                None
            } else {
                Some((key, value))
            }
        })
        .collect()
}

fn read_codex_doc_from_path(path: &Path) -> Result<DocumentMut, String> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    let text =
        fs::read_to_string(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    text.parse::<DocumentMut>()
        .map_err(|e| format!("{} 不是合法 Codex TOML 配置：{e}", path.display()))
}

fn write_codex_doc_to_path(path: &Path, doc: &DocumentMut) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, doc.to_string()).map_err(|e| format!("写入 {} 失败：{e}", path.display()))
}

fn read_codex_doc(app: &AppHandle) -> Result<(PathBuf, DocumentMut), String> {
    let path = codex_config_path(app)?;
    let doc = read_codex_doc_from_path(&path)?;
    Ok((path, doc))
}

fn mcp_servers_table(doc: &DocumentMut) -> Option<&Table> {
    doc.as_table().get(CODEX_MCP_ROOT).and_then(Item::as_table)
}

fn ensure_mcp_servers_table_mut(doc: &mut DocumentMut) -> Result<&mut Table, String> {
    if !doc.as_table().contains_key(CODEX_MCP_ROOT) {
        let mut table = Table::new();
        table.set_implicit(true);
        doc.as_table_mut()
            .insert(CODEX_MCP_ROOT, Item::Table(table));
    }
    doc.as_table_mut()
        .get_mut(CODEX_MCP_ROOT)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| "mcp_servers 不是 TOML table，无法修改 Codex MCP server".to_string())
}

fn find_server_key(table: &Table, name: &str) -> Option<String> {
    table
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(key, _)| key.to_string())
}

fn string_array(
    item: Option<&Item>,
    name: &str,
    field: &str,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let Some(item) = item else {
        return Vec::new();
    };
    let Some(array) = item.as_array() else {
        warnings.push(format!("Codex MCP server {name} 的 {field} 不是字符串数组"));
        return Vec::new();
    };
    let mut out = Vec::new();
    for value in array.iter() {
        if let Some(text) = value.as_str() {
            out.push(text.to_string());
        } else {
            warnings.push(format!("Codex MCP server {name} 的 {field} 含非字符串项"));
        }
    }
    out
}

fn env_keys(item: Option<&Item>, name: &str, warnings: &mut Vec<String>) -> Vec<String> {
    let Some(item) = item else {
        return Vec::new();
    };
    let Some(table) = item.as_table_like() else {
        warnings.push(format!("Codex MCP server {name} 的 env 不是 table"));
        return Vec::new();
    };
    let mut keys: Vec<String> = table
        .iter()
        .filter_map(|(key, value)| {
            if value.as_str().is_some() {
                Some(key.to_string())
            } else {
                warnings.push(format!("Codex MCP server {name} 的 env.{key} 不是字符串"));
                None
            }
        })
        .collect();
    keys.sort_by_key(|key| key.to_lowercase());
    keys
}

fn transport_for(table: &dyn TableLike, command: &str) -> String {
    let url = table.get("url").and_then(Item::as_str).unwrap_or("").trim();
    if !command.is_empty() && url.is_empty() {
        return TRANSPORT_STDIO.to_string();
    }
    if !url.is_empty() {
        let has_oauth_hint = table.contains_key("oauth_resource")
            || table.contains_key("scopes")
            || table.contains_key("scope");
        return if has_oauth_hint {
            TRANSPORT_OAUTH.to_string()
        } else {
            TRANSPORT_HTTP.to_string()
        };
    }
    TRANSPORT_UNKNOWN.to_string()
}

fn public_codex_mcp_server(name: &str, item: &Item, warnings: &mut Vec<String>) -> CodexMcpServer {
    let Some(table) = item.as_table_like() else {
        warnings.push(format!("Codex MCP server {name} 不是 table"));
        return CodexMcpServer {
            name: name.to_string(),
            command: String::new(),
            args: Vec::new(),
            env_keys: Vec::new(),
            enabled: true,
            transport: TRANSPORT_UNKNOWN.to_string(),
            editable: false,
        };
    };
    let command = table
        .get("command")
        .and_then(Item::as_str)
        .unwrap_or("")
        .to_string();
    let args = string_array(table.get("args"), name, "args", warnings);
    let env_keys = env_keys(table.get("env"), name, warnings);
    let transport = transport_for(table, command.trim());
    let enabled = table.get("enabled").and_then(Item::as_bool).unwrap_or(true);
    CodexMcpServer {
        name: name.to_string(),
        command,
        args,
        env_keys,
        enabled,
        editable: transport == TRANSPORT_STDIO,
        transport,
    }
}

#[cfg(test)]
fn parse_codex_mcp_servers(text: &str) -> (Vec<CodexMcpServer>, Vec<String>) {
    if text.trim().is_empty() {
        return (Vec::new(), Vec::new());
    }
    let doc = match text.parse::<DocumentMut>() {
        Ok(doc) => doc,
        Err(e) => return (Vec::new(), vec![format!("config.toml 不是合法 TOML：{e}")]),
    };
    list_codex_mcp_servers_from_doc(&doc)
}

fn list_codex_mcp_servers_from_doc(doc: &DocumentMut) -> (Vec<CodexMcpServer>, Vec<String>) {
    let mut warnings = Vec::new();
    let Some(table) = mcp_servers_table(doc) else {
        if doc.as_table().get(CODEX_MCP_ROOT).is_some() {
            warnings.push("mcp_servers 不是 TOML table".to_string());
        }
        return (Vec::new(), warnings);
    };
    let mut servers: Vec<CodexMcpServer> = table
        .iter()
        .map(|(name, item)| public_codex_mcp_server(name, item, &mut warnings))
        .collect();
    servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    (servers, warnings)
}

pub fn list_codex_mcp_servers(app: &AppHandle) -> (Vec<CodexMcpServer>, Vec<String>) {
    let path = match codex_config_path(app) {
        Ok(p) => p,
        Err(e) => return (Vec::new(), vec![e]),
    };
    if !path.exists() {
        return (Vec::new(), Vec::new());
    }
    let doc = match read_codex_doc_from_path(&path) {
        Ok(doc) => doc,
        Err(e) => return (Vec::new(), vec![e]),
    };
    list_codex_mcp_servers_from_doc(&doc)
}

fn string_array_item(args: Vec<String>) -> Item {
    let mut array = Array::default();
    for arg in args {
        array.push(arg);
    }
    value(array)
}

fn env_item(env: BTreeMap<String, String>) -> Option<Item> {
    if env.is_empty() {
        return None;
    }
    let mut table = InlineTable::default();
    for (key, value) in env {
        table.insert(&key, Value::from(value));
    }
    table.fmt();
    Some(value(table))
}

fn set_stdio_fields(table: &mut dyn TableLike, command: String, args: Vec<String>) {
    table.insert("command", value(command));
    table.insert("args", string_array_item(args));
}

fn set_enabled_field(table: &mut dyn TableLike, enabled: bool) {
    if enabled {
        table.remove("enabled");
    } else {
        table.insert("enabled", value(false));
    }
}

fn apply_env_update(
    table: &mut dyn TableLike,
    env: Option<BTreeMap<String, String>>,
    remove_env_keys: Vec<String>,
) -> Result<(), String> {
    let env = env.map(normalize_env).unwrap_or_default();
    let remove_env_keys: Vec<String> = remove_env_keys
        .into_iter()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .collect();
    if env.is_empty() && remove_env_keys.is_empty() {
        return Ok(());
    }
    if table.get("env").is_none() {
        if let Some(item) = env_item(env) {
            table.insert("env", item);
        }
        return Ok(());
    }
    let Some(env_table) = table.get_mut("env").and_then(Item::as_table_like_mut) else {
        return Err("env 不是 table，无法安全更新 env key".to_string());
    };
    for key in remove_env_keys {
        env_table.remove(&key);
    }
    for (key, env_value) in env {
        env_table.insert(&key, value(env_value));
    }
    if env_table.is_empty() {
        table.remove("env");
    }
    Ok(())
}

fn stdio_table_from_input(input: CodexMcpServerInput) -> Result<Table, String> {
    let command = normalize_command(&input.command)?;
    let mut table = Table::new();
    set_stdio_fields(&mut table, command, normalize_args(input.args));
    if let Some(env) = env_item(normalize_env(input.env.unwrap_or_default())) {
        table.insert("env", env);
    }
    Ok(table)
}

fn ensure_editable_stdio(table: &Table, key: &str) -> Result<(), String> {
    let item = table
        .get(key)
        .ok_or_else(|| format!("未找到 Codex MCP server：{key}"))?;
    let mut warnings = Vec::new();
    let server = public_codex_mcp_server(key, item, &mut warnings);
    if server.editable {
        Ok(())
    } else {
        Err(format!(
            "Codex MCP server {key} 是 {} transport，只能打开 config.toml 手动编辑",
            server.transport
        ))
    }
}

pub fn create_codex_mcp_server(
    app: &AppHandle,
    input: CodexMcpServerInput,
) -> Result<CodexMcpServer, String> {
    let name = sanitize_codex_mcp_name(&input.name)?;
    let (path, mut doc) = read_codex_doc(app)?;
    let table = ensure_mcp_servers_table_mut(&mut doc)?;
    if find_server_key(table, &name).is_some() {
        return Err(format!("同名 Codex MCP server 已存在：{name}"));
    }
    let server_table = stdio_table_from_input(input)?;
    table.insert(&name, Item::Table(server_table));
    write_codex_doc_to_path(&path, &doc)?;
    let (servers, _) = list_codex_mcp_servers_from_doc(&doc);
    servers
        .into_iter()
        .find(|server| server.name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| format!("创建 Codex MCP server 后读取失败：{name}"))
}

pub fn update_codex_mcp_server(
    app: &AppHandle,
    name: &str,
    input: CodexMcpServerInput,
) -> Result<CodexMcpServer, String> {
    let current_name = sanitize_codex_mcp_name(name)?;
    let next_name = sanitize_codex_mcp_name(&input.name)?;
    let command = normalize_command(&input.command)?;
    let args = normalize_args(input.args);
    let (path, mut doc) = read_codex_doc(app)?;
    let table = ensure_mcp_servers_table_mut(&mut doc)?;
    let current_key = find_server_key(table, &current_name)
        .ok_or_else(|| format!("未找到 Codex MCP server：{current_name}"))?;
    ensure_editable_stdio(table, &current_key)?;
    if let Some(existing_key) = find_server_key(table, &next_name) {
        if !existing_key.eq_ignore_ascii_case(&current_key) {
            return Err(format!("同名 Codex MCP server 已存在：{next_name}"));
        }
    }
    if current_key == next_name {
        let item = table
            .get_mut(&current_key)
            .ok_or_else(|| format!("未找到 Codex MCP server：{current_name}"))?;
        let server_table = item
            .as_table_like_mut()
            .ok_or_else(|| format!("Codex MCP server {current_key} 不是 table"))?;
        set_stdio_fields(server_table, command, args);
        apply_env_update(server_table, input.env, input.remove_env_keys)?;
    } else {
        let mut item = table
            .remove(&current_key)
            .ok_or_else(|| format!("未找到 Codex MCP server：{current_name}"))?;
        let Some(server_table) = item.as_table_like_mut() else {
            return Err(format!("Codex MCP server {current_key} 不是 table"));
        };
        set_stdio_fields(server_table, command, args);
        apply_env_update(server_table, input.env, input.remove_env_keys)?;
        table.insert(&next_name, item);
    }
    write_codex_doc_to_path(&path, &doc)?;
    let (servers, _) = list_codex_mcp_servers_from_doc(&doc);
    servers
        .into_iter()
        .find(|server| server.name.eq_ignore_ascii_case(&next_name))
        .ok_or_else(|| format!("更新 Codex MCP server 后读取失败：{next_name}"))
}

pub fn delete_codex_mcp_server(app: &AppHandle, name: &str) -> Result<(), String> {
    let name = sanitize_codex_mcp_name(name)?;
    let (path, mut doc) = read_codex_doc(app)?;
    let table = ensure_mcp_servers_table_mut(&mut doc)?;
    let key =
        find_server_key(table, &name).ok_or_else(|| format!("未找到 Codex MCP server：{name}"))?;
    ensure_editable_stdio(table, &key)?;
    table.remove(&key);
    write_codex_doc_to_path(&path, &doc)
}

pub fn set_codex_mcp_server_enabled(
    app: &AppHandle,
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    let name = sanitize_codex_mcp_name(name)?;
    let (path, mut doc) = read_codex_doc(app)?;
    let table = ensure_mcp_servers_table_mut(&mut doc)?;
    let key =
        find_server_key(table, &name).ok_or_else(|| format!("未找到 Codex MCP server：{name}"))?;
    ensure_editable_stdio(table, &key)?;
    let item = table
        .get_mut(&key)
        .ok_or_else(|| format!("未找到 Codex MCP server：{name}"))?;
    let server_table = item
        .as_table_like_mut()
        .ok_or_else(|| format!("Codex MCP server {key} 不是 table"))?;
    set_enabled_field(server_table, enabled);
    write_codex_doc_to_path(&path, &doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_toml_parses_stdio_http_oauth_and_env_keys() {
        let text = r#"
[mcp_servers.weather]
command = "node"
args = ["weather-mcp.js", "--port", "5151"]
env = { WEATHER_TOKEN = "secret", BAD = 1 }

[mcp_servers.remote]
url = "https://example.com/mcp"

[mcp_servers.oauth_remote]
url = "https://example.com/oauth-mcp"
scopes = ["repo:read"]
enabled = false
"#;
        let (servers, warnings) = parse_codex_mcp_servers(text);
        assert_eq!(servers.len(), 3);
        assert!(warnings.iter().any(|warning| warning.contains("env.BAD")));

        let weather = servers
            .iter()
            .find(|server| server.name == "weather")
            .unwrap();
        assert_eq!(weather.transport, TRANSPORT_STDIO);
        assert!(weather.editable);
        assert!(weather.enabled);
        assert_eq!(weather.command, "node");
        assert_eq!(weather.args, vec!["weather-mcp.js", "--port", "5151"]);
        assert_eq!(weather.env_keys, vec!["WEATHER_TOKEN"]);

        let remote = servers
            .iter()
            .find(|server| server.name == "remote")
            .unwrap();
        assert_eq!(remote.transport, TRANSPORT_HTTP);
        assert!(!remote.editable);

        let oauth = servers
            .iter()
            .find(|server| server.name == "oauth_remote")
            .unwrap();
        assert_eq!(oauth.transport, TRANSPORT_OAUTH);
        assert!(!oauth.editable);
        assert!(!oauth.enabled);
    }

    #[test]
    fn codex_toml_ignores_non_mcp_sections() {
        let text = "[other]\nfoo = 1\n[mcp_servers.x]\ncommand = \"echo\"\n";
        let (servers, warnings) = parse_codex_mcp_servers(text);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "x");
        assert_eq!(servers[0].command, "echo");
    }

    #[test]
    fn codex_toml_updates_stdio_without_dropping_other_config() {
        let mut doc = r#"
model = "gpt-5.5"

[profiles.default]
approval_policy = "on-request"

[mcp_servers.weather]
command = "node"
args = ["old.js"]
enabled = false
env = { KEEP = "old", REMOVE = "gone" }
custom = "preserve"

[mcp_servers.remote]
url = "https://example.com/mcp"
"#
        .parse::<DocumentMut>()
        .unwrap();
        let table = ensure_mcp_servers_table_mut(&mut doc).unwrap();
        let weather = table
            .get_mut("weather")
            .unwrap()
            .as_table_like_mut()
            .unwrap();
        set_stdio_fields(
            weather,
            "uvx".to_string(),
            vec!["weather-mcp".to_string(), "--debug".to_string()],
        );
        set_enabled_field(weather, true);
        apply_env_update(
            weather,
            Some(BTreeMap::from([
                ("ADD".to_string(), "".to_string()),
                ("KEEP".to_string(), "new".to_string()),
            ])),
            vec!["REMOVE".to_string()],
        )
        .unwrap();
        let text = doc.to_string();
        assert!(text.contains("model = \"gpt-5.5\""));
        assert!(text.contains("[profiles.default]"));
        assert!(text.contains("[mcp_servers.remote]"));
        assert!(text.contains("custom = \"preserve\""));
        assert!(!text.contains("enabled = false"));
        assert!(text.contains("command = \"uvx\""));
        assert!(text.contains("\"weather-mcp\""));
        assert!(text.contains("KEEP = \"new\""));
        assert!(text.contains("ADD = \"\""));
        assert!(!text.contains("REMOVE"));
    }

    #[test]
    fn codex_toml_rejects_mutating_readonly_server() {
        let doc = r#"
[mcp_servers.remote]
url = "https://example.com/mcp"
"#
        .parse::<DocumentMut>()
        .unwrap();
        let table = mcp_servers_table(&doc).unwrap();
        assert!(ensure_editable_stdio(table, "remote").is_err());
    }
}
