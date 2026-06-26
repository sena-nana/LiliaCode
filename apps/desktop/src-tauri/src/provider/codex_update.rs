use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;

use serde::Deserialize;

use crate::process_command::hide_console_window;

use super::codex_probe::{
    build_codex_app_server_probe_status_cached, codex_candidate_filenames, command_output_result,
    managed_codex_home_dir, managed_codex_install_dir, parse_codex_cli_version,
};
use super::types::CodexAppServerStatus;

const CODEX_NPM_LATEST_URL: &str = "https://registry.npmjs.org/@openai%2Fcodex/latest";
const CODEX_GITHUB_RELEASE_URL_PREFIX: &str =
    "https://api.github.com/repos/openai/codex/releases/tags/rust-v";
const CODEX_INSTALL_SCRIPT_PS1: &str = "https://chatgpt.com/codex/install.ps1";
const CODEX_INSTALL_SCRIPT_SH: &str = "https://chatgpt.com/codex/install.sh";
const CODEX_UPDATE_STATE_IDLE: &str = "idle";
const CODEX_UPDATE_STATE_AVAILABLE: &str = "available";
const CODEX_UPDATE_STATE_DOWNLOADING: &str = "downloading";
const CODEX_UPDATE_STATE_READY: &str = "ready";
const CODEX_UPDATE_STATE_FAILED: &str = "failed";

#[derive(Debug, Deserialize)]
struct NpmLatestPackage {
    version: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexInstallCommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedCodexUpdate {
    version: String,
    install_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CodexUpdateManagerState {
    Idle,
    Downloading(String),
    Ready(String),
    Failed { version: String, error: String },
}

fn codex_update_manager() -> &'static Mutex<CodexUpdateManagerState> {
    static MANAGER: OnceLock<Mutex<CodexUpdateManagerState>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(CodexUpdateManagerState::Idle))
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("Lilia Codex app-server updater")
        .build()
        .map_err(|err| format!("创建 HTTP 客户端失败：{err}"))
}

fn fetch_latest_version_with(client: &reqwest::blocking::Client) -> Result<String, String> {
    let pkg = client
        .get(CODEX_NPM_LATEST_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("查询 Codex 最新版本失败：{err}"))?
        .json::<NpmLatestPackage>()
        .map_err(|err| format!("解析 Codex 最新版本失败：{err}"))?;
    let version = pkg.version.trim();
    if version.is_empty() {
        Err("Codex 最新版本为空。".to_string())
    } else {
        Ok(version.to_string())
    }
}

fn fetch_release_notes_with(
    client: &reqwest::blocking::Client,
    version: &str,
) -> Result<Vec<String>, String> {
    let url = format!("{CODEX_GITHUB_RELEASE_URL_PREFIX}{version}");
    let release = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("查询 Codex 更新内容失败：{err}"))?
        .json::<GithubRelease>()
        .map_err(|err| format!("解析 Codex 更新内容失败：{err}"))?;
    Ok(summarize_release_notes(
        release.body.as_deref().unwrap_or(""),
    ))
}

pub(crate) fn summarize_release_notes(body: &str) -> Vec<String> {
    body.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("- ").map(str::trim))
        .filter(|line| !line.is_empty())
        .take(5)
        .map(ToString::to_string)
        .collect()
}

fn parsed_version(value: Option<&str>) -> Option<(u32, u32, u32)> {
    value.and_then(parse_codex_cli_version)
}

pub(crate) fn enrich_codex_status_with_update<F, G>(
    mut status: CodexAppServerStatus,
    mut fetch_latest_version: F,
    mut fetch_release_notes: G,
) -> CodexAppServerStatus
where
    F: FnMut() -> Result<String, String>,
    G: FnMut(&str) -> Result<Vec<String>, String>,
{
    match fetch_latest_version() {
        Ok(latest) => {
            let current = parsed_version(status.version.as_deref());
            let latest_parsed = parse_codex_cli_version(&latest);
            status.latest_version = Some(latest.clone());
            status.update_available = latest_parsed.is_some()
                && (!status.managed
                    || current
                        .map(|version| version < latest_parsed.unwrap())
                        .unwrap_or(true));
            status.release_notes = fetch_release_notes(&latest).unwrap_or_default();
            status.update_error = None;
        }
        Err(err) => {
            status.latest_version = None;
            status.update_available = false;
            status.release_notes = Vec::new();
            status.update_error = Some(err);
        }
    }
    status
}

pub(crate) fn check_codex_app_server_update_status() -> CodexAppServerStatus {
    let status = build_codex_app_server_probe_status_cached(false).public;
    check_codex_app_server_update_status_for(status)
}

fn sanitized_version_path(version: &str) -> String {
    version
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn prepared_codex_root(version: &str) -> PathBuf {
    managed_codex_home_dir()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prepared")
        .join(sanitized_version_path(version))
}

fn prepared_codex_update(version: &str) -> PreparedCodexUpdate {
    let root = prepared_codex_root(version);
    PreparedCodexUpdate {
        version: version.to_string(),
        install_dir: root.join("bin"),
    }
}

fn apply_update_state(mut status: CodexAppServerStatus) -> CodexAppServerStatus {
    status.update_state = if status.update_available {
        CODEX_UPDATE_STATE_AVAILABLE.to_string()
    } else {
        CODEX_UPDATE_STATE_IDLE.to_string()
    };
    status.prepared_version = None;

    let latest = status.latest_version.clone();
    let state = codex_update_manager()
        .lock()
        .map(|state| state.clone())
        .unwrap_or(CodexUpdateManagerState::Idle);
    match (latest.as_deref(), state) {
        (Some(latest), CodexUpdateManagerState::Downloading(version)) if version == latest => {
            status.update_state = CODEX_UPDATE_STATE_DOWNLOADING.to_string();
            status.prepared_version = Some(version);
        }
        (Some(latest), CodexUpdateManagerState::Ready(version)) if version == latest => {
            status.update_state = CODEX_UPDATE_STATE_READY.to_string();
            status.prepared_version = Some(version);
        }
        (Some(latest), CodexUpdateManagerState::Failed { version, error }) if version == latest => {
            status.update_state = CODEX_UPDATE_STATE_FAILED.to_string();
            status.prepared_version = Some(version);
            status.update_error = Some(error);
        }
        _ => {}
    }
    status
}

fn should_start_download_for(version: &str, state: &CodexUpdateManagerState) -> bool {
    match state {
        CodexUpdateManagerState::Downloading(active)
        | CodexUpdateManagerState::Ready(active)
        | CodexUpdateManagerState::Failed {
            version: active, ..
        } => active != version,
        CodexUpdateManagerState::Idle => true,
    }
}

fn maybe_start_codex_update_download(status: &CodexAppServerStatus) {
    if !status.update_available {
        return;
    }
    let Some(version) = status.latest_version.clone() else {
        return;
    };

    {
        let Ok(mut state) = codex_update_manager().lock() else {
            return;
        };
        if !should_start_download_for(&version, &state) {
            return;
        }
        *state = CodexUpdateManagerState::Downloading(version.clone());
    }

    thread::spawn(move || {
        let result = prepare_codex_update(&version);
        if let Ok(mut state) = codex_update_manager().lock() {
            *state = match result {
                Ok(()) => CodexUpdateManagerState::Ready(version),
                Err(error) => CodexUpdateManagerState::Failed { version, error },
            };
        }
    });
}

fn powershell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn comparable_path(path: &Path) -> String {
    let mut text = path
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_string();
    if let Some(stripped) = text.strip_prefix(r"\\?\") {
        text = stripped.to_string();
    }
    if cfg!(windows) {
        text.to_ascii_lowercase()
    } else {
        text
    }
}

fn path_starts_with(candidate: &Path, root: &Path) -> bool {
    let candidate = comparable_path(candidate);
    let root = comparable_path(root);
    candidate == root || candidate.starts_with(&format!("{root}\\"))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn prepare_managed_codex_install_dir(install_dir: &Path, codex_home: &Path) -> Result<(), String> {
    if let Some(metadata) = fs::symlink_metadata(install_dir).map(Some).or_else(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(format!(
                "检查旧 Codex 安装链接 {} 失败：{err}",
                install_dir.display()
            ))
        }
    })? {
        let standalone_root = codex_home.join("packages").join("standalone");
        if is_reparse_point(&metadata)
            && !path_starts_with(
                &fs::read_link(install_dir).map_err(|err| {
                    format!(
                        "读取旧 Codex 安装链接 {} 失败：{err}",
                        install_dir.display()
                    )
                })?,
                &standalone_root,
            )
        {
            fs::remove_dir(install_dir).map_err(|err| {
                format!(
                    "移除旧 Codex 安装链接 {} 失败：{err}",
                    install_dir.display()
                )
            })?;
        }
    }
    fs::create_dir_all(codex_home)
        .map_err(|err| format!("创建 Codex home 目录 {} 失败：{err}", codex_home.display()))?;
    fs::create_dir_all(install_dir)
        .map_err(|err| format!("创建 Codex 安装目录 {} 失败：{err}", install_dir.display()))
}

#[cfg(test)]
fn codex_install_command_spec(install_dir: &Path, codex_home: &Path) -> CodexInstallCommandSpec {
    codex_install_command_spec_for_release(install_dir, codex_home, None)
}

fn codex_install_command_spec_for_release(
    install_dir: &Path,
    codex_home: &Path,
    release: Option<&str>,
) -> CodexInstallCommandSpec {
    let install_dir = install_dir.to_string_lossy().to_string();
    let codex_home = codex_home.to_string_lossy().to_string();
    let mut env = vec![
        ("CODEX_NON_INTERACTIVE".to_string(), "1".to_string()),
        ("CODEX_HOME".to_string(), codex_home.clone()),
        ("CODEX_INSTALL_DIR".to_string(), install_dir.clone()),
    ];
    if let Some(release) = release {
        env.push(("CODEX_RELEASE".to_string(), release.to_string()));
    }
    if cfg!(windows) {
        let release_script = release
            .map(|release| format!(" $env:CODEX_RELEASE={};", powershell_single_quoted(release)))
            .unwrap_or_default();
        let script = format!(
            "$ErrorActionPreference='Stop'; $env:CODEX_NON_INTERACTIVE='1'; $env:CODEX_HOME={}; $env:CODEX_INSTALL_DIR={};{} irm {} | iex",
            powershell_single_quoted(&codex_home),
            powershell_single_quoted(&install_dir),
            release_script,
            CODEX_INSTALL_SCRIPT_PS1,
        );
        CodexInstallCommandSpec {
            program: "powershell.exe".to_string(),
            args: vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Command".to_string(),
                script,
            ],
            env,
        }
    } else {
        CodexInstallCommandSpec {
            program: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                format!("curl -fsSL {CODEX_INSTALL_SCRIPT_SH} | sh"),
            ],
            env,
        }
    }
}

fn run_install_command(spec: &CodexInstallCommandSpec) -> Result<(), String> {
    let mut command = Command::new(&spec.program);
    hide_console_window(&mut command);
    command
        .args(&spec.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|err| format!("启动 Codex 安装器失败：{err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    Err(if detail.is_empty() {
        format!("Codex 安装器退出失败：{}", output.status)
    } else {
        format!("Codex 安装器退出失败：{detail}")
    })
}

fn codex_help_supports_app_server(help: &str) -> bool {
    help.contains("codex app-server") || help.contains("Usage:")
}

fn verify_prepared_codex_update(prepared: &PreparedCodexUpdate) -> Result<(), String> {
    for filename in codex_candidate_filenames() {
        let candidate = prepared.install_dir.join(filename);
        if !candidate.exists() {
            continue;
        }
        let candidate = candidate.to_string_lossy().to_string();
        let version_output = command_output_result(&candidate, &["--version"])
            .map_err(|err| format!("验证已下载 Codex {} 版本失败：{err}", prepared.version))?;
        let parsed = parse_codex_cli_version(&version_output)
            .ok_or_else(|| format!("无法解析已下载 Codex 版本：{version_output}"))?;
        let expected = parse_codex_cli_version(&prepared.version)
            .ok_or_else(|| format!("无法解析 Codex 目标版本：{}", prepared.version))?;
        if parsed != expected {
            return Err(format!(
                "已下载 Codex 版本不匹配：期望 {}，实际 {}",
                prepared.version, version_output
            ));
        }
        let help = command_output_result(&candidate, &["app-server", "--help"])
            .map_err(|err| format!("验证已下载 Codex app-server 失败：{err}"))?;
        if !codex_help_supports_app_server(&help) {
            return Err("已下载 Codex 不支持 app-server。".to_string());
        }
        return Ok(());
    }
    Err(format!(
        "已下载 Codex {} 缺少可执行文件。",
        prepared.version
    ))
}

fn prepare_codex_update(version: &str) -> Result<(), String> {
    let prepared = prepared_codex_update(version);
    let codex_home = prepared_codex_root(version).join("home");
    if let Some(root) = prepared.install_dir.parent().and_then(Path::parent) {
        fs::create_dir_all(root)
            .map_err(|err| format!("创建 Codex 更新缓存目录 {} 失败：{err}", root.display()))?;
    }
    let root = prepared_codex_root(version);
    if root.exists() {
        fs::remove_dir_all(&root)
            .map_err(|err| format!("清理旧 Codex 更新缓存 {} 失败：{err}", root.display()))?;
    }
    prepare_managed_codex_install_dir(&prepared.install_dir, &codex_home)?;
    let spec =
        codex_install_command_spec_for_release(&prepared.install_dir, &codex_home, Some(version));
    run_install_command(&spec)?;
    verify_prepared_codex_update(&prepared)?;
    Ok(())
}

fn active_bin_backup_path(active_bin: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    active_bin.with_file_name(format!(
        "{}.backup.{}",
        active_bin
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("bin"),
        stamp
    ))
}

#[cfg(windows)]
fn create_directory_link(link_path: &Path, target_path: &Path) -> Result<(), String> {
    let script = format!(
        "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path {} -Target {} | Out-Null",
        powershell_single_quoted(&link_path.to_string_lossy()),
        powershell_single_quoted(&target_path.to_string_lossy()),
    );
    let mut command = Command::new("powershell.exe");
    hide_console_window(&mut command);
    let output = command
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("创建 Codex 切换链接失败：{err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(if detail.is_empty() {
            format!("创建 Codex 切换链接失败：{}", output.status)
        } else {
            format!("创建 Codex 切换链接失败：{detail}")
        })
    }
}

#[cfg(not(windows))]
fn create_directory_link(link_path: &Path, target_path: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target_path, link_path).map_err(|err| {
        format!(
            "创建 Codex 切换链接 {} -> {} 失败：{err}",
            link_path.display(),
            target_path.display()
        )
    })
}

fn replace_managed_bin_with_prepared(prepared: &PreparedCodexUpdate) -> Result<(), String> {
    let active_bin = managed_codex_install_dir();
    if let Some(parent) = active_bin.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建 Codex runtime 目录 {} 失败：{err}", parent.display()))?;
    }
    let backup = if active_bin.exists() {
        let metadata = fs::symlink_metadata(&active_bin).map_err(|err| {
            format!(
                "检查当前 Codex 安装入口 {} 失败：{err}",
                active_bin.display()
            )
        })?;
        if is_reparse_point(&metadata) {
            fs::remove_dir(&active_bin).map_err(|err| {
                format!(
                    "移除当前 Codex 安装链接 {} 失败：{err}",
                    active_bin.display()
                )
            })?;
            None
        } else {
            let backup = active_bin_backup_path(&active_bin);
            fs::rename(&active_bin, &backup).map_err(|err| {
                format!(
                    "备份当前 Codex 安装入口 {} 失败：{err}",
                    active_bin.display()
                )
            })?;
            Some(backup)
        }
    } else {
        None
    };

    match create_directory_link(&active_bin, &prepared.install_dir).and_then(|_| {
        verify_prepared_codex_update(&PreparedCodexUpdate {
            version: prepared.version.clone(),
            install_dir: active_bin.clone(),
        })
    }) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_dir(&active_bin);
            if let Some(backup) = backup {
                let _ = fs::rename(&backup, &active_bin);
            }
            Err(err)
        }
    }
}

pub(crate) fn install_or_update_codex_app_server() -> Result<CodexAppServerStatus, String> {
    let version = {
        let mut state = codex_update_manager()
            .lock()
            .map_err(|_| "Codex 更新状态不可用。".to_string())?;
        match state.clone() {
            CodexUpdateManagerState::Ready(version) => {
                *state = CodexUpdateManagerState::Idle;
                version
            }
            CodexUpdateManagerState::Downloading(_) => {
                return Err("Codex app-server 更新仍在后台下载。".to_string());
            }
            CodexUpdateManagerState::Failed { error, .. } => return Err(error),
            _ => return Err("Codex app-server 更新尚未准备好。".to_string()),
        }
    };
    let prepared = prepared_codex_update(&version);

    if let Err(err) = replace_managed_bin_with_prepared(&prepared) {
        if let Ok(mut state) = codex_update_manager().lock() {
            *state = CodexUpdateManagerState::Failed {
                version,
                error: err.clone(),
            };
        }
        return Err(err);
    }
    if let Ok(mut state) = codex_update_manager().lock() {
        *state = CodexUpdateManagerState::Idle;
    }
    let status = build_codex_app_server_probe_status_cached(true).public;
    Ok(check_codex_app_server_update_status_for(status))
}

fn check_codex_app_server_update_status_for(status: CodexAppServerStatus) -> CodexAppServerStatus {
    let Ok(client) = http_client() else {
        return apply_update_state(enrich_codex_status_with_update(
            status,
            || Err("创建 HTTP 客户端失败。".to_string()),
            |_| Ok(Vec::new()),
        ));
    };
    let status = enrich_codex_status_with_update(
        status,
        || fetch_latest_version_with(&client),
        |version| fetch_release_notes_with(&client, version),
    );
    maybe_start_codex_update_download(&status);
    apply_update_state(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(version: Option<&str>, managed: bool) -> CodexAppServerStatus {
        CodexAppServerStatus {
            version: version.map(ToString::to_string),
            install_path: None,
            managed,
            available: version.is_some(),
            supports_required_protocol: version.is_some(),
            failure_kind: None,
            issues: Vec::new(),
            latest_version: None,
            update_available: false,
            release_notes: Vec::new(),
            update_error: None,
            update_state: CODEX_UPDATE_STATE_IDLE.to_string(),
            prepared_version: None,
        }
    }

    fn update_manager_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_update_manager_state<T>(state: CodexUpdateManagerState, run: impl FnOnce() -> T) -> T {
        let _guard = update_manager_test_lock().lock().unwrap();
        *codex_update_manager().lock().unwrap() = state;
        let result = run();
        *codex_update_manager().lock().unwrap() = CodexUpdateManagerState::Idle;
        result
    }

    #[test]
    fn update_status_marks_missing_managed_copy_as_update_available() {
        let updated = enrich_codex_status_with_update(
            status(Some("codex 0.141.0"), false),
            || Ok("0.141.0".to_string()),
            |_| Ok(vec!["notes".to_string()]),
        );

        assert!(updated.update_available);
        assert_eq!(updated.latest_version.as_deref(), Some("0.141.0"));
        assert_eq!(updated.release_notes, vec!["notes"]);
    }

    #[test]
    fn update_status_does_not_mark_current_managed_copy() {
        let updated = enrich_codex_status_with_update(
            status(Some("codex 0.141.0"), true),
            || Ok("0.141.0".to_string()),
            |_| Ok(Vec::new()),
        );

        assert!(!updated.update_available);
    }

    #[test]
    fn update_status_handles_release_note_failure_as_empty_notes() {
        let updated = enrich_codex_status_with_update(
            status(Some("codex 0.140.0"), true),
            || Ok("0.141.0".to_string()),
            |_| Err("release missing".to_string()),
        );

        assert!(updated.update_available);
        assert!(updated.release_notes.is_empty());
        assert!(updated.update_error.is_none());
    }

    #[test]
    fn update_status_records_latest_version_failure() {
        let updated = enrich_codex_status_with_update(
            status(Some("codex 0.140.0"), true),
            || Err("offline".to_string()),
            |_| Ok(Vec::new()),
        );

        assert!(!updated.update_available);
        assert_eq!(updated.update_error.as_deref(), Some("offline"));
    }

    #[test]
    fn release_note_summary_uses_first_bullets() {
        let notes = summarize_release_notes(
            "## New Features\n\n- First item\n- Second item\n\n## Chores\n- Third item",
        );

        assert_eq!(notes, vec!["First item", "Second item", "Third item"]);
    }

    #[test]
    fn install_command_uses_non_interactive_managed_directory() {
        let spec = codex_install_command_spec(
            Path::new("C:/Users/me/.lilia/runtime/codex/bin"),
            Path::new("C:/Users/me/.lilia/runtime/codex/home"),
        );

        assert!(spec
            .env
            .contains(&("CODEX_NON_INTERACTIVE".to_string(), "1".to_string())));
        assert!(spec
            .env
            .iter()
            .any(|(key, value)| key == "CODEX_HOME" && value.contains("runtime/codex/home")));
        assert!(spec.env.iter().any(|(key, value)| {
            key == "CODEX_INSTALL_DIR" && value.contains("runtime/codex/bin")
        }));
    }

    #[test]
    fn install_command_can_pin_release_for_background_download() {
        let spec = codex_install_command_spec_for_release(
            Path::new("C:/Users/me/.lilia/runtime/codex/prepared/0.141.0/bin"),
            Path::new("C:/Users/me/.lilia/runtime/codex/prepared/0.141.0/home"),
            Some("0.141.0"),
        );

        assert!(spec
            .env
            .contains(&("CODEX_RELEASE".to_string(), "0.141.0".to_string())));
        if cfg!(windows) {
            let script = spec.args.last().expect("PowerShell command should exist");
            assert!(script.contains("$env:CODEX_RELEASE='0.141.0'"));
        }
    }

    #[test]
    fn update_state_reports_ready_prepared_version() {
        let mut base = status(Some("codex 0.140.0"), true);
        base.latest_version = Some("0.141.0".to_string());
        base.update_available = true;

        let updated = with_update_manager_state(
            CodexUpdateManagerState::Ready("0.141.0".to_string()),
            || apply_update_state(base),
        );

        assert_eq!(updated.update_state, CODEX_UPDATE_STATE_READY);
        assert_eq!(updated.prepared_version.as_deref(), Some("0.141.0"));
    }

    #[test]
    fn same_version_download_is_not_started_twice() {
        assert!(!should_start_download_for(
            "0.141.0",
            &CodexUpdateManagerState::Downloading("0.141.0".to_string())
        ));
        assert!(should_start_download_for(
            "0.142.0",
            &CodexUpdateManagerState::Downloading("0.141.0".to_string())
        ));
    }

    #[test]
    fn install_update_without_ready_state_does_not_download() {
        let error = with_update_manager_state(CodexUpdateManagerState::Idle, || {
            install_or_update_codex_app_server().unwrap_err()
        });

        assert!(error.contains("尚未准备好"));
    }

    #[test]
    fn windows_install_command_writes_codex_home_and_install_dir() {
        let spec = codex_install_command_spec(
            Path::new("C:/Users/me/.lilia/runtime/codex/bin"),
            Path::new("C:/Users/me/.lilia/runtime/codex/home"),
        );
        if !cfg!(windows) {
            return;
        }

        let script = spec.args.last().expect("PowerShell command should exist");
        assert!(script.contains("$env:CODEX_HOME='C:/Users/me/.lilia/runtime/codex/home'"));
        assert!(script.contains("$env:CODEX_INSTALL_DIR='C:/Users/me/.lilia/runtime/codex/bin'"));
    }

    #[test]
    fn old_managed_bin_link_target_outside_lilia_standalone_root_is_removed() {
        let standalone_root =
            Path::new("C:/Users/me/.lilia/runtime/codex/home/packages/standalone");

        assert!(!path_starts_with(
            Path::new("C:/Users/me/.codex/packages/standalone/current/bin"),
            standalone_root,
        ));
        assert!(path_starts_with(
            Path::new("C:/Users/me/.lilia/runtime/codex/home/packages/standalone/current/bin"),
            standalone_root,
        ));
        assert!(!path_starts_with(
            Path::new("C:/Users/me/.lilia/runtime/codex/home/packages/standalone-old/current/bin"),
            standalone_root,
        ));
    }
}
