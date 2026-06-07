use std::path::Path;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use keyring_core::{Entry, Error as KeyringError};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, LINK, USER_AGENT};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::project_shell::{
    derive_repo_dir_name, load_project_settings, run_git_clone, save_project_settings,
    unique_target_path, GitHubBindingMetadata,
};
use crate::util::now_millis;

const GITHUB_CLIENT_ID: &str = "Ov23liJWTEjz4jgqx19u";
const GITHUB_SCOPE: &str = "repo read:user";
const GITHUB_SERVICE: &str = "com.lilia.desktop.github";
const GITHUB_ACCEPT: &str = "application/vnd.github+json";
const GITHUB_USER_AGENT: &str = "LiliaCode/0.1";
const GITHUB_CLIENT_ID_SOURCE_BUNDLED: &str = "bundled";
const GITHUB_CLIENT_ID_SOURCE_NONE: &str = "none";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubBindingStatus {
    pub state: String,
    pub client_id_configured: bool,
    pub client_id_source: String,
    pub binding: Option<GitHubBindingMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubDeviceFlowStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: i64,
    pub interval_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubDeviceFlowPollResult {
    pub status: String,
    pub interval_seconds: i64,
    pub binding_status: Option<GitHubBindingStatus>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepoSummary {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner_login: String,
    pub private: bool,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    pub updated_at: String,
    pub clone_url: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepoPage {
    pub items: Vec<GitHubRepoSummary>,
    pub next_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: i64,
    interval: i64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    scope: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
    login: String,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoOwnerResponse {
    login: String,
}

#[derive(Debug, Deserialize)]
struct RepoResponse {
    id: u64,
    name: String,
    full_name: String,
    private: bool,
    description: Option<String>,
    default_branch: Option<String>,
    updated_at: String,
    clone_url: String,
    html_url: String,
    owner: RepoOwnerResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedGitHubRepo {
    owner: String,
    name: String,
    full_name: String,
    clone_url: String,
}

struct KeyringGuard;

impl Drop for KeyringGuard {
    fn drop(&mut self) {
        keyring::release_store();
    }
}

fn client_id() -> Option<&'static str> {
    let trimmed = GITHUB_CLIENT_ID.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn client_id_source() -> &'static str {
    if client_id().is_some() {
        GITHUB_CLIENT_ID_SOURCE_BUNDLED
    } else {
        GITHUB_CLIENT_ID_SOURCE_NONE
    }
}

fn build_binding_status(binding: Option<GitHubBindingMetadata>) -> GitHubBindingStatus {
    GitHubBindingStatus {
        state: if binding.is_some() {
            "bound".to_string()
        } else {
            "unbound".to_string()
        },
        client_id_configured: client_id().is_some(),
        client_id_source: client_id_source().to_string(),
        binding,
    }
}

pub(crate) fn build_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| format!("构造 GitHub HTTP 客户端失败：{e}"))
}

fn init_keyring() -> Result<KeyringGuard, String> {
    keyring::use_native_store(true).map_err(|e| format!("系统钥匙串不可用：{e}"))?;
    Ok(KeyringGuard)
}

fn keyring_entry(login: &str) -> Result<Entry, String> {
    Entry::new(GITHUB_SERVICE, login).map_err(|e| format!("创建 GitHub 凭证项失败：{e}"))
}

fn read_token(login: &str) -> Result<Option<String>, String> {
    let _guard = init_keyring()?;
    let entry = keyring_entry(login)?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) => Err(format!("读取 GitHub 凭证失败：{err}")),
    }
}

fn write_token(login: &str, token: &str) -> Result<(), String> {
    let _guard = init_keyring()?;
    let entry = keyring_entry(login)?;
    entry
        .set_password(token)
        .map_err(|e| format!("保存 GitHub 凭证失败：{e}"))
}

fn delete_token(login: &str) -> Result<(), String> {
    let _guard = init_keyring()?;
    let entry = keyring_entry(login)?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(err) => Err(format!("删除 GitHub 凭证失败：{err}")),
    }
}

fn normalize_scope_list(scope: Option<&str>) -> Vec<String> {
    scope
        .unwrap_or("")
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().to_string())
        .collect()
}

pub(crate) fn reconcile_binding(
    app: &AppHandle,
    token_required: bool,
) -> Result<(Option<GitHubBindingMetadata>, Option<String>), String> {
    let mut settings = load_project_settings(app);
    let Some(binding) = settings.github_binding.clone() else {
        return Ok((None, None));
    };
    let token = read_token(&binding.login)?;
    if token_required && token.is_none() {
        settings.github_binding = None;
        save_project_settings(app, &settings)?;
        return Ok((None, None));
    }
    Ok((Some(binding), token))
}

pub(crate) fn github_request_headers(
    builder: reqwest::blocking::RequestBuilder,
    token: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    let builder = builder
        .header(USER_AGENT, GITHUB_USER_AGENT)
        .header(ACCEPT, GITHUB_ACCEPT)
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = token {
        builder.bearer_auth(token)
    } else {
        builder
    }
}

fn github_auth_header(token: &str) -> String {
    let encoded = STANDARD.encode(format!("x-access-token:{token}"));
    format!("AUTHORIZATION: basic {encoded}")
}

fn normalize_github_repo_input(input: &str) -> Result<NormalizedGitHubRepo, String> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("仓库输入不能为空".to_string());
    }

    let path = if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://github.com/") {
        rest
    } else {
        trimmed
    };

    let path = path.trim_end_matches(".git");
    let parts = path
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();

    if parts.len() != 2 {
        return Err("请输入 owner/repo 或 https://github.com/owner/repo.git".to_string());
    }

    let owner = parts[0].trim();
    let name = parts[1].trim();
    if owner.is_empty() || name.is_empty() {
        return Err("请输入 owner/repo 或 https://github.com/owner/repo.git".to_string());
    }

    Ok(NormalizedGitHubRepo {
        owner: owner.to_string(),
        name: name.to_string(),
        full_name: format!("{owner}/{name}"),
        clone_url: format!("https://github.com/{owner}/{name}.git"),
    })
}

fn parse_next_page(link: Option<&str>) -> Option<u32> {
    let link = link?;
    for part in link.split(',') {
        if !part.contains("rel=\"next\"") {
            continue;
        }
        let page_part = part.split('?').nth(1)?;
        let query = page_part.split('>').next()?;
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=')?;
            if key == "page" {
                if let Ok(page) = value.parse::<u32>() {
                    return Some(page);
                }
            }
        }
    }
    None
}

fn store_binding(
    app: &AppHandle,
    user: GitHubUserResponse,
    scopes: Vec<String>,
    token: &str,
) -> Result<GitHubBindingStatus, String> {
    let mut settings = load_project_settings(app);
    let previous_login = settings.github_binding.as_ref().map(|binding| binding.login.clone());
    let binding = GitHubBindingMetadata {
        login: user.login.clone(),
        avatar_url: user.avatar_url.clone(),
        bound_at: now_millis(),
        scopes,
        client_id_source: GITHUB_CLIENT_ID_SOURCE_BUNDLED.to_string(),
    };

    write_token(&user.login, token)?;
    settings.github_binding = Some(binding.clone());
    save_project_settings(app, &settings)?;

    if let Some(previous_login) = previous_login {
        if previous_login != user.login {
            let _ = delete_token(&previous_login);
        }
    }

    Ok(build_binding_status(Some(binding)))
}

#[tauri::command]
pub fn github_get_binding_status(app: AppHandle) -> Result<GitHubBindingStatus, String> {
    let (binding, _) = reconcile_binding(&app, true)?;
    Ok(build_binding_status(binding))
}

#[tauri::command]
pub fn github_start_device_flow() -> Result<GitHubDeviceFlowStart, String> {
    let Some(client_id) = client_id() else {
        return Err("GitHub Client ID 未配置".to_string());
    };
    let client = build_client()?;
    let response = client
        .post("https://github.com/login/device/code")
        .header(USER_AGENT, GITHUB_USER_AGENT)
        .header(ACCEPT, "application/json")
        .form(&[("client_id", client_id), ("scope", GITHUB_SCOPE)])
        .send()
        .map_err(|e| format!("启动 GitHub 设备授权失败：{e}"))?;

    if !response.status().is_success() {
        return Err(format!("启动 GitHub 设备授权失败：HTTP {}", response.status()));
    }

    let body = response
        .json::<DeviceCodeResponse>()
        .map_err(|e| format!("解析 GitHub 设备授权响应失败：{e}"))?;

    Ok(GitHubDeviceFlowStart {
        device_code: body.device_code,
        user_code: body.user_code,
        verification_uri: body.verification_uri,
        expires_at: now_millis() + body.expires_in * 1000,
        interval_seconds: body.interval,
    })
}

#[tauri::command]
pub fn github_poll_device_flow(
    app: AppHandle,
    device_code: String,
    interval_seconds: Option<i64>,
) -> Result<GitHubDeviceFlowPollResult, String> {
    let Some(client_id) = client_id() else {
        return Err("GitHub Client ID 未配置".to_string());
    };
    let next_interval = interval_seconds.unwrap_or(5).max(1);
    let client = build_client()?;
    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header(USER_AGENT, GITHUB_USER_AGENT)
        .header(ACCEPT, "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code.trim()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .map_err(|e| format!("轮询 GitHub 设备授权失败：{e}"))?;

    if !response.status().is_success() {
        return Err(format!("轮询 GitHub 设备授权失败：HTTP {}", response.status()));
    }

    let body = response
        .json::<TokenResponse>()
        .map_err(|e| format!("解析 GitHub 设备授权结果失败：{e}"))?;

    if let Some(token) = body.access_token {
        let scopes = normalize_scope_list(body.scope.as_deref());
        let user = github_request_headers(client.get("https://api.github.com/user"), Some(&token))
            .send()
            .map_err(|e| format!("读取 GitHub 账号信息失败：{e}"))?;

        if !user.status().is_success() {
            return Err(format!("读取 GitHub 账号信息失败：HTTP {}", user.status()));
        }

        let user = user
            .json::<GitHubUserResponse>()
            .map_err(|e| format!("解析 GitHub 账号信息失败：{e}"))?;
        let binding_status = store_binding(&app, user, scopes, &token)?;
        return Ok(GitHubDeviceFlowPollResult {
            status: "authorized".to_string(),
            interval_seconds: next_interval,
            binding_status: Some(binding_status),
            error: None,
        });
    }

    let error = body.error.unwrap_or_else(|| "unknown_error".to_string());
    if error == "authorization_pending" {
        return Ok(GitHubDeviceFlowPollResult {
            status: "pending".to_string(),
            interval_seconds: next_interval,
            binding_status: None,
            error: None,
        });
    }
    if error == "slow_down" {
        return Ok(GitHubDeviceFlowPollResult {
            status: "pending".to_string(),
            interval_seconds: next_interval + 5,
            binding_status: None,
            error: None,
        });
    }

    Ok(GitHubDeviceFlowPollResult {
        status: "expired".to_string(),
        interval_seconds: next_interval,
        binding_status: None,
        error: Some(error),
    })
}

#[tauri::command]
pub fn github_unbind(app: AppHandle) -> Result<(), String> {
    let mut settings = load_project_settings(&app);
    if let Some(binding) = settings.github_binding.as_ref() {
        delete_token(&binding.login)?;
    }
    settings.github_binding = None;
    save_project_settings(&app, &settings)
}

#[tauri::command]
pub fn github_list_repos(app: AppHandle, page: Option<u32>) -> Result<GitHubRepoPage, String> {
    let page = page.unwrap_or(1).max(1);
    let (binding, token) = reconcile_binding(&app, true)?;
    let Some(binding) = binding else {
        return Err("请先绑定 GitHub".to_string());
    };
    let Some(token) = token else {
        return Err("GitHub 绑定已失效，请重新绑定".to_string());
    };

    let client = build_client()?;
    let response = github_request_headers(
        client.get("https://api.github.com/user/repos").query(&[
            ("affiliation", "owner"),
            ("visibility", "all"),
            ("sort", "updated"),
            ("per_page", "100"),
            ("page", &page.to_string()),
        ]),
        Some(&token),
    )
    .send()
    .map_err(|e| format!("读取 GitHub 仓库失败：{e}"))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(format!(
            "GitHub 绑定已失效，请重新绑定（账号 {}）",
            binding.login
        ));
    }

    if !response.status().is_success() {
        return Err(format!(
            "读取 GitHub 仓库失败：HTTP {}（账号 {}）",
            response.status(),
            binding.login
        ));
    }

    let next_page = parse_next_page(
        response
            .headers()
            .get(LINK)
            .and_then(|value| value.to_str().ok()),
    );
    let repos = response
        .json::<Vec<RepoResponse>>()
        .map_err(|e| format!("解析 GitHub 仓库列表失败：{e}"))?;

    Ok(GitHubRepoPage {
        items: repos
            .into_iter()
            .map(|repo| GitHubRepoSummary {
                id: repo.id,
                name: repo.name,
                full_name: repo.full_name,
                owner_login: repo.owner.login,
                private: repo.private,
                description: repo.description,
                default_branch: repo.default_branch,
                updated_at: repo.updated_at,
                clone_url: repo.clone_url,
                html_url: repo.html_url,
            })
            .collect(),
        next_page,
    })
}

#[tauri::command]
pub fn github_clone_repo(
    app: AppHandle,
    repo: String,
    parent_dir: String,
) -> Result<String, String> {
    let parent_path = Path::new(parent_dir.trim());
    if !parent_path.is_dir() {
        return Err(format!("目标父目录不存在：{}", parent_path.display()));
    }

    let normalized = normalize_github_repo_input(&repo)?;
    let target = unique_target_path(parent_path, &derive_repo_dir_name(&normalized.clone_url));

    let auth_header = match reconcile_binding(&app, true) {
        Ok((Some(_), Some(token))) => Some(github_auth_header(&token)),
        Ok(_) => None,
        Err(err) => return Err(err),
    };
    run_git_clone(
        &normalized.clone_url,
        &target,
        auth_header.as_deref(),
    )?;

    Ok(target.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_scope_list_splits_commas_and_spaces() {
        assert_eq!(
            normalize_scope_list(Some("repo, read:user workflow")),
            vec!["repo", "read:user", "workflow"]
        );
    }

    #[test]
    fn normalize_github_repo_input_accepts_owner_repo() {
        let normalized = normalize_github_repo_input("sena-nana/Lilia").unwrap();
        assert_eq!(normalized.owner, "sena-nana");
        assert_eq!(normalized.name, "Lilia");
        assert_eq!(normalized.full_name, "sena-nana/Lilia");
        assert_eq!(normalized.clone_url, "https://github.com/sena-nana/Lilia.git");
    }

    #[test]
    fn normalize_github_repo_input_accepts_https_url() {
        let normalized =
            normalize_github_repo_input("https://github.com/sena-nana/Lilia.git").unwrap();
        assert_eq!(normalized.full_name, "sena-nana/Lilia");
    }

    #[test]
    fn normalize_github_repo_input_rejects_invalid_values() {
        assert!(normalize_github_repo_input("sena-nana").is_err());
        assert!(normalize_github_repo_input("https://example.com/foo/bar").is_err());
    }

    #[test]
    fn parse_next_page_reads_link_header() {
        let link = r#"<https://api.github.com/user/repos?page=2>; rel="next", <https://api.github.com/user/repos?page=4>; rel="last""#;
        assert_eq!(parse_next_page(Some(link)), Some(2));
        assert_eq!(parse_next_page(None), None);
    }

    #[test]
    fn github_auth_header_uses_basic_prefix() {
        let header = github_auth_header("secret-token");
        assert!(header.starts_with("AUTHORIZATION: basic "));
    }
}
