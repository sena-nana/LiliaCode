use std::fs;

use tauri::AppHandle;

use super::paths::{
    claude_root_for, list_subdirs, sanitize_extension_name, PLUGINS_SUBDIR, PLUGIN_MANIFEST,
    SCOPE_USER,
};
use super::types::ClaudePlugin;

fn sanitize_plugin_name(raw: &str) -> Result<String, String> {
    sanitize_extension_name(raw, "plugin")
}

#[derive(Debug, Default)]
struct PluginManifest {
    description: Option<String>,
    version: Option<String>,
    disabled: bool,
}

/// 极简 JSON 字段抽取：只读顶层字符串/布尔字段。
fn parse_plugin_manifest(text: &str) -> Option<PluginManifest> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let obj = v.as_object()?;
    let mut m = PluginManifest::default();
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
    // 一期 plugin 只看 user scope。
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
            name: dir_name,
            description: m.description.unwrap_or_default(),
            version: m.version.unwrap_or_default(),
            enabled: !m.disabled,
            path: dir_path.to_string_lossy().to_string(),
        });
    }
    (out, warnings)
}

pub fn set_claude_plugin_enabled(
    app: &AppHandle,
    scope: &str,
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    if scope != SCOPE_USER {
        return Err("当前仅支持用户级 Claude plugin".to_string());
    }
    let name = sanitize_plugin_name(name)?;
    let root = claude_root_for(app, scope, None, PLUGINS_SUBDIR)?;
    let manifest_path = root.join(&name).join(PLUGIN_MANIFEST);
    let original = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("读取 {} 失败：{e}", manifest_path.display()))?;
    let updated = rewrite_plugin_disabled_field(&original, !enabled)
        .map_err(|e| format!("更新 plugin.json 失败：{e}"))?;
    fs::write(&manifest_path, updated).map_err(|e| format!("写入 plugin.json 失败：{e}"))?;
    Ok(())
}

fn rewrite_plugin_disabled_field(text: &str, want_disabled: bool) -> Result<String, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("不是合法 JSON：{e}"))?;
    let Some(obj) = value.as_object_mut() else {
        return Err("plugin.json 顶层必须是对象".to_string());
    };
    if want_disabled {
        obj.insert("disabled".to_string(), serde_json::Value::Bool(true));
    } else {
        obj.remove("disabled");
    }
    serde_json::to_string_pretty(&value)
        .map_err(|e| e.to_string())
        .map(|mut out| {
            out.push('\n');
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_manifest_recognizes_disabled() {
        let text = r#"{
  "name": "demo",
  "description": "Demo plugin",
  "version": "1.0.0",
  "disabled": true
}"#;
        let manifest = parse_plugin_manifest(text).unwrap();
        assert!(manifest.disabled);
    }

    #[test]
    fn plugin_rewrite_toggles_disabled() {
        let src = r#"{"name":"demo","disabled":true}"#;
        let enabled = rewrite_plugin_disabled_field(src, false).unwrap();
        assert!(!enabled.contains("disabled"));

        let disabled = rewrite_plugin_disabled_field(&enabled, true).unwrap();
        let value: serde_json::Value = serde_json::from_str(&disabled).unwrap();
        assert_eq!(value.get("disabled").and_then(|v| v.as_bool()), Some(true));
    }
}
