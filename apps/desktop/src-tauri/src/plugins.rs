//! Lilia · 插件 / 技能管理。
//!
//! 这里桥接两套不一样的扩展机制：
//!
//! - **Claude Code**
//!   - Skills：`<root>/.claude/skills/<name>/SKILL.md`，frontmatter 至少有 `name`
//!     和 `description`。Lilia 额外承认一个自定义字段 `disabled: true` 表示关闭——
//!     Claude 官方现在不读这个字段，但我们在启动 agent 子进程时按它决定 pluginRoots。
//!   - Plugins (marketplace beta)：`<root>/.claude/plugins/<name>/plugin.json`。
//!     一期只做只读列出（version / description），安装功能等接 marketplace。
//! - **Codex**
//!   - `~/.codex/config.toml` 的 `[mcp_servers.<name>]` 节。Codex 不区分启用/禁用，
//!     一期做只读 + 「打开配置文件」按钮。
//!
//! 文件解析全部手写（极小子集），避免引入 yaml / toml 依赖让 Cargo.toml 膨胀。
//! 解析失败按「跳过该条目 + 记一条 warning」处理，不让单个坏文件阻塞整个面板。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const CLAUDE_DIR: &str = ".claude";
const SKILLS_SUBDIR: &str = "skills";
const PLUGINS_SUBDIR: &str = "plugins";
const SKILL_FILE: &str = "SKILL.md";
const PLUGIN_MANIFEST: &str = "plugin.json";

const SCOPE_USER: &str = "user";
const SCOPE_PROJECT: &str = "project";

// ---------- 对外契约（与 packages/contracts 同形） ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSkill {
    pub scope: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudePlugin {
    pub scope: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMcpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginsOverview {
    pub claude_user_skills: Vec<ClaudeSkill>,
    pub claude_project_skills: Vec<ClaudeSkill>,
    pub claude_user_plugins: Vec<ClaudePlugin>,
    pub codex_mcp_servers: Vec<CodexMcpServer>,
    pub warnings: Vec<String>,
}

// ---------- 目录定位 ----------

fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|e| format!("无法获取用户主目录：{e}"))
}

/// 给定 scope + 可选 project_cwd，算出 `.claude/<sub>` 的根目录。
fn claude_root_for(
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

fn ensure_dir(p: &Path) -> Result<(), String> {
    if !p.exists() {
        fs::create_dir_all(p).map_err(|e| format!("创建目录 {} 失败：{e}", p.display()))?;
    }
    Ok(())
}

// ---------- Claude Skill ----------

/// 把扫描某个目录下所有子文件夹这件事抽出来：每个子目录返回 (name, full_path)，
/// 跳过隐藏目录和非目录条目。
fn list_subdirs(root: &Path) -> Vec<(String, PathBuf)> {
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

/// 解析 SKILL.md：读 frontmatter（YAML 风格的 `key: value`，不支持嵌套）。
/// 解析失败或 frontmatter 缺失返回 None；上层会附一条 warning。
fn parse_skill_frontmatter(text: &str) -> Option<SkillFrontmatter> {
    let mut lines = text.lines();
    // 必须以 --- 开头
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut fm = SkillFrontmatter::default();
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed.trim() == "---" {
            return Some(fm);
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim();
            let val = v.trim().trim_matches('"').trim_matches('\'');
            match key {
                "name" => fm.name = Some(val.to_string()),
                "description" => fm.description = Some(val.to_string()),
                "disabled" => fm.disabled = matches!(val, "true" | "True" | "yes" | "1"),
                _ => {}
            }
        }
    }
    // 闭合 --- 没找到，按损坏处理。
    None
}

#[derive(Default)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    disabled: bool,
}

pub fn list_claude_skills(
    app: &AppHandle,
    scope: &str,
    project_cwd: Option<&str>,
) -> (Vec<ClaudeSkill>, Vec<String>) {
    let mut warnings = Vec::new();
    let root = match claude_root_for(app, scope, project_cwd, SKILLS_SUBDIR) {
        Ok(p) => p,
        Err(_) => return (Vec::new(), warnings),
    };
    if !root.exists() {
        return (Vec::new(), warnings);
    }

    let mut out = Vec::new();
    for (dir_name, dir_path) in list_subdirs(&root) {
        let skill_file = dir_path.join(SKILL_FILE);
        if !skill_file.exists() {
            // 子目录里没有 SKILL.md：不是 skill，跳过（可能是用户放的素材）。
            continue;
        }
        let text = match fs::read_to_string(&skill_file) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(format!("读取 {} 失败：{e}", skill_file.display()));
                continue;
            }
        };
        let fm = parse_skill_frontmatter(&text).unwrap_or_else(|| {
            warnings.push(format!("{} 缺少有效的 frontmatter", skill_file.display()));
            SkillFrontmatter::default()
        });
        out.push(ClaudeSkill {
            scope: scope.to_string(),
            name: fm.name.unwrap_or(dir_name),
            description: fm.description.unwrap_or_default(),
            enabled: !fm.disabled,
            path: skill_file.to_string_lossy().to_string(),
        });
    }
    (out, warnings)
}

fn sanitize_skill_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("skill 名称不能为空".to_string());
    }
    if trimmed.len() > 64 {
        return Err("skill 名称太长（>64 字符）".to_string());
    }
    // 只允许字母 / 数字 / - / _；避免穿目录或写非法字符到 frontmatter。
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("skill 名称只能包含 a-z / A-Z / 0-9 / - / _".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn create_claude_skill(
    app: &AppHandle,
    scope: &str,
    project_cwd: Option<&str>,
    name: &str,
    description: &str,
) -> Result<ClaudeSkill, String> {
    let name = sanitize_skill_name(name)?;
    let root = claude_root_for(app, scope, project_cwd, SKILLS_SUBDIR)?;
    ensure_dir(&root)?;
    let skill_dir = root.join(&name);
    if skill_dir.exists() {
        return Err(format!("同名 skill 已存在：{name}"));
    }
    ensure_dir(&skill_dir)?;
    let skill_file = skill_dir.join(SKILL_FILE);
    let desc_one_line = description.replace('\n', " ");
    // 用单引号包字符串，避免 description 里有 `:` 时把 YAML 弄成嵌套结构。
    let body = format!(
        "---\nname: {name}\ndescription: '{}'\n---\n\n在这里写 Skill 内容。Claude 会按需读取。\n",
        desc_one_line.replace('\'', "''")
    );
    fs::write(&skill_file, body)
        .map_err(|e| format!("写入 SKILL.md 失败：{e}"))?;
    Ok(ClaudeSkill {
        scope: scope.to_string(),
        name,
        description: desc_one_line,
        enabled: true,
        path: skill_file.to_string_lossy().to_string(),
    })
}

pub fn delete_claude_skill(
    app: &AppHandle,
    scope: &str,
    project_cwd: Option<&str>,
    name: &str,
) -> Result<(), String> {
    let name = sanitize_skill_name(name)?;
    let root = claude_root_for(app, scope, project_cwd, SKILLS_SUBDIR)?;
    let skill_dir = root.join(&name);
    if !skill_dir.exists() {
        return Err(format!("未找到 skill：{name}"));
    }
    fs::remove_dir_all(&skill_dir)
        .map_err(|e| format!("删除 skill 目录失败：{e}"))?;
    Ok(())
}

/// 切 `disabled` 字段：保留原有 frontmatter 其它键 + body 原样。
/// 缺失时往 frontmatter 末尾追加；存在时改值或删行（enabled=true 时删行更整洁）。
pub fn set_claude_skill_enabled(
    app: &AppHandle,
    scope: &str,
    project_cwd: Option<&str>,
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    let name = sanitize_skill_name(name)?;
    let root = claude_root_for(app, scope, project_cwd, SKILLS_SUBDIR)?;
    let skill_file = root.join(&name).join(SKILL_FILE);
    let original = fs::read_to_string(&skill_file)
        .map_err(|e| format!("读取 {} 失败：{e}", skill_file.display()))?;
    let updated = rewrite_disabled_field(&original, !enabled);
    fs::write(&skill_file, updated)
        .map_err(|e| format!("写入 SKILL.md 失败：{e}"))?;
    Ok(())
}

/// 在 frontmatter 段内更新 `disabled` 字段。
/// - want_disabled = true：插入或改成 `disabled: true`
/// - want_disabled = false：删除现有的 `disabled` 行（让文件回到「无该字段」原态）
fn rewrite_disabled_field(text: &str, want_disabled: bool) -> String {
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    // 找到首尾 ---
    let mut start = None;
    let mut end = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "---" {
            continue;
        }
        if start.is_none() {
            start = Some(i);
        } else {
            end = Some(i);
            break;
        }
    }
    let (Some(s), Some(e)) = (start, end) else {
        // 没有有效 frontmatter，重新加一段，仅为了承载 disabled 状态。
        let mut head = String::from("---\n");
        if want_disabled {
            head.push_str("disabled: true\n");
        }
        head.push_str("---\n");
        head.push_str(text);
        return head;
    };
    let mut found = None;
    for i in (s + 1)..e {
        if lines[i].trim_start().starts_with("disabled:") {
            found = Some(i);
            break;
        }
    }
    match (found, want_disabled) {
        (Some(i), true) => lines[i] = "disabled: true".to_string(),
        (Some(i), false) => {
            lines.remove(i);
        }
        (None, true) => {
            lines.insert(e, "disabled: true".to_string());
        }
        (None, false) => {}
    }
    let mut out = lines.join("\n");
    // 保留末尾换行（原文有的话）。
    if text.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

// ---------- Claude Plugin (marketplace beta) ----------

#[derive(Debug, Default)]
struct PluginManifest {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    disabled: bool,
}

/// 极简 JSON 字段抽取：只读顶层字符串/布尔字段，避免引入 serde_json 解析未知 schema。
/// 借用 serde_json 已经在依赖里这件事，直接走 Value -> as_str() 风格。
fn parse_plugin_manifest(text: &str) -> Option<PluginManifest> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let obj = v.as_object()?;
    let mut m = PluginManifest::default();
    if let Some(s) = obj.get("name").and_then(|x| x.as_str()) {
        m.name = Some(s.to_string());
    }
    if let Some(s) = obj.get("description").and_then(|x| x.as_str()) {
        m.description = Some(s.to_string());
    }
    if let Some(s) = obj.get("version").and_then(|x| x.as_str()) {
        m.version = Some(s.to_string());
    }
    if let Some(b) = obj.get("disabled").and_then(|x| x.as_bool()) {
        m.disabled = b;
    }
    Some(m)
}

pub fn list_claude_plugins(app: &AppHandle, scope: &str) -> (Vec<ClaudePlugin>, Vec<String>) {
    let mut warnings = Vec::new();
    // 第一期 plugin 只看 user scope。项目级 plugin 等 marketplace 接好再加。
    if scope != SCOPE_USER {
        return (Vec::new(), warnings);
    }
    let root = match claude_root_for(app, scope, None, PLUGINS_SUBDIR) {
        Ok(p) => p,
        Err(_) => return (Vec::new(), warnings),
    };
    if !root.exists() {
        return (Vec::new(), warnings);
    }
    let mut out = Vec::new();
    for (dir_name, dir_path) in list_subdirs(&root) {
        let manifest_path = dir_path.join(PLUGIN_MANIFEST);
        if !manifest_path.exists() {
            continue;
        }
        let text = match fs::read_to_string(&manifest_path) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(format!("读取 {} 失败：{e}", manifest_path.display()));
                continue;
            }
        };
        let m = parse_plugin_manifest(&text).unwrap_or_else(|| {
            warnings.push(format!("{} 不是合法 JSON", manifest_path.display()));
            PluginManifest::default()
        });
        out.push(ClaudePlugin {
            scope: scope.to_string(),
            name: m.name.unwrap_or(dir_name),
            description: m.description.unwrap_or_default(),
            version: m.version.unwrap_or_default(),
            enabled: !m.disabled,
            path: dir_path.to_string_lossy().to_string(),
        });
    }
    (out, warnings)
}

// ---------- Codex MCP servers ----------

pub fn codex_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(home_dir(app)?.join(".codex").join("config.toml"))
}

/// 极简 TOML 解析：只抽 `[mcp_servers.<name>]` 节里 `command` / `args` 两个字段。
/// 不处理多行字符串、表数组、内联表等高级语法；遇到不认识的就保留原样跳过。
pub fn parse_codex_mcp_servers(text: &str) -> (Vec<CodexMcpServer>, Vec<String>) {
    let mut servers: Vec<CodexMcpServer> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut current: Option<CodexMcpServer> = None;

    let flush = |cur: &mut Option<CodexMcpServer>, out: &mut Vec<CodexMcpServer>| {
        if let Some(s) = cur.take() {
            out.push(s);
        }
    };

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            // 节头
            flush(&mut current, &mut servers);
            if let Some(name) = rest.strip_prefix("mcp_servers.") {
                let name = name.trim().trim_matches('"');
                if name.is_empty() {
                    warnings.push(format!("第 {} 行 mcp_servers 名称为空", idx + 1));
                    continue;
                }
                current = Some(CodexMcpServer {
                    name: name.to_string(),
                    command: String::new(),
                    args: Vec::new(),
                    enabled: true,
                });
            }
            continue;
        }
        let Some(server) = current.as_mut() else {
            continue;
        };
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();
        match key {
            "command" => {
                if let Some(s) = parse_toml_string(val) {
                    server.command = s;
                }
            }
            "args" => {
                if let Some(arr) = parse_toml_string_array(val) {
                    server.args = arr;
                } else {
                    warnings.push(format!("第 {} 行 args 不是字符串数组", idx + 1));
                }
            }
            _ => {}
        }
    }
    flush(&mut current, &mut servers);
    servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    (servers, warnings)
}

fn parse_toml_string(raw: &str) -> Option<String> {
    let s = raw.trim();
    let s = s.strip_prefix('"').and_then(|s| s.strip_suffix('"'))?;
    Some(s.to_string())
}

fn parse_toml_string_array(raw: &str) -> Option<Vec<String>> {
    let s = raw.trim();
    let inner = s.strip_prefix('[').and_then(|s| s.strip_suffix(']'))?;
    let mut out = Vec::new();
    for part in split_top_level_commas(inner) {
        let item = part.trim();
        if item.is_empty() {
            continue;
        }
        let unq = item
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))?;
        out.push(unq.to_string());
    }
    Some(out)
}

/// 按 `,` 拆分但忽略引号内的逗号。只够应付 `["a", "b,c", "d"]` 这种简单数组。
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut prev = '\0';
    for c in s.chars() {
        if c == '"' && prev != '\\' {
            in_str = !in_str;
        }
        if c == ',' && !in_str {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
        prev = c;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn list_codex_mcp_servers(app: &AppHandle) -> (Vec<CodexMcpServer>, Vec<String>) {
    let path = match codex_config_path(app) {
        Ok(p) => p,
        Err(e) => return (Vec::new(), vec![e]),
    };
    if !path.exists() {
        return (Vec::new(), Vec::new());
    }
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            return (
                Vec::new(),
                vec![format!("读取 {} 失败：{e}", path.display())],
            );
        }
    };
    parse_codex_mcp_servers(&text)
}

// ---------- 聚合 ----------

pub fn overview(app: &AppHandle, project_cwd: Option<&str>) -> PluginsOverview {
    let mut warnings = Vec::new();
    let (user_skills, w1) = list_claude_skills(app, SCOPE_USER, None);
    warnings.extend(w1);
    let (project_skills, w2) = if project_cwd.is_some() {
        list_claude_skills(app, SCOPE_PROJECT, project_cwd)
    } else {
        (Vec::new(), Vec::new())
    };
    warnings.extend(w2);
    let (user_plugins, w3) = list_claude_plugins(app, SCOPE_USER);
    warnings.extend(w3);
    let (codex_mcp_servers, w4) = list_codex_mcp_servers(app);
    warnings.extend(w4);
    PluginsOverview {
        claude_user_skills: user_skills,
        claude_project_skills: project_skills,
        claude_user_plugins: user_plugins,
        codex_mcp_servers,
        warnings,
    }
}

// ---------- 内部单测 ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_basic_parse() {
        let text = "---\nname: foo\ndescription: bar baz\n---\n正文";
        let fm = parse_skill_frontmatter(text).unwrap();
        assert_eq!(fm.name.as_deref(), Some("foo"));
        assert_eq!(fm.description.as_deref(), Some("bar baz"));
        assert!(!fm.disabled);
    }

    #[test]
    fn frontmatter_recognizes_disabled() {
        let text = "---\nname: foo\ndisabled: true\n---";
        let fm = parse_skill_frontmatter(text).unwrap();
        assert!(fm.disabled);
    }

    #[test]
    fn rewrite_inserts_disabled_when_missing() {
        let src = "---\nname: foo\ndescription: bar\n---\nbody\n";
        let out = rewrite_disabled_field(src, true);
        assert!(out.contains("disabled: true"));
        // body 不被破坏
        assert!(out.ends_with("body\n"));
    }

    #[test]
    fn rewrite_removes_disabled_when_enabling() {
        let src = "---\nname: foo\ndisabled: true\n---\nbody\n";
        let out = rewrite_disabled_field(src, false);
        assert!(!out.contains("disabled:"));
    }

    #[test]
    fn rewrite_handles_missing_frontmatter() {
        let src = "no frontmatter here\n";
        let out = rewrite_disabled_field(src, true);
        assert!(out.starts_with("---\ndisabled: true\n---\n"));
        assert!(out.ends_with("no frontmatter here\n"));
    }

    #[test]
    fn codex_toml_parses_simple_block() {
        let text = r#"
# 顶层注释
[mcp_servers.weather]
command = "node"
args = ["weather-mcp.js", "--port", "5151"]

[mcp_servers.linear]
command = "uvx"
args = ["linear-mcp"]
"#;
        let (servers, warnings) = parse_codex_mcp_servers(text);
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(servers.len(), 2);
        let weather = &servers[1]; // sort 后 linear 在前
        assert_eq!(weather.name, "weather");
        assert_eq!(weather.command, "node");
        assert_eq!(weather.args, vec!["weather-mcp.js", "--port", "5151"]);
    }

    #[test]
    fn codex_toml_ignores_non_mcp_sections() {
        let text = "[other]\nfoo = 1\n[mcp_servers.x]\ncommand = \"echo\"\n";
        let (servers, _) = parse_codex_mcp_servers(text);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "x");
        assert_eq!(servers[0].command, "echo");
    }

    #[test]
    fn sanitize_rejects_path_traversal() {
        assert!(sanitize_skill_name("../escape").is_err());
        assert!(sanitize_skill_name("ok-name_1").is_ok());
        assert!(sanitize_skill_name("").is_err());
    }
}
