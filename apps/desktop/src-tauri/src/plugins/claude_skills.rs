use std::fs;

use tauri::AppHandle;

use super::paths::{
    claude_root_for, ensure_dir, list_subdirs, sanitize_extension_name, SKILLS_SUBDIR, SKILL_FILE,
};
use super::types::ClaudeSkill;

/// 解析 SKILL.md frontmatter（YAML 风格的 `key: value`，不支持嵌套）。
fn parse_skill_frontmatter(text: &str) -> Option<SkillFrontmatter> {
    let mut lines = text.lines();
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
            // 子目录里没有 SKILL.md：不是 skill，跳过。
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
    sanitize_extension_name(raw, "skill")
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
    fs::write(&skill_file, body).map_err(|e| format!("写入 SKILL.md 失败：{e}"))?;
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
    fs::remove_dir_all(&skill_dir).map_err(|e| format!("删除 skill 目录失败：{e}"))?;
    Ok(())
}

/// 切 `disabled` 字段：保留原有 frontmatter 其它键 + body 原样。
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
    fs::write(&skill_file, updated).map_err(|e| format!("写入 SKILL.md 失败：{e}"))?;
    Ok(())
}

/// 在 frontmatter 段内更新 `disabled` 字段。
/// - want_disabled=true：插入或改成 `disabled: true`
/// - want_disabled=false：删除现有的 `disabled` 行
fn rewrite_disabled_field(text: &str, want_disabled: bool) -> String {
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
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
        // 没有有效 frontmatter，重新加一段。
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
    // 保留末尾换行。
    if text.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

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
}
