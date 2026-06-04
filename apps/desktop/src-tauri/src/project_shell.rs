use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::provider::CodexProfileSettings;
use crate::settings_store::{load_store_value, save_store_value};

const PROJECT_CLONE_PARENT_KEY: &str = "project.cloneParentDir";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSettings {
    pub(crate) clone_parent_dir: Option<String>,
    #[serde(default)]
    pub(crate) codex_defaults: Option<CodexProfileSettings>,
}

// ---------- Project / Git ----------

pub(crate) fn load_project_settings(app: &AppHandle) -> ProjectSettings {
    load_store_value(app, PROJECT_CLONE_PARENT_KEY)
        // 兼容历史可能存的纯字符串。
        .or_else(|| {
            load_store_value::<String>(app, PROJECT_CLONE_PARENT_KEY).map(|clone_parent_dir| {
                ProjectSettings {
                    clone_parent_dir: Some(clone_parent_dir),
                    codex_defaults: None,
                }
            })
        })
        .unwrap_or_default()
}

#[tauri::command]
pub fn project_get_settings(app: AppHandle) -> ProjectSettings {
    load_project_settings(&app)
}

#[tauri::command]
pub fn project_set_settings(app: AppHandle, settings: ProjectSettings) -> Result<(), String> {
    save_store_value(&app, PROJECT_CLONE_PARENT_KEY, &settings)
}

/// 从 git URL 推断仓库目录名。`https://github.com/foo/bar.git` → `bar`。
pub(crate) fn derive_repo_dir_name(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let stripped = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let last = stripped
        .rsplit(|c| c == '/' || c == ':')
        .next()
        .unwrap_or("");
    let cleaned = last.trim().trim_end_matches('/');
    if cleaned.is_empty() {
        "repo".to_string()
    } else {
        cleaned.to_string()
    }
}

/// 在已有的同级目录里挑一个不冲突的名字：`bar`、`bar-2`、`bar-3`…
pub(crate) fn unique_target_path(parent: &Path, base_name: &str) -> PathBuf {
    let candidate = parent.join(base_name);
    if !candidate.exists() {
        return candidate;
    }
    for i in 2..1024 {
        let p = parent.join(format!("{base_name}-{i}"));
        if !p.exists() {
            return p;
        }
    }
    parent.join(base_name)
}

/// 用系统默认文件管理器打开 `path` 指向的目录/文件。
/// Windows: 资源管理器；macOS: Finder；Linux: xdg-open。
#[tauri::command]
pub fn system_open_path(app: AppHandle, path: String) -> Result<(), String> {
    let p = path.trim();
    if p.is_empty() {
        return Err("路径为空".to_string());
    }
    if !Path::new(p).exists() {
        return Err(format!("路径不存在：{p}"));
    }
    app.opener()
        .open_path(p.to_string(), None::<&str>)
        .map_err(|e| format!("打开路径失败：{e}"))
}

/// 尝试用 VSCode 打开 `path`。
/// PATH 里依次找 `code` / `code.cmd` / `code.exe`；都找不到时返回友好错误。
#[tauri::command]
pub fn system_open_in_vscode(path: String) -> Result<(), String> {
    let p = path.trim();
    if p.is_empty() {
        return Err("路径为空".to_string());
    }
    if !Path::new(p).exists() {
        return Err(format!("路径不存在：{p}"));
    }
    let candidates: &[&str] = if cfg!(windows) {
        &["code.cmd", "code.exe", "code"]
    } else {
        &["code"]
    };
    let mut last_err: Option<String> = None;
    for cmd_name in candidates {
        match Command::new(cmd_name)
            .arg(p)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e.to_string());
                continue;
            }
        }
    }
    Err(format!(
        "未能启动 VSCode（请确认 `code` 命令在 PATH 中；可在 VSCode 内执行 Shell Command: Install 'code' command in PATH）：{}",
        last_err.unwrap_or_else(|| "unknown".to_string())
    ))
}

/// 同步调用 `git clone <url> <target>`；成功后返回 target 绝对路径。
#[tauri::command]
pub fn git_clone_repo(url: String, parent_dir: String) -> Result<String, String> {
    let url_trim = url.trim();
    if url_trim.is_empty() {
        return Err("仓库 URL 不能为空".to_string());
    }
    let parent_path = Path::new(parent_dir.trim());
    if !parent_path.is_dir() {
        return Err(format!("目标父目录不存在：{}", parent_path.display()));
    }
    let base = derive_repo_dir_name(url_trim);
    let target = unique_target_path(parent_path, &base);

    let output = Command::new("git")
        .arg("clone")
        .arg("--progress")
        .arg(url_trim)
        .arg(&target)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("无法启动 git（请确认 git 在 PATH 中）：{e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!(
            "git clone 失败：{}",
            if stderr.trim().is_empty() {
                format!("exit {}", output.status.code().unwrap_or(-1))
            } else {
                stderr.trim().to_string()
            }
        ));
    }

    Ok(target.to_string_lossy().to_string())
}
