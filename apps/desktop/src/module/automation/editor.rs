use serde_json::{Value, json};

#[derive(Debug, Clone, Default)]
pub(crate) struct AutomationNodeInspectorDraft {
    pub(crate) node_id: Option<String>,
    pub(crate) title: String,
    pub(crate) config: String,
}

impl AutomationNodeInspectorDraft {
    pub(crate) fn validated_config(&self) -> Result<Value, String> {
        let mut config = parse_automation_node_config(&self.config)?;
        if let Some(cases) = config.get("cases") {
            let cases: lilia_contracts::AutomationSwitchCases =
                serde_json::from_value(cases.clone())
                    .map_err(|_| "分支需要填写匹配值列表。".to_owned())?;
            let values = cases.values();
            let mut ports = std::collections::HashSet::new();
            for value in &values {
                let port = lilia_feature_automation::automation_json_value_to_port(&json!(value))
                    .to_ascii_lowercase();
                if port.is_empty()
                    || matches!(port.as_str(), "default" | "output")
                    || !ports.insert(port)
                {
                    return Err("分支匹配值无法区分，请使用不同的名称。".into());
                }
            }
            config["cases"] = json!(values);
        }
        Ok(config)
    }
    pub(crate) fn snapshot(&self, kind: &str) -> Option<NodeEditorSnapshot> {
        let node_id = self.node_id.clone()?;
        let config = parse_automation_node_config(&self.config).ok();
        Some(NodeEditorSnapshot {
            node_id,
            title: self.title.clone(),
            fields: automation_node_config_fields(kind, &self.config)
                .into_iter()
                .map(|field| {
                    let value = config
                        .as_ref()
                        .and_then(|config| config.get(field).cloned())
                        .unwrap_or(Value::Null);
                    NodeEditorField {
                        key: field.to_owned(),
                        value,
                    }
                })
                .collect(),
        })
    }
    pub(crate) fn set_field(&mut self, field: String, value: Value) -> Result<(), String> {
        if self.node_id.is_none() {
            return Ok(());
        }
        let mut config = parse_automation_node_config(&self.config)?;
        config
            .as_object_mut()
            .expect("validated object")
            .insert(field, value);
        self.config = config.to_string();
        Ok(())
    }

    pub(crate) fn cycle_field(&mut self, field: &str) -> Result<(), String> {
        let choices = field_choices(field);
        if choices.is_empty() {
            return Ok(());
        }
        let value = automation_config_string(&self.config, field);
        let index = choices
            .iter()
            .position(|(key, _)| *key == value)
            .map(|index| (index + 1) % choices.len())
            .unwrap_or(0);
        self.set_field(field.to_owned(), json!(choices[index].0))
    }

    pub(crate) fn toggle_field(&mut self, field: &str) -> Result<(), String> {
        let current = automation_config_boolean(&self.config, field);
        self.set_field(field.to_owned(), json!(!current))
    }
}

#[derive(Clone, Debug)]
pub struct NodeEditorSnapshot {
    pub node_id: String,
    pub title: String,
    pub fields: Vec<NodeEditorField>,
}

#[derive(Clone, Debug)]
pub struct NodeEditorField {
    pub key: String,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub enum NodeEditorAction {
    Title(String),
    Field { key: String, value: Value },
    Save,
    Close,
}

pub(crate) fn field_choices(field: &str) -> &'static [(&'static str, &'static str)] {
    match field {
        "triggerKind" => &[
            ("manual", "手动运行"),
            ("task_changed", "任务变化"),
            ("timeline_event", "时间线更新"),
            ("todo_changed", "待办变化"),
            ("interaction_request", "交互请求"),
        ],
        "permission" => &[
            ("ask", "操作前询问"),
            ("full", "完整权限"),
            ("readonly", "只读"),
        ],
        "logic" => &[
            ("condition", "条件判断"),
            ("switch", "多分支"),
            ("stop", "结束运行"),
        ],
        "action" => &[
            ("record_timeline", "记录时间线"),
            ("create_task", "创建任务"),
            ("update_task_status", "更新任务状态"),
            ("add_todo", "添加待办"),
            ("send_guide", "发送指导"),
        ],
        "priority" => &[("low", "低"), ("normal", "普通"), ("high", "高")],
        "status" => &[
            ("waiting", "等待中"),
            ("running", "进行中"),
            ("blocked", "已阻塞"),
            ("done", "已完成"),
        ],
        _ => &[],
    }
}

pub(crate) fn field_label(field: &str) -> &'static str {
    match field {
        "triggerKind" => "触发方式",
        "permission" => "执行权限",
        "logic" => "判断方式",
        "action" => "执行动作",
        "priority" => "优先级",
        "status" => "任务状态",
        "createTask" => "创建新任务",
        "taskId" => "任务",
        "projectId" => "项目",
        "title" => "标题",
        "model" => "模型",
        "projectCwd" => "工作目录",
        "prompt" => "指令",
        "path" => "判断字段",
        "equals" => "匹配值",
        "cases" => "分支",
        "text" => "内容",
        "summary" => "摘要",
        "backend" => "来源",
        _ => "值",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_draft_is_preserved_when_a_field_is_edited() {
        let mut editor = AutomationNodeInspectorDraft {
            node_id: Some("node".into()),
            title: "Node".into(),
            config: "{unfinished".into(),
        };
        assert!(editor.set_field("prompt".into(), json!("new")).is_err());
        assert_eq!(editor.config, "{unfinished");
        assert!(editor.cycle_field("permission").is_err());
        assert_eq!(editor.config, "{unfinished");
    }
    #[test]
    fn field_edits_keep_unrelated_configuration_and_json_types() {
        let mut editor = AutomationNodeInspectorDraft {
            node_id: Some("node".into()),
            title: "Node".into(),
            config: json!({"prompt":"keep", "nested":{"enabled":true}}).to_string(),
        };
        editor.toggle_field("createTask").unwrap();
        editor.cycle_field("permission").unwrap();
        let value = parse_automation_node_config(&editor.config).unwrap();
        assert_eq!(value["prompt"], "keep");
        assert_eq!(value["nested"]["enabled"], true);
        assert_eq!(value["createTask"], true);
        assert_eq!(value["permission"], "ask");
    }
    #[test]
    fn branch_input_keeps_newlines_until_save() {
        let mut editor = AutomationNodeInspectorDraft {
            node_id: Some("node".into()),
            title: "Branch".into(),
            config: "{}".into(),
        };
        editor
            .set_field("cases".into(), json!("first\nsecond\n"))
            .unwrap();
        assert_eq!(
            parse_automation_node_config(&editor.config).unwrap()["cases"],
            "first\nsecond\n"
        );
        assert_eq!(
            editor.validated_config().unwrap()["cases"],
            json!(["first", "second"])
        );
        editor
            .set_field("cases".into(), json!("review ready\nreview_ready"))
            .unwrap();
        assert!(editor.validated_config().is_err());
    }
}

pub(crate) fn parse_automation_node_config(config: &str) -> Result<Value, String> {
    match serde_json::from_str::<Value>(config) {
        Ok(Value::Object(config)) => Ok(Value::Object(config)),
        Ok(_) => Err("节点配置必须是 JSON 对象。".to_owned()),
        Err(error) => Err(format!("节点配置不是有效 JSON：{error}")),
    }
}

pub(crate) fn automation_config_string(config: &str, field: &str) -> String {
    parse_automation_node_config(config)
        .ok()
        .and_then(|config| config.get(field).cloned())
        .map(|value| match value {
            Value::String(value) => value,
            Value::Null => String::new(),
            value => value.to_string(),
        })
        .unwrap_or_default()
}

pub(crate) fn automation_config_boolean(config: &str, field: &str) -> bool {
    parse_automation_node_config(config)
        .ok()
        .and_then(|config| config.get(field).and_then(Value::as_bool))
        .unwrap_or(false)
}

pub(crate) fn automation_node_config_fields(kind: &str, config: &str) -> Vec<&'static str> {
    match kind {
        "trigger" => vec!["triggerKind"],
        "agent" => vec![
            "createTask",
            "taskId",
            "projectId",
            "title",
            "model",
            "projectCwd",
            "prompt",
            "permission",
        ],
        "logic" => {
            let logic = automation_config_string(config, "logic");
            let mut fields = vec!["logic", "path", "equals"];
            if logic == "switch" {
                fields.push("cases");
            }
            fields
        }
        "tool" => {
            let action = automation_config_string(config, "action");
            let mut fields = vec!["action"];
            fields.extend_from_slice(automation_tool_action_fields(&action));
            fields
        }
        "human" => vec!["prompt"],
        _ => Vec::new(),
    }
}

pub(crate) fn automation_node_config_cycle_field(field: &str) -> bool {
    matches!(
        field,
        "triggerKind" | "permission" | "logic" | "action" | "priority"
    )
}

pub(crate) fn automation_node_config_boolean_field(field: &str) -> bool {
    field == "createTask"
}

pub(crate) fn automation_node_config_input_field(field: &str) -> bool {
    !automation_node_config_cycle_field(field) && !automation_node_config_boolean_field(field)
}

pub(crate) fn automation_tool_action_fields(action: &str) -> &'static [&'static str] {
    match action {
        "create_task" => &["projectId", "title", "status"],
        "update_task_status" => &["taskId", "status"],
        "add_todo" => &["taskId", "text"],
        "send_guide" => &["taskId", "text", "priority"],
        _ => &["taskId", "title", "summary", "status", "backend"],
    }
}
