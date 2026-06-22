use std::fs;
use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use super::claude_mcp::{
    claude_mcp_config_path, create_claude_mcp_server, delete_claude_mcp_server,
    set_claude_mcp_server_enabled, update_claude_mcp_server,
};
use super::claude_plugins::set_claude_plugin_enabled;
use super::claude_skills::{create_claude_skill, delete_claude_skill, set_claude_skill_enabled};
use super::codex_mcp::{
    codex_config_path, create_codex_mcp_server, delete_codex_mcp_server,
    set_codex_mcp_server_enabled, update_codex_mcp_server,
};
use super::hooks::{
    create_hook_source, delete_hook_source, hooks_overview, read_hook_source,
    set_hook_source_enabled, update_hook_source, FORMAT_CLAUDE_SETTINGS_JSON,
    FORMAT_CODEX_HOOKS_JSON,
};
use super::runtime::overview;
use super::types::{
    HookDocumentUpdateInput, HookDocumentView, HookSourceSummary, HooksOverview, PluginMcpServer,
    PluginMcpServerInput, PluginSkill, PluginsOverview,
};
use crate::{BACKEND_CLAUDE, BACKEND_CODEX};

enum PluginBackend {
    Claude,
    Codex,
}

fn plugin_backend(value: &str) -> Result<PluginBackend, String> {
    match value {
        BACKEND_CLAUDE => Ok(PluginBackend::Claude),
        BACKEND_CODEX => Ok(PluginBackend::Codex),
        _ => Err(format!("未知插件后端：{value}")),
    }
}

// ---------- Plugins / Skills ----------
// command 层只做参数转译 + Tauri 错误包装。

#[tauri::command]
pub fn plugins_overview(app: AppHandle, project_cwd: Option<String>) -> PluginsOverview {
    overview(&app, project_cwd.as_deref())
}

#[tauri::command]
pub fn plugins_hooks_overview(app: AppHandle, project_cwd: Option<String>) -> HooksOverview {
    hooks_overview(&app, project_cwd.as_deref())
}

#[tauri::command]
pub fn plugins_create_skill(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
    description: String,
) -> Result<PluginSkill, String> {
    create_claude_skill(&app, &scope, project_cwd.as_deref(), &name, &description)
}

#[tauri::command]
pub fn plugins_delete_skill(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
) -> Result<(), String> {
    delete_claude_skill(&app, &scope, project_cwd.as_deref(), &name)
}

#[tauri::command]
pub fn plugins_set_skill_enabled(
    app: AppHandle,
    scope: String,
    project_cwd: Option<String>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    set_claude_skill_enabled(&app, &scope, project_cwd.as_deref(), &name, enabled)
}

#[tauri::command]
pub fn plugins_set_package_enabled(
    app: AppHandle,
    backend: String,
    scope: String,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    match plugin_backend(&backend)? {
        PluginBackend::Claude => set_claude_plugin_enabled(&app, &scope, &name, enabled),
        PluginBackend::Codex => Err("Codex 不支持 package 管理".to_string()),
    }
}

#[tauri::command]
pub fn plugins_create_mcp_server(
    app: AppHandle,
    backend: String,
    input: PluginMcpServerInput,
) -> Result<PluginMcpServer, String> {
    match plugin_backend(&backend)? {
        PluginBackend::Claude => create_claude_mcp_server(input),
        PluginBackend::Codex => create_codex_mcp_server(&app, input),
    }
}

#[tauri::command]
pub fn plugins_update_mcp_server(
    app: AppHandle,
    backend: String,
    name: String,
    input: PluginMcpServerInput,
) -> Result<PluginMcpServer, String> {
    match plugin_backend(&backend)? {
        PluginBackend::Claude => update_claude_mcp_server(&name, input),
        PluginBackend::Codex => update_codex_mcp_server(&app, &name, input),
    }
}

#[tauri::command]
pub fn plugins_delete_mcp_server(
    app: AppHandle,
    backend: String,
    name: String,
) -> Result<(), String> {
    match plugin_backend(&backend)? {
        PluginBackend::Claude => delete_claude_mcp_server(&name),
        PluginBackend::Codex => delete_codex_mcp_server(&app, &name),
    }
}

#[tauri::command]
pub fn plugins_set_mcp_server_enabled(
    app: AppHandle,
    backend: String,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    match plugin_backend(&backend)? {
        PluginBackend::Claude => set_claude_mcp_server_enabled(&name, enabled),
        PluginBackend::Codex => set_codex_mcp_server_enabled(&app, &name, enabled),
    }
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

#[tauri::command]
pub fn plugins_open_mcp_config(app: AppHandle, backend: String) -> Result<(), String> {
    match plugin_backend(&backend)? {
        PluginBackend::Claude => ensure_file_and_open(
            &app,
            claude_mcp_config_path(),
            b"{\n  \"servers\": []\n}\n",
            "创建 Claude MCP 配置目录失败",
            "初始化 Claude MCP 配置失败",
            "打开 Claude MCP 配置失败",
        ),
        PluginBackend::Codex => ensure_file_and_open(
            &app,
            codex_config_path(&app)?,
            b"",
            "创建 .codex 目录失败",
            "初始化 config.toml 失败",
            "打开 config.toml 失败",
        ),
    }
}

#[tauri::command]
pub fn plugins_read_hook_source(
    app: AppHandle,
    source: HookSourceSummary,
) -> Result<HookDocumentView, String> {
    read_hook_source(&app, &source)
}

#[tauri::command]
pub fn plugins_update_hook_source(
    app: AppHandle,
    source: HookSourceSummary,
    input: HookDocumentUpdateInput,
) -> Result<HookDocumentView, String> {
    update_hook_source(&app, &source, input)
}

#[tauri::command]
pub fn plugins_create_hook_source(
    app: AppHandle,
    backend: String,
    scope: String,
    project_cwd: Option<String>,
) -> Result<HookSourceSummary, String> {
    create_hook_source(&app, &backend, &scope, project_cwd.as_deref())
}

#[tauri::command]
pub fn plugins_delete_hook_source(app: AppHandle, source: HookSourceSummary) -> Result<(), String> {
    delete_hook_source(&app, &source)
}

#[tauri::command]
pub fn plugins_set_hook_source_enabled(
    app: AppHandle,
    source: HookSourceSummary,
    enabled: bool,
) -> Result<HookSourceSummary, String> {
    set_hook_source_enabled(&app, &source, enabled)
}

#[tauri::command]
pub fn plugins_open_hook_config(app: AppHandle, source: HookSourceSummary) -> Result<(), String> {
    let path = PathBuf::from(&source.path);
    let default_contents: &[u8] = match (source.backend.as_str(), source.format.as_str()) {
        (BACKEND_CLAUDE, FORMAT_CLAUDE_SETTINGS_JSON)
        | (BACKEND_CODEX, FORMAT_CODEX_HOOKS_JSON) => b"{\n  \"hooks\": {}\n}\n",
        _ => b"",
    };
    ensure_file_and_open(
        &app,
        path,
        default_contents,
        "创建 hooks 配置目录失败",
        "初始化 hooks 配置失败",
        "打开 hooks 配置失败",
    )
}
