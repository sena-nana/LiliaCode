use tauri::AppHandle;

use super::claude_mcp::{
    claude_mcp_config_path, list_claude_mcp_servers, runtime_claude_mcp_servers,
};
use super::claude_plugins::list_claude_plugins;
use super::claude_skills::list_claude_skills;
use super::codex_mcp::{codex_config_path, list_codex_mcp_servers};
use super::paths::{SCOPE_PROJECT, SCOPE_USER};
use super::types::{
    AgentRuntimeExtensions, ClaudeRuntimeExtensions, ClaudeRuntimePlugin, CodexRuntimeExtensions,
    ClaudeSkill, PluginsOverview,
};

fn list_scoped_claude_skills(
    app: &AppHandle,
    project_cwd: Option<&str>,
) -> (Vec<ClaudeSkill>, Vec<ClaudeSkill>, Vec<String>) {
    let mut warnings = Vec::new();
    let (user_skills, w1) = list_claude_skills(app, SCOPE_USER, None);
    warnings.extend(w1);
    let (project_skills, w2) = if project_cwd.is_some() {
        list_claude_skills(app, SCOPE_PROJECT, project_cwd)
    } else {
        (Vec::new(), Vec::new())
    };
    warnings.extend(w2);
    (user_skills, project_skills, warnings)
}

pub fn overview(app: &AppHandle, project_cwd: Option<&str>) -> PluginsOverview {
    let (user_skills, project_skills, mut warnings) = list_scoped_claude_skills(app, project_cwd);
    let (user_plugins, w3) = list_claude_plugins(app, SCOPE_USER);
    warnings.extend(w3);
    let (claude_mcp_servers, w4) = list_claude_mcp_servers();
    warnings.extend(w4);
    let claude_mcp_config_path = Some(claude_mcp_config_path().to_string_lossy().to_string());
    let (codex_mcp_servers, w5) = list_codex_mcp_servers(app);
    warnings.extend(w5);
    let codex_config_path = codex_config_path(app)
        .ok()
        .map(|path| path.to_string_lossy().to_string());
    PluginsOverview {
        claude_user_skills: user_skills,
        claude_project_skills: project_skills,
        claude_user_plugins: user_plugins,
        claude_mcp_servers,
        claude_mcp_config_path,
        codex_mcp_servers,
        codex_config_path,
        warnings,
    }
}

pub fn runtime_extensions(app: &AppHandle, project_cwd: Option<&str>) -> AgentRuntimeExtensions {
    let (user_skills, project_skills, mut claude_warnings) =
        list_scoped_claude_skills(app, project_cwd);
    let skills = user_skills
        .into_iter()
        .chain(project_skills)
        .filter(|skill| skill.enabled)
        .map(|skill| skill.name)
        .collect();

    let (plugins, w3) = list_claude_plugins(app, SCOPE_USER);
    claude_warnings.extend(w3);
    let runtime_plugins = plugins
        .into_iter()
        .filter(|plugin| plugin.enabled)
        .map(|plugin| ClaudeRuntimePlugin {
            r#type: "local".to_string(),
            path: plugin.path,
        })
        .collect();
    let (claude_mcp_servers, mcp_warnings) = runtime_claude_mcp_servers();
    claude_warnings.extend(mcp_warnings);

    let config_path = codex_config_path(app).ok();
    let (mcp_servers, codex_warnings) = list_codex_mcp_servers(app);

    AgentRuntimeExtensions {
        claude: ClaudeRuntimeExtensions {
            skills,
            plugins: runtime_plugins,
            mcp_servers: claude_mcp_servers,
            warnings: claude_warnings,
        },
        codex: CodexRuntimeExtensions {
            mcp_servers,
            config_path: config_path.map(|path| path.to_string_lossy().to_string()),
            warnings: codex_warnings,
        },
    }
}
