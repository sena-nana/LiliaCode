use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use crate::{BACKEND_CODEX, MIN_CODEX_APP_SERVER_VERSION};

use super::types::{CodexAppServerProbeStatus, CodexAppServerStatus};

static CODEX_APP_SERVER_PROBE_CACHE: OnceLock<Mutex<Option<CodexAppServerProbeStatus>>> =
    OnceLock::new();

const CODEX_FAILURE_MISSING_CLI: &str = "missingCli";
const CODEX_FAILURE_APP_SERVER_UNAVAILABLE: &str = "appServerUnavailable";
const CODEX_FAILURE_EXPERIMENTAL_API_UNSUPPORTED: &str = "experimentalApiUnsupported";
const CODEX_FAILURE_PROVIDER_INCOMPATIBLE: &str = "providerIncompatible";

pub(crate) fn cli_available(name: &str) -> bool {
    let candidates: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    for ext in candidates {
        let candidate = format!("{name}{ext}");
        let ok = Command::new(&candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    false
}

pub(crate) fn codex_candidate_filenames() -> &'static [&'static str] {
    if cfg!(windows) {
        &["codex.cmd", "codex.exe", "codex.bat", "codex"]
    } else {
        &["codex"]
    }
}

pub(crate) fn windows_native_codex_paths() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let Some(local_app_data) = env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };
    let bin = PathBuf::from(local_app_data)
        .join("OpenAI")
        .join("Codex")
        .join("bin");
    let mut paths = Vec::new();
    let root_codex = bin.join("codex.exe");
    if root_codex.exists() {
        paths.push(root_codex.to_string_lossy().to_string());
    }
    if let Ok(entries) = fs::read_dir(&bin) {
        for entry in entries.flatten() {
            let path = entry.path().join("codex.exe");
            if path.exists() {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    paths
}

pub(crate) fn normalize_candidate_key(path: &str) -> String {
    if cfg!(windows) {
        path.to_ascii_lowercase()
    } else {
        path.to_string()
    }
}

pub(crate) fn push_unique_candidate(
    candidates: &mut Vec<String>,
    seen: &mut HashSet<String>,
    path: String,
) {
    if path.trim().is_empty() {
        return;
    }
    let key = normalize_candidate_key(path.trim());
    if seen.insert(key) {
        candidates.push(path.trim().to_string());
    }
}

pub(crate) fn is_windows_apps_candidate(path: &str) -> bool {
    cfg!(windows) && path.to_ascii_lowercase().contains("\\windowsapps\\")
}

pub(crate) fn where_codex_candidates() -> Vec<String> {
    let output = Command::new(if cfg!(windows) { "where.exe" } else { "which" })
        .arg("codex")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(crate) fn path_codex_candidates() -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            for filename in codex_candidate_filenames() {
                let candidate = dir.join(filename);
                if candidate.exists() {
                    paths.push(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    paths
}

pub(crate) fn codex_cli_candidate_paths() -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let path_candidates = path_codex_candidates();
    let where_candidates = where_codex_candidates();
    for path in &path_candidates {
        if !is_windows_apps_candidate(path) {
            push_unique_candidate(&mut candidates, &mut seen, path.clone());
        }
    }
    for path in windows_native_codex_paths() {
        push_unique_candidate(&mut candidates, &mut seen, path);
    }
    for path in &where_candidates {
        if !is_windows_apps_candidate(path) {
            push_unique_candidate(&mut candidates, &mut seen, path.clone());
        }
    }
    for candidate in codex_candidate_filenames() {
        push_unique_candidate(&mut candidates, &mut seen, (*candidate).to_string());
    }
    for path in path_candidates.into_iter().chain(where_candidates) {
        if is_windows_apps_candidate(&path) {
            push_unique_candidate(&mut candidates, &mut seen, path);
        }
    }
    candidates
}

pub(crate) fn command_output_result(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !err.is_empty() { err } else { out };
        return Err(if detail.is_empty() {
            format!("command exited with {}", output.status)
        } else {
            detail
        });
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !text.is_empty() {
        return Ok(text);
    }
    let text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if text.is_empty() {
        Err("command produced no output".to_string())
    } else {
        Ok(text)
    }
}

fn codex_app_server_help_available(help: &str) -> bool {
    help.contains("codex app-server") || help.contains("Usage:")
}

pub(crate) fn parse_codex_cli_version(output: &str) -> Option<(u32, u32, u32)> {
    let version = output
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
    let patch_text = parts.next().unwrap_or("0");
    let patch_digits: String = patch_text
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    let patch = patch_digits.parse::<u32>().ok()?;
    Some((major, minor, patch))
}

pub(crate) fn codex_version_at_least(version: (u32, u32, u32), minimum: (u32, u32, u32)) -> bool {
    version >= minimum
}

pub(crate) fn build_codex_app_server_probe_status_with<F>(
    candidates: &[String],
    mut command_output: F,
) -> CodexAppServerProbeStatus
where
    F: FnMut(&str, &[&str]) -> Result<String, String>,
{
    let mut app_server_unavailable: Option<CodexAppServerProbeStatus> = None;
    let mut experimental_unsupported: Option<CodexAppServerProbeStatus> = None;

    for candidate in candidates {
        let version = match command_output(candidate, &["--version"]) {
            Ok(output) => output,
            Err(_) => continue,
        };
        let path = Some(candidate.clone());
        let parsed_version = parse_codex_cli_version(&version);
        let help = match command_output(candidate, &["app-server", "--help"]) {
            Ok(output) => output,
            Err(_) => {
                if app_server_unavailable.is_none() {
                    app_server_unavailable = Some(CodexAppServerProbeStatus {
                        public: CodexAppServerStatus {
                            version: Some(version),
                            available: false,
                            supports_required_protocol: false,
                            failure_kind: Some(CODEX_FAILURE_APP_SERVER_UNAVAILABLE.to_string()),
                            issues: vec!["当前 codex CLI 不支持 app-server 子命令。".to_string()],
                        },
                        path,
                    });
                }
                continue;
            }
        };
        let available = codex_app_server_help_available(&help);
        if !available {
            if app_server_unavailable.is_none() {
                app_server_unavailable = Some(CodexAppServerProbeStatus {
                    public: CodexAppServerStatus {
                        version: Some(version),
                        available: false,
                        supports_required_protocol: false,
                        failure_kind: Some(CODEX_FAILURE_APP_SERVER_UNAVAILABLE.to_string()),
                        issues: vec!["当前 codex CLI 不支持 app-server 子命令。".to_string()],
                    },
                    path,
                });
            }
            continue;
        }

        let version_supported = parsed_version
            .map(|version| codex_version_at_least(version, MIN_CODEX_APP_SERVER_VERSION))
            .unwrap_or(false);
        if version_supported {
            return CodexAppServerProbeStatus {
                public: CodexAppServerStatus {
                    version: Some(version),
                    available: true,
                    supports_required_protocol: true,
                    failure_kind: None,
                    issues: Vec::new(),
                },
                path,
            };
        }

        if experimental_unsupported.is_none() {
            let issue = if parsed_version.is_none() {
                "无法读取 codex CLI 版本，无法确认 experimental API 协议能力。".to_string()
            } else {
                "当前 codex CLI 版本过低，需要 0.128.0 或更新版本以支持 Lilia 的流式事件、工具审批和 AskUser。".to_string()
            };
            experimental_unsupported = Some(CodexAppServerProbeStatus {
                public: CodexAppServerStatus {
                    version: Some(version),
                    available: true,
                    supports_required_protocol: false,
                    failure_kind: Some(CODEX_FAILURE_EXPERIMENTAL_API_UNSUPPORTED.to_string()),
                    issues: vec![issue],
                },
                path,
            });
        }
    }

    if let Some(status) = experimental_unsupported {
        return status;
    }
    if let Some(status) = app_server_unavailable {
        return status;
    }

    CodexAppServerProbeStatus {
        public: CodexAppServerStatus {
            version: None,
            available: false,
            supports_required_protocol: false,
            failure_kind: Some(CODEX_FAILURE_MISSING_CLI.to_string()),
            issues: vec!["未找到 codex CLI。请先安装 Codex CLI 后重新检测。".to_string()],
        },
        path: None,
    }
}

pub(crate) fn build_codex_app_server_probe_status() -> CodexAppServerProbeStatus {
    build_codex_app_server_probe_status_with(&codex_cli_candidate_paths(), command_output_result)
}

pub(crate) fn build_codex_app_server_probe_status_cached(
    force_refresh: bool,
) -> CodexAppServerProbeStatus {
    let cache = CODEX_APP_SERVER_PROBE_CACHE.get_or_init(|| Mutex::new(None));
    if !force_refresh {
        if let Ok(guard) = cache.lock() {
            if let Some(status) = guard.clone() {
                return status;
            }
        }
    }
    let status = build_codex_app_server_probe_status();
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(status.clone());
    }
    status
}

pub(crate) fn codex_send_block_reason(status: &CodexAppServerStatus) -> Option<String> {
    if status.supports_required_protocol {
        return None;
    }

    let detail = if status.issues.is_empty() {
        "Codex app-server 环境不满足。".to_string()
    } else {
        status.issues.join(" ")
    };
    if status.failure_kind.as_deref() == Some(CODEX_FAILURE_PROVIDER_INCOMPATIBLE) {
        return Some(format!(
            "{detail} 请确认 CC-Switch 当前选中的上游 provider 支持 OpenAI Responses API 与 Codex 模型白名单。"
        ));
    }

    Some(format!(
        "{detail} 请安装或升级 Codex CLI 到 0.128.0 或更新版本后重新检测。"
    ))
}

pub(crate) fn validate_backend_ready_for_send(active_backend: &str) -> Result<(), String> {
    if active_backend != BACKEND_CODEX {
        return Ok(());
    }
    let status = build_codex_app_server_probe_status();
    if let Some(reason) = codex_send_block_reason(&status.public) {
        return Err(reason);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_codex_cli_version_accepts_missing_patch_and_suffix() {
        assert_eq!(parse_codex_cli_version("codex 0.128.1"), Some((0, 128, 1)));
        assert_eq!(parse_codex_cli_version("codex 0.129"), Some((0, 129, 0)));
        assert_eq!(
            parse_codex_cli_version("codex 0.128.0-beta.1"),
            Some((0, 128, 0))
        );
        assert_eq!(parse_codex_cli_version("codex unknown"), None);
    }

    #[test]
    fn codex_probe_reports_empty_candidates_as_missing_cli() {
        let status =
            build_codex_app_server_probe_status_with(&[], |_, _| Err("unexpected".to_string()));

        assert!(!status.public.available);
        assert!(!status.public.supports_required_protocol);
        assert!(status.path.is_none());
        assert_eq!(
            status.public.failure_kind.as_deref(),
            Some(CODEX_FAILURE_MISSING_CLI)
        );
        assert!(status.public.issues[0].contains("未找到 codex CLI"));
    }

    #[test]
    fn codex_probe_picks_supported_candidate() {
        let candidates = vec!["old".to_string(), "new".to_string()];
        let status =
            build_codex_app_server_probe_status_with(&candidates, |program, args| {
                match (program, args) {
                    ("old", ["--version"]) => Ok("codex 0.127.0".to_string()),
                    ("old", ["app-server", "--help"]) => Ok("Usage: codex app-server".to_string()),
                    ("new", ["--version"]) => Ok("codex 0.128.0".to_string()),
                    ("new", ["app-server", "--help"]) => Ok("Usage: codex app-server".to_string()),
                    _ => Err("unknown command".to_string()),
                }
            });

        assert_eq!(status.path.as_deref(), Some("new"));
        assert!(status.public.supports_required_protocol);
        assert!(status.public.failure_kind.is_none());
    }

    #[test]
    fn codex_probe_reports_old_cli_as_experimental_api_unsupported() {
        let candidates = vec!["old".to_string()];
        let status = build_codex_app_server_probe_status_with(&candidates, |_, args| match args {
            ["--version"] => Ok("codex 0.127.0".to_string()),
            ["app-server", "--help"] => Ok("Usage: codex app-server".to_string()),
            _ => Err("unknown command".to_string()),
        });

        assert_eq!(status.path.as_deref(), Some("old"));
        assert!(status.public.available);
        assert!(!status.public.supports_required_protocol);
        assert_eq!(
            status.public.failure_kind.as_deref(),
            Some(CODEX_FAILURE_EXPERIMENTAL_API_UNSUPPORTED)
        );
        assert!(status.public.issues.join(" ").contains("版本过低"));
    }

    #[test]
    fn codex_probe_reports_missing_app_server_subcommand() {
        let candidates = vec!["codex".to_string()];
        let status = build_codex_app_server_probe_status_with(&candidates, |_, args| match args {
            ["--version"] => Ok("codex 0.128.0".to_string()),
            ["app-server", "--help"] => Err("unknown subcommand".to_string()),
            _ => Err("unknown command".to_string()),
        });

        assert_eq!(status.path.as_deref(), Some("codex"));
        assert!(!status.public.available);
        assert!(!status.public.supports_required_protocol);
        assert_eq!(
            status.public.failure_kind.as_deref(),
            Some(CODEX_FAILURE_APP_SERVER_UNAVAILABLE)
        );
        assert!(status.public.issues.join(" ").contains("app-server"));
    }
}
