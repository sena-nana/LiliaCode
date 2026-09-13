//! The composer input surface as a UI module.
//!
//! Owns the per-window draft, the editor, and the slash / mention / reference
//! suggestion lists. Submit, todo, goal and the task session itself stay in the
//! shell. One instance per window: a popup's composer is that window's host,
//! not a field on the popup struct.

pub mod pending_view;
pub mod presentation;
pub mod view;

use lilia_contracts::{ChatContextSearchResult, ChatConversationReference, ProjectId, TaskId};
use lilia_kernel::FeatureId;

use self::presentation::{ComposerAttachment, ComposerMentionItem, ComposerSlashItem};
use crate::application::{
    DesktopComposerCommand, DesktopComposerState, DesktopSlashCommandSearchResult,
};
use crate::text_editor_state::TextEditorState;
use crate::ui_module::{UiModule, UiModuleContext, UiModuleOutcome};

const TEXTAREA_MIN_HEIGHT: f32 = 32.0;
const TEXTAREA_LINE_HEIGHT: f32 = 20.0;
const TEXTAREA_MAX_HEIGHT: f32 = 72.0;

/// The composer's own message vocabulary. Window identity comes from the host,
/// so popup and primary share one enum.
#[derive(Debug, Clone)]
pub enum ComposerMessage {
    SetMenu(Option<view::ComposerMenuKind>),
    ToggleMenu(view::ComposerMenuKind),
    Refresh,
    LoadTransient {
        composer: DesktopComposerState,
        project_id: Option<ProjectId>,
    },
    Clear,
    SetContent(String),
    SetContentAt {
        task_id: TaskId,
        expected_revision: u64,
        expected_content: String,
        value: String,
    },
    ApplyCommand(DesktopComposerCommand),
    SelectConversationReference(String),
    SelectContextAttachment(String),
    RefreshSuggestions,
    ClearSuggestions,
}

pub struct ComposerModule {
    menu: Option<view::ComposerMenuKind>,
    composer: Option<DesktopComposerState>,
    composer_editor: TextEditorState,
    slash_commands: Vec<DesktopSlashCommandSearchResult>,
    conversation_reference_results: Vec<ChatConversationReference>,
    context_attachment_results: Vec<ChatContextSearchResult>,
    transient: bool,
    transient_project: Option<ProjectId>,
    error: Option<String>,
}

impl Default for ComposerModule {
    fn default() -> Self {
        Self {
            menu: None,
            composer: None,
            composer_editor: TextEditorState::new(),
            slash_commands: Vec::new(),
            conversation_reference_results: Vec::new(),
            context_attachment_results: Vec::new(),
            transient: false,
            transient_project: None,
            error: None,
        }
    }
}

impl ComposerModule {
    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.composer").expect("the composer feature id is not blank")
    }

    pub fn composer(&self) -> Option<&DesktopComposerState> {
        self.composer.as_ref()
    }

    pub(crate) fn matching_task(&self, selected: Option<TaskId>) -> Option<TaskId> {
        let selected = selected?;
        self.composer
            .as_ref()
            .filter(|composer| composer.task_id == selected)
            .map(|_| selected)
    }

    pub fn composer_editor(&self) -> &TextEditorState {
        &self.composer_editor
    }

    pub fn slash_commands(&self) -> &[DesktopSlashCommandSearchResult] {
        &self.slash_commands
    }

    pub fn conversation_reference_results(&self) -> &[ChatConversationReference] {
        &self.conversation_reference_results
    }

    pub fn context_attachment_results(&self) -> &[ChatContextSearchResult] {
        &self.context_attachment_results
    }

    pub(crate) fn view_snapshot(
        &self,
        window_id: nana_ui_platform::WindowId,
    ) -> view::ComposerViewSnapshot {
        let mut snapshot = view::ComposerViewSnapshot {
            window_id,
            composer_plus_open: self.menu == Some(view::ComposerMenuKind::Actions),
            composer_permission_menu_open: self.menu == Some(view::ComposerMenuKind::Permission),
            composer_worktree_menu_open: self.menu == Some(view::ComposerMenuKind::Worktree),
            composer: self.composer_editor.text(),
            composer_height: textarea_height(&self.composer_editor),
            composer_placeholder: "输入消息".to_owned(),
            permission_label: "询问".to_owned(),
            permission_selection: "ask".to_owned(),
            worktree_selection: "current".to_owned(),
            ..Default::default()
        };
        if let Some(composer) = &self.composer {
            snapshot.composer = composer.content.clone();
            snapshot.composer_atom_spans = composer.content_atom_spans();
            snapshot.composer_task_id = Some(composer.task_id.to_string());
            snapshot.composer_revision = composer.revision;
            snapshot.attachments = composer
                .attachments
                .iter()
                .map(|item| ComposerAttachment {
                    id: item.id.clone(),
                    label: item.name.clone(),
                })
                .collect();
            snapshot.plan_mode = composer.plan_mode;
            snapshot.goal_mode = composer.goal_mode;
            let (id, label) =
                crate::module::composer::presentation::permission_selection(composer.permission);
            snapshot.permission_selection = id.to_owned();
            snapshot.permission_label = label.to_owned();
            snapshot.reasoning = crate::module::composer::presentation::reasoning_selection(
                composer.reasoning_effort.as_deref(),
            )
            .to_owned();
            snapshot.model = composer.model.clone().unwrap_or_default();
        }
        snapshot.apply_failed = self.error.is_some();
        snapshot.slash_items = self
            .slash_commands
            .iter()
            .map(|item| ComposerSlashItem {
                name: item.command.name.clone(),
                label: if item.command.description.trim().is_empty() {
                    item.command.title.clone()
                } else {
                    item.command.description.clone()
                },
            })
            .collect();
        snapshot.reference_items = self
            .conversation_reference_results
            .iter()
            .map(|item| ComposerMentionItem {
                id: item.task_id.clone(),
                label: item.title.clone(),
            })
            .collect();
        snapshot.mention_items = self
            .context_attachment_results
            .iter()
            .map(|item| ComposerMentionItem {
                id: item.relative_path.clone(),
                label: item.attachment.name.clone(),
            })
            .collect();
        snapshot
    }

    fn apply_state(&mut self, composer: DesktopComposerState) {
        if self
            .composer
            .as_ref()
            .is_none_or(|previous| previous.task_id != composer.task_id)
        {
            self.menu = None;
        }
        crate::desktop::sync_hosted_textarea(&self.composer_editor, &composer.content);
        self.composer = Some(composer);
    }

    fn clear_state(&mut self) {
        self.menu = None;
        self.composer = None;
        self.composer_editor.clear();
        self.transient = false;
        self.transient_project = None;
        self.error = None;
        self.clear_suggestions();
    }

    fn clear_suggestions(&mut self) {
        self.slash_commands.clear();
        self.conversation_reference_results.clear();
        self.context_attachment_results.clear();
    }

    fn refresh(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(task_id) = cx.selected_task() else {
            self.clear_state();
            return UiModuleOutcome::dirty();
        };
        let service = match cx
            .kernel()
            .service::<crate::application::ComposerInputServiceKey>()
            .map_err(|error| error.to_string())
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.composer_state(&task_id) {
            Ok(composer) => {
                self.transient = false;
                self.transient_project = None;
                self.error = None;
                self.apply_state(composer);
                self.refresh_suggestions(cx);
                UiModuleOutcome::dirty()
            }
            Err(_) => {
                self.clear_state();
                self.error = Some("无法读取输入内容，请重试。".to_owned());
                UiModuleOutcome::dirty()
            }
        }
    }

    fn load_transient(
        &mut self,
        composer: DesktopComposerState,
        project_id: Option<ProjectId>,
    ) -> UiModuleOutcome {
        self.transient = true;
        self.transient_project = project_id;
        self.apply_state(composer);
        self.clear_suggestions();
        UiModuleOutcome::dirty()
    }

    fn attachments_locked(&self, cx: &UiModuleContext<'_>) -> bool {
        if self.transient {
            return false;
        }
        cx.task_session()
            .is_some_and(|session| session.has_open_composer_interaction())
    }

    fn apply_command(
        &mut self,
        command: DesktopComposerCommand,
        cx: &UiModuleContext<'_>,
        refresh_suggestions: bool,
    ) -> UiModuleOutcome {
        if self.attachments_locked(cx) && attachment_command(&command) {
            return UiModuleOutcome::clean();
        }
        if self.transient {
            let Some(composer) = self.composer.as_mut() else {
                return UiModuleOutcome::clean();
            };
            return match composer.apply_transient_command(command) {
                Ok(_) => {
                    self.error = None;
                    let composer = composer.clone();
                    self.apply_state(composer);
                    if refresh_suggestions {
                        self.refresh_suggestions(cx);
                    }
                    UiModuleOutcome::dirty()
                }
                Err(error) => {
                    eprintln!("failed to update Native transient composer: {error}");
                    self.error = Some("无法更新输入内容，请重试。".to_owned());
                    UiModuleOutcome::dirty()
                }
            };
        }
        let Some(task_id) = self.matching_task(cx.selected_task()) else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<crate::application::ComposerInputServiceKey>()
            .map_err(|error| error.to_string())
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.execute_composer_command(&task_id, command) {
            Ok(composer) => {
                self.error = None;
                self.apply_state(composer);
                if refresh_suggestions {
                    self.refresh_suggestions(cx);
                }
                UiModuleOutcome::dirty()
            }
            Err(_) => {
                if let Ok(composer) = service.composer_state(&task_id) {
                    self.apply_state(composer);
                    self.error =
                        Some("输入未能更新，已重新读取当前草稿，请确认后重试。".to_owned());
                } else {
                    self.error = Some("无法更新输入内容，请重试。".to_owned());
                }
                UiModuleOutcome::dirty()
            }
        }
    }

    fn set_content_at(
        &mut self,
        task_id: TaskId,
        expected_revision: u64,
        expected_content: String,
        value: String,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(composer) = self
            .composer
            .as_ref()
            .filter(|composer| composer.task_id == task_id)
        else {
            return UiModuleOutcome::clean();
        };
        if composer.revision != expected_revision || composer.content != expected_content {
            self.error = Some("输入内容已更新，请确认当前草稿后重试。".to_owned());
            return UiModuleOutcome::dirty();
        }
        self.apply_command(
            DesktopComposerCommand::ApplyPaste {
                expected_revision,
                expected_content,
                content: value,
                attachments: Vec::new(),
            },
            cx,
            true,
        )
    }

    fn set_content(&mut self, value: String, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        self.apply_command(DesktopComposerCommand::SetContent(value), cx, true)
    }

    fn select_conversation_reference(
        &mut self,
        referenced_task_id: &str,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(reference) = self
            .conversation_reference_results
            .iter()
            .find(|reference| reference.task_id == referenced_task_id)
            .cloned()
        else {
            return UiModuleOutcome::clean();
        };
        let Some(composer) = self.composer.clone() else {
            return UiModuleOutcome::clean();
        };
        let Some(content) =
            crate::desktop::composer_content_without_trigger(&composer.content, '#')
        else {
            return UiModuleOutcome::clean();
        };
        let outcome = self.apply_command(
            DesktopComposerCommand::ApplyConversationReference {
                expected_revision: composer.revision,
                content,
                reference,
            },
            cx,
            false,
        );
        if self.error.is_none() && outcome.dirty {
            self.clear_suggestions();
        }
        outcome
    }

    fn select_context_attachment(
        &mut self,
        relative_path: &str,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(attachment) = self
            .context_attachment_results
            .iter()
            .find(|result| result.relative_path == relative_path)
            .map(|result| result.attachment.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let Some(composer) = self.composer.clone() else {
            return UiModuleOutcome::clean();
        };
        let Some(content) =
            crate::desktop::composer_content_without_trigger(&composer.content, '@')
        else {
            return UiModuleOutcome::clean();
        };
        let outcome = self.apply_command(
            DesktopComposerCommand::ApplyContextAttachment {
                expected_revision: composer.revision,
                content,
                attachment,
            },
            cx,
            false,
        );
        if self.error.is_none() && outcome.dirty {
            self.clear_suggestions();
        }
        outcome
    }

    fn refresh_suggestions(&mut self, cx: &UiModuleContext<'_>) {
        self.refresh_slash_commands(cx);
        self.refresh_conversation_references(cx);
        self.refresh_context_attachments(cx);
    }

    fn refresh_slash_commands(&mut self, cx: &UiModuleContext<'_>) {
        let Some(composer) = self.composer.as_ref() else {
            self.slash_commands.clear();
            return;
        };
        let Some(query) = crate::desktop::composer_slash_query(&composer.content) else {
            self.slash_commands.clear();
            return;
        };
        let service = match cx
            .kernel()
            .service::<crate::application::ComposerInputServiceKey>()
            .map_err(|error| error.to_string())
        {
            Ok(service) => service,
            Err(_) => {
                self.slash_commands.clear();
                return;
            }
        };
        let commands = if self.transient {
            service.search_project_slash_commands(self.transient_project.as_ref(), &query, 8)
        } else {
            service.search_task_slash_commands(&composer.task_id, &query, 8)
        };
        match commands {
            Ok(commands) => self.slash_commands = commands,
            Err(error) => {
                eprintln!("failed to search Native slash commands: {error}");
                self.slash_commands.clear();
            }
        }
    }

    fn refresh_conversation_references(&mut self, cx: &UiModuleContext<'_>) {
        let Some(composer) = self.composer.as_ref() else {
            self.conversation_reference_results.clear();
            return;
        };
        let Some(query) = crate::desktop::composer_conversation_query(&composer.content) else {
            self.conversation_reference_results.clear();
            return;
        };
        let service = match cx
            .kernel()
            .service::<crate::application::ComposerInputServiceKey>()
            .map_err(|error| error.to_string())
        {
            Ok(service) => service,
            Err(_) => {
                self.conversation_reference_results.clear();
                return;
            }
        };
        let references = if self.transient {
            service.search_conversation_references_from(&composer.task_id, &query, 8)
        } else {
            service.search_conversation_references(&composer.task_id, &query, 8)
        };
        match references {
            Ok(references) => self.conversation_reference_results = references,
            Err(error) => {
                eprintln!("failed to search Native conversation references: {error}");
                self.conversation_reference_results.clear();
            }
        }
    }

    fn refresh_context_attachments(&mut self, cx: &UiModuleContext<'_>) {
        let Some(composer) = self.composer.as_ref() else {
            self.context_attachment_results.clear();
            return;
        };
        let Some(query) = crate::desktop::composer_context_query(&composer.content) else {
            self.context_attachment_results.clear();
            return;
        };
        let service = match cx
            .kernel()
            .service::<crate::application::ComposerInputServiceKey>()
            .map_err(|error| error.to_string())
        {
            Ok(service) => service,
            Err(_) => {
                self.context_attachment_results.clear();
                return;
            }
        };
        let attachments = if self.transient {
            let Some(project_id) = self.transient_project.as_ref() else {
                self.context_attachment_results.clear();
                return;
            };
            service.search_project_context_attachments(project_id, &query, 8)
        } else {
            match service
                .task_has_project(&composer.task_id)
                .ok()
                .filter(|has_project| *has_project)
            {
                Some(_) => service.search_task_context_attachments(&composer.task_id, &query, 8),
                None => {
                    self.context_attachment_results.clear();
                    return;
                }
            }
        };
        match attachments {
            Ok(attachments) => self.context_attachment_results = attachments,
            Err(error) => {
                eprintln!("failed to search Native project context: {error}");
                self.context_attachment_results.clear();
            }
        }
    }
}

fn attachment_command(command: &DesktopComposerCommand) -> bool {
    matches!(
        command,
        DesktopComposerCommand::ReplaceAttachments(_)
            | DesktopComposerCommand::RemoveAttachment(_)
            | DesktopComposerCommand::ApplyContextAttachment { .. }
            | DesktopComposerCommand::ApplyConversationReference { .. }
            | DesktopComposerCommand::RemoveConversationReference(_)
    )
}

pub(crate) fn textarea_height(state: &TextEditorState) -> f32 {
    let additional_lines = state.line_count().saturating_sub(1) as f32;
    (TEXTAREA_MIN_HEIGHT + additional_lines * TEXTAREA_LINE_HEIGHT).min(TEXTAREA_MAX_HEIGHT)
}

impl UiModule for ComposerModule {
    type Projection<'a> = crate::ui_module::projection::ComposerProjection<'a>;

    type Message = ComposerMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            ComposerMessage::SetMenu(menu) => {
                self.menu = menu;
                UiModuleOutcome::dirty()
            }
            ComposerMessage::ToggleMenu(menu) => {
                self.menu = if self.menu == Some(menu) {
                    None
                } else {
                    Some(menu)
                };
                UiModuleOutcome::dirty()
            }
            ComposerMessage::Refresh => self.refresh(cx),
            ComposerMessage::LoadTransient {
                composer,
                project_id,
            } => self.load_transient(composer, project_id),
            ComposerMessage::Clear => {
                self.clear_state();
                UiModuleOutcome::dirty()
            }
            ComposerMessage::SetContent(value) => self.set_content(value, cx),
            ComposerMessage::SetContentAt {
                task_id,
                expected_revision,
                expected_content,
                value,
            } => self.set_content_at(task_id, expected_revision, expected_content, value, cx),
            ComposerMessage::ApplyCommand(command) => self.apply_command(command, cx, false),
            ComposerMessage::SelectConversationReference(task_id) => {
                self.select_conversation_reference(&task_id, cx)
            }
            ComposerMessage::SelectContextAttachment(relative_path) => {
                self.select_context_attachment(&relative_path, cx)
            }
            ComposerMessage::RefreshSuggestions => {
                self.refresh_suggestions(cx);
                UiModuleOutcome::dirty()
            }
            ComposerMessage::ClearSuggestions => {
                self.clear_suggestions();
                UiModuleOutcome::dirty()
            }
        }
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(event) = envelope.downcast::<crate::application::ComposerChanged>() else {
            return UiModuleOutcome::clean();
        };
        if cx.selected_task().as_ref() != Some(&event.task_id) {
            return UiModuleOutcome::clean();
        }
        self.refresh(cx)
    }

    fn project_fields(&self, cx: &UiModuleContext<'_>, into: Self::Projection<'_>) {
        if !crate::module::conversation_is_visible(cx) {
            return;
        }
        if let Some(error) = &self.error {
            *into.error = Some(error.clone());
        }
        let state = self.view_snapshot(into.composer.window_id);
        into.composer.composer = state.composer;
        into.composer.composer_atom_spans = state.composer_atom_spans;
        into.composer.composer_task_id = state.composer_task_id;
        into.composer.composer_revision = state.composer_revision;
        into.composer.composer_height = state.composer_height;
        into.composer.attachments = state.attachments;
        into.composer.plan_mode = state.plan_mode;
        into.composer.goal_mode = state.goal_mode;
        into.composer.permission_label = state.permission_label;
        into.composer.permission_selection = state.permission_selection;
        into.composer.reasoning = state.reasoning;
        into.composer.model = state.model;
        into.composer.slash_items = state.slash_items;
        into.composer.mention_items = state.mention_items;
        into.composer.reference_items = state.reference_items;
        into.composer.composer_plus_open = state.composer_plus_open;
        into.composer.composer_permission_menu_open = state.composer_permission_menu_open;
        into.composer.composer_worktree_menu_open = state.composer_worktree_menu_open;
        into.composer.apply_failed = state.apply_failed;
        into.composer.pending_blocks_send = cx
            .task_session()
            .is_some_and(|session| session.blocking_pending_count > 0);
    }
}

#[cfg(test)]
mod tests {
    use lilia_contracts::TaskId;
    use lilia_kernel::Kernel;
    use nana_ui_platform::WindowId;

    use super::*;
    use crate::application::ApplicationWorkspaceSurface;
    use crate::runtime_shell::{ShellProjectPage, empty_snapshot};

    fn loaded_draft() -> ComposerModule {
        let mut module = ComposerModule::default();
        let mut composer = DesktopComposerState::transient(
            TaskId::new("draft-task").expect("the id is not blank"),
        );
        composer.content = "hello".to_owned();
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId::PRIMARY);
        module.reduce(
            ComposerMessage::LoadTransient {
                composer,
                project_id: None,
            },
            &cx,
        );
        module
    }

    #[test]
    fn menus_are_window_local_and_reset_when_the_resource_changes() {
        use view::ComposerMenuKind;
        let kernel = Kernel::new();
        let primary = UiModuleContext::new(&kernel, WindowId::PRIMARY);
        let popup = UiModuleContext::new(&kernel, WindowId(42));
        let mut main = loaded_draft();
        let mut other = loaded_draft();
        main.reduce(
            ComposerMessage::ToggleMenu(ComposerMenuKind::Permission),
            &primary,
        );
        other.reduce(
            ComposerMessage::ToggleMenu(ComposerMenuKind::Actions),
            &popup,
        );
        assert!(
            main.view_snapshot(WindowId::PRIMARY)
                .composer_permission_menu_open
        );
        assert!(!main.view_snapshot(WindowId::PRIMARY).composer_plus_open);
        assert!(other.view_snapshot(WindowId(42)).composer_plus_open);
        let same = main.composer().unwrap().clone();
        main.apply_state(same);
        assert!(
            main.view_snapshot(WindowId::PRIMARY)
                .composer_permission_menu_open
        );
        main.apply_state(DesktopComposerState::transient(
            TaskId::new("another-task").unwrap(),
        ));
        assert!(
            !main
                .view_snapshot(WindowId::PRIMARY)
                .composer_permission_menu_open
        );
        assert!(other.view_snapshot(WindowId(42)).composer_plus_open);
        other.reduce(
            ComposerMessage::ToggleMenu(ComposerMenuKind::Actions),
            &popup,
        );
        assert!(!other.view_snapshot(WindowId(42)).composer_plus_open);
    }

    #[test]
    fn addressed_input_keeps_newer_draft_and_accepts_sequential_edits_including_empty() {
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId(42));
        let mut module = loaded_draft();
        let task_id = module.composer().unwrap().task_id.clone();
        for (revision, before, value) in [
            (0, "hello", "first"),
            (1, "first", "second"),
            (2, "second", ""),
        ] {
            assert!(
                module
                    .reduce(
                        ComposerMessage::SetContentAt {
                            task_id: task_id.clone(),
                            expected_revision: revision,
                            expected_content: before.to_owned(),
                            value: value.to_owned()
                        },
                        &cx
                    )
                    .dirty
            );
            assert_eq!(module.composer().unwrap().content, value);
        }
        module.reduce(
            ComposerMessage::SetContentAt {
                task_id: task_id.clone(),
                expected_revision: 1,
                expected_content: "first".to_owned(),
                value: "late old draft".to_owned(),
            },
            &cx,
        );
        assert!(module.error.is_some());
        assert_eq!(module.composer().unwrap().content, "");
        module.error = None;
        module.reduce(
            ComposerMessage::SetContentAt {
                task_id: task_id.clone(),
                expected_revision: 3,
                expected_content: "stale event base".to_owned(),
                value: "queued old text".to_owned(),
            },
            &cx,
        );
        assert!(module.error.is_some());
        assert_eq!(module.composer().unwrap().content, "");
        let outcome = module.reduce(
            ComposerMessage::SetContentAt {
                task_id: TaskId::new("another-task").unwrap(),
                expected_revision: 3,
                expected_content: String::new(),
                value: "other task draft".to_owned(),
            },
            &cx,
        );
        assert!(!outcome.dirty);
        assert_eq!(module.composer().unwrap().content, "");
        assert_eq!(module.composer().unwrap().revision, 3);
    }

    #[test]
    fn composer_target_rejects_navigation_changes_missing_selection_and_cleared_state() {
        let mut module = loaded_draft();
        let original = TaskId::new("draft-task").unwrap();
        assert_eq!(
            module.matching_task(Some(original.clone())),
            Some(original.clone())
        );
        assert_eq!(
            module.matching_task(Some(TaskId::new("another-task").unwrap())),
            None
        );
        assert_eq!(module.matching_task(None), None);
        module.clear_state();
        assert_eq!(module.matching_task(Some(original)), None);
    }

    #[test]
    fn persistent_composer_cannot_write_when_its_window_has_no_selected_task() {
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId(42));
        let mut module = loaded_draft();
        module.transient = false;
        let outcome = module.reduce(ComposerMessage::SetContent("wrong window".to_owned()), &cx);
        assert!(!outcome.dirty);
        assert!(outcome.error.is_none());
        assert_eq!(module.composer().unwrap().content, "hello");
        assert_eq!(module.composer_editor().text(), "hello");
    }

    #[test]
    fn a_transient_draft_projects_its_own_content() {
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId::PRIMARY);
        let module = loaded_draft();

        let mut snapshot = empty_snapshot();
        module.project(&cx, &mut snapshot);
        assert_eq!(snapshot.composer.composer, "hello");
        assert_eq!(
            snapshot.composer.composer_task_id.as_deref(),
            Some("draft-task")
        );
        assert_eq!(snapshot.composer.composer_revision, 0);
        assert!(module.transient);
    }

    #[test]
    fn clearing_the_input_surface_drops_the_projected_draft() {
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId::PRIMARY);
        let mut module = loaded_draft();
        module.reduce(ComposerMessage::Clear, &cx);

        let mut snapshot = empty_snapshot();
        module.project(&cx, &mut snapshot);
        assert_eq!(snapshot.composer.composer, "");
        assert!(module.composer().is_none());
    }

    #[test]
    fn hidden_surfaces_and_pages_do_not_project_the_draft() {
        let kernel = Kernel::new();
        let module = loaded_draft();
        let mut snapshot = empty_snapshot();
        snapshot.composer.composer = "stale".to_owned();

        module.project(
            &UiModuleContext::new(&kernel, WindowId::PRIMARY)
                .showing_surface(Some(ApplicationWorkspaceSurface::Settings)),
            &mut snapshot,
        );
        assert_eq!(snapshot.composer.composer, "stale");

        snapshot.composer.composer = "stale".to_owned();
        module.project(
            &UiModuleContext::new(&kernel, WindowId::PRIMARY)
                .showing(Some(ShellProjectPage::Architecture)),
            &mut snapshot,
        );
        assert_eq!(snapshot.composer.composer, "stale");
    }

    #[test]
    fn a_transient_draft_keeps_typed_content() {
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId::PRIMARY);
        let mut module = ComposerModule::default();
        let composer = DesktopComposerState::transient(
            TaskId::new("draft-task").expect("the id is not blank"),
        );
        module.reduce(
            ComposerMessage::LoadTransient {
                composer,
                project_id: None,
            },
            &cx,
        );
        module.reduce(
            ComposerMessage::SetContent("first question".to_owned()),
            &cx,
        );

        let mut snapshot = empty_snapshot();
        module.project(&cx, &mut snapshot);
        assert_eq!(snapshot.composer.composer, "first question");
        assert!(module.transient);
    }
}

#[cfg(test)]
use crate::ui_module::ErasedUiModule;
