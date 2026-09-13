//! The Memory page as a UI module.
//!
//! Owns the memory list, the editor fields, the injection settings and the
//! selected task's injection state. One instance per window: the editor is
//! window state, and the settings are read back from the service on refresh so
//! two windows cannot disagree about what is stored.

use lilia_feature_memory::MemoryServiceKey;
use lilia_kernel::FeatureId;

use crate::application::ProjectWorkspaceSurface;
use crate::runtime_shell::ShellProjectPage;
pub mod view;
use crate::text_editor_state::TextEditorState;
use crate::ui_module::{ShellEffect, UiModule, UiModuleContext, UiModuleOutcome};
use lilia_feature_memory::{
    DesktopMemory, MemoryInjectionState, MemoryScope, MemorySettings, MemoryUpsertInput,
};
use view::{MemoryCard, MemoryViewSnapshot};

/// The memory domain's own message vocabulary.
#[derive(Debug, Clone)]
pub enum MemoryMessage {
    Open,
    Refresh,
    Select(String),
    New,
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
    ToggleTaskInjection,
    ResetTaskCooldown,
    ToggleTaskMenu,
    SelectInjectionTask(String),
}

pub struct MemoryModule {
    memories: Vec<DesktopMemory>,
    selected: Option<String>,
    title: String,
    body: TextEditorState,
    tags: String,
    scope: MemoryScope,
    loaded_title: String,
    loaded_body: String,
    loaded_tags: String,
    loaded_scope: MemoryScope,
    updated_at: Option<i64>,
    error: Option<String>,
    settings: MemorySettings,
    cooldown_input: String,
    injection: Option<MemoryInjectionState>,
    injection_task: Option<String>,
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
            loaded_title: String::new(),
            loaded_body: String::new(),
            loaded_tags: String::new(),
            loaded_scope: MemoryScope::Project,
            updated_at: None,
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
        self.selected = memory_id
            .filter(|selected| self.has(selected))
            .or_else(|| self.memories.first().map(|memory| memory.id.clone()));
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
        let service = match cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.settings() {
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
        match service.list(Some(project_id.as_str())) {
            Ok(memories) => {
                self.memories = memories;
                self.apply_loaded_records(false);
                self.error = None;
            }
            Err(error) => self.error = Some(format!("无法读取 Memory：{error}")),
        }
        UiModuleOutcome::dirty()
    }

    fn reload_settings(&mut self, cx: &UiModuleContext<'_>) {
        let Ok(service) = cx.kernel().service::<MemoryServiceKey>() else {
            return;
        };
        match service.settings() {
            Ok(settings) => {
                if settings.cooldown_turns != self.settings.cooldown_turns {
                    self.cooldown_input = settings.cooldown_turns.to_string();
                }
                self.settings = settings;
            }
            Err(error) => self.error = Some(format!("无法读取 Memory 设置：{error}")),
        }
        self.refresh_injection(cx);
    }

    fn editor_is_dirty(&self) -> bool {
        if self.selected.is_none() {
            return !self.title.is_empty()
                || !self.body.text().is_empty()
                || !self.tags.is_empty();
        }
        self.title != self.loaded_title
            || self.body.text() != self.loaded_body
            || self.tags != self.loaded_tags
            || self.scope != self.loaded_scope
    }

    fn apply_loaded_records(&mut self, reload_editor: bool) {
        let selected_missing = self
            .selected
            .as_deref()
            .is_some_and(|selected| !self.has(selected));
        if !reload_editor && self.editor_is_dirty() {
            if selected_missing {
                self.selected = None;
                self.updated_at = None;
            } else if let Some(memory) = self.memory() {
                if memory.title == self.loaded_title
                    && memory.body == self.loaded_body
                    && memory.tags.join(", ") == self.loaded_tags
                    && memory.scope == self.loaded_scope
                {
                    self.updated_at = Some(memory.updated_at);
                }
            }
            return;
        }
        if self
            .selected
            .as_deref()
            .is_none_or(|selected| !self.has(selected))
        {
            self.selected = self.memories.first().map(|memory| memory.id.clone());
        }
        self.load_selected();
    }

    fn select(&mut self, memory_id: String) -> UiModuleOutcome {
        if !self.has(&memory_id) {
            return UiModuleOutcome::clean();
        }
        self.selected = Some(memory_id);
        self.load_selected();
        UiModuleOutcome::dirty()
    }

    /// Empties the editor, which is also how a new memory is started: an unsaved
    /// draft is the absence of a selection.
    fn clear_editor(&mut self) {
        self.selected = None;
        self.title.clear();
        self.body.clear();
        self.tags.clear();
        self.scope = MemoryScope::Project;
        self.loaded_title.clear();
        self.loaded_body.clear();
        self.loaded_tags.clear();
        self.loaded_scope = MemoryScope::Project;
        self.updated_at = None;
        self.error = None;
    }

    fn load_selected(&mut self) {
        let loaded = self.memory().map(|memory| {
            (
                memory.title.clone(),
                memory.body.clone(),
                memory.tags.join(", "),
                memory.scope,
                memory.updated_at,
            )
        });
        match loaded {
            Some((title, body, tags, scope, updated_at)) => {
                self.title = title.clone();
                self.body.set_text(&body);
                self.tags = tags.clone();
                self.scope = scope;
                self.loaded_title = title;
                self.loaded_body = body;
                self.loaded_tags = tags;
                self.loaded_scope = scope;
                self.updated_at = Some(updated_at);
            }
            None => self.clear_editor(),
        }
    }

    fn save(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let service = match cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        {
            Ok(service) => service,
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
            enabled: self.memory().is_none_or(|memory| memory.enabled),
            source_task_id: self
                .memory()
                .and_then(|memory| memory.source_task_id.clone()),
            expected_updated_at: self.updated_at,
        };
        match service.save(input) {
            Ok(memory) => {
                self.selected = Some(memory.id);
                let outcome = self.refresh(cx);
                self.load_selected();
                outcome
            }
            Err(error) => {
                self.error = Some(format!("无法保存 Memory：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn toggle_enabled(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some((memory_id, enabled, expected_updated_at)) = self
            .memory()
            .map(|memory| (memory.id.clone(), !memory.enabled, Some(memory.updated_at)))
        else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.set_enabled_if_unmodified(&memory_id, enabled, expected_updated_at) {
            Ok(_) => self.refresh(cx),
            Err(error) => {
                self.error = Some(format!("无法更新 Memory 状态：{error}"));
                UiModuleOutcome::dirty()
            }
        }
    }

    fn delete(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let Some(memory_id) = self.selected.clone() else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.delete_if_unmodified(&memory_id, self.updated_at) {
            Ok(_) => {
                self.clear_editor();
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
        let service = match cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        let cooldown_changed = settings.cooldown_turns != self.settings.cooldown_turns;
        match service.save_settings(settings) {
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

    fn injection_task_id(&self, cx: &UiModuleContext<'_>) -> Option<lilia_contracts::TaskId> {
        self.injection_task
            .as_ref()
            .and_then(|id| lilia_contracts::TaskId::new(id).ok())
            .or_else(|| cx.selected_task())
    }

    /// Reloads the injection state of the selected injection task, falling back
    /// to the window's open task.
    fn refresh_injection(&mut self, cx: &UiModuleContext<'_>) {
        let Some(task_id) = self
            .injection_task
            .as_ref()
            .and_then(|id| lilia_contracts::TaskId::new(id).ok())
            .or_else(|| cx.selected_task())
        else {
            self.injection = None;
            return;
        };
        let Ok(service) = cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        else {
            return;
        };
        match service.injection_state(task_id.as_str()) {
            Ok(state) => self.injection = Some(state),
            Err(error) => self.error = Some(format!("无法读取 Memory 注入状态：{error}")),
        }
    }

    fn toggle_task_injection(&mut self, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        let (Some(task_id), Some(state)) = (self.injection_task_id(cx), self.injection.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.set_task_enabled_if_unmodified(
            task_id.as_str(),
            !state.enabled,
            Some(state.updated_at),
        ) {
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
        let (Some(task_id), Some(state)) = (self.injection_task_id(cx), self.injection.clone())
        else {
            return UiModuleOutcome::clean();
        };
        let service = match cx
            .kernel()
            .service::<MemoryServiceKey>()
            .map_err(|error| format!("Memory 服务不可用：{error}"))
        {
            Ok(service) => service,
            Err(error) => {
                self.error = Some(error);
                return UiModuleOutcome::dirty();
            }
        };
        match service.reset_task_cooldown_if_unmodified(task_id.as_str(), Some(state.updated_at)) {
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
    type Projection<'a> = crate::ui_module::projection::MemoryProjection<'a>;

    type Message = MemoryMessage;

    fn feature(&self) -> FeatureId {
        Self::feature_id()
    }

    fn reduce(&mut self, message: Self::Message, cx: &UiModuleContext<'_>) -> UiModuleOutcome {
        match message {
            MemoryMessage::Open => UiModuleOutcome::effect(ShellEffect::RevealProjectSurface(
                ProjectWorkspaceSurface::Memory,
            )),
            MemoryMessage::Refresh => self.refresh(cx),
            MemoryMessage::Select(memory_id) => self.select(memory_id),
            MemoryMessage::New => {
                self.clear_editor();
                UiModuleOutcome::dirty()
            }
            MemoryMessage::TitleChanged(value) => {
                self.title = value;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::BodyReplaced(value) => {
                self.body.set_text(&value);
                self.error = None;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::TagsChanged(value) => {
                self.tags = value;
                self.error = None;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::ToggleScope => {
                self.scope = match self.scope {
                    MemoryScope::User => MemoryScope::Project,
                    MemoryScope::Project => MemoryScope::User,
                };
                UiModuleOutcome::dirty()
            }
            MemoryMessage::Save => self.save(cx),
            MemoryMessage::ToggleEnabled => self.toggle_enabled(cx),
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
                if crate::desktop::parse_memory_cooldown(&self.cooldown_input).is_ok() {
                    self.save_cooldown(cx)
                } else {
                    UiModuleOutcome::dirty()
                }
            }
            MemoryMessage::SaveCooldown => self.save_cooldown(cx),
            MemoryMessage::ToggleTaskInjection => self.toggle_task_injection(cx),
            MemoryMessage::ResetTaskCooldown => self.reset_task_cooldown(cx),
            MemoryMessage::ToggleTaskMenu => {
                self.task_menu_open = !self.task_menu_open;
                UiModuleOutcome::dirty()
            }
            MemoryMessage::SelectInjectionTask(task_id) => {
                self.injection_task = Some(task_id);
                self.task_menu_open = false;
                self.refresh_injection(cx);
                UiModuleOutcome::dirty()
            }
        }
    }

    fn invalidate(
        &mut self,
        envelope: &lilia_kernel::EventEnvelope,
        cx: &UiModuleContext<'_>,
    ) -> UiModuleOutcome {
        if let Some(event) = envelope.downcast::<lilia_feature_memory::MemoryChanged>() {
            let matches_project = event
                .project_id
                .as_ref()
                .is_none_or(|project_id| cx.selected_project().as_ref() == Some(project_id));
            let listed = event
                .memory_id
                .as_ref()
                .is_some_and(|memory_id| self.has(memory_id));
            if matches_project || listed {
                return self.refresh(cx);
            }
        }
        if envelope.is::<lilia_feature_memory::MemoryInjectionChanged>() {
            self.refresh_injection(cx);
            return UiModuleOutcome::dirty();
        }
        if envelope.is::<lilia_feature_memory::MemorySettingsChanged>() {
            self.reload_settings(cx);
            return UiModuleOutcome::dirty();
        }
        UiModuleOutcome::clean()
    }

    fn project_fields(&self, cx: &UiModuleContext<'_>, into: Self::Projection<'_>) {
        if !cx.shows(ShellProjectPage::Memory) {
            return;
        }
        *into.memory = MemoryViewSnapshot {
            project_id: cx.selected_project().map(|id| id.as_str().to_owned()),
            selected: self.selected.clone(),
            title: self.title.clone(),
            body: self.body.text(),
            tags: self.tags.clone(),
            error: self.error.clone(),
            scope_label: match self.scope {
                MemoryScope::User => "用户".to_owned(),
                MemoryScope::Project => "项目".to_owned(),
            },
            task_menu_open: self.task_menu_open,
            enabled: self.memory().is_some_and(|memory| memory.enabled),
            global_enabled: self.settings.enabled,
            baseline_enabled: self.settings.baseline_injection_enabled,
            task_injection: self.injection.as_ref().map(|state| state.enabled),
            cooldown: self.cooldown_input.clone(),
            tasks: cx
                .workspace_snapshot()
                .map(|snapshot| {
                    snapshot
                        .tasks
                        .into_iter()
                        .map(|task| {
                            (
                                task.id.as_str().to_owned(),
                                if task.title.trim().is_empty() {
                                    "未命名会话".to_owned()
                                } else {
                                    task.title
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            cards: self
                .memories
                .iter()
                .map(|memory| MemoryCard {
                    id: memory.id.clone(),
                    title: memory.title.clone(),
                    subtitle: format!(
                        "{} · {}",
                        match memory.scope {
                            MemoryScope::User => "用户",
                            MemoryScope::Project => "项目",
                        },
                        if memory.enabled { "启用" } else { "停用" }
                    ),
                })
                .collect(),
        };
    }
}
