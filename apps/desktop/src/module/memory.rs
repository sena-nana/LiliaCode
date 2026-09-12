//! The Memory page as a UI module.
//!
//! Owns the memory list, the editor fields, the injection settings and the
//! selected task's injection state. One instance per window: the editor is
//! window state, and the settings are read back from the service on refresh so
//! two windows cannot disagree about what is stored.

use lilia_kernel::FeatureId;

use crate::application::{
    DesktopMemory, MemoryInjectionState, MemoryScope, MemorySettings, MemoryUpsertInput,
    ProjectWorkspaceSurface,
};
use crate::runtime_shell::{PrimaryShellSnapshot, ShellMemoryCard, ShellProjectPage};
use crate::text_editor_state::TextEditorState;
use crate::ui_module::{ShellEffect, UiModule, UiModuleContext, UiModuleOutcome};

/// The memory domain's own message vocabulary.
#[derive(Debug, Clone)]
pub enum MemoryMessage {
    Open,
    Refresh,
    Select(String),
    New,
    NewInScope(MemoryScope),
    SetScope(MemoryScope),
    ToggleDraftEnabled,
    ToggleEntry(String),
    DeleteEntry(String),
    CommitCooldown(u64),
    TitleChanged(String),
    BodyReplaced(String),
    TagsChanged(String),
    ToggleScope,
    Save,
    ToggleEnabled,
    Delete,
    ToggleGlobal,
    ToggleBaseline,
    CycleCooldown,
    CooldownChanged(String),
    SaveCooldown,
    TaskMenuOpen(bool),
    SelectInjectionTask(String),
    ToggleTaskInjection,
    ResetTaskCooldown,
}

pub struct MemoryModule {
    memories: Vec<DesktopMemory>,
    selected: Option<String>,
    title: String,
    body: TextEditorState,
    tags: String,
    scope: MemoryScope,
    draft_enabled: bool,
    updated_at: Option<i64>,
    editor_dirty: bool,
    editor_generation: u64,
    loaded_project: Option<lilia_contracts::ProjectId>,
    error: Option<String>,
    settings: MemorySettings,
    cooldown_input: String,
    injection: Option<MemoryInjectionState>,
    injection_task: Option<lilia_contracts::TaskId>,
    task_menu_open: bool,
}

impl Default for MemoryModule {
    fn default() -> Self {
        let settings = MemorySettings::default();
        Self {
            memories: Vec::new(),
            selected: None,
            title: String::new(),
            body: TextEditorState::new(),
            tags: String::new(),
            scope: MemoryScope::Project,
            draft_enabled: true,
            updated_at: None,
            editor_dirty: false,
            editor_generation: 0,
            loaded_project: None,
            error: None,
            cooldown_input: settings.cooldown_turns.to_string(),
            settings,
            injection: None,
            injection_task: None,
            task_menu_open: false,
        }
    }
}

impl MemoryModule {
    pub fn feature_id() -> FeatureId {
        FeatureId::new("lilia.memory").expect("the memory feature id is not blank")
    }

    pub fn memories(&self) -> &[DesktopMemory] {
        &self.memories
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn body(&self) -> &TextEditorState {
        &self.body
    }

    pub fn draft_enabled(&self) -> bool {
        self.draft_enabled
    }

    pub fn editor_generation(&self) -> u64 {
        self.editor_generation
    }

    pub fn scope(&self) -> MemoryScope {
        self.scope
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn settings(&self) -> &MemorySettings {
        &self.settings
    }

    pub fn cooldown_input(&self) -> &str {
        &self.cooldown_input
    }

    pub fn injection(&self) -> Option<&MemoryInjectionState> {
        self.injection.as_ref()
    }

    /// Restores the memory a window remembered, falling back to the first one
    /// when the saved id no longer exists.
    pub fn restore_selection(&mut self, memory_id: Option<String>) {
        let selected = memory_id
            .filter(|selected| self.has(selected))
            .or_else(|| self.memories.first().map(|memory| memory.id.clone()));
        if self.editor_dirty && (self.selected == selected || self.selected.is_none()) {
            return;
        }
        if self.selected != selected {
            self.editor_generation = self.editor_generation.wrapping_add(1);
        }
        self.selected = selected;
        self.load_selected();
    }

    fn has(&self, memory_id: &str) -> bool {
        self.memories.iter().any(|memory| memory.id == memory_id)
    }

    fn memory(&self) -> Option<&DesktopMemory> {
        let selected = self.selected.as_deref()?;
        self.memories.iter().find(|memory| memory.id == selected)
    }

    fn refresh(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match application.memory_settings() {
            Ok(settings) => {
                if settings.cooldown_turns != self.settings.cooldown_turns {
                    self.cooldown_input = settings.cooldown_turns.to_string();
                }
                self.settings = settings;
            }
            Err(error) => self.error = Some(format!("无法读取 Memory 设置：{error}")),
        }
        self.refresh_injection(cx);
        let Some(project_id) = cx.selected_project() else {
            self.memories.clear();
            self.selected = None;
            self.clear_editor();
            return UiModuleOutcome::dirty();
        };
        match application.list_memories(Some(&project_id)) {
            Ok(memories) => {
                let project_changed = self.loaded_project.as_ref() != Some(&project_id);
                let previous_selection = self.selected.clone();
                let preserve_draft = self.editor_dirty
                    && self.loaded_project.as_ref() == Some(&project_id)
                    && self
                        .selected
                        .as_ref()
                        .is_none_or(|id| memories.iter().any(|memory| memory.id == *id));
                self.loaded_project = Some(project_id);
                self.memories = memories;
                if !preserve_draft
                    && !self
                        .selected
                        .as_deref()
                        .is_some_and(|selected| self.has(selected))
                {
                    self.selected = self.memories.first().map(|memory| memory.id.clone());
                }
                if !preserve_draft {
                    if project_changed || previous_selection != self.selected {
                        self.editor_generation = self.editor_generation.wrapping_add(1);
                    }
                    self.load_selected();
                }
                self.error = None;
            }
            Err(error) => self.error = Some(format!("无法读取 Memory：{error}")),
        }
        UiModuleOutcome::dirty()
    }

    fn select(&mut self, memory_id: String) -> UiModuleOutcome {
        if !self.has(&memory_id) {
            return UiModuleOutcome::clean();
        }
        if self.selected.as_deref() != Some(memory_id.as_str()) {
            self.editor_generation = self.editor_generation.wrapping_add(1);
        }
        self.selected = Some(memory_id);
        self.load_selected();
        UiModuleOutcome::dirty()
    }

    /// Empties the editor, which is also how a new memory is started: an unsaved
    /// draft is the absence of a selection.
    fn clear_editor(&mut self) {
        self.editor_generation = self.editor_generation.wrapping_add(1);
        self.selected = None;
        self.title.clear();
        self.body.clear();
        self.tags.clear();
        self.scope = MemoryScope::Project;
        self.draft_enabled = true;
        self.updated_at = None;
        self.editor_dirty = false;
        self.error = None;
    }

    fn load_selected(&mut self) {
        self.editor_dirty = false;
        let loaded = self.memory().map(|memory| {
            (
                memory.title.clone(),
                memory.body.clone(),
                memory.tags.join(", "),
                memory.scope,
                memory.enabled,
                memory.updated_at,
            )
        });
        match loaded {
            Some((title, body, tags, scope, enabled, updated_at)) => {
                self.title = title;
                self.body.set_text(&body);
                self.tags = tags;
                self.scope = scope;
                self.draft_enabled = enabled;
                self.updated_at = Some(updated_at);
            }
            None => self.clear_editor(),
        }
    }

    fn save(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let project_id = match self.scope {
            MemoryScope::User => None,
            MemoryScope::Project => cx
                .selected_project()
                .map(|project_id| project_id.as_str().to_owned()),
        };
        let input = MemoryUpsertInput {
            id: self.selected.clone(),
            scope: self.scope,
            project_id,
            title: self.title.clone(),
            body: self.body.text(),
            tags: crate::desktop::parse_memory_tags(&self.tags),
            enabled: self.draft_enabled,
            source_task_id: self
                .memory()
                .and_then(|memory| memory.source_task_id.clone()),
            expected_updated_at: self.updated_at,
        };
        match application.save_memory(input) {
            Ok(memory) => {
                self.editor_dirty = false;
                self.selected = Some(memory.id);
                self.refresh(cx)
            }
            Err(error) => {
                self.error = Some(format!("无法保存 Memory：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn change_entry(
        &mut self,
        id: String,
        delete: bool,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let Some(memory) = self.memories.iter().find(|memory| memory.id == id).cloned() else {
            return UiModuleOutcome::clean();
        };
        let result = cx.application().and_then(|app| {
            if delete {
                app.delete_memory(&id, Some(memory.updated_at))
                    .map(|_| None)
                    .map_err(|error| error.to_string())
            } else {
                app.set_memory_enabled(&id, !memory.enabled, Some(memory.updated_at))
                    .map(Some)
                    .map_err(|error| error.to_string())
            }
        });
        match result {
            Ok(saved) => {
                if self.selected.as_ref() == Some(&id) {
                    if let Some(saved) = saved {
                        if !self.editor_dirty || self.updated_at == Some(memory.updated_at) {
                            self.draft_enabled = saved.enabled;
                            self.updated_at = Some(saved.updated_at);
                        }
                    } else {
                        self.clear_editor();
                    }
                }
                self.refresh(cx)
            }
            Err(error) => {
                self.error = Some(error);
                UiModuleOutcome::dirty()
            }
        }
    }

    fn delete(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(memory_id) = self.selected.clone() else {
            return UiModuleOutcome::clean();
        };
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match application.delete_memory(&memory_id, self.updated_at) {
            Ok(_) => {
                self.selected = None;
                self.refresh(cx)
            }
            Err(error) => {
                self.error = Some(format!("无法删除 Memory：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn save_settings(
        &mut self,
        settings: MemorySettings,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let cooldown_changed = settings.cooldown_turns != self.settings.cooldown_turns;
        match application.save_memory_settings(settings) {
            Ok(settings) => {
                if cooldown_changed {
                    self.cooldown_input = settings.cooldown_turns.to_string();
                }
                self.settings = settings;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("无法保存 Memory 设置：{error}")),
        }
        UiModuleOutcome::dirty()
    }

    fn save_cooldown(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match crate::desktop::parse_memory_cooldown(&self.cooldown_input) {
            Ok(cooldown_turns) => {
                let mut settings = self.settings.clone();
                settings.cooldown_turns = cooldown_turns;
                self.save_settings(settings, cx)
            }
            Err(error) => {
                self.error = Some(error.to_owned());
                UiModuleOutcome::dirty()
            }
        }
    }

    /// Reloads the injection state of the task this window has open. Task-scoped
    /// rather than project-scoped, so a window with no task has none.
    fn refresh_injection(&mut self, cx: &UiModuleContext<'_>) {
        let tasks = cx
            .workspace()
            .and_then(|workspace| workspace.snapshot().ok())
            .map(|snapshot| snapshot.tasks)
            .unwrap_or_default();
        self.injection_task = cx
            .selected_task()
            .or_else(|| {
                self.injection_task
                    .clone()
                    .filter(|id| tasks.iter().any(|task| task.id == *id))
            })
            .or_else(|| tasks.first().map(|task| task.id.clone()));
        let Some(task_id) = self.injection_task.clone() else {
            self.injection = None;
            return;
        };
        let Ok(application) = cx.application() else {
            return;
        };
        match application.memory_injection_state(&task_id) {
            Ok(state) => self.injection = Some(state),
            Err(error) => self.error = Some(format!("无法读取 Memory 注入状态：{error}")),
        }
    }

    fn toggle_task_injection(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let (Some(task_id), Some(state)) = (self.injection_task.clone(), self.injection.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match application.set_task_memory_enabled(&task_id, !state.enabled, Some(state.updated_at))
        {
            Ok(state) => {
                self.injection = Some(state);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("无法更新 Memory 注入状态：{error}")),
        }
        self.refresh_injection(cx);
        UiModuleOutcome::dirty()
    }

    fn reset_task_cooldown(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let (Some(task_id), Some(state)) = (self.injection_task.clone(), self.injection.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let application = match cx.application() {
            Ok(application) => application,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match application.reset_task_memory_cooldown(&task_id, Some(state.updated_at)) {
            Ok(state) => {
                self.injection = Some(state);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("无法重置 Memory 冷却：{error}")),
        }
        UiModuleOutcome::dirty()
    }
}

impl UiModule for MemoryModule {
    type Message = MemoryMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            MemoryMessage::Open => {
                if let Some(task_id) = cx.selected_task() {
                    self.injection_task = Some(task_id);
                }
                UiModuleOutcome::effect(ShellEffect::RevealProjectSurface(
                    ProjectWorkspaceSurface::Memory,
                ))
            }
            MemoryMessage::Refresh => self.refresh(cx),
            MemoryMessage::Select(memory_id) => self.select(memory_id),
            MemoryMessage::New => {
                self.clear_editor();
                self.editor_dirty = true;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::NewInScope(scope) => {
                self.clear_editor();
                self.scope = scope;
                self.editor_dirty = true;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::SetScope(scope) => {
                self.scope = scope;
                self.editor_dirty = true;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::ToggleDraftEnabled => {
                self.draft_enabled = !self.draft_enabled;
                self.editor_dirty = true;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::ToggleEntry(id) => self.change_entry(id, false, cx),
            MemoryMessage::DeleteEntry(id) => self.change_entry(id, true, cx),
            MemoryMessage::CommitCooldown(turns) => {
                self.cooldown_input = turns.to_string();
                self.save_cooldown(cx)
            }
            MemoryMessage::TitleChanged(value) => {
                self.title = value;
                self.editor_dirty = true;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::BodyReplaced(value) => {
                self.body.set_text(&value);
                self.editor_dirty = true;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::TagsChanged(value) => {
                self.tags = value;
                self.editor_dirty = true;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::ToggleScope => {
                self.editor_dirty = true;
                self.scope = match self.scope {
                    MemoryScope::User => MemoryScope::Project,
                    MemoryScope::Project => MemoryScope::User,
                };
                UiModuleOutcome::dirty()
            }
            MemoryMessage::Save => self.save(cx),
            MemoryMessage::ToggleEnabled => match self.selected.clone() {
                Some(id) => self.change_entry(id, false, cx),
                None => UiModuleOutcome::clean(),
            },
            MemoryMessage::Delete => self.delete(cx),
            MemoryMessage::ToggleGlobal => {
                let mut settings = self.settings.clone();
                settings.enabled = !settings.enabled;
                self.save_settings(settings, cx)
            }
            MemoryMessage::ToggleBaseline => {
                let mut settings = self.settings.clone();
                settings.baseline_injection_enabled = !settings.baseline_injection_enabled;
                self.save_settings(settings, cx)
            }
            MemoryMessage::CycleCooldown => {
                let mut settings = self.settings.clone();
                settings.cooldown_turns =
                    crate::desktop::next_memory_cooldown(settings.cooldown_turns);
                self.save_settings(settings, cx)
            }
            MemoryMessage::CooldownChanged(value) => {
                self.cooldown_input = value;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::SaveCooldown => self.save_cooldown(cx),
            MemoryMessage::TaskMenuOpen(open) => {
                self.task_menu_open = open;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::SelectInjectionTask(id) => {
                self.task_menu_open = false;
                self.injection_task = lilia_contracts::TaskId::new(id).ok();
                self.refresh_injection(cx);
                UiModuleOutcome::dirty()
            }
            MemoryMessage::ToggleTaskInjection => self.toggle_task_injection(cx),
            MemoryMessage::ResetTaskCooldown => self.reset_task_cooldown(cx),
        }
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        if let Some(event) = envelope.downcast::<crate::application::MemoryChanged>() {
            if event
                .project_id
                .as_ref()
                .is_none_or(|project_id| cx.selected_project().as_ref() == Some(project_id))
            {
                return self.refresh(cx);
            }
        }
        if envelope.is::<crate::application::MemoryInjectionChanged>() {
            self.refresh_injection(cx);
            return UiModuleOutcome::dirty();
        }
        if envelope.is::<crate::application::MemorySettingsChanged>() {
            return self.refresh(cx);
        }
        UiModuleOutcome::clean()
    }

    fn project(&self, cx: &UiModuleContext<'_>, into: &mut PrimaryShellSnapshot) {
        if !cx.shows(ShellProjectPage::Memory) {
            return;
        }
        into.project_page_body = self.error.clone().unwrap_or_default();
        into.memory_enabled = self.memory().map(|memory| memory.enabled);
        into.memory_draft_enabled = self.draft_enabled;
        into.memory_editor_generation = self.editor_generation;
        into.memory_selected = self.selected.clone();
        into.memory_global_enabled = self.settings.enabled;
        into.memory_baseline_enabled = self.settings.baseline_injection_enabled;
        into.memory_cooldown = self.cooldown_input.clone();
        into.memory_task_enabled = self.injection.as_ref().map(|state| state.enabled);
        into.memory_task_menu_open = self.task_menu_open;
        into.memory_task_options = cx
            .workspace()
            .and_then(|workspace| workspace.snapshot().ok())
            .map(|snapshot| {
                snapshot
                    .tasks
                    .into_iter()
                    .map(|task| {
                        (
                            task.id.to_string(),
                            task.title,
                            self.injection_task.as_ref() == Some(&task.id),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        into.memory_task_label = into
            .memory_task_options
            .iter()
            .find(|(_, _, selected)| *selected)
            .map(|(_, title, _)| format!("注入会话：{title}"))
            .unwrap_or_else(|| "选择会话".to_owned());
        into.memory_title = self.title.clone();
        into.memory_body = self.body.text();
        into.memory_tags = self.tags.clone();
        into.memory_scope_label = match self.scope {
            MemoryScope::User => "用户".to_owned(),
            MemoryScope::Project => "项目".to_owned(),
        };
        let mut memories = self.memories.iter().collect::<Vec<_>>();
        memories.sort_by_key(|memory| {
            (
                matches!(memory.scope, MemoryScope::Project),
                memory.title.to_lowercase(),
            )
        });
        into.memory_cards = memories
            .into_iter()
            .map(|memory| ShellMemoryCard {
                id: memory.id.clone(),
                title: memory.title.clone(),
                enabled: memory.enabled,
                selected: self.selected.as_ref() == Some(&memory.id),
                body: memory.body.clone(),
                updated_at: memory.updated_at,
                scope_label: match memory.scope {
                    MemoryScope::User => "用户",
                    MemoryScope::Project => "项目",
                }
                .to_owned(),
                subtitle: format!(
                    "{} · {}\n{}",
                    match memory.scope {
                        MemoryScope::User => "用户",
                        MemoryScope::Project => "项目",
                    },
                    if memory.enabled { "启用" } else { "停用" },
                    memory.body.lines().take(3).collect::<Vec<_>>().join("\n")
                ),
            })
            .collect();
    }
}
