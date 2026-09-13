use crate::runtime_shell::{IntentSink, ShellIntent, emit};
use nana_ui::runtime::{
    AppContext, DocumentId, Entity, FormField, FrameworkError, StableNodeId, TextArea, TextChanged,
    TextInput,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) enum ProductField {
    Single(Entity<TextInput>),
    Multiline(Entity<TextArea>),
}
impl ProductField {
    pub(crate) fn stable_id(self) -> StableNodeId {
        match self {
            Self::Single(field) => field.stable_id(),
            Self::Multiline(field) => field.stable_id(),
        }
    }
    #[cfg(test)]
    pub(crate) fn value(self, context: &AppContext) -> String {
        match self {
            Self::Single(field) => context
                .read(field, |field| field.state.value.clone())
                .unwrap(),
            Self::Multiline(field) => context
                .read(field, |field| field.state.value.clone())
                .unwrap(),
        }
    }
}

pub(crate) struct ProductFields {
    sink: IntentSink,
    pub(crate) editors: HashMap<String, ProductField>,
    pub(crate) wrappers: HashMap<String, Entity<FormField>>,
}
impl ProductFields {
    pub(crate) fn new(sink: IntentSink) -> Self {
        Self {
            sink,
            editors: HashMap::new(),
            wrappers: HashMap::new(),
        }
    }
    pub(crate) fn retain(
        &mut self,
        context: &mut AppContext,
        keep: &HashSet<String>,
    ) -> Result<(), FrameworkError> {
        let stale: Vec<_> = self
            .editors
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(wrapper) = self.wrappers.remove(&key) {
                context.remove_view(wrapper)?;
            }
            self.editors.remove(&key);
        }
        Ok(())
    }
    pub(crate) fn upsert(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        keep: &mut HashSet<String>,
        order: &mut Vec<StableNodeId>,
        id: &str,
        value: &str,
        intent: impl Fn(String) -> ShellIntent + Send + Sync + 'static,
    ) -> Result<(), FrameworkError> {
        keep.insert(id.to_owned());
        let label = settings_field_label(id);
        let editor = if let Some(field) = self.editors.get(id).copied() {
            match field {
                ProductField::Single(field) => {
                    context.update_component(field, |editor, _| {
                        if editor.state.value != value {
                            editor.state.replace_value(value.to_owned());
                        }
                    })?;
                }
                ProductField::Multiline(field) => {
                    context.update_component(field, |editor, _| {
                        if editor.state.value != value {
                            editor.state.replace_value(value.to_owned());
                        }
                    })?;
                }
            }
            field
        } else {
            let sink = Arc::clone(&self.sink);
            let field = if matches!(
                id,
                "agent_instruction" | "mcp_args" | "worktree-instructions"
            ) {
                let field = context.create_detached_component(
                    document_id,
                    TextArea::new(value.to_owned()).height(120.0),
                )?;
                context.on(field, move |_, event: &TextChanged, _| {
                    emit(&sink, intent(event.value.clone()))
                })?;
                ProductField::Multiline(field)
            } else {
                let field = context.create_detached_component(
                    document_id,
                    TextInput::new(value.to_owned()).secure(id == "provider_secret"),
                )?;
                context.on(field, move |_, event: &TextChanged, _| {
                    emit(&sink, intent(event.value.clone()))
                })?;
                ProductField::Single(field)
            };
            self.editors.insert(id.to_owned(), field);
            field
        };
        let wrapper = if let Some(wrapper) = self.wrappers.get(id).copied() {
            context.update_component(wrapper, |field, _| {
                *field = FormField::new(label).control_child(editor.stable_id());
            })?;
            wrapper
        } else {
            let wrapper = context.create_detached_component(
                document_id,
                FormField::new(label).control_child(editor.stable_id()),
            )?;
            crate::runtime_layout::reconcile_children(
                context,
                wrapper.stable_id(),
                &[editor.stable_id()],
            )?;
            self.wrappers.insert(id.to_owned(), wrapper);
            wrapper
        };
        order.push(wrapper.stable_id());
        Ok(())
    }
}
fn settings_field_label(id: &str) -> &'static str {
    match id {
        "provider_secret" => "访问密钥",
        "provider_model" => "模型",
        "provider_openai" => "OpenAI 端点",
        "provider_anthropic" => "Anthropic 端点",
        "agent_name" => "名称",
        "agent_description" => "说明",
        "agent_instruction" => "指令",
        "extensions-search" => "搜索",
        "skill_id" => "技能标识",
        "skill_description" => "技能说明",
        "mcp_server_id" => "MCP 标识",
        "mcp_location" => "位置",
        "mcp_args" => "参数",
        "remote-name" => "电脑名称",
        "worktree-instructions" => "创建后自动指令",
        "project-clone-repository" => "仓库",
        "project-settings-name" => "项目名称",
        _ => "值",
    }
}
