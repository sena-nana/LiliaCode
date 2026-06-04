use std::fs;
use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use super::claude_mcp::{
    claude_mcp_config_path, create_claude_mcp_server, delete_claude_mcp_server,
    set_claude_mcp_server_enabled, update_claude_mcp_server,
};
use super::claude_plugins::{list_claude_plugins, set_claude_plugin_enabled};
use super::claude_skills::{
    create_claude_skill, delete_claude_skill, list_claude_skills, set_claude_skill_enabled,
};
use super::codex_mcp::{codex_config_path, list_codex_mcp_servers};
use super::runtime::overview;
use super::types::{
    ClaudeMcpServer, ClaudeMcpServerInput, ClaudePlugin, ClaudeSkill, CodexMcpServer,
    PluginsOverview,
};

// ---------- Plugins / Skills ----------
// command 层只做参数转译 + Tauri 错误包装。

#[tauri::command]
pub fn plugins_overview(app: AppHandle, project_cwd: Option<String>) -> PluginsOverview {
    overview(&app, project_cwd.as_deref())
}

#[tauri::command]
pub fn plugins_list_claude_skills(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
) -> Vec<ClaudeSkill> {
    list_claude_skills(&app, &scope, project_cwd.as_deref()).0
}

#[tauri::command]
pub fn plugins_create_claude_skill(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
    description: String,
) -> Result<ClaudeSkill, String> {
    create_claude_skill(&app, &scope, project_cwd.as_deref(), &name, &description)
}

#[tauri::command]
pub fn plugins_delete_claude_skill(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
) -> Result<(), String> {
    delete_claude_skill(&app, &scope, project_cwd.as_deref(), &name)
}

#[tauri::command]
pub fn plugins_set_claude_skill_enabled(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    set_claude_skill_enabled(&app, &scope, project_cwd.as_deref(), &name, enabled)
}

#[tauri::command]
pub fn plugins_list_claude_plugins(app: AppHandle, scope: String) -> Vec<ClaudePlugin> {
    list_claude_plugins(&app, &scope).0
}

#[tauri::command]
pub fn plugins_set_claude_plugin_enabled(
    app: AppHandle,
    scope: String,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    set_claude_plugin_enabled(&app, &scope, &name, enabled)
}

#[tauri::command]
pub fn plugins_create_claude_mcp_server(
    input: ClaudeMcpServerInput,
) -> Result<ClaudeMcpServer, String> {
    create_claude_mcp_server(input)
}

#[tauri::command]
pub fn plugins_update_claude_mcp_server(
    name: String,
    input: ClaudeMcpServerInput,
) -> Result<ClaudeMcpServer, String> {
    update_claude_mcp_server(&name, input)
}

#[tauri::command]
pub fn plugins_delete_claude_mcp_server(name: String) -> Result<(), String> {
    delete_claude_mcp_server(&name)
}

#[tauri::command]
pub fn plugins_set_claude_mcp_server_enabled(name: String, enabled: bool) -> Result<(), String> {
    set_claude_mcp_server_enabled(&name, enabled)
}

fn ensure_file_and_open(
    app: &AppHandle,
    path: PathBuf,
    default_contents: &[u8],
    create_dir_error: &str,
    init_file_error: &str,
    open_error: &str,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("{create_dir_error}：{e}"))?;
        }
    }
    if !path.exists() {
        fs::write(&path, default_contents).map_err(|e| format!("{init_file_error}：{e}"))?;
    }
    let path_str = path.to_string_lossy().to_string();
    app.opener()
        .open_path(path_str, None::<&str>)
        .map_err(|e| format!("{open_error}：{e}"))?;
    Ok(())
}

/// 用系统默认编辑器打开 Lilia 自管 Claude MCP 配置。
#[tauri::command]
pub fn plugins_open_claude_mcp_config(app: AppHandle) -> Result<(), String> {
    ensure_file_and_open(
        &app,
        claude_mcp_config_path(),
        b"{\n  \"servers\": []\n}\n",
        "创建 Claude MCP 配置目录失败",
        "初始化 Claude MCP 配置失败",
        "打开 Claude MCP 配置失败",
    )
}

#[tauri::command]
pub fn plugins_list_codex_mcp_servers(app: AppHandle) -> Vec<CodexMcpServer> {
    list_codex_mcp_servers(&app).0
}

/// 用系统默认编辑器打开 `~/.codex/config.toml`，文件不存在时先建一个空文件。
#[tauri::command]
pub fn plugins_open_codex_config(app: AppHandle) -> Result<(), String> {
    ensure_file_and_open(
        &app,
        codex_config_path(&app)?,
        b"",
        "创建 .codex 目录失败",
        "初始化 config.toml 失败",
        "打开 config.toml 失败",
    )
}
