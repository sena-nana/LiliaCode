use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Deserialize;

use super::codex_probe::{
    build_codex_app_server_probe_status_cached, managed_codex_install_dir, parse_codex_cli_version,
};
use super::types::CodexAppServerStatus;

const CODEX_NPM_LATEST_URL: &str = "https://registry.npmjs.org/@openai%2Fcodex/latest";
const CODEX_GITHUB_RELEASE_URL_PREFIX: &str =
    "https://api.github.com/repos/openai/codex/releases/tags/rust-v";
const CODEX_INSTALL_SCRIPT_PS1: &str = "https://chatgpt.com/codex/install.ps1";
const CODEX_INSTALL_SCRIPT_SH: &str = "https://chatgpt.com/codex/install.sh";

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

fn powershell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub(crate) fn codex_install_command_spec(install_dir: &Path) -> CodexInstallCommandSpec {
    let install_dir = install_dir.to_string_lossy().to_string();
    let env = vec![
        ("CODEX_NON_INTERACTIVE".to_string(), "1".to_string()),
        ("CODEX_INSTALL_DIR".to_string(), install_dir.clone()),
    ];
    if cfg!(windows) {
        let script = format!(
            "$ErrorActionPreference='Stop'; $env:CODEX_NON_INTERACTIVE='1'; $env:CODEX_INSTALL_DIR={}; irm {} | iex",
            powershell_single_quoted(&install_dir),
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

pub(crate) fn install_or_update_codex_app_server() -> Result<CodexAppServerStatus, String> {
    let install_dir = managed_codex_install_dir();
    fs::create_dir_all(&install_dir)
        .map_err(|err| format!("创建 Codex 安装目录 {} 失败：{err}", install_dir.display()))?;
    let spec = codex_install_command_spec(&install_dir);
    run_install_command(&spec)?;
    let status = build_codex_app_server_probe_status_cached(true).public;
    Ok(check_codex_app_server_update_status_for(status))
}

fn check_codex_app_server_update_status_for(status: CodexAppServerStatus) -> CodexAppServerStatus {
    let Ok(client) = http_client() else {
        return enrich_codex_status_with_update(
            status,
            || Err("创建 HTTP 客户端失败。".to_string()),
            |_| Ok(Vec::new()),
        );
    };
    enrich_codex_status_with_update(
        status,
        || fetch_latest_version_with(&client),
        |version| fetch_release_notes_with(&client, version),
    )
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
        }
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
        let spec = codex_install_command_spec(Path::new("C:/Users/me/.lilia/runtime/codex/bin"));

        assert!(spec
            .env
            .contains(&("CODEX_NON_INTERACTIVE".to_string(), "1".to_string())));
        assert!(spec.env.iter().any(|(key, value)| {
            key == "CODEX_INSTALL_DIR" && value.contains("runtime/codex/bin")
        }));
    }
}
