use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Runtime};

use crate::agent_timeline::AgentTimelineEventInput;
use crate::chat::contract;
use crate::chat::timeline_sink::persist_and_emit_input;
use crate::chat::types::{
    ChatSlashCommand, ChatSlashCommandSearchResult, ChatSlashCommandSource, DoneEvent,
};
use crate::util::now_millis;

const PROJECT_COMMAND_DIR: &str = ".lilia/commands";

#[derive(Debug, Clone)]
struct ProjectCommandDefinition {
    command: ChatSlashCommand,
    body: String,
    path: PathBuf,
}

fn native_commands() -> Vec<ChatSlashCommand> {
    vec![
        ChatSlashCommand {
            id: "native:help".to_string(),
            name: "help".to_string(),
            title: "显示可用斜杠命令".to_string(),
            description: "列出 Lilia 当前可执行的内置命令和项目命令。".to_string(),
            source: ChatSlashCommandSource::Native,
            parameters: Vec::new(),
        },
        ChatSlashCommand {
            id: "native:status".to_string(),
            name: "status".to_string(),
            title: "显示当前会话状态".to_string(),
            description: "写入当前后端和工作目录状态。".to_string(),
            source: ChatSlashCommandSource::Native,
            parameters: Vec::new(),
        },
    ]
}

fn command_name_from_path(path: &Path) -> Option<String> {
    let name = path.file_stem()?.to_string_lossy().trim().to_string();
    normalize_command_name(&name)
}

fn normalize_command_name(value: &str) -> Option<String> {
    let name = value.trim().trim_start_matches('/').to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }
    Some(name)
}

fn markdown_title_and_body(text: &str, fallback_title: &str) -> (String, String) {
    let mut title = fallback_title.to_string();
    let mut body_lines = Vec::new();
    let mut title_seen = false;
    for line in text.lines() {
        if !title_seen {
            let trimmed = line.trim();
            if let Some(heading) = trimmed.strip_prefix("# ") {
                let heading = heading.trim();
                if !heading.is_empty() {
                    title = heading.to_string();
                    title_seen = true;
                    continue;
                }
            }
        }
        body_lines.push(line);
    }
    (title, body_lines.join("\n").trim().to_string())
}

fn project_command_dir(project_cwd: &str) -> PathBuf {
    Path::new(project_cwd).join(PROJECT_COMMAND_DIR)
}

fn list_project_command_definitions(project_cwd: &str) -> Vec<ProjectCommandDefinition> {
    let dir = project_command_dir(project_cwd);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut commands = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = command_name_from_path(&path) else {
            continue;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let (title, body) = markdown_title_and_body(&text, &name);
        let description = body
            .lines()
            .find_map(|line| {
                let trimmed = line.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            })
            .unwrap_or_else(|| "项目自定义命令".to_string());
        commands.push(ProjectCommandDefinition {
            command: ChatSlashCommand {
                id: format!("project:{name}"),
                name,
                title,
                description,
                source: ChatSlashCommandSource::Project,
                parameters: Vec::new(),
            },
            body,
            path,
        });
    }
    commands.sort_by(|a, b| a.command.name.cmp(&b.command.name));
    commands
}

fn matches_command(command: &ChatSlashCommand, query: &str) -> Option<&'static str> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() || command.name.contains(&query) {
        return Some("name");
    }
    if command.title.to_ascii_lowercase().contains(&query) {
        return Some("title");
    }
    if command.description.to_ascii_lowercase().contains(&query) {
        return Some("description");
    }
    None
}

pub(crate) fn list_slash_commands(
    project_cwd: &str,
    query: &str,
    limit: usize,
) -> Vec<ChatSlashCommandSearchResult> {
    let mut results = Vec::new();
    for command in native_commands() {
        if let Some(matched_by) = matches_command(&command, query) {
            results.push(ChatSlashCommandSearchResult {
                command,
                matched_by: matched_by.to_string(),
            });
        }
    }
    for definition in list_project_command_definitions(project_cwd) {
        if let Some(matched_by) = matches_command(&definition.command, query) {
            results.push(ChatSlashCommandSearchResult {
                command: definition.command,
                matched_by: matched_by.to_string(),
            });
        }
    }
    results.truncate(limit.max(1));
    results
}

#[tauri::command]
pub fn chat_search_slash_commands(
    project_cwd: String,
    query: String,
    limit: Option<usize>,
) -> Vec<ChatSlashCommandSearchResult> {
    list_slash_commands(&project_cwd, &query, limit.unwrap_or(12))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SlashCommandExecution {
    pub(crate) command_id: String,
    pub(crate) name: String,
    pub(crate) source: ChatSlashCommandSource,
    pub(crate) title: String,
    pub(crate) result: String,
}

fn native_command_execution(
    command_id: &str,
    project_cwd: &str,
    backend: &str,
) -> Result<SlashCommandExecution, String> {
    match command_id {
        "native:help" => {
            let project_count = list_project_command_definitions(project_cwd).len();
            let result = format!(
                "内置命令：/help、/status\n项目命令：{} 个\n项目命令文件目录：{}",
                project_count,
                project_command_dir(project_cwd).display()
            );
            Ok(SlashCommandExecution {
                command_id: command_id.to_string(),
                name: "help".to_string(),
                source: ChatSlashCommandSource::Native,
                title: "显示可用斜杠命令".to_string(),
                result,
            })
        }
        "native:status" => Ok(SlashCommandExecution {
            command_id: command_id.to_string(),
            name: "status".to_string(),
            source: ChatSlashCommandSource::Native,
            title: "显示当前会话状态".to_string(),
            result: format!("当前后端：{backend}\n工作目录：{project_cwd}"),
        }),
        _ => Err(format!("未知内置斜杠命令：{command_id}")),
    }
}

fn project_command_execution(
    command_id: &str,
    project_cwd: &str,
) -> Result<SlashCommandExecution, String> {
    let name = command_id
        .strip_prefix("project:")
        .and_then(normalize_command_name)
        .ok_or_else(|| format!("无效项目斜杠命令：{command_id}"))?;
    let definition = list_project_command_definitions(project_cwd)
        .into_iter()
        .find(|candidate| candidate.command.name == name)
        .ok_or_else(|| format!("未找到项目斜杠命令：/{name}"))?;
    let result = if definition.body.trim().is_empty() {
        definition.command.title.clone()
    } else {
        definition.body.clone()
    };
    Ok(SlashCommandExecution {
        command_id: definition.command.id,
        name: definition.command.name,
        source: ChatSlashCommandSource::Project,
        title: definition.command.title,
        result: format!("{}\n\n来源：{}", result, definition.path.display()),
    })
}

pub(crate) fn execute_slash_command(
    command_id: &str,
    project_cwd: &str,
    backend: &str,
) -> Result<SlashCommandExecution, String> {
    if command_id.starts_with("native:") {
        return native_command_execution(command_id, project_cwd, backend);
    }
    if command_id.starts_with("project:") {
        return project_command_execution(command_id, project_cwd);
    }
    Err(format!("未知斜杠命令：{command_id}"))
}

pub(crate) fn persist_and_emit_slash_command_result<R: Runtime>(
    app: &AppHandle<R>,
    task_id: &str,
    turn_id: &str,
    backend: &str,
    execution: &SlashCommandExecution,
) {
    let now = now_millis() as i64;
    persist_and_emit_input(
        app,
        AgentTimelineEventInput {
            id: Some(format!("{task_id}:{turn_id}:slash-command")),
            task_id: task_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            backend: backend.to_string(),
            kind: "command".to_string(),
            status: "success".to_string(),
            title: format!("/{}", execution.name),
            summary: Some(execution.result.clone()),
            payload: json!({
                "command": format!("/{}", execution.name),
                "source": execution.source,
                "title": execution.title,
                "output": execution.result,
                "exitCode": 0,
                "subkind": "slash_command",
            }),
            created_at: Some(now),
            updated_at: Some(now),
        },
    );
}

pub(crate) fn emit_slash_command_done<R: Runtime>(app: &AppHandle<R>, task_id: String) {
    let _ = app.emit(
        contract::done_event_name(),
        DoneEvent {
            task_id,
            session_id: None,
            subtype: Some("slash_command".to_string()),
            rollback: None,
        },
    );
}
