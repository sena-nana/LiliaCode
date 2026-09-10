mod keyboard;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use nana_ui::runtime::{
    AlignSpec, AppContext, Button, Chip, DocumentId, Dropdown, DropdownOption, Entity,
    FrameworkError, IconGlyph, JustifySpec, LengthSpec, NativeMarkdown, PositionSpec,
    SemanticColorRole, Stack, Text, TextArea, TextChanged,
};
use nana_ui::{ButtonKind, ControlSize, DropdownEvent, DropdownSelection, Icon};
use nana_ui_platform::WindowId;

use crate::runtime_layout::{pill_button, token_chip};
use crate::runtime_shell::{bind_activate, emit, ShellIntent};
mod links;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationAction {
    PasteClipboard(ComposerPasteRequest),
    CompletionChanged,
    Model(String),
    Reasoning(String),
    Optimize,
    ApplyPromptWorkflow,
    DismissPromptWorkflow,
    Compact,
    SelectReference(String),
    RemoveReference(String),
    PreviewAttachment(String),
    PreviewTimelineAttachment {
        event_id: String,
        attachment_id: String,
    },
    CloseImage,
    OpenImage(nana_ui::MarkdownImage),
    SelectText {
        event_id: String,
        text: Option<String>,
    },
    CopySelection,
    QuoteSelection,
    AskSelection,
    RemoveAttachment(String),
    ReviewTarget(String),
    ReviewValue(String),
    SubmitReview,
    CancelReview,
    ClearBranch,
    Continue(String),
    Fork(String),
    ApplySuggestion(String),
    Retry(String),
    Copy(String),
    ToggleEvent(String),
    LoadEarlier,
    AddFiles,
    AddDirectories,
    Reference,
    Plan,
    Goal,
    NewGuide,
    EditGoal,
    Permission(String),
    Worktree(String),
    RequestWorktreeMerge(String),
    ConfirmWorktreeMerge(String),
    CancelWorktreeMerge,
    Slash(String),
    Context(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerPasteRequest {
    pub binding: (String, u64, Option<String>),
    pub editor: nana_ui::runtime::TextInputState,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConversationControls {
    pub paste_binding: Option<(String, u64, Option<String>)>,
    pub model: String,
    pub model_label: String,
    pub models: Vec<(String, String)>,
    pub reasoning: String,
    pub context_label: Option<String>,
    pub can_compact: bool,
    pub can_optimize: bool,
    pub optimizing: bool,
    pub prompt_workflow: Option<bool>,
    pub locked: bool,
    pub reference_results: Vec<(String, String)>,
    pub reference_query: bool,
    pub references: Vec<(String, String)>,
    pub attachments: Vec<(String, String)>,
    pub review: Option<(String, String)>,
    pub branch: Option<String>,
    pub plan: bool,
    pub goal: bool,
    pub can_manage_todos: bool,
    pub permission: String,
    pub worktree: Option<(String, String)>,
    pub worktree_locked: bool,
    pub worktree_merge_task: Option<String>,
    pub worktree_merge_confirmation: Option<String>,
    pub slash_results: Vec<(String, String)>,
    pub context_results: Vec<(String, String)>,
}

type Sink = Arc<dyn Fn(ShellIntent) + Send + Sync>;

pub struct ConversationControlsHandles {
    pub root: Entity<Stack>,
    pub toolbar: Entity<Stack>,
    popup_tools_row: Entity<Stack>,
    popup_model_row: Entity<Stack>,
    popup_context_row: Entity<Stack>,
    chips: Entity<Stack>,
    completion: Entity<Stack>,
    workflow: Entity<Stack>,
    model: Entity<Dropdown>,
    reasoning: Entity<Dropdown>,
    tools: Entity<Dropdown>,
    permission: Entity<Dropdown>,
    worktree: Entity<Dropdown>,
    review_target: Entity<Dropdown>,
    review_value: Entity<TextArea>,
    empty: Entity<Text>,
    buttons: HashMap<String, Entity<Chip>>,
    window_id: WindowId,
    sink: Sink,
    popup: bool,
    keyboard: keyboard::ComposerKeyboard,
}

pub fn intent(window_id: WindowId, action: ConversationAction) -> ShellIntent {
    ShellIntent::Conversation { window_id, action }
}

pub fn timeline_container() -> Stack {
    Stack::column(0.0)
        .width(LengthSpec::Fill)
        .max_width(788.0)
        .with_layout(|layout| {
            layout.margin_left = Some(LengthSpec::Auto);
            layout.margin_right = Some(LengthSpec::Auto);
            layout.padding_right = Some(LengthSpec::Px(28.0));
        })
}

pub fn timeline_stack(row: &crate::runtime_shell::ShellTimelineRow) -> Stack {
    match row.role.as_str() {
        "user" => Stack::column(6.0)
            .padding_xy(14.0, 10.0)
            .surface(SemanticColorRole::AccentSoft)
            .radius(12.0)
            .width(LengthSpec::FitContent)
            .with_layout(|layout| {
                layout.position = PositionSpec::Relative;
                layout.max_width = Some(LengthSpec::Min2(
                    nana_ui_core::LengthAtom::Percent(76.0),
                    nana_ui_core::LengthAtom::Px(620.0),
                ));
                layout.margin_left = Some(LengthSpec::Auto);
                layout.paint.border_radii = Some([12.0, 12.0, 4.0, 12.0].map(LengthSpec::Px));
            }),
        _ => Stack::bar(0.0)
            .align(AlignSpec::Start)
            .padding_xy(
                0.0,
                if row.role.starts_with("process-child:") {
                    4.0
                } else {
                    7.0
                },
            )
            .with_layout(|layout| layout.position = PositionSpec::Relative),
    }
}

pub struct TimelineContent {
    attachments: Entity<Stack>,
    attachment_buttons: HashMap<String, Entity<Chip>>,
    attachment_event: String,
    attachment_rows: HashMap<String, Entity<Stack>>,
    attachment_thumbnails: HashMap<String, Entity<NativeMarkdown>>,
    images: Vec<crate::runtime_shell::ShellTimelineImage>,
    markdown: Entity<NativeMarkdown>,
    markdown_links_bound: bool,
    selection_visible: bool,
    title: Entity<Text>,
    actions: Entity<Stack>,
    body: Entity<Stack>,
    header: Entity<Stack>,
    preview: Entity<Text>,
    rail: Entity<Stack>,
    rail_line: Entity<Stack>,
    node: Entity<IconGlyph>,
    buttons: HashMap<String, Entity<Button>>,
    source: String,
}

impl TimelineContent {
    #[cfg(debug_assertions)]
    pub(crate) fn debug_targets(&self) -> Vec<(String, nana_ui::runtime::StableNodeId)> {
        self.buttons
            .iter()
            .map(|(id, button)| (id.clone(), button.stable_id()))
            .chain(std::iter::once((
                "markdown".into(),
                self.markdown.stable_id(),
            )))
            .chain(
                self.attachment_buttons
                    .iter()
                    .map(|(id, button)| (format!("attachment-open-{id}"), button.stable_id())),
            )
            .chain(
                self.attachment_thumbnails
                    .iter()
                    .map(|(id, image)| (format!("attachment-image-{id}"), image.stable_id())),
            )
            .collect()
    }
    pub fn mount(context: &mut AppContext, document: DocumentId) -> Result<Self, FrameworkError> {
        let rail = context.create_detached_component(
            document,
            Stack::row(0.0)
                .width(LengthSpec::Px(28.0))
                .height(LengthSpec::Px(22.0))
                .align(AlignSpec::Center)
                .justify(JustifySpec::Center)
                .surface(SemanticColorRole::Background),
        )?;
        let node = context
            .create_detached_component(document, IconGlyph::new(Icon::Activity).size(13.0))?;
        context.append_child(rail, node)?;
        let rail_line = context.create_detached_component(
            document,
            Stack::column(0.0)
                .surface(SemanticColorRole::BorderSoft)
                .width(LengthSpec::Px(1.0))
                .with_layout(|layout| {
                    layout.position = PositionSpec::Absolute;
                    layout.offset_left = Some(LengthSpec::Px(13.5));
                    layout.offset_top = Some(LengthSpec::Px(0.0));
                    layout.offset_bottom = Some(LengthSpec::Px(0.0));
                }),
        )?;
        Ok(Self {
            attachments: context.create_detached_component(document, Stack::column(4.0))?,
            attachment_buttons: HashMap::new(),
            attachment_event: String::new(),
            attachment_rows: HashMap::new(),
            attachment_thumbnails: HashMap::new(),
            images: Vec::new(),
            body: context.create_detached_component(
                document,
                Stack::column(7.0)
                    .grow(1.0)
                    .shrink(1.0)
                    .min_width(LengthSpec::Px(0.0)),
            )?,
            header: context.create_detached_component(
                document,
                Stack::bar(10.0).min_width(LengthSpec::Px(0.0)),
            )?,
            preview: context.create_detached_component(document, Text::new(""))?,
            rail,
            rail_line,
            node,
            markdown: context.create_detached_component(document, NativeMarkdown::parse(""))?,
            markdown_links_bound: false,
            selection_visible: false,
            title: context.create_detached_component(document, Text::new(""))?,
            actions: context.create_detached_component(document, Stack::row(6.0))?,
            buttons: HashMap::new(),
            source: String::new(),
        })
    }

    /// Release optional parked views as well as attached content when a
    /// virtual row leaves the materializer's live/retained key set.
    pub(crate) fn dispose_owned(self, context: &mut AppContext) -> Result<(), FrameworkError> {
        fn remove<V: nana_ui::runtime::View>(
            context: &mut AppContext,
            entity: Entity<V>,
        ) -> Result<(), FrameworkError> {
            if context.world().node(entity.stable_id()).is_some() {
                context.remove_view(entity)?;
            }
            Ok(())
        }
        for entity in [
            self.attachments,
            self.actions,
            self.body,
            self.header,
            self.rail,
            self.rail_line,
        ] {
            remove(context, entity)?;
        }
        for entity in [self.title, self.preview] {
            remove(context, entity)?;
        }
        remove(context, self.markdown)?;
        remove(context, self.node)?;
        for entity in self.attachment_rows.into_values() {
            remove(context, entity)?;
        }
        for entity in self.attachment_buttons.into_values() {
            remove(context, entity)?;
        }
        for entity in self.attachment_thumbnails.into_values() {
            remove(context, entity)?;
        }
        for entity in self.buttons.into_values() {
            remove(context, entity)?;
        }
        Ok(())
    }

    pub fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        root: Entity<Stack>,
        window: WindowId,
        row: &crate::runtime_shell::ShellTimelineRow,
        sink: &Sink,
    ) -> Result<(), FrameworkError> {
        if !self.markdown_links_bound {
            links::bind_markdown_links(
                context,
                self.markdown,
                window,
                row.id.clone(),
                sink.clone(),
            )?;
            self.markdown_links_bound = true;
        }
        if self.selection_visible && row.selected_text.is_none() {
            context.update_component(self.markdown, |view, _| view.clear_selection())?;
        }
        self.selection_visible = row.selected_text.is_some();
        context.update_component(root, |view, _| {
            *view = timeline_stack(row);
        })?;
        let images_changed = self.images != row.images;
        if self.source != row.markdown || images_changed {
            context.update_component(self.markdown, |view, _| {
                *view = NativeMarkdown::parse(&row.markdown);
                for image in &row.images {
                    view.resolve_image(
                        &image.source,
                        image.data_url.clone(),
                        image.width,
                        image.height,
                    );
                }
            })?;
            context.assemble_markdown(self.markdown)?;
            self.source = row.markdown.clone();
            self.images = row.images.clone();
        }
        context.update_component(self.markdown, |view, _| {
            let layout = Arc::make_mut(&mut view.style.layout);
            layout.width = Some(if row.role == "user" {
                LengthSpec::FitContent
            } else {
                LengthSpec::Fill
            });
            layout.max_width = Some(LengthSpec::Percent(100.0));
        })?;
        let role = row.role.strip_prefix("process-child:").unwrap_or(&row.role);
        let process = role != "user" && role != "assistant";
        let tone = match row.status.as_str() {
            "failed" | "error" | "cancelled" => SemanticColorRole::Danger,
            "running" | "started" | "in_progress" | "in-progress" | "pending" | "queued" => {
                SemanticColorRole::Accent
            }
            _ => SemanticColorRole::Muted,
        };
        let status = match row.status.as_str() {
            "running" | "started" | "in_progress" | "in-progress" => " · 进行中",
            "pending" => " · 等待处理",
            "queued" => " · 已排队",
            "failed" | "error" => " · 失败",
            "cancelled" => " · 已取消",
            _ => "",
        };
        let title = format!("{}{status}", row.title);
        context.update_component(self.title, |view, _| {
            *view = Text::new(title.clone());
            view.style.foreground = Some(if role == "reasoning" {
                SemanticColorRole::Text
            } else {
                tone
            });
            let layout = Arc::make_mut(&mut view.style.layout);
            layout.font_size = Some(13.0);
            layout.font_weight = Some(if role == "reasoning" { 600 } else { 500 });
            layout.min_height = Some(LengthSpec::Px(22.0));
        })?;
        context.update_component(self.preview, |view, _| {
            *view = Text::new(row.markdown.lines().next().unwrap_or_default());
            view.style.foreground = Some(SemanticColorRole::Muted);
            let layout = Arc::make_mut(&mut view.style.layout);
            layout.font_size = Some(12.0);
            layout.flex_grow = Some(1.0);
            layout.flex_shrink = Some(1.0);
            layout.min_width = Some(LengthSpec::Px(0.0));
            layout.white_space = nana_ui_core::WhiteSpaceSpec::Nowrap;
            layout.text_overflow_ellipsis = true;
        })?;
        context.update_component(self.node, |node, _| {
            *node = IconGlyph::new(match role {
                "assistant" | "reasoning" => Icon::Bot,
                "plan" => Icon::Nodes,
                "goal" => Icon::Sparkles,
                "file" | "file_change" => Icon::File,
                _ => Icon::Activity,
            })
            .size(13.0)
            .role(tone);
        })?;
        context.update_component(self.body, |body, _| {
            *body = Stack::column(if process && !row.expanded { 0.0 } else { 7.0 })
                .grow(1.0)
                .shrink(1.0)
                .min_width(LengthSpec::Px(0.0))
                .with_layout(|layout| layout.position = PositionSpec::Relative);
        })?;
        let mut actions = Vec::new();
        let mut desired = Vec::new();
        if row.can_expand {
            desired.push((
                "expand",
                if row.expanded { "收起" } else { "展开" },
                ConversationAction::ToggleEvent(row.id.clone()),
            ));
        }
        if row.can_copy && (!process || row.expanded) {
            desired.push(("copy", "复制", ConversationAction::Copy(row.id.clone())));
        }
        if row.can_retry {
            desired.push(("retry", "重试", ConversationAction::Retry(row.id.clone())));
        }
        if row.can_branch {
            desired.push((
                "continue",
                "从这里继续",
                ConversationAction::Continue(row.id.clone()),
            ));
            desired.push((
                "fork",
                "从这里分叉",
                ConversationAction::Fork(row.id.clone()),
            ));
        }
        if row.can_apply {
            desired.push((
                "apply",
                "应用建议",
                ConversationAction::ApplySuggestion(row.id.clone()),
            ));
        }
        if row.selected_text.is_some() {
            desired.retain(|(id, _, _)| process && *id == "expand");
            desired.extend([
                ("selection-copy", "复制", ConversationAction::CopySelection),
                (
                    "selection-quote",
                    "引用",
                    ConversationAction::QuoteSelection,
                ),
                ("selection-ask", "追问", ConversationAction::AskSelection),
            ]);
        }
        for (id, label, action) in desired {
            let button = if let Some(button) = self.buttons.get(id).copied() {
                context.update_component(button, |button, _| {
                    *button = pill_button(label, ButtonKind::Subtle);
                })?;
                button
            } else {
                let button = context
                    .create_detached_component(document, pill_button(label, ButtonKind::Subtle))?;
                bind_activate(context, button, sink.clone(), intent(window, action))?;
                self.buttons.insert(id.into(), button);
                button
            };
            if id == "expand" && process {
                context.update_component(button, |button, _| {
                    *button =
                        Button::new(format!("{} {title}", if row.expanded { "⌄" } else { "›" }))
                            .kind(ButtonKind::Text)
                            .size(ControlSize::Small)
                            .layout(
                                Stack::row(0.0)
                                    .height(LengthSpec::Px(22.0))
                                    .padding(0.0)
                                    .node_style()
                                    .layout,
                            );
                    button.style.foreground = Some(if role == "reasoning" {
                        SemanticColorRole::Text
                    } else {
                        tone
                    });
                    Arc::make_mut(&mut button.style.layout).font_size = Some(13.0);
                    Arc::make_mut(&mut button.style.layout).font_weight =
                        Some(if role == "reasoning" { 600 } else { 500 });
                })?;
            } else if id != "retry" || !process {
                actions.push(button.stable_id());
            }
        }
        context.reconcile_children(self.actions.stable_id(), &actions)?;
        let toolbar = links::selection_toolbar(
            context,
            self.markdown,
            if role == "user" { root } else { self.body },
            row.selected_text.is_some(),
        );
        context.update_component(self.actions, |view, _| *view = toolbar)?;
        let mut children = Vec::new();
        if process {
            let mut header = vec![if row.can_expand {
                self.buttons["expand"].stable_id()
            } else {
                self.title.stable_id()
            }];
            if !row.expanded
                && !row.markdown.trim().is_empty()
                && row.markdown.trim() != row.title.trim()
            {
                header.push(self.preview.stable_id());
            }
            if row.can_retry {
                header.push(self.buttons["retry"].stable_id());
            }
            context.reconcile_children(self.header.stable_id(), &header)?;
            children.push(self.header.stable_id());
        }
        if !row.markdown.trim().is_empty() && (!process || row.expanded) {
            children.push(self.markdown.stable_id());
        }
        if self.attachment_event != row.id {
            for (_, button) in self.attachment_buttons.drain() {
                context.remove_view(button)?;
            }
            for (_, image) in self.attachment_thumbnails.drain() {
                context.remove_view(image)?;
            }
            for (_, row) in self.attachment_rows.drain() {
                context.remove_view(row)?;
            }
            self.attachment_event = row.id.clone();
        }
        let mut attachment_nodes = Vec::new();
        let wanted = row
            .attachments
            .iter()
            .map(|attachment| attachment.id.as_str())
            .collect::<HashSet<_>>();
        for id in self
            .attachment_buttons
            .keys()
            .filter(|id| !wanted.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>()
        {
            if let Some(button) = self.attachment_buttons.remove(&id) {
                context.remove_view(button)?;
            }
            if let Some(image) = self.attachment_thumbnails.remove(&id) {
                context.remove_view(image)?;
            }
            if let Some(row) = self.attachment_rows.remove(&id) {
                context.remove_view(row)?;
            }
        }
        for attachment in &row.attachments {
            let label = format!("{} {}", attachment.reference_label(), attachment.name);
            let view = token_chip(&label, false).disabled(!attachment.exists);
            let button = if let Some(button) = self.attachment_buttons.get(&attachment.id).copied()
            {
                context.update_component(button, |button, _| *button = view)?;
                button
            } else {
                let button = context.create_detached_component(document, view)?;
                bind_activate(
                    context,
                    button,
                    sink.clone(),
                    intent(
                        window,
                        ConversationAction::PreviewTimelineAttachment {
                            event_id: row.id.clone(),
                            attachment_id: attachment.id.clone(),
                        },
                    ),
                )?;
                self.attachment_buttons
                    .insert(attachment.id.clone(), button);
                button
            };
            let attachment_row =
                if let Some(row) = self.attachment_rows.get(&attachment.id).copied() {
                    row
                } else {
                    let row = context.create_detached_component(document, Stack::row(4.0))?;
                    self.attachment_rows.insert(attachment.id.clone(), row);
                    row
                };
            let mut attachment_children = Vec::new();
            if let Some(resource) = row
                .images
                .iter()
                .find(|image| attachment.is_image() && image.source == attachment.path)
            {
                let thumbnail = if let Some(image) =
                    self.attachment_thumbnails.get(&attachment.id).copied()
                {
                    image
                } else {
                    let image =
                        context.create_detached_component(document, NativeMarkdown::parse(""))?;
                    let action = intent(
                        window,
                        ConversationAction::PreviewTimelineAttachment {
                            event_id: row.id.clone(),
                            attachment_id: attachment.id.clone(),
                        },
                    );
                    let sink = sink.clone();
                    context.on(
                        image,
                        move |_, event: &nana_ui::runtime::RichTextEvent, _| {
                            if matches!(event, nana_ui::runtime::RichTextEvent::ImageActivated(_)) {
                                emit(&sink, action.clone());
                            }
                        },
                    )?;
                    self.attachment_thumbnails
                        .insert(attachment.id.clone(), image);
                    image
                };
                if images_changed
                    || context
                        .world()
                        .node(thumbnail.stable_id())
                        .is_some_and(|node| node.parent.is_none())
                {
                    context.update_component(thumbnail, |view, _| {
                        *view = NativeMarkdown::parse("![图片](lilia-attachment-preview)");
                        let scale = 24.0 / resource.width.max(resource.height).max(1) as f32;
                        view.resolve_image(
                            "lilia-attachment-preview",
                            resource.data_url.clone(),
                            (resource.width as f32 * scale).round().max(1.0) as u32,
                            (resource.height as f32 * scale).round().max(1.0) as u32,
                        );
                        view.style = Stack::row(0.0)
                            .width(LengthSpec::Px(30.0))
                            .height(LengthSpec::Px(30.0))
                            .padding(3.0)
                            .surface(SemanticColorRole::Surface)
                            .radius(5.0)
                            .node_style();
                    })?;
                    context.assemble_markdown(thumbnail)?;
                }
                attachment_children.push(thumbnail.stable_id());
            } else if let Some(image) = self.attachment_thumbnails.remove(&attachment.id) {
                context.remove_view(image)?;
            }
            attachment_children.push(button.stable_id());
            context.reconcile_children(attachment_row.stable_id(), &attachment_children)?;
            attachment_nodes.push(attachment_row.stable_id());
        }
        context.reconcile_children(self.attachments.stable_id(), &attachment_nodes)?;
        if !attachment_nodes.is_empty() {
            children.push(self.attachments.stable_id());
        }
        if !actions.is_empty() {
            children.push(self.actions.stable_id());
        }
        if role == "user" {
            context
                .reconcile_children(root.stable_id(), &children)
                .map(|_| ())
        } else {
            context.reconcile_children(self.body.stable_id(), &children)?;
            context
                .reconcile_children(
                    root.stable_id(),
                    &[
                        self.rail_line.stable_id(),
                        self.rail.stable_id(),
                        self.body.stable_id(),
                    ],
                )
                .map(|_| ())
        }
    }
}

impl ConversationControlsHandles {
    #[cfg(debug_assertions)]
    pub(crate) fn debug_targets(&self) -> Vec<(String, nana_ui::runtime::StableNodeId)> {
        let mut targets = vec![
            ("model".into(), self.model.stable_id()),
            ("reasoning".into(), self.reasoning.stable_id()),
            ("worktree".into(), self.worktree.stable_id()),
            ("tools".into(), self.tools.stable_id()),
            ("permission".into(), self.permission.stable_id()),
            ("review-target".into(), self.review_target.stable_id()),
            ("review-value".into(), self.review_value.stable_id()),
        ];
        targets.extend(
            self.buttons
                .iter()
                .map(|(id, button)| (id.clone(), button.stable_id())),
        );
        targets
    }
    pub fn mount(
        context: &mut AppContext,
        document: DocumentId,
        window_id: WindowId,
        sink: Sink,
        popup: bool,
    ) -> Result<Self, FrameworkError> {
        let root = context.create_detached_component(document, Stack::column(6.0))?;
        let toolbar = context.create_detached_component(
            document,
            if popup {
                Stack::column(6.0).shrink(1.0)
            } else {
                wrapping_controls_row(6.0)
            },
        )?;
        let popup_tools_row =
            context.create_detached_component(document, wrapping_controls_row(6.0))?;
        let popup_model_row =
            context.create_detached_component(document, wrapping_controls_row(6.0))?;
        let popup_context_row =
            context.create_detached_component(document, wrapping_controls_row(6.0))?;
        let chips = context.create_detached_component(document, Stack::bar(4.0))?;
        let completion = context.create_detached_component(document, Stack::column(3.0))?;
        let workflow = context.create_detached_component(document, Stack::column(6.0))?;
        let model = context.create_detached_component(
            document,
            Dropdown::single(Some("")).size(ControlSize::Small),
        )?;
        let reasoning = context.create_detached_component(
            document,
            Dropdown::single(Some("medium")).size(ControlSize::Small),
        )?;
        let tools = context.create_detached_component(
            document,
            Dropdown::single(None::<String>)
                .placeholder("＋")
                .size(ControlSize::Small),
        )?;
        let permission = context.create_detached_component(
            document,
            Dropdown::single(Some("ask"))
                .size(ControlSize::Small)
                .options([
                    DropdownOption::new("readonly", "只读"),
                    DropdownOption::new("ask", "需要确认"),
                    DropdownOption::new("full", "完全访问"),
                ]),
        )?;
        let worktree = context.create_detached_component(
            document,
            Dropdown::single(Some("current")).size(ControlSize::Small),
        )?;
        for (field, width) in [
            (model, if popup { 150.0 } else { 204.0 }),
            (reasoning, 74.0),
            (tools, 40.0),
            (permission, 98.0),
            (worktree, 132.0),
        ] {
            context.update_component(field, |field, _| {
                let layout = Arc::make_mut(&mut field.style.layout);
                layout.width = Some(LengthSpec::Px(width));
                layout.min_width = Some(LengthSpec::Px(width));
                layout.border_width = Some(0.0);
                field.style.border = None;
                field.style.background = None;
            })?;
        }
        let tool_sink = sink.clone();
        context.on(tools, move |_, event: &DropdownEvent<Arc<str>>, _| {
            if let DropdownEvent::Select(value) = event {
                let action = match value.as_ref() {
                    "files" => ConversationAction::AddFiles,
                    "directories" => ConversationAction::AddDirectories,
                    "reference" => ConversationAction::Reference,
                    "plan" => ConversationAction::Plan,
                    "goal" => ConversationAction::Goal,
                    "new-guide" => ConversationAction::NewGuide,
                    "edit-goal" => ConversationAction::EditGoal,
                    _ => return,
                };
                emit(&tool_sink, intent(window_id, action));
            }
        })?;
        let review_target = context.create_detached_component(
            document,
            Dropdown::single(Some("changes"))
                .size(ControlSize::Small)
                .options([
                    DropdownOption::new("changes", "未提交的改动"),
                    DropdownOption::new("branch", "与分支比较"),
                    DropdownOption::new("commit", "指定提交"),
                ]),
        )?;
        for (field, map) in [
            (
                model,
                ConversationAction::Model as fn(String) -> ConversationAction,
            ),
            (reasoning, ConversationAction::Reasoning),
            (review_target, ConversationAction::ReviewTarget),
            (permission, ConversationAction::Permission),
            (worktree, ConversationAction::Worktree),
        ] {
            let sink = sink.clone();
            context.on(field, move |_, event: &DropdownEvent<Arc<str>>, _| {
                if let DropdownEvent::Select(value) = event {
                    emit(&sink, intent(window_id, map(value.to_string())));
                }
            })?;
        }
        let review_value =
            context.create_detached_component(document, TextArea::new("").height(36.0))?;
        let field_sink = sink.clone();
        context.on(review_value, move |_, event: &TextChanged, _| {
            emit(
                &field_sink,
                intent(
                    window_id,
                    ConversationAction::ReviewValue(event.value.clone()),
                ),
            );
        })?;
        let empty = context.create_detached_component(document, Text::new("没有匹配的对话"))?;
        Ok(Self {
            root,
            toolbar,
            popup_tools_row,
            popup_model_row,
            popup_context_row,
            chips,
            completion,
            workflow,
            model,
            reasoning,
            tools,
            permission,
            worktree,
            review_target,
            review_value,
            empty,
            buttons: HashMap::new(),
            window_id,
            sink,
            popup,
            keyboard: keyboard::ComposerKeyboard::default(),
        })
    }

    pub fn bind_composer_keyboard(
        &self,
        context: &mut AppContext,
        composer: Entity<TextArea>,
    ) -> Result<(), FrameworkError> {
        self.keyboard
            .bind(context, composer, self.window_id, self.sink.clone())
    }

    pub fn sync_composer_keyboard(
        &self,
        state: &ConversationControls,
        content: &str,
        can_send: bool,
    ) {
        self.keyboard.sync(state, content, can_send);
    }

    pub fn active_completion(&self, action: &ConversationAction) -> bool {
        self.keyboard.is_active(action)
    }

    fn button(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        keep: &mut HashSet<String>,
        id: String,
        label: &str,
        action: ConversationAction,
        disabled: bool,
    ) -> Result<nana_ui::runtime::StableNodeId, FrameworkError> {
        keep.insert(id.clone());
        let selected = self.keyboard.is_active(&action);
        let button = if let Some(button) = self.buttons.get(&id).copied() {
            context.update_component(button, |button, _| {
                *button = token_chip(label, selected).disabled(disabled);
            })?;
            button
        } else {
            let view = token_chip(label, selected).disabled(disabled);
            let button = context.create_detached_component(document, view)?;
            bind_activate(
                context,
                button,
                self.sink.clone(),
                intent(self.window_id, action),
            )?;
            self.buttons.insert(id, button);
            button
        };
        Ok(button.stable_id())
    }

    pub fn sync(
        &mut self,
        context: &mut AppContext,
        document: DocumentId,
        state: &ConversationControls,
    ) -> Result<(), FrameworkError> {
        let mut keep = HashSet::new();
        context.update_component(self.model, |field, _| {
            field.selection = DropdownSelection::Single(Some(Arc::from(state.model.as_str())));
            field.options = state
                .models
                .iter()
                .map(|(id, label)| {
                    DropdownOption::new(
                        id.clone(),
                        if id.is_empty() {
                            state.model_label.clone()
                        } else {
                            label.clone()
                        },
                    )
                })
                .collect();
            field.disabled = state.locked;
        })?;
        context.update_component(self.reasoning, |field, _| {
            field.selection = DropdownSelection::Single(Some(Arc::from(state.reasoning.as_str())));
            field.options = [
                ("low", "低"),
                ("medium", "中"),
                ("high", "高"),
                ("xhigh", "超高"),
                ("max", "最高"),
            ]
            .into_iter()
            .map(|(id, label)| DropdownOption::new(id, label))
            .collect();
            field.disabled = state.locked;
        })?;
        let mut toolbar = vec![self.model.stable_id(), self.reasoning.stable_id()];
        if self.popup {
            context.update_component(self.tools, |field, _| {
                field.selection = DropdownSelection::Single(None);
                field.options = vec![
                    DropdownOption::new("files", "添加文件"),
                    DropdownOption::new("directories", "添加目录"),
                    DropdownOption::new("reference", "引用对话"),
                    DropdownOption::new(
                        "plan",
                        if state.plan {
                            "关闭计划模式"
                        } else {
                            "计划模式"
                        },
                    ),
                    DropdownOption::new(
                        "goal",
                        if state.goal {
                            "关闭目标模式"
                        } else {
                            "目标模式"
                        },
                    ),
                ];
                if state.can_manage_todos {
                    field
                        .options
                        .push(DropdownOption::new("new-guide", "添加引导"));
                    field
                        .options
                        .push(DropdownOption::new("edit-goal", "设置目标"));
                }
                field.disabled = state.locked;
            })?;
            context.update_component(self.permission, |field, _| {
                field.selection =
                    DropdownSelection::Single(Some(Arc::from(state.permission.as_str())));
                field.disabled = state.locked;
            })?;
            if let Some((selection, label)) = &state.worktree {
                context.update_component(self.worktree, |field, _| {
                    field.selection =
                        DropdownSelection::Single(Some(Arc::from(selection.as_str())));
                    field.options = [
                        DropdownOption::new("current", "当前目录"),
                        DropdownOption::new("create", "新建工作树"),
                        DropdownOption::new(
                            "existing",
                            if selection == "existing" {
                                label.as_str()
                            } else {
                                "已有工作树…"
                            },
                        ),
                    ]
                    .into();
                    field.disabled = state.worktree_locked;
                })?;
                toolbar.insert(0, self.worktree.stable_id());
            }
            toolbar.insert(0, self.permission.stable_id());
            toolbar.insert(0, self.tools.stable_id());
        }
        let mut context_actions = Vec::new();
        if let Some(label) = &state.context_label {
            context_actions.push(self.button(
                context,
                document,
                &mut keep,
                "compact".into(),
                label,
                ConversationAction::Compact,
                !state.can_compact,
            )?);
        }
        toolbar.push(self.button(
            context,
            document,
            &mut keep,
            "optimize".into(),
            if state.optimizing {
                "优化中…"
            } else if self.popup {
                "优化"
            } else {
                "优化提示词"
            },
            ConversationAction::Optimize,
            !state.can_optimize || state.optimizing,
        )?);
        if self.popup {
            let mut tools = vec![self.tools.stable_id(), self.permission.stable_id()];
            if state.worktree.is_some() {
                tools.push(self.worktree.stable_id());
            }
            if let Some(task_id) = &state.worktree_merge_task {
                tools.push(self.button(
                    context,
                    document,
                    &mut keep,
                    format!("worktree-merge-{task_id}"),
                    "合并并归档",
                    ConversationAction::RequestWorktreeMerge(task_id.clone()),
                    state.worktree_locked,
                )?);
            }
            context.reconcile_children(self.popup_tools_row.stable_id(), &tools)?;
            context.reconcile_children(
                self.popup_model_row.stable_id(),
                &[
                    self.model.stable_id(),
                    self.reasoning.stable_id(),
                    self.buttons["optimize"].stable_id(),
                ],
            )?;
            context.reconcile_children(self.popup_context_row.stable_id(), &context_actions)?;
            let mut rows = vec![
                self.popup_tools_row.stable_id(),
                self.popup_model_row.stable_id(),
            ];
            if !context_actions.is_empty() {
                rows.push(self.popup_context_row.stable_id());
            }
            context.reconcile_children(self.toolbar.stable_id(), &rows)?;
        } else {
            toolbar.extend(context_actions);
            context.reconcile_children(self.toolbar.stable_id(), &toolbar)?;
        }

        let mut chips = Vec::new();
        for (id, label) in &state.attachments {
            chips.push(self.button(
                context,
                document,
                &mut keep,
                format!("attachment-open-{id}"),
                label,
                ConversationAction::PreviewAttachment(id.clone()),
                false,
            )?);
            chips.push(self.button(
                context,
                document,
                &mut keep,
                format!("attachment-remove-{id}"),
                "移除附件",
                ConversationAction::RemoveAttachment(id.clone()),
                state.locked,
            )?);
        }
        for (id, label) in &state.references {
            chips.push(self.button(
                context,
                document,
                &mut keep,
                format!("reference-remove-{id}"),
                &format!("# {label} ×"),
                ConversationAction::RemoveReference(id.clone()),
                state.locked,
            )?);
        }
        context.reconcile_children(self.chips.stable_id(), &chips)?;

        let mut results = Vec::new();
        for (id, label) in &state.reference_results {
            results.push(self.button(
                context,
                document,
                &mut keep,
                format!("reference-result-{id}"),
                label,
                ConversationAction::SelectReference(id.clone()),
                state.locked,
            )?);
        }
        if state.reference_query && results.is_empty() {
            results.push(self.empty.stable_id());
        }
        if self.popup {
            for (id, label) in &state.slash_results {
                results.push(self.button(
                    context,
                    document,
                    &mut keep,
                    format!("slash-{id}"),
                    label,
                    ConversationAction::Slash(id.clone()),
                    state.locked,
                )?);
            }
            for (id, label) in &state.context_results {
                results.push(self.button(
                    context,
                    document,
                    &mut keep,
                    format!("context-{id}"),
                    label,
                    ConversationAction::Context(id.clone()),
                    state.locked,
                )?);
            }
        }
        context.reconcile_children(self.completion.stable_id(), &results)?;

        let mut workflow = Vec::new();
        if let Some(task_id) = &state.worktree_merge_confirmation {
            workflow.push(self.button(
                context,
                document,
                &mut keep,
                format!("worktree-merge-confirm-{task_id}"),
                "确认合并、删除工作树并归档",
                ConversationAction::ConfirmWorktreeMerge(task_id.clone()),
                state.worktree_locked,
            )?);
            workflow.push(self.button(
                context,
                document,
                &mut keep,
                "worktree-merge-cancel".into(),
                "取消",
                ConversationAction::CancelWorktreeMerge,
                false,
            )?);
        }

        if let Some((target, value)) = &state.review {
            context.update_component(self.review_target, |field, _| {
                field.selection = DropdownSelection::Single(Some(Arc::from(target.as_str())));
            })?;
            workflow.push(self.review_target.stable_id());
            if target != "changes" {
                context.update_component(self.review_value, |field, _| {
                    field.placeholder = Arc::from(if target == "branch" {
                        "分支名称"
                    } else {
                        "提交 SHA"
                    });
                    if field.state.value != *value {
                        field.state.replace_value(value.clone());
                    }
                })?;
                workflow.push(self.review_value.stable_id());
            }
            workflow.push(self.button(
                context,
                document,
                &mut keep,
                "review-submit".into(),
                "开始审查",
                ConversationAction::SubmitReview,
                state.locked || (target != "changes" && value.trim().is_empty()),
            )?);
            workflow.push(self.button(
                context,
                document,
                &mut keep,
                "review-cancel".into(),
                "取消",
                ConversationAction::CancelReview,
                false,
            )?);
        }
        if let Some(label) = &state.branch {
            workflow.push(self.button(
                context,
                document,
                &mut keep,
                "branch-clear".into(),
                &format!("{label} · 取消"),
                ConversationAction::ClearBranch,
                false,
            )?);
        }
        if let Some(applied) = state.prompt_workflow {
            workflow.push(self.button(
                context,
                document,
                &mut keep,
                "prompt-workflow-apply".into(),
                if applied {
                    "已采用建议流程"
                } else {
                    "采用建议流程"
                },
                ConversationAction::ApplyPromptWorkflow,
                state.locked || applied,
            )?);
            workflow.push(self.button(
                context,
                document,
                &mut keep,
                "prompt-workflow-dismiss".into(),
                "仅发送提示词",
                ConversationAction::DismissPromptWorkflow,
                state.locked,
            )?);
        }
        context.reconcile_children(self.workflow.stable_id(), &workflow)?;
        let mut root = Vec::new();
        if !results.is_empty() {
            root.push(self.completion.stable_id());
        }
        if !chips.is_empty() {
            root.push(self.chips.stable_id());
        }
        if !workflow.is_empty() {
            root.push(self.workflow.stable_id());
        }
        context.reconcile_children(self.root.stable_id(), &root)?;
        let stale = self
            .buttons
            .keys()
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(button) = self.buttons.remove(&key) {
                let _ = context.remove_view(button);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn reference_selection_and_review_submission_use_real_control_events() {
        let document_id = DocumentId::new(901).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, Stack::column(4.0))
            .unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let events = observed.clone();
        let mut controls = ConversationControlsHandles::mount(
            context,
            document_id,
            WindowId(42),
            Arc::new(move |event| {
                events.lock().unwrap().push(event);
            }),
            true,
        )
        .unwrap();
        context.append_child(root, controls.root).unwrap();
        controls
            .sync(
                context,
                document_id,
                &ConversationControls {
                    reference_results: vec![("other-task".into(), "相关对话".into())],
                    review: Some(("changes".into(), String::new())),
                    models: vec![(String::new(), "自动".into())],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(context
            .activate_node(controls.buttons["reference-result-other-task"].stable_id())
            .unwrap());
        assert!(context
            .activate_node(controls.buttons["review-submit"].stable_id())
            .unwrap());
        let events = observed.lock().unwrap();
        assert!(
            matches!(&events[0], ShellIntent::Conversation { window_id: WindowId(42), action: ConversationAction::SelectReference(id) } if id == "other-task")
        );
        assert!(matches!(
            &events[1],
            ShellIntent::Conversation {
                window_id: WindowId(42),
                action: ConversationAction::SubmitReview
            }
        ));
    }

    #[test]
    fn popup_worktree_selection_uses_the_popup_window_and_disables_during_operations() {
        let document_id = DocumentId::new(905).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, Stack::column(4.0))
            .unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let events = observed.clone();
        let mut controls = ConversationControlsHandles::mount(
            context,
            document_id,
            WindowId(44),
            Arc::new(move |event| events.lock().unwrap().push(event)),
            true,
        )
        .unwrap();
        context.append_child(root, controls.toolbar).unwrap();
        let mut state = ConversationControls {
            worktree: Some(("current".into(), "当前目录".into())),
            ..Default::default()
        };
        controls.sync(context, document_id, &state).unwrap();
        assert!(context
            .focus_node(document_id, controls.worktree.stable_id())
            .unwrap());
        assert!(context.adjust_focused_dropdown(document_id, 1).unwrap());
        assert!(context.adjust_focused_dropdown(document_id, 1).unwrap());
        assert!(context.commit_focused_dropdown(document_id).unwrap());
        assert!(observed.lock().unwrap().iter().any(|event| matches!(event, ShellIntent::Conversation { window_id: WindowId(44), action: ConversationAction::Worktree(value) } if value == "create")));
        state.worktree_locked = true;
        controls.sync(context, document_id, &state).unwrap();
        assert!(!context.adjust_focused_dropdown(document_id, 1).unwrap());
    }

    #[test]
    fn popup_worktree_merge_requires_visible_confirmation_and_keeps_task_identity() {
        let document_id = DocumentId::new(906).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, Stack::column(4.0))
            .unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let events = observed.clone();
        let mut controls = ConversationControlsHandles::mount(
            context,
            document_id,
            WindowId(44),
            Arc::new(move |event| events.lock().unwrap().push(event)),
            true,
        )
        .unwrap();
        context.append_child(root, controls.toolbar).unwrap();
        context.append_child(root, controls.root).unwrap();
        let mut state = ConversationControls {
            worktree: Some(("existing".into(), "branch".into())),
            worktree_merge_task: Some("task-one".into()),
            ..Default::default()
        };
        controls.sync(context, document_id, &state).unwrap();
        assert!(!controls
            .buttons
            .contains_key("worktree-merge-confirm-task-one"));
        assert!(context
            .activate_node(controls.buttons["worktree-merge-task-one"].stable_id())
            .unwrap());
        assert!(
            matches!(observed.lock().unwrap().last(), Some(ShellIntent::Conversation {
            window_id: WindowId(44), action: ConversationAction::RequestWorktreeMerge(id),
        }) if id == "task-one")
        );
        state.worktree_merge_confirmation = Some("task-one".into());
        controls.sync(context, document_id, &state).unwrap();
        assert!(context
            .activate_node(controls.buttons["worktree-merge-cancel"].stable_id())
            .unwrap());
        assert!(matches!(
            observed.lock().unwrap().last(),
            Some(ShellIntent::Conversation {
                window_id: WindowId(44),
                action: ConversationAction::CancelWorktreeMerge,
            })
        ));
        assert!(context
            .activate_node(controls.buttons["worktree-merge-confirm-task-one"].stable_id())
            .unwrap());
        assert!(
            matches!(observed.lock().unwrap().last(), Some(ShellIntent::Conversation {
            window_id: WindowId(44), action: ConversationAction::ConfirmWorktreeMerge(id),
        }) if id == "task-one")
        );
        state.worktree_locked = true;
        controls.sync(context, document_id, &state).unwrap();
        assert!(!context
            .activate_node(controls.buttons["worktree-merge-confirm-task-one"].stable_id())
            .unwrap());
        state.worktree_merge_confirmation = None;
        state.worktree_merge_task = Some("task-two".into());
        controls.sync(context, document_id, &state).unwrap();
        assert!(!controls
            .buttons
            .contains_key("worktree-merge-confirm-task-one"));
        assert!(!controls.buttons.contains_key("worktree-merge-task-one"));
    }

    #[test]
    fn user_bubbles_fit_short_messages_and_limit_long_messages() {
        let document_id = DocumentId::new(903).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let parent = context
            .create_component(document_id, Stack::fill_column(4.0))
            .unwrap();
        let sink: Sink = Arc::new(|_| {});
        let mut bubbles = Vec::new();
        let mut action_rows = Vec::new();
        for (id, markdown) in [
            ("short", "你好".to_owned()),
            ("long", "一段需要换行的长消息。".repeat(40)),
            ("review", "Review the changes introduced by commit \"0123456789abcdef0123456789abcdef01234567\" compared with its parent. Inspect the actual diff and enough surrounding code to verify each issue. Do not modify files. Prioritize actionable correctness, regression, security, and missing-test findings; include severity, file and line, trigger, and impact. If no actionable issues are found, say so and describe relevant validation limits. Report the findings in this conversation.\n\nAdditional user input:\n验证审批与草稿恢复".to_owned()),
        ] {
            let root = context
                .create_detached_component(document_id, Stack::column(0.0))
                .unwrap();
            context.append_child(parent, root).unwrap();
            let mut content = TimelineContent::mount(context, document_id).unwrap();
            content
                .sync(
                    context,
                    document_id,
                    root,
                    WindowId::PRIMARY,
                    &crate::runtime_shell::ShellTimelineRow {
                        selected_text: None,
                        attachments: Vec::new(),
                        images: Vec::new(),
                        id: id.into(),
                        title: String::new(),
                        role: "user".into(),
                        status: "completed".into(),
                        markdown,
                        expanded: true,
                        can_expand: false,
                        can_retry: false,
                        can_copy: true,
                        can_branch: false,
                        can_apply: false,
                    },
                    &sink,
                )
                .unwrap();
            bubbles.push(root.stable_id());
            action_rows.push(content.actions.stable_id());
        }
        context
            .layout_document(
                document_id,
                nana_ui::runtime::LayoutViewport::new(760.0, 900.0),
            )
            .unwrap();
        for (bubble, actions) in bubbles.iter().zip(&action_rows) {
            let bubble = context.world().layout_box(*bubble).unwrap();
            let actions = context.world().layout_box(*actions).unwrap();
            assert!(
                actions.y + actions.height <= bubble.y + bubble.height,
                "flow actions must fit the bubble: {bubble:?} {actions:?}"
            );
        }
        let short = context.world().layout_box(bubbles[0]).unwrap();
        let long = context.world().layout_box(bubbles[1]).unwrap();
        assert!(
            short.width < 160.0,
            "short bubble must fit its text and actions: {short:?}"
        );
        assert!(
            long.width <= 760.0 * 0.76 + 0.5 && long.width > short.width,
            "long bubble must wrap at its maximum: {long:?}"
        );
        for bounds in [short, long] {
            assert!(
                (bounds.x + bounds.width - 760.0).abs() < 0.5,
                "bubble must align right: {bounds:?}"
            );
        }
    }

    #[test]
    fn shared_timeline_fork_button_preserves_window_and_event_identity() {
        let document_id = DocumentId::new(902).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let root = context
            .create_component(document_id, Stack::column(4.0))
            .unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let events = observed.clone();
        let sink: Sink = Arc::new(move |event| {
            events.lock().unwrap().push(event);
        });
        let mut content = TimelineContent::mount(context, document_id).unwrap();
        content
            .sync(
                context,
                document_id,
                root,
                WindowId(43),
                &crate::runtime_shell::ShellTimelineRow {
                    selected_text: None,
                    attachments: Vec::new(),
                    images: Vec::new(),
                    id: "reply-7".into(),
                    title: "回复".into(),
                    role: "assistant".into(),
                    status: "completed".into(),
                    markdown: "已完成修改。".into(),
                    expanded: true,
                    can_expand: false,
                    can_retry: false,
                    can_copy: true,
                    can_branch: true,
                    can_apply: true,
                },
                &sink,
            )
            .unwrap();
        assert!(context
            .activate_node(content.buttons["fork"].stable_id())
            .unwrap());
        assert!(
            matches!(&observed.lock().unwrap()[0], ShellIntent::Conversation { window_id: WindowId(43), action: ConversationAction::Fork(id) } if id == "reply-7")
        );
    }
    #[test]
    fn process_timeline_has_historical_rail_geometry_and_normal_expand_action() {
        let document_id = DocumentId::new(922).unwrap();
        let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
        let context = document.context_mut();
        let parent = context
            .create_component(document_id, Stack::column(0.0))
            .unwrap();
        let root = context
            .create_detached_component(document_id, Stack::column(0.0))
            .unwrap();
        context.append_child(parent, root).unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let messages = observed.clone();
        let sink: Sink = Arc::new(move |event| messages.lock().unwrap().push(event));
        let mut content = TimelineContent::mount(context, document_id).unwrap();
        let mut row = crate::runtime_shell::ShellTimelineRow {
            selected_text: None,
            images: Vec::new(),
            attachments: Vec::new(),
            id: "tool-event".into(),
            title: "读取文件".into(),
            role: "tool".into(),
            status: "running".into(),
            can_branch: false,
            can_apply: false,
            markdown: "src/main.rs".into(),
            expanded: false,
            can_expand: true,
            can_retry: false,
            can_copy: true,
        };
        content
            .sync(context, document_id, root, WindowId(43), &row, &sink)
            .unwrap();
        context
            .layout_document(
                document_id,
                nana_ui::runtime::LayoutViewport::new(760.0, 600.0),
            )
            .unwrap();
        let bounds = context.world().layout_box(root.stable_id()).unwrap();
        let rail = context
            .world()
            .layout_box(content.rail.stable_id())
            .unwrap();
        let line = context
            .world()
            .layout_box(content.rail_line.stable_id())
            .unwrap();
        let body = context
            .world()
            .layout_box(content.body.stable_id())
            .unwrap();
        let node = context
            .world()
            .layout_box(content.node.stable_id())
            .unwrap();
        assert!((rail.width - 28.0).abs() < 0.1, "{rail:?}");
        assert!((body.x - bounds.x - 28.0).abs() < 0.1, "{body:?}");
        assert!(
            (line.x - bounds.x - 13.5).abs() < 0.1 && (line.width - 1.0).abs() < 0.1,
            "{line:?}"
        );
        assert!(
            (node.x + node.width * 0.5 - bounds.x - 14.0).abs() < 0.1,
            "{node:?}"
        );
        assert!(
            bounds.height <= 40.0,
            "collapsed process stays one row: {bounds:?}"
        );
        assert!(context
            .activate_node(content.buttons["expand"].stable_id())
            .unwrap());
        assert!(
            matches!(&observed.lock().unwrap()[0], ShellIntent::Conversation { window_id: WindowId(43), action: ConversationAction::ToggleEvent(id) } if id == "tool-event")
        );
        row.expanded = true;
        content
            .sync(context, document_id, root, WindowId(43), &row, &sink)
            .unwrap();
        assert!(context
            .world()
            .node(content.body.stable_id())
            .unwrap()
            .children
            .contains(&content.markdown.stable_id()));
        assert!(context
            .activate_node(content.buttons["copy"].stable_id())
            .unwrap());
        assert!(
            matches!(&observed.lock().unwrap()[1], ShellIntent::Conversation { window_id: WindowId(43), action: ConversationAction::Copy(id) } if id == "tool-event")
        );
    }

    #[test]
    fn history_attachment_buttons_preview_the_bound_event_in_both_windows() {
        for (index, window) in [WindowId::PRIMARY, WindowId(44)].into_iter().enumerate() {
            let document_id = DocumentId::new(930 + index as u64).unwrap();
            let mut document = nana_ui::runtime::RuntimeDocument::new(document_id);
            let context = document.context_mut();
            let root = context
                .create_component(document_id, Stack::column(0.0))
                .unwrap();
            let observed = Arc::new(Mutex::new(Vec::new()));
            let messages = observed.clone();
            let sink: Sink = Arc::new(move |event| messages.lock().unwrap().push(event));
            let mut content = TimelineContent::mount(context, document_id).unwrap();
            let attachment = crate::application::ChatAttachment {
                id: "capture".into(),
                name: "capture.png".into(),
                path: "/tmp/capture.png".into(),
                kind: crate::application::ChatAttachmentKind::File,
                size: Some(42),
                exists: true,
                mime: Some("image/png".into()),
                directory: None,
            };
            let mut row = crate::runtime_shell::ShellTimelineRow {
                selected_text: None,
                images: Vec::new(),
                attachments: vec![attachment],
                id: "sent-message".into(),
                title: "消息".into(),
                role: "user".into(),
                status: "completed".into(),
                markdown: String::new(),
                expanded: false,
                can_expand: false,
                can_copy: false,
                can_retry: false,
                can_branch: false,
                can_apply: false,
            };
            content
                .sync(context, document_id, root, window, &row, &sink)
                .unwrap();
            context
                .layout_document(
                    document_id,
                    nana_ui::runtime::LayoutViewport::new(760.0, 600.0),
                )
                .unwrap();
            let button = content.attachment_buttons["capture"];
            assert!(
                context
                    .world()
                    .layout_box(button.stable_id())
                    .unwrap()
                    .height
                    > 0.0
            );
            assert!(context.activate_node(button.stable_id()).unwrap());
            assert!(
                matches!(&observed.lock().unwrap()[0], ShellIntent::Conversation { window_id, action: ConversationAction::PreviewTimelineAttachment { event_id, attachment_id } }
                if *window_id == window && event_id == "sent-message" && attachment_id == "capture")
            );
            row.id = "older-message".into();
            row.attachments[0].name = "older.png".into();
            content
                .sync(context, document_id, root, window, &row, &sink)
                .unwrap();
            assert!(!context.world().contains(button.stable_id()));
            assert!(context
                .activate_node(content.attachment_buttons["capture"].stable_id())
                .unwrap());
            assert!(
                matches!(&observed.lock().unwrap()[1], ShellIntent::Conversation { window_id, action: ConversationAction::PreviewTimelineAttachment { event_id, .. } }
                if *window_id == window && event_id == "older-message")
            );
            row.markdown = "![capture](/tmp/capture.png)".into();
            content
                .sync(context, document_id, root, window, &row, &sink)
                .unwrap();
            assert!(context.read(content.markdown, |markdown| markdown.blocks().iter().all(|block| {
                !matches!(block, nana_ui::MarkdownBlock::Text { spans, .. } if spans.iter().any(|span| span.image_resource.is_some()))
            })).unwrap());
            let loaded = crate::markdown_images::load_markdown_image(concat!("data:image/png;base64,",
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")).unwrap();
            row.images.push(crate::runtime_shell::ShellTimelineImage {
                source: "/tmp/capture.png".into(),
                data_url: loaded.data_url(),
                width: loaded.pixels.width,
                height: loaded.pixels.height,
            });
            content
                .sync(context, document_id, root, window, &row, &sink)
                .unwrap();
            assert!(context.read(content.markdown, |markdown| markdown.blocks().iter().any(|block| {
                matches!(block, nana_ui::MarkdownBlock::Text { spans, .. } if spans.iter().any(|span| span.image_resource.as_ref().is_some_and(|image| image.width == 1 && image.height == 1)))
            })).unwrap());
            let thumbnail = content.attachment_thumbnails["capture"];
            context
                .layout_document(
                    document_id,
                    nana_ui::runtime::LayoutViewport::new(760.0, 600.0),
                )
                .unwrap();
            context.rebuild_hit_test(document_id);
            let bounds = context.world().layout_box(thumbnail.stable_id()).unwrap();
            assert_eq!((bounds.width, bounds.height), (30.0, 30.0));
            observed.lock().unwrap().clear();
            let (x, y) = (
                bounds.x + bounds.width * 0.5,
                bounds.y + bounds.height * 0.5,
            );
            assert!(
                context
                    .begin_rich_text_pointer(document_id, 9, thumbnail.stable_id(), x, y)
                    .unwrap(),
                "thumbnail {bounds:?}, root {:?}, row {:?}",
                context.world().layout_box(root.stable_id()),
                context
                    .world()
                    .layout_box(content.attachment_rows["capture"].stable_id())
            );
            assert!(context
                .end_rich_text_pointer(document_id, 9, x, y, false)
                .unwrap());
            assert!(
                matches!(&observed.lock().unwrap()[0], ShellIntent::Conversation { window_id, action: ConversationAction::PreviewTimelineAttachment { event_id, attachment_id } }
                if *window_id == window && event_id == "older-message" && attachment_id == "capture")
            );
            row.images.clear();
            row.attachments.clear();
            content
                .sync(context, document_id, root, window, &row, &sink)
                .unwrap();
            assert!(content.attachment_buttons.is_empty());
            assert!(!context
                .world()
                .node(root.stable_id())
                .unwrap()
                .children
                .contains(&content.attachments.stable_id()));
        }
    }
}

pub(crate) fn wrapping_controls_row(gap: f32) -> Stack {
    Stack::row(gap)
        .min_width(LengthSpec::Px(0.0))
        .shrink(1.0)
        .with_layout(|layout| {
            layout.max_width = Some(LengthSpec::Percent(100.0));
            layout.flex_wrap = nana_ui_core::FlexWrap::Wrap;
        })
}
