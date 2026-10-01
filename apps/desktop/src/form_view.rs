use crate::runtime_layout::Bound;
use crate::runtime_shell::{emit, IntentSink, ShellIntent};
use nana_ui::runtime::view::{entity_ref, signal, widget, with_refs, Signal};
use nana_ui::runtime::{
    AppContext, DocumentId, Entity, FormField, FrameworkError, MountedView, StableNodeId, TextArea,
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
                .unwrap()
                .to_string(),
            Self::Multiline(field) => context
                .read(field, |field| field.state.value.clone())
                .unwrap()
                .to_string(),
        }
    }
}

pub(crate) struct ProductFields {
    sink: IntentSink,
    pub(crate) editors: HashMap<String, ProductField>,
    pub(crate) wrappers: HashMap<String, Entity<FormField>>,
    signals: HashMap<String, Signal<String>>,
    mounted: HashMap<String, MountedView>,
}
impl ProductFields {
    pub(crate) fn new(sink: IntentSink) -> Self {
        Self {
            sink,
            editors: HashMap::new(),
            wrappers: HashMap::new(),
            signals: HashMap::new(),
            mounted: HashMap::new(),
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
            if let Some(mounted) = self.mounted.remove(&key) {
                mounted.unmount(context)?;
            }
            self.wrappers.remove(&key);
            self.editors.remove(&key);
            self.signals.remove(&key);
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
        if let Some(value_signal) = self.signals.get(id).copied() {
            value_signal.set(value.to_owned());
            context.flush_reactive()?;
        } else {
            let sink = Arc::clone(&self.sink);
            let multiline = matches!(
                id,
                "agent_instruction" | "mcp_args" | "worktree-instructions"
            );
            let secure = id == "provider_secret";
            let (mounted, wrapper, editor, value_signal) = if multiline {
                mount_multiline(context, document_id, label, value, sink, intent)?
            } else {
                mount_single(context, document_id, label, value, secure, sink, intent)?
            };
            self.mounted.insert(id.to_owned(), mounted);
            self.wrappers.insert(id.to_owned(), wrapper);
            self.editors.insert(id.to_owned(), editor);
            self.signals.insert(id.to_owned(), value_signal);
        }
        order.push(self.wrappers[id].stable_id());
        Ok(())
    }
}

fn mount_single(
    context: &mut AppContext,
    document_id: DocumentId,
    label: &'static str,
    value: &str,
    secure: bool,
    sink: IntentSink,
    intent: impl Fn(String) -> ShellIntent + Send + Sync + 'static,
) -> Result<(MountedView, Entity<FormField>, ProductField, Signal<String>), FrameworkError> {
    let slot = Bound::new();
    let installed = slot.clone();
    let initial = value.to_owned();
    let (mounted, (wrapper, editor)) = context.mount_view_detached(document_id, move || {
        let value_signal = installed.install(signal(initial.clone()));
        let editor = entity_ref();
        let wrapper = entity_ref();
        with_refs(
            widget(FormField::new(label)).entity_ref(wrapper).control(
                widget(TextInput::new(initial).secure(secure))
                    .entity_ref(editor)
                    .value(value_signal)
                    .on_input(move |event| emit(&sink, intent(event.value.to_string()))),
            ),
            (wrapper, editor),
        )
    })?;
    Ok((
        mounted,
        wrapper,
        ProductField::Single(editor),
        slot.signal(),
    ))
}

fn mount_multiline(
    context: &mut AppContext,
    document_id: DocumentId,
    label: &'static str,
    value: &str,
    sink: IntentSink,
    intent: impl Fn(String) -> ShellIntent + Send + Sync + 'static,
) -> Result<(MountedView, Entity<FormField>, ProductField, Signal<String>), FrameworkError> {
    let slot = Bound::new();
    let installed = slot.clone();
    let initial = value.to_owned();
    let (mounted, (wrapper, editor)) = context.mount_view_detached(document_id, move || {
        let value_signal = installed.install(signal(initial.clone()));
        let editor = entity_ref();
        let wrapper = entity_ref();
        with_refs(
            widget(FormField::new(label)).entity_ref(wrapper).control(
                widget(TextArea::new(initial).height(120.0))
                    .entity_ref(editor)
                    .value(value_signal)
                    .on_input(move |event| emit(&sink, intent(event.value.to_string()))),
            ),
            (wrapper, editor),
        )
    })?;
    Ok((
        mounted,
        wrapper,
        ProductField::Multiline(editor),
        slot.signal(),
    ))
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
