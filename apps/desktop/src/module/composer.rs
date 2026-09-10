//! The composer input surface as a UI module.
//!
//! Owns the per-window draft, the editor, and the slash / mention / reference
//! suggestion lists. Submit, todo, goal and the task session itself stay in the
//! shell. One instance per window: a popup's composer is that window's host,
//! not a field on the popup struct.

use lilia_contracts::{ChatContextSearchResult, ChatConversationReference, ProjectId};
use lilia_kernel::FeatureId;

use crate::application::{
    DesktopComposerCommand, DesktopComposerState, DesktopSlashCommandSearchResult,
};
use crate::runtime_shell::{
    PrimaryShellSnapshot, ShellAttachmentRow, ShellMentionItem, ShellSlashItem,
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
    Refresh,
    LoadTransient {
        composer: DesktopComposerState,
        project_id: Option<ProjectId>,
    },
    Clear,
    SetContent(String),
    ApplyCommand(DesktopComposerCommand),
    SelectConversationReference(String),
    SelectContextAttachment(String),
    RefreshSuggestions,
    ClearSuggestions,
}

pub struct ComposerModule {
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

    fn apply_state(&mut self, composer: DesktopComposerState) {
        crate::desktop::sync_hosted_textarea(&self.composer_editor, &composer.content);
        self.composer = Some(composer);
    }

    fn clear_state(&mut self) {
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
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match application.composer_state(&task_id) {
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
            if matches!(command, DesktopComposerCommand::ApplyPaste { .. }) {
                return self.command_failed("当前无法添加附件，请稍后重试。");
            }
            return UiModuleOutcome::clean();
        }
        if self.transient {
            let Some(composer) = self.composer.as_mut() else {
                return self.command_failed("输入内容已不可用，请重新打开会话。");
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
                    self.command_failed("无法更新输入内容，请重试。")
                }
            };
        }
        let Some(task_id) = self
            .composer
            .as_ref()
            .map(|composer| composer.task_id.clone())
            .or_else(|| cx.selected_task())
        else {
            return self.command_failed("输入内容已不可用，请重新打开会话。");
        };
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                return self.command_failed(error);
            }
        };
        match application.execute_composer_command(&task_id, command) {
            Ok(composer) => {
                self.error = None;
                self.apply_state(composer);
                if refresh_suggestions {
                    self.refresh_suggestions(cx);
                }
                UiModuleOutcome::dirty()
            }
            Err(_) => self.command_failed("无法更新输入内容，请重试。"),
        }
    }

    fn command_failed(&mut self, error: impl Into<String>) -> UiModuleOutcome {
        let error = error.into();
        self.error = Some(error.clone());
        UiModuleOutcome::failed(error)
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
        let application = match cx.application() {
            Ok(application) => application,
            Err(_) => {
                self.slash_commands.clear();
                return;
            }
        };
        let commands = if self.transient {
            application.search_project_slash_commands(self.transient_project.as_ref(), &query, 8)
        } else {
            application.search_task_slash_commands(&composer.task_id, &query, 8)
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
        let application = match cx.application() {
            Ok(application) => application,
            Err(_) => {
                self.conversation_reference_results.clear();
                return;
            }
        };
        let references = if self.transient {
            application.search_conversation_references_from(&composer.task_id, &query, 8)
        } else {
            application.search_conversation_references(&composer.task_id, &query, 8)
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
        let application = match cx.application() {
            Ok(application) => application,
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
            application.search_project_context_attachments(project_id, &query, 8)
        } else {
            match application
                .get_task(&composer.task_id)
                .ok()
                .and_then(|task| task.project_id)
            {
                Some(_) => {
                    application.search_task_context_attachments(&composer.task_id, &query, 8)
                }
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
    if let DesktopComposerCommand::ApplyPaste { attachments, .. } = command {
        return !attachments.is_empty();
    }
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
    type Message = ComposerMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
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

    fn project(&self, cx: &UiModuleContext<'_>, into: &mut PrimaryShellSnapshot) {
        if !crate::module::conversation_is_visible(cx) {
            return;
        }
        into.pending_blocks_send = cx
            .task_session()
            .is_some_and(|session| session.blocking_pending_count > 0);
        if let Some(error) = &self.error {
            into.error = Some(error.clone());
        }
        let Some(composer) = self.composer.as_ref() else {
            into.composer = self.composer_editor.text();
            into.composer_atom_spans.clear();
            into.composer_task_id = None;
            into.composer_revision = 0;
            into.composer_height = textarea_height(&self.composer_editor);
            into.attachments.clear();
            into.plan_mode = false;
            into.goal_mode = false;
            into.slash_items.clear();
            into.mention_items.clear();
            return;
        };
        into.composer = composer.content.clone();
        into.composer_atom_spans = composer.content_atom_spans();
        into.composer_task_id = Some(composer.task_id.as_str().to_owned());
        into.composer_revision = composer.revision;
        into.composer_height = textarea_height(&self.composer_editor);
        into.attachments = composer
            .effective_attachments()
            .map(|attachment| ShellAttachmentRow {
                id: attachment.id.clone(),
                label: attachment.name.clone(),
            })
            .collect();
        into.plan_mode = composer.plan_mode;
        into.goal_mode = composer.goal_mode;
        let (permission_selection, permission_label) =
            crate::runtime_shell::permission_selection(composer.permission);
        into.permission_selection = permission_selection.to_owned();
        into.permission_label = permission_label.to_owned();
        into.slash_items = self
            .slash_commands
            .iter()
            .map(|item| ShellSlashItem {
                name: item.command.name.clone(),
                label: if item.command.description.trim().is_empty() {
                    item.command.title.clone()
                } else {
                    item.command.description.clone()
                },
            })
            .collect();
        into.mention_items = self
            .context_attachment_results
            .iter()
            .map(|result| ShellMentionItem {
                id: result.relative_path.clone(),
                label: result.attachment.name.clone(),
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lilia_contracts::{LiliaAgentWorkflow, LiliaReviewTarget, TaskId};
    use lilia_kernel::Kernel;
    use lilia_service::ServiceAuthority;
    use nana_ui_platform::WindowId;

    use super::*;
    use crate::application::ApplicationWorkspaceSurface;
    use crate::runtime_shell::{empty_snapshot, ShellProjectPage};

    fn replacement_commands(expected_revision: u64) -> [DesktopComposerCommand; 2] {
        [
            DesktopComposerCommand::ApplyPromptOptimization {
                expected_revision,
                content: "optimized old prompt".to_owned(),
            },
            DesktopComposerCommand::ApplySlashWorkflow {
                expected_revision,
                workflow: LiliaAgentWorkflow::LiliaReview {
                    target: LiliaReviewTarget::BaseBranch {
                        branch: "main".to_owned(),
                    },
                    instructions: None,
                    delivery: None,
                },
            },
        ]
    }

    struct NoopHost;

    impl crate::application::DesktopHost for NoopHost {
        fn execute(
            &self,
            _: &crate::application::DesktopHostContext,
            _: crate::application::DesktopHostAction,
        ) -> Result<crate::application::DesktopHostResult, crate::application::DesktopHostError>
        {
            Ok(crate::application::DesktopHostResult::Completed)
        }
    }

    fn durable_fixture() -> (
        tempfile::TempDir,
        crate::application::DesktopApplication,
        Kernel,
        ComposerModule,
        rusqlite::Connection,
    ) {
        use crate::application::{DesktopApplication, DesktopApplicationConfig, DesktopTaskCreate};
        let home = tempfile::tempdir().unwrap();
        let config = DesktopApplicationConfig::new(home.path(), "composer-module-test").unwrap();
        let authority = ServiceAuthority::bootstrap_with_home(home.path()).unwrap();
        let app = DesktopApplication::from_authority(config.clone(), authority, Arc::new(NoopHost))
            .unwrap();
        let task = app
            .create_task(DesktopTaskCreate::new(None, "Draft"))
            .unwrap();
        let state = app
            .execute_composer_command(
                &task.id,
                DesktopComposerCommand::SetContent("original prompt".to_owned()),
            )
            .unwrap();
        let kernel = Kernel::new();
        kernel
            .mount(Arc::new(crate::shell_service::ApplicationFeature::new(
                app.clone(),
            )))
            .unwrap();
        let mut module = ComposerModule::default();
        module.apply_state(state);
        let connection = rusqlite::Connection::open(config.domain_database_path()).unwrap();
        (home, app, kernel, module, connection)
    }

    fn assert_rejected(
        module: &ComposerModule,
        outcome: UiModuleOutcome,
        expected: &DesktopComposerState,
    ) {
        assert!(
            outcome.error.is_some(),
            "the shell must not continue after rejection"
        );
        assert!(outcome.dirty);
        assert!(outcome.effects.is_empty());
        assert_eq!(module.error, outcome.error);
        assert_eq!(module.composer(), Some(expected));
        assert_eq!(module.composer_editor().text(), expected.content);
    }

    #[test]
    fn stale_transient_replacements_report_failure_and_keep_the_newer_draft() {
        for window in [WindowId::PRIMARY, WindowId(42)] {
            let kernel = Kernel::new();
            let cx = UiModuleContext::new(&kernel, window);
            let mut module = loaded_draft();
            let revision = module.composer().unwrap().revision;
            module.reduce(ComposerMessage::SetContent("new question".to_owned()), &cx);
            let newer = module.composer().unwrap().clone();
            for command in replacement_commands(revision) {
                let outcome = module.reduce(ComposerMessage::ApplyCommand(command), &cx);
                assert_rejected(&module, outcome, &newer);
            }
        }
    }

    #[test]
    fn stale_durable_replacements_report_failure_without_publishing_a_change() {
        for window in [WindowId::PRIMARY, WindowId(42)] {
            let (_home, app, kernel, mut module, _connection) = durable_fixture();
            let cx = UiModuleContext::new(&kernel, window);
            let revision = module.composer().unwrap().revision;
            module.reduce(ComposerMessage::SetContent("new question".to_owned()), &cx);
            let newer = module.composer().unwrap().clone();
            let events = app.subscribe_events();
            for command in replacement_commands(revision) {
                let outcome = module.reduce(ComposerMessage::ApplyCommand(command), &cx);
                assert_rejected(&module, outcome, &newer);
                assert_eq!(app.composer_state(&newer.task_id).unwrap(), newer);
                assert!(events.try_recv().is_err());
            }
        }
    }

    #[test]
    fn durable_replacement_write_errors_reach_the_shell_and_allow_retry() {
        for window in [WindowId::PRIMARY, WindowId(42)] {
            for command_index in 0..2 {
                let (_home, app, kernel, mut module, connection) = durable_fixture();
                let cx = UiModuleContext::new(&kernel, window);
                let original = module.composer().unwrap().clone();
                let command = replacement_commands(original.revision)[command_index].clone();
                connection.execute_batch(
                    "CREATE TRIGGER reject_composer_write BEFORE INSERT ON desktop_composer_drafts
                     BEGIN SELECT RAISE(ABORT, 'injected composer persistence failure'); END;",
                ).unwrap();
                let events = app.subscribe_events();
                let outcome = module.reduce(ComposerMessage::ApplyCommand(command.clone()), &cx);
                assert_rejected(&module, outcome, &original);
                assert_eq!(app.composer_state(&original.task_id).unwrap(), original);
                assert!(events.try_recv().is_err());

                connection
                    .execute_batch("DROP TRIGGER reject_composer_write;")
                    .unwrap();
                let retried = module.reduce(ComposerMessage::ApplyCommand(command), &cx);
                assert!(retried.error.is_none());
                assert!(module.error.is_none());
                let updated = app.composer_state(&original.task_id).unwrap();
                assert_eq!(module.composer(), Some(&updated));
                assert_eq!(updated.revision, original.revision + 1);
                if command_index == 0 {
                    assert_eq!(updated.content, "optimized old prompt");
                } else {
                    assert!(updated.content.is_empty());
                    assert!(matches!(
                        updated.workflow,
                        Some(LiliaAgentWorkflow::LiliaReview { .. })
                    ));
                }
            }
        }
    }

    #[test]
    fn unavailable_composer_or_application_is_a_failed_command() {
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId::PRIMARY);
        let mut missing = ComposerModule::default();
        let outcome = missing.reduce(
            ComposerMessage::ApplyCommand(replacement_commands(0)[0].clone()),
            &cx,
        );
        assert!(outcome.error.is_some());
        let mut unavailable_service = loaded_draft();
        unavailable_service.transient = false;
        let original = unavailable_service.composer().unwrap().clone();
        let outcome = unavailable_service.reduce(
            ComposerMessage::ApplyCommand(replacement_commands(0)[0].clone()),
            &cx,
        );
        assert_rejected(&unavailable_service, outcome, &original);
    }

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
    fn a_transient_draft_projects_its_own_content() {
        let kernel = Kernel::new();
        let cx = UiModuleContext::new(&kernel, WindowId::PRIMARY);
        let module = loaded_draft();

        let mut snapshot = empty_snapshot();
        module.project(&cx, &mut snapshot);
        assert_eq!(snapshot.composer, "hello");
        assert_eq!(snapshot.composer_task_id.as_deref(), Some("draft-task"));
        assert_eq!(snapshot.composer_revision, 0);
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
        assert_eq!(snapshot.composer, "");
        assert!(module.composer().is_none());
    }

    #[test]
    fn hidden_surfaces_and_pages_do_not_project_the_draft() {
        let kernel = Kernel::new();
        let module = loaded_draft();
        let mut snapshot = empty_snapshot();
        snapshot.composer = "stale".to_owned();

        module.project(
            &UiModuleContext::new(&kernel, WindowId::PRIMARY)
                .showing_surface(Some(ApplicationWorkspaceSurface::Settings)),
            &mut snapshot,
        );
        assert_eq!(snapshot.composer, "stale");

        snapshot.composer = "stale".to_owned();
        module.project(
            &UiModuleContext::new(&kernel, WindowId::PRIMARY)
                .showing(Some(ShellProjectPage::Architecture)),
            &mut snapshot,
        );
        assert_eq!(snapshot.composer, "stale");
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
        assert_eq!(snapshot.composer, "first question");
        assert!(module.transient);
    }
}
