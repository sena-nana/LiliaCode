use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

pub(super) const CLAUDE_DIR: &str = ".claude";
pub(super) const SKILLS_SUBDIR: &str = "skills";
pub(super) const PLUGINS_SUBDIR: &str = "plugins";
pub(super) const SKILL_FILE: &str = "SKILL.md";
pub(super) const PLUGIN_MANIFEST: &str = "plugin.json";
pub(super) const LILIA_CONFIG_SUBDIR: &str = "config";
pub(super) const CLAUDE_MCP_CONFIG_FILE: &str = "claude-mcp-servers.json";
pub(super) const BUILTIN_CLAUDE_MCP_SERVER: &str = "lilia";

pub(super) const SCOPE_USER: &str = "user";
pub(super) const SCOPE_PROJECT: &str = "project";

pub(super) fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|e| format!("无法获取用户主目录：{e}"))
}

/// 给定 scope + 可选 project_cwd，算出 .claude/<sub> 的根目录。
pub(super) fn claude_root_for(
    app: &AppHandle,
    scope: &str,
    project_cwd: Option<&str>,
    sub: &str,
) -> Result<PathBuf, String> {
    match scope {
        SCOPE_USER => Ok(home_dir(app)?.join(CLAUDE_DIR).join(sub)),
        SCOPE_PROJECT => {
            let cwd = project_cwd
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "项目 scope 需要提供 projectCwd".to_string())?;
            Ok(PathBuf::from(cwd).join(CLAUDE_DIR).join(sub))
        }
        other => Err(format!("未知 scope: {other}")),
    }
}

pub(super) fn ensure_dir(p: &Path) -> Result<(), String> {
    if !p.exists() {
        fs::create_dir_all(p).map_err(|e| format!("创建目录 {} 失败：{e}", p.display()))?;
    }
    Ok(())
}

pub(super) fn lilia_config_dir() -> PathBuf {
    crate::store::resolve_lilia_home().join(LILIA_CONFIG_SUBDIR)
}

/// 扫描某个目录下所有子文件夹，跳过隐藏目录和非目录条目。
pub(super) fn list_subdirs(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let Ok(ft) = ent.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        out.push((name, ent.path()));
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    out
}

pub(super) fn sanitize_extension_name(raw: &str, label: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} 名称不能为空"));
    }
    if trimmed.len() > 64 {
        return Err(format!("{label} 名称太长（>64 字符）"));
    }
    // 避免穿目录或写非法字符到 frontmatter。
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("{label} 名称只能包含 a-z / A-Z / 0-9 / - / _"));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_path_traversal() {
        assert!(sanitize_extension_name("../escape", "skill").is_err());
        assert!(sanitize_extension_name("ok-name_1", "skill").is_ok());
        assert!(sanitize_extension_name("", "skill").is_err());
        assert!(sanitize_extension_name("ok-plugin", "plugin").is_ok());
    }
}
