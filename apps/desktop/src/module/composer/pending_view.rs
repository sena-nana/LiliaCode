use crate::runtime_layout::{pending_actions_row, pending_interaction_card, reconcile_children};
use lilia_contracts::TaskId;
use nana_ui::runtime::{
    Activate, AppContext, Button, Card, DocumentId, Entity, FormField, FrameworkError,
    StableNodeId, Stack, Text, TextArea, TextChanged,
};
use nana_ui::{ButtonKind, ControlSize};
use nana_ui_platform::WindowId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnStopTarget {
    pub task_id: TaskId,
    pub turn_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingKind {
    PermissionApproval,
    PlanApproval,
    AskUser,
    ToolConsent,
    McpElicitation,
    ArchitectureChange,
    TitleUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingOption {
    pub id: String,
    pub label: String,
    pub selected: bool,
    pub danger: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolConsentPending {
    pub command: String,
    pub message: String,
    pub command_editable: bool,
    pub can_allow: bool,
    pub can_deny: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AskUserPending {
    pub show_other: bool,
    pub other_selected: bool,
    pub freeform: String,
    pub show_freeform: bool,
    pub show_skip: bool,
    pub show_back: bool,
    pub show_cancel: bool,
    pub show_reject: bool,
    pub can_submit: bool,
    pub submit_label: String,
    pub reject_label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpFieldOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
    pub multi: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpField {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub value: String,
    pub enabled: bool,
    pub options: Vec<McpFieldOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpPending {
    pub url: Option<String>,
    pub raw_json: Option<String>,
    pub fields: Vec<McpField>,
    pub can_accept: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingSnapshot {
    pub stop_target: Option<TurnStopTarget>,
    pub request_id: String,
    pub kind: PendingKind,
    pub title: String,
    pub prompt: String,
    pub draft: String,
    pub options: Vec<PendingOption>,
    pub tool: Option<ToolConsentPending>,
    pub ask: Option<AskUserPending>,
    pub mcp: Option<McpPending>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PendingAction {
    RespondApproval {
        approved: bool,
    },
    RespondTitle {
        accepted: bool,
    },
    RespondArchitecture {
        approved: bool,
    },
    RespondPlan {
        action: String,
    },
    RespondToolConsent {
        approved: bool,
    },
    ToolConsentDraftChanged {
        command: String,
        message: String,
    },
    AskUserPending {
        action: String,
        value: String,
    },
    PendingDraftChanged {
        value: String,
    },
    SelectPendingOption {
        option_id: String,
    },
    RespondMcp {
        action: String,
    },
    McpFieldChanged {
        field_key: String,
        value: String,
    },
    McpRawJsonChanged {
        value: String,
    },
    McpToggleOption {
        field_key: String,
        value: String,
        multi: bool,
    },
    McpToggleBoolean {
        field_key: String,
    },
    OpenMarkdownLink(String),
    InterruptTurn(Option<TurnStopTarget>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingTarget {
    pub window_id: WindowId,
    pub task_id: Option<String>,
    pub request_id: String,
}
impl PendingTarget {
    pub(crate) fn matches(&self, current: &Self) -> bool {
        self.task_id.is_some() && !self.request_id.is_empty() && self == current
    }
}
type Sink = Arc<dyn Fn(PendingTarget, PendingAction) + Send + Sync>;

pub(crate) struct PendingView {
    pub(crate) root: Entity<Card>,
    request: Option<RequestView>,
    sink: Sink,
}
struct PendingField {
    editor: Entity<TextArea>,
    wrapper: Entity<FormField>,
}
struct PendingButton {
    node: Entity<Button>,
    action: Arc<Mutex<PendingAction>>,
}
struct RequestView {
    target: PendingTarget,
    kind: PendingKind,
    title: Entity<Text>,
    prompt: Entity<Text>,
    actions: Entity<Stack>,
    fields: HashMap<String, PendingField>,
    buttons: HashMap<String, PendingButton>,
    tool_draft: Arc<Mutex<(String, String)>>,
    sink: Sink,
}
impl PendingView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: Sink,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, pending_interaction_card())?;
        context.world_mut().register_focus_scope(root.stable_id())?;
        Ok(Self {
            root,
            request: None,
            sink,
        })
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        window_id: WindowId,
        task_id: Option<String>,
        pending: Option<&PendingSnapshot>,
    ) -> Result<(), FrameworkError> {
        let target = pending.map(|pending| PendingTarget {
            window_id,
            task_id,
            request_id: pending.request_id.clone(),
        });
        let changed = self.request.as_ref().map(|r| (&r.target, r.kind))
            != target.as_ref().zip(pending.map(|p| p.kind));
        if changed {
            if let Some(request) = self.request.take() {
                request.dispose(context)?;
            }
        }
        let (Some(pending), Some(target)) = (pending, target) else {
            return reconcile_children(context, self.root.stable_id(), &[]);
        };
        if self.request.is_none() {
            self.request = Some(RequestView {
                target,
                kind: pending.kind,
                title: context.create_detached_component(document, Text::new(""))?,
                prompt: context.create_detached_component(document, Text::new(""))?,
                actions: context.create_detached_component(document, pending_actions_row())?,
                fields: HashMap::new(),
                buttons: HashMap::new(),
                tool_draft: Arc::new(Mutex::new((String::new(), String::new()))),
                sink: Arc::clone(&self.sink),
            });
        }
        self.request
            .as_mut()
            .unwrap()
            .sync(context, document, self.root.stable_id(), pending)
    }
    pub(crate) fn restore_focus(
        &self,
        context: &mut AppContext,
        document: DocumentId,
    ) -> Result<(), FrameworkError> {
        if context.world().focused(document).is_none() && self.request.is_some() {
            let mut changes = nana_ui::runtime::MutationQueue::new();
            changes.restore_focus_within(self.root.stable_id());
            context.commit_mutations(changes)?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn actions(&self) -> Option<StableNodeId> {
        self.request.as_ref().map(|r| r.actions.stable_id())
    }

    pub(crate) fn debug_nodes(&self) -> Vec<(String, StableNodeId)> {
        let Some(request) = &self.request else {
            return Vec::new();
        };
        request
            .fields
            .iter()
            .map(|(id, field)| (id.clone(), field.editor.stable_id()))
            .chain(
                request
                    .buttons
                    .iter()
                    .map(|(id, button)| (id.clone(), button.node.stable_id())),
            )
            .collect()
    }
}
impl RequestView {
    fn field(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        key: &str,
        label: &str,
        value: &str,
        action: impl Fn(String) -> PendingAction + Send + Sync + 'static,
    ) -> Result<StableNodeId, FrameworkError> {
        if let Some(field) = self.fields.get(key) {
            context.update_component(field.editor, |editor, _| {
                if editor.state.value != value {
                    editor.state.replace_value(value.to_owned());
                }
            })?;
            context.update_component(field.wrapper, |wrapper, _| {
                *wrapper = FormField::new(label).control_child(field.editor.stable_id());
            })?;
            return Ok(field.wrapper.stable_id());
        }
        let editor =
            context.create_detached_component(document, TextArea::new(value).height(48.0))?;
        let sink = Arc::clone(&self.sink);
        let target = self.target.clone();
        context.on(editor, move |_, event: &TextChanged, _| {
            sink(target.clone(), action(event.value.clone()))
        })?;
        let wrapper = context.create_detached_component(
            document,
            FormField::new(label).control_child(editor.stable_id()),
        )?;
        context.append_child(wrapper, editor)?;
        let id = wrapper.stable_id();
        self.fields
            .insert(key.to_owned(), PendingField { editor, wrapper });
        Ok(id)
    }
    fn button(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        key: &str,
        label: &str,
        kind: ButtonKind,
        action: PendingAction,
        disabled: bool,
    ) -> Result<StableNodeId, FrameworkError> {
        let button = Button::new(label).kind(kind).size(ControlSize::Small);
        if let Some(view) = self.buttons.get(key) {
            context.update_component(view.node, |node, _| {
                *node = button;
                node.disabled = disabled;
            })?;
            if let Ok(mut current) = view.action.lock() {
                *current = action;
            }
            return Ok(view.node.stable_id());
        }
        let node = context.create_detached_component(document, button)?;
        context.update_component(node, |node, _| node.disabled = disabled)?;
        let action = Arc::new(Mutex::new(action));
        let current = Arc::clone(&action);
        let sink = Arc::clone(&self.sink);
        let target = self.target.clone();
        context.on(node, move |_, _: &Activate, _| {
            if let Ok(action) = current.lock() {
                sink(target.clone(), action.clone());
            }
        })?;
        self.buttons
            .insert(key.to_owned(), PendingButton { node, action });
        Ok(node.stable_id())
    }
    fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        root: StableNodeId,
        pending: &PendingSnapshot,
    ) -> Result<(), FrameworkError> {
        context.update_component(self.title, |text, _| {
            *text = Text::new(pending.title.clone())
        })?;
        context.update_component(self.prompt, |text, _| {
            *text = Text::new(pending.prompt.clone())
        })?;
        let mut order = vec![self.title.stable_id(), self.prompt.stable_id()];
        let request = &pending.request_id;
        match pending.kind {
            PendingKind::PlanApproval => order.push(self.field(
                context,
                document,
                "draft",
                "补充说明",
                &pending.draft,
                |value| PendingAction::PendingDraftChanged { value },
            )?),
            PendingKind::AskUser => {
                if let Some(ask) = &pending.ask {
                    if ask.show_freeform {
                        order.push(self.field(
                            context,
                            document,
                            "ask",
                            "补充说明",
                            &ask.freeform,
                            |value| PendingAction::AskUserPending {
                                action: "freeform".into(),
                                value,
                            },
                        )?);
                    }
                }
            }
            PendingKind::ToolConsent => {
                if let Some(tool) = &pending.tool {
                    if let Ok(mut draft) = self.tool_draft.lock() {
                        *draft = (tool.command.clone(), tool.message.clone());
                    }
                    if tool.command_editable {
                        let draft = Arc::clone(&self.tool_draft);
                        order.push(self.field(
                            context,
                            document,
                            "command",
                            "确认执行的命令",
                            &tool.command,
                            move |value| {
                                let mut draft = draft.lock().unwrap();
                                draft.0 = value;
                                PendingAction::ToolConsentDraftChanged {
                                    command: draft.0.clone(),
                                    message: draft.1.clone(),
                                }
                            },
                        )?);
                    }
                    let draft = Arc::clone(&self.tool_draft);
                    order.push(self.field(
                        context,
                        document,
                        "message",
                        "拒绝理由",
                        &tool.message,
                        move |value| {
                            let mut draft = draft.lock().unwrap();
                            draft.1 = value;
                            PendingAction::ToolConsentDraftChanged {
                                command: draft.0.clone(),
                                message: draft.1.clone(),
                            }
                        },
                    )?);
                }
            }
            PendingKind::McpElicitation => {
                if let Some(mcp) = &pending.mcp {
                    if let Some(url) = &mcp.url {
                        order.push(self.button(
                            context,
                            document,
                            &format!("pending-mcp-url-{request}"),
                            "打开链接",
                            ButtonKind::Subtle,
                            PendingAction::OpenMarkdownLink(url.clone()),
                            false,
                        )?);
                    }
                    if let Some(raw) = &mcp.raw_json {
                        order.push(self.field(
                            context,
                            document,
                            "raw",
                            "原始 JSON",
                            raw,
                            |value| PendingAction::McpRawJsonChanged { value },
                        )?);
                    }
                    for field in &mcp.fields {
                        if field.options.is_empty() && field.kind != "boolean" {
                            let field_key = field.key.clone();
                            order.push(self.field(
                                context,
                                document,
                                &format!("mcp-{}", field.key),
                                &field.label,
                                &field.value,
                                move |value| PendingAction::McpFieldChanged {
                                    field_key: field_key.clone(),
                                    value,
                                },
                            )?);
                        } else if field.kind == "boolean" {
                            order.push(self.button(
                                context,
                                document,
                                &format!("pending-mcp-bool-{request}-{}", field.key),
                                &format!(
                                    "{} · {}",
                                    field.label,
                                    if field.enabled {
                                        "已开启"
                                    } else {
                                        "已关闭"
                                    }
                                ),
                                if field.enabled {
                                    ButtonKind::Primary
                                } else {
                                    ButtonKind::Subtle
                                },
                                PendingAction::McpToggleBoolean {
                                    field_key: field.key.clone(),
                                },
                                false,
                            )?);
                        } else {
                            for option in &field.options {
                                order.push(self.button(
                                    context,
                                    document,
                                    &format!(
                                        "pending-mcp-opt-{request}-{}-{}",
                                        field.key, option.value
                                    ),
                                    &format!("{} · {}", field.label, option.label),
                                    if option.selected {
                                        ButtonKind::Primary
                                    } else {
                                        ButtonKind::Subtle
                                    },
                                    PendingAction::McpToggleOption {
                                        field_key: field.key.clone(),
                                        value: option.value.clone(),
                                        multi: option.multi,
                                    },
                                    false,
                                )?);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        for option in &pending.options {
            order.push(self.button(
                context,
                document,
                &format!("pending-opt-{request}-{}", option.id),
                &option.label,
                if option.selected {
                    ButtonKind::Primary
                } else if option.danger {
                    ButtonKind::Danger
                } else {
                    ButtonKind::Subtle
                },
                PendingAction::SelectPendingOption {
                    option_id: option.id.clone(),
                },
                false,
            )?);
        }
        if let Some(ask) = &pending.ask {
            if ask.show_other {
                order.push(self.button(
                    context,
                    document,
                    &format!("pending-ask-other-{request}"),
                    if ask.other_selected {
                        "✓ 其他"
                    } else {
                        "其他"
                    },
                    if ask.other_selected {
                        ButtonKind::Primary
                    } else {
                        ButtonKind::Subtle
                    },
                    PendingAction::AskUserPending {
                        action: "select".into(),
                        value: "other".into(),
                    },
                    false,
                )?);
            }
        }
        let mut actions = Vec::new();
        for (id, label, kind, action, disabled) in action_specs(pending) {
            actions.push(self.button(context, document, &id, &label, kind, action, disabled)?);
        }
        reconcile_children(context, self.actions.stable_id(), &actions)?;
        if !actions.is_empty() {
            order.push(self.actions.stable_id());
        }
        reconcile_children(context, root, &order)
    }
    fn dispose(self, context: &mut AppContext) -> Result<(), FrameworkError> {
        for (_, field) in self.fields {
            context.remove_view(field.wrapper)?;
        }
        for (_, button) in self.buttons {
            context.remove_view(button.node)?;
        }
        context.remove_view(self.title)?;
        context.remove_view(self.prompt)?;
        context.remove_view(self.actions)?;
        Ok(())
    }
}

fn action_specs(
    pending: &PendingSnapshot,
) -> Vec<(String, String, ButtonKind, PendingAction, bool)> {
    let request_id = pending.request_id.clone();
    match pending.kind {
        PendingKind::PermissionApproval => vec![
            (
                format!("pending-approve-{request_id}"),
                "允许".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondApproval { approved: true },
                false,
            ),
            (
                format!("pending-reject-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondApproval { approved: false },
                false,
            ),
        ],
        PendingKind::PlanApproval => vec![
            (
                format!("pending-plan-approve-{request_id}"),
                "执行计划".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondPlan {
                    action: "approve".to_owned(),
                },
                false,
            ),
            (
                format!("pending-plan-revise-{request_id}"),
                "要求修改".to_owned(),
                ButtonKind::Subtle,
                PendingAction::RespondPlan {
                    action: "revise".to_owned(),
                },
                pending.draft.trim().is_empty(),
            ),
            (
                format!("pending-plan-decline-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondPlan {
                    action: "decline".to_owned(),
                },
                false,
            ),
            (
                format!("pending-plan-interrupt-{request_id}"),
                "取消任务".to_owned(),
                ButtonKind::Subtle,
                PendingAction::InterruptTurn(pending.stop_target.clone()),
                pending.stop_target.is_none(),
            ),
        ],
        PendingKind::ToolConsent => {
            let tool = pending.tool.as_ref();
            vec![
                (
                    format!("pending-consent-allow-{request_id}"),
                    "允许".to_owned(),
                    ButtonKind::Primary,
                    PendingAction::RespondToolConsent { approved: true },
                    tool.is_some_and(|tool| !tool.can_allow),
                ),
                (
                    format!("pending-consent-deny-{request_id}"),
                    "拒绝".to_owned(),
                    ButtonKind::Danger,
                    PendingAction::RespondToolConsent { approved: false },
                    tool.is_some_and(|tool| !tool.can_deny),
                ),
            ]
        }
        PendingKind::AskUser => {
            let ask = pending.ask.as_ref();
            let mut actions = Vec::new();
            if ask.is_some_and(|ask| ask.show_skip) {
                actions.push((
                    format!("pending-ask-skip-{request_id}"),
                    "跳过".to_owned(),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "skip".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            if ask.is_some_and(|ask| ask.show_back) {
                actions.push((
                    format!("pending-ask-back-{request_id}"),
                    "上一题".to_owned(),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "back".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            if ask.is_some_and(|ask| ask.show_cancel) {
                actions.push((
                    format!("pending-ask-cancel-{request_id}"),
                    "关闭".to_owned(),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "cancel".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            if ask.is_some_and(|ask| ask.show_reject) {
                actions.push((
                    format!("pending-ask-reject-{request_id}"),
                    ask.map(|ask| ask.reject_label.clone())
                        .unwrap_or_else(|| "不要".to_owned()),
                    ButtonKind::Subtle,
                    PendingAction::AskUserPending {
                        action: "reject".to_owned(),
                        value: String::new(),
                    },
                    false,
                ));
            }
            actions.push((
                format!("pending-ask-submit-{request_id}"),
                ask.map(|ask| ask.submit_label.clone())
                    .unwrap_or_else(|| "提交".to_owned()),
                ButtonKind::Primary,
                PendingAction::AskUserPending {
                    action: "submit".to_owned(),
                    value: String::new(),
                },
                ask.is_some_and(|ask| !ask.can_submit),
            ));
            actions
        }
        PendingKind::ArchitectureChange => vec![
            (
                format!("pending-arch-allow-{request_id}"),
                "允许".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondArchitecture { approved: true },
                false,
            ),
            (
                format!("pending-arch-deny-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondArchitecture { approved: false },
                false,
            ),
        ],
        PendingKind::TitleUpdate => vec![
            (
                format!("pending-title-accept-{request_id}"),
                "采用".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondTitle { accepted: true },
                false,
            ),
            (
                format!("pending-title-reject-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Danger,
                PendingAction::RespondTitle { accepted: false },
                false,
            ),
        ],
        PendingKind::McpElicitation => vec![
            (
                format!("pending-mcp-accept-{request_id}"),
                "接受".to_owned(),
                ButtonKind::Primary,
                PendingAction::RespondMcp {
                    action: "accept".to_owned(),
                },
                pending.mcp.as_ref().is_some_and(|mcp| !mcp.can_accept),
            ),
            (
                format!("pending-mcp-decline-{request_id}"),
                "拒绝".to_owned(),
                ButtonKind::Subtle,
                PendingAction::RespondMcp {
                    action: "decline".to_owned(),
                },
                false,
            ),
            (
                format!("pending-mcp-cancel-{request_id}"),
                "取消".to_owned(),
                ButtonKind::Subtle,
                PendingAction::RespondMcp {
                    action: "cancel".to_owned(),
                },
                false,
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::{MutationQueue, TextSelection};

    fn pending(kind: PendingKind) -> PendingSnapshot {
        PendingSnapshot {
            stop_target: Some(TurnStopTarget {
                task_id: TaskId::new("task-a").unwrap(),
                turn_id: "turn-a".into(),
            }),
            request_id: "request-a".into(),
            kind,
            title: "处理请求".into(),
            prompt: "请选择".into(),
            draft: "draft".into(),
            options: vec![PendingOption {
                id: "one".into(),
                label: "One".into(),
                selected: false,
                danger: false,
            }],
            tool: Some(ToolConsentPending {
                command: "echo one".into(),
                message: String::new(),
                command_editable: true,
                can_allow: true,
                can_deny: true,
            }),
            ask: Some(AskUserPending {
                show_other: true,
                other_selected: true,
                freeform: "my answer".into(),
                show_freeform: true,
                show_skip: true,
                show_back: true,
                show_cancel: true,
                show_reject: true,
                can_submit: true,
                submit_label: "回答".into(),
                reject_label: "拒绝".into(),
            }),
            mcp: Some(McpPending {
                url: Some("https://example.test".into()),
                raw_json: Some("{}".into()),
                can_accept: true,
                fields: vec![
                    McpField {
                        key: "name".into(),
                        label: "名称".into(),
                        kind: "string".into(),
                        value: "value".into(),
                        enabled: true,
                        options: vec![],
                    },
                    McpField {
                        key: "enabled".into(),
                        label: "开启".into(),
                        kind: "boolean".into(),
                        value: "true".into(),
                        enabled: true,
                        options: vec![],
                    },
                    McpField {
                        key: "choice".into(),
                        label: "选项".into(),
                        kind: "string".into(),
                        value: String::new(),
                        enabled: true,
                        options: vec![McpFieldOption {
                            value: "v".into(),
                            label: "V".into(),
                            selected: false,
                            multi: true,
                        }],
                    },
                ],
            }),
        }
    }

    #[test]
    fn long_approval_text_wraps_above_actions_in_a_narrow_card() {
        use nana_ui::runtime::{LayoutViewport, RuntimeDocument};
        let document_id = DocumentId::new(643).unwrap();
        let mut document = RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, nana_ui::runtime::Stack::column(0.0))
            .unwrap();
        let mut view = PendingView::mount(context, document_id, Arc::new(|_, _| {})).unwrap();
        context.append_child(root, view.root).unwrap();
        let mut snapshot = pending(PendingKind::PermissionApproval);
        snapshot.options.clear();
        snapshot.ask = None;
        snapshot.prompt =
            "请确认是否允许在当前任务的浏览器中执行操作，完成后会返回页面的最新状态。".repeat(3);
        view.sync(
            context,
            document_id,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        let request = view.request.as_ref().unwrap();
        let prompt = request.prompt.stable_id();
        let actions = request.actions.stable_id();
        document
            .flush(
                LayoutViewport::new(360.0, 800.0),
                &mut nana_ui::NanaTextShaper::default(),
            )
            .unwrap();
        let text = document.scene().node_bounds(prompt).unwrap();
        let buttons = document.scene().node_bounds(actions).unwrap();
        let card = document.scene().node_bounds(view.root.stable_id()).unwrap();
        assert!(
            text.height >= 60.0,
            "long prompt must occupy multiple lines: {text:?}"
        );
        assert!(
            text.x > card.x && text.x + text.width < card.x + card.width,
            "text stays within the padded card: {text:?}, {card:?}"
        );
        assert!(
            buttons.y >= text.y + text.height + 7.0,
            "actions must clear wrapped text: {text:?}, {buttons:?}"
        );
    }

    #[test]
    fn all_seven_forms_share_window_addressed_actions_and_dispose_previous_requests() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut context = AppContext::new();
        let forms = [
            PendingKind::PermissionApproval,
            PendingKind::PlanApproval,
            PendingKind::AskUser,
            PendingKind::ToolConsent,
            PendingKind::McpElicitation,
            PendingKind::ArchitectureChange,
            PendingKind::TitleUpdate,
        ];
        for (number, window) in [(641, WindowId::PRIMARY), (642, WindowId(19))] {
            let document = DocumentId::new(number).unwrap();
            let host = context
                .create_component(document, Stack::fill_column(0.0))
                .unwrap();
            let received = Arc::clone(&events);
            let mut view = PendingView::mount(
                &mut context,
                document,
                Arc::new(move |target, action| received.lock().unwrap().push((target, action))),
            )
            .unwrap();
            context.append_child(host, view.root).unwrap();
            for kind in forms {
                let snapshot = pending(kind);
                view.sync(
                    &mut context,
                    document,
                    window,
                    Some("task-a".into()),
                    Some(&snapshot),
                )
                .unwrap();
                let request = view.request.as_ref().unwrap();
                let key = action_specs(&snapshot)[0].0.clone();
                let button = request.buttons[&key].node;
                context
                    .update_component(button, |_, cx| cx.emit(Activate))
                    .unwrap();
                let (target, action) = events.lock().unwrap().last().unwrap().clone();
                assert_eq!(target, request.target);
                assert_eq!(action, action_specs(&snapshot)[0].3);
                let mut another = target.clone();
                another.window_id = if window == WindowId::PRIMARY {
                    WindowId(19)
                } else {
                    WindowId::PRIMARY
                };
                assert!(!target.matches(&another));
                another = target.clone();
                another.request_id = "request-b".into();
                assert!(!target.matches(&another));
                another = target.clone();
                another.task_id = Some("task-b".into());
                assert!(!target.matches(&another));
                another.task_id = None;
                assert!(!target.matches(&another));
                let nodes: Vec<_> = request
                    .fields
                    .values()
                    .flat_map(|f| [f.editor.stable_id(), f.wrapper.stable_id()])
                    .chain(request.buttons.values().map(|b| b.node.stable_id()))
                    .chain([
                        request.title.stable_id(),
                        request.prompt.stable_id(),
                        request.actions.stable_id(),
                    ])
                    .collect();
                view.sync(&mut context, document, window, Some("task-a".into()), None)
                    .unwrap();
                for node in nodes {
                    assert!(
                        !context.world().contains(node),
                        "leaked pending node {node:?}"
                    );
                }
            }
        }
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 14);
        for i in 0..7 {
            assert_eq!(events[i].1, events[i + 7].1);
        }
    }

    #[test]
    fn same_request_preserves_draft_selection_and_focus_through_parking() {
        let mut context = AppContext::new();
        let document = DocumentId::new(643).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut view = PendingView::mount(
            &mut context,
            document,
            Arc::new(move |target, action| received.lock().unwrap().push((target, action))),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let mut snapshot = pending(PendingKind::AskUser);
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        let editor = view.request.as_ref().unwrap().fields["ask"].editor;
        let selection = TextSelection {
            anchor: 2,
            focus: 5,
        };
        context
            .update_component(editor, |input, cx| {
                input.state.selection = selection;
                cx.emit(TextChanged {
                    value: "my answer".into(),
                    selection,
                });
            })
            .unwrap();
        let old_event = events.lock().unwrap()[0].clone();
        context.focus_node(document, editor.stable_id()).unwrap();
        let mut changes = MutationQueue::new();
        changes.park_subtree(view.root.stable_id());
        context.commit_mutations(changes).unwrap();
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        view.restore_focus(&mut context, document).unwrap();
        assert_eq!(context.world().focused(document), Some(editor.stable_id()));
        assert_eq!(
            context
                .read(editor, |input| (
                    input.state.value.clone(),
                    input.state.selection
                ))
                .unwrap(),
            ("my answer".into(), selection)
        );
        snapshot.ask.as_mut().unwrap().show_freeform = false;
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        assert!(context.world().contains(editor.stable_id()));
        snapshot.ask.as_mut().unwrap().show_freeform = true;
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        view.restore_focus(&mut context, document).unwrap();
        assert_eq!(context.world().focused(document), Some(editor.stable_id()));
        assert_eq!(view.request.as_ref().unwrap().fields["ask"].editor, editor);
        snapshot.request_id = "request-b".into();
        view.sync(
            &mut context,
            document,
            WindowId::PRIMARY,
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        assert!(!context.world().contains(editor.stable_id()));
        assert!(!old_event.0.matches(&view.request.as_ref().unwrap().target));
    }

    #[test]
    fn tool_fields_merge_current_values_and_stop_keeps_the_displayed_turn() {
        let mut context = AppContext::new();
        let document = DocumentId::new(644).unwrap();
        let host = context
            .create_component(document, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&events);
        let mut view = PendingView::mount(
            &mut context,
            document,
            Arc::new(move |target, action| received.lock().unwrap().push((target, action))),
        )
        .unwrap();
        context.append_child(host, view.root).unwrap();
        let snapshot = pending(PendingKind::ToolConsent);
        view.sync(
            &mut context,
            document,
            WindowId(19),
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        for (key, value) in [("command", "echo changed"), ("message", "reason")] {
            let editor = view.request.as_ref().unwrap().fields[key].editor;
            context
                .update_component(editor, |_, cx| {
                    cx.emit(TextChanged {
                        value: value.into(),
                        selection: TextSelection::default(),
                    })
                })
                .unwrap();
        }
        assert_eq!(
            events.lock().unwrap().last().unwrap().1,
            PendingAction::ToolConsentDraftChanged {
                command: "echo changed".into(),
                message: "reason".into()
            }
        );
        let mut snapshot = pending(PendingKind::PlanApproval);
        view.sync(
            &mut context,
            document,
            WindowId(19),
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        let button =
            view.request.as_ref().unwrap().buttons["pending-plan-interrupt-request-a"].node;
        context
            .update_component(button, |_, cx| cx.emit(Activate))
            .unwrap();
        let old = events.lock().unwrap().last().unwrap().clone();
        snapshot.stop_target.as_mut().unwrap().turn_id = "turn-b".into();
        view.sync(
            &mut context,
            document,
            WindowId(19),
            Some("task-a".into()),
            Some(&snapshot),
        )
        .unwrap();
        context
            .update_component(button, |_, cx| cx.emit(Activate))
            .unwrap();
        assert_eq!(
            old.1,
            PendingAction::InterruptTurn(Some(TurnStopTarget {
                task_id: TaskId::new("task-a").unwrap(),
                turn_id: "turn-a".into()
            }))
        );
        assert_eq!(
            events.lock().unwrap().last().unwrap().1,
            PendingAction::InterruptTurn(snapshot.stop_target)
        );
    }
}
