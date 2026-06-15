use std::process::Command;

use serde_json::Value as JsonValue;
use tauri::AppHandle;

use crate::{github, project_shell::GitHubBindingMetadata};

use super::generation::{compact_line, truncate_chars};
use super::types::{
    GitHubActivitySample, GitHubRepoRef, GITHUB_ACTIVITY_LIMIT, GITHUB_EVENT_FETCH_LIMIT,
    SAMPLE_TEXT_LIMIT,
};

pub(super) fn load_github_activity_context(
    app: &AppHandle,
    cwd: &str,
) -> Result<Option<(GitHubRepoRef, Vec<GitHubActivitySample>)>, String> {
    let Some(repo) = resolve_github_repo_from_cwd(cwd)? else {
        return Ok(None);
    };
    let (binding, token) = github::reconcile_binding(app, true)?;
    let Some(binding) = binding else {
        return Ok(None);
    };
    let Some(token) = token else {
        return Ok(None);
    };
    let activities = fetch_github_repo_activities(&repo, &binding, &token)?;
    if activities.is_empty() {
        return Ok(None);
    }
    Ok(Some((repo, activities)))
}

fn resolve_github_repo_from_cwd(cwd: &str) -> Result<Option<GitHubRepoRef>, String> {
    for key in ["remote.origin.url", "remote.upstream.url"] {
        if let Some(value) = git_config_value(cwd, key)? {
            if let Some(repo) = parse_github_repo_url(&value) {
                return Ok(Some(repo));
            }
        }
    }
    Ok(None)
}

fn git_config_value(cwd: &str, key: &str) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .arg("config")
        .arg("--get")
        .arg(key)
        .output()
        .map_err(|e| format!("读取 Git remote 失败：{e}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!value.is_empty()).then_some(value))
}

pub(super) fn parse_github_repo_url(input: &str) -> Option<GitHubRepoRef> {
    let trimmed = input.trim().trim_end_matches('/');
    let path = if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        rest
    } else {
        return None;
    };
    let path = path.trim_end_matches(".git").trim_end_matches('/');
    let parts = path
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    let owner = parts[0].trim();
    let name = parts[1].trim();
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(GitHubRepoRef {
        owner: owner.to_string(),
        name: name.to_string(),
        full_name: format!("{owner}/{name}"),
    })
}

fn fetch_github_repo_activities(
    repo: &GitHubRepoRef,
    binding: &GitHubBindingMetadata,
    token: &str,
) -> Result<Vec<GitHubActivitySample>, String> {
    let client = github::build_client()?;
    let url = format!(
        "https://api.github.com/repos/{}/{}/events",
        repo.owner, repo.name
    );
    let response = github::github_request_headers(
        client
            .get(url)
            .query(&[("per_page", GITHUB_EVENT_FETCH_LIMIT.to_string())]),
        Some(token),
    )
    .send()
    .map_err(|e| format!("读取 GitHub 仓库活动失败：{e}"))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(format!("GitHub 绑定已失效（账号 {}）", binding.login));
    }
    if !response.status().is_success() {
        return Err(format!(
            "读取 GitHub 仓库活动失败：HTTP {}（{}）",
            response.status(),
            repo.full_name
        ));
    }
    let events = response
        .json::<Vec<JsonValue>>()
        .map_err(|e| format!("解析 GitHub 仓库活动失败：{e}"))?;
    Ok(normalize_github_events(repo, &events))
}

pub(super) fn normalize_github_events(
    repo: &GitHubRepoRef,
    events: &[JsonValue],
) -> Vec<GitHubActivitySample> {
    events
        .iter()
        .filter_map(|event| normalize_github_event(repo, event))
        .take(GITHUB_ACTIVITY_LIMIT)
        .collect()
}

fn normalize_github_event(repo: &GitHubRepoRef, event: &JsonValue) -> Option<GitHubActivitySample> {
    let id = event.get("id").and_then(|v| v.as_str())?.to_string();
    let event_type = event.get("type").and_then(|v| v.as_str())?;
    let payload = event.get("payload")?;
    match event_type {
        "PullRequestEvent" => {
            normalize_numbered_github_event(repo, id, payload, "pull_request", "pull_request", "PR")
        }
        "IssuesEvent" => {
            normalize_numbered_github_event(repo, id, payload, "issue", "issue", "Issue")
        }
        "PushEvent" => normalize_push_event(repo, id, payload),
        _ => None,
    }
}

fn normalize_numbered_github_event(
    repo: &GitHubRepoRef,
    event_id: String,
    payload: &JsonValue,
    payload_key: &str,
    kind: &str,
    label: &str,
) -> Option<GitHubActivitySample> {
    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("updated");
    let subject = payload.get(payload_key)?;
    let number = subject.get("number").and_then(|v| v.as_i64())?;
    let title = compact_line(subject.get("title").and_then(|v| v.as_str()).unwrap_or(""));
    if title.is_empty() {
        return None;
    }
    let state = subject
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let url = subject
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let title = format!("{label} #{number}: {title}");
    let fingerprint = format!(
        "{}|{}|{}|{}|{}|{}",
        event_id, kind, action, number, title, state
    );
    Some(GitHubActivitySample {
        id: format!("gh-{event_id}"),
        repo_full_name: repo.full_name.clone(),
        kind: kind.to_string(),
        action: action.to_string(),
        title,
        url,
        details: vec![format!("state: {state}")],
        fingerprint,
    })
}

fn normalize_push_event(
    repo: &GitHubRepoRef,
    event_id: String,
    payload: &JsonValue,
) -> Option<GitHubActivitySample> {
    let reference = payload.get("ref").and_then(|v| v.as_str()).unwrap_or("");
    let branch = reference
        .strip_prefix("refs/heads/")
        .unwrap_or(reference)
        .trim();
    let branch = if branch.is_empty() { "unknown" } else { branch };
    let commits = payload
        .get("commits")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let messages = commits
        .iter()
        .filter_map(|commit| {
            commit
                .get("message")
                .and_then(|v| v.as_str())
                .map(compact_line)
                .filter(|message| !message.is_empty())
        })
        .take(3)
        .collect::<Vec<_>>();
    if messages.is_empty() {
        return None;
    }
    let head = payload
        .get("head")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let title = format!("Push {branch}: {} 个 commit", commits.len());
    let details = messages
        .iter()
        .map(|message| truncate_chars(message, SAMPLE_TEXT_LIMIT))
        .collect::<Vec<_>>();
    let fingerprint = format!(
        "{}|push|{}|{}|{}",
        event_id,
        branch,
        head,
        messages.join(" / ")
    );
    Some(GitHubActivitySample {
        id: format!("gh-{event_id}"),
        repo_full_name: repo.full_name.clone(),
        kind: "push".to_string(),
        action: "pushed".to_string(),
        title,
        url: Some(format!("https://github.com/{}", repo.full_name)),
        details,
        fingerprint,
    })
}
