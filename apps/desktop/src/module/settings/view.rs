use super::presentation::{
    CredentialRow, CustomAgentRow, McpEditor, McpRow, ProviderRow, SkillRow,
};
use crate::runtime_layout::{pill_button as extra_button, reconcile_children};
use crate::runtime_shell::{IntentSink, ShellActionRow, ShellIntent, emit};
use nana_ui::runtime::{
    AboutMetadata, AboutSection, Activate, AppContext, AppearanceSection, Button, DocumentId,
    DonutChart, DonutSlice, Dropdown, DropdownOption, Entity, FormField, FrameworkError,
    LengthSpec, NodeStyle, QrCode, SearchDropdown, SearchDropdownEvent, SearchDropdownOption,
    SemanticColorRole, SettingsBack, SettingsCard, SettingsPage, SettingsRow, SettingsSidebar,
    SettingsTabSelected, StableNodeId, Stack, Switch, Text, TextChanged, TextInput,
    TimeSeriesChart, ToggleChanged,
};
use nana_ui::{
    AppearanceEvent, AppearanceSettings, ButtonKind, ControlSize, DropdownEvent, DropdownSelection,
    SettingsModel, SettingsState,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SettingsSnapshot {
    pub model: SettingsModel,
    pub state: SettingsState,
    pub appearance: AppearanceSettings,
    pub material_status: String,
    pub project_name: String,
    pub project_workspace: String,
    pub project_error: Option<String>,
    pub providers: Vec<ProviderRow>,
    pub provider_status: String,
    pub agent_actions: Vec<ShellActionRow>,
    pub quota_status: String,
    pub extensions_status: String,
    pub extensions_search: String,
    pub extensions: Option<crate::runtime_extensions::ExtensionBrowserSnapshot>,
    pub remote_status: String,
    pub remote_host_enabled: bool,
    pub remote_keep_awake: bool,
    pub remote_pc_name: String,
    pub remote_pairing_active: bool,
    pub remote_pairing_uri: String,
    pub remote_devices: Vec<(String, String)>,
    pub project_clone_parent: String,
    pub project_worktree_mode: String,
    pub project_worktree_parent: String,
    pub project_worktree_instructions: String,
    pub project_cleanup_on_archive: bool,
    pub desktop_status: String,
    pub data_status: String,
    pub data_can_import: bool,
    pub provider_secret: String,
    pub provider_model: String,
    pub provider_openai_endpoint: String,
    pub provider_anthropic_endpoint: String,
    pub can_save_credential: bool,
    pub credentials: Vec<CredentialRow>,
    pub custom_agents: Vec<CustomAgentRow>,
    pub custom_agent_editor_open: bool,
    pub custom_agent_name: String,
    pub custom_agent_description: String,
    pub custom_agent_instruction: String,
    pub quota_days_label: String,
    pub quota_backend_label: String,
    pub quota_values: Vec<f64>,
    pub quota_axis_labels: Vec<String>,
    pub quota_daily: Vec<lilia_feature_usage::QuotaUsageDailyBucket>,
    pub quota_project_slices: Vec<(String, f64)>,
    pub quota_conversation_slices: Vec<(String, f64)>,
    pub quota_tool_slices: Vec<(String, f64)>,
    pub skills: Vec<SkillRow>,
    pub skill_id: String,
    pub skill_description: String,
    pub can_create_skill: bool,
    pub mcp_servers: Vec<McpRow>,
    pub mcp_editor: Option<McpEditor>,
    pub github_state: String,
    pub github_login: String,
    pub github_busy: bool,
    pub github_can_bind: bool,
    pub shortcut: String,
    pub shortcut_capturing: bool,
    pub shortcut_registered: bool,
    pub sidebar_display_mode: String,
}

pub(crate) struct SettingsView {
    sink: IntentSink,
    action_bindings: HashMap<String, Arc<Mutex<ShellIntent>>>,
    pub(crate) fields: crate::form_view::ProductFields,
    pub(crate) settings_sidebar: Entity<SettingsSidebar>,
    pub(crate) settings_page: Entity<SettingsPage>,
    pub(crate) appearance: Entity<AppearanceSection>,
    pub(crate) appearance_page: Entity<Stack>,
    pub(crate) sidebar_mode: Entity<Dropdown>,
    pub(crate) about: Entity<AboutSection>,
    pub(crate) product_settings: Entity<Stack>,
    pub(crate) product_body: Entity<Text>,
    pub(crate) product_error: Entity<Text>,
    pub(crate) project_name: Entity<TextInput>,
    pub(crate) project_name_field: Entity<FormField>,
    pub(crate) project_workspace: Entity<Text>,
    pub(crate) project_workspace_row: Entity<SettingsRow>,
    pub(crate) clone_parent: Entity<Text>,
    pub(crate) clone_parent_row: Entity<SettingsRow>,
    pub(crate) worktree_parent: Entity<Text>,
    pub(crate) worktree_parent_row: Entity<SettingsRow>,
    pub(crate) worktree_mode: Entity<Dropdown>,
    pub(crate) worktree_mode_row: Entity<SettingsRow>,
    pub(crate) remote_qr: Option<Entity<QrCode>>,
    pub(crate) remote_pair_hint: Entity<Text>,
    pub(crate) product_actions: HashMap<String, Entity<Button>>,
    provider: Entity<SearchDropdown>,
    provider_field: Entity<FormField>,
    card_body: Entity<Stack>,
    toolbar: Entity<Stack>,
    pub(crate) settings_card: Entity<SettingsCard>,
    pub(crate) quota_chart: Option<Entity<TimeSeriesChart>>,
    pub(crate) quota_donuts: HashMap<String, Entity<DonutChart>>,
    pub(crate) form_switches: HashMap<String, Entity<Switch>>,
    pub(crate) extensions: crate::runtime_extensions::ExtensionBrowser,
}
impl SettingsView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document_id: DocumentId,
        settings: &SettingsSnapshot,
        theme: nana_ui::ThemeMode,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        let settings_sidebar = context.create_detached_component(
            document_id,
            SettingsSidebar::new(settings.model.clone(), settings.state.clone()),
        )?;
        context.on(settings_sidebar, {
            let sink = Arc::clone(&sink);
            move |_, _event: &SettingsBack, _| emit(&sink, ShellIntent::CloseSettings)
        })?;
        context.on(settings_sidebar, {
            let sink = Arc::clone(&sink);
            move |_, event: &SettingsTabSelected, _| {
                emit(&sink, ShellIntent::SelectSettingsTab(event.tab.clone()))
            }
        })?;
        let appearance = context.create_detached_component(
            document_id,
            AppearanceSection::new(theme, settings.appearance.clone())
                .material_status(settings.material_status.clone()),
        )?;
        context.on(appearance, {
            let sink = Arc::clone(&sink);
            move |_, event: &AppearanceEvent, _| emit(&sink, ShellIntent::Appearance(*event))
        })?;
        let about = context.create_detached_component(
            document_id,
            AboutSection::new(
                AboutMetadata::new("LiliaCode", env!("CARGO_PKG_VERSION"))
                    .description("本机工作区"),
            ),
        )?;
        let product_settings =
            context.create_detached_component(document_id, Stack::column(16.0).max_width(760.0))?;
        let card_body = context.create_detached_component(document_id, Stack::column(12.0))?;
        let toolbar = context.create_detached_component(document_id, Stack::bar(8.0).wrap(true))?;
        let provider = context.create_detached_component(
            document_id,
            SearchDropdown::new(None::<String>).placeholder("选择模型服务"),
        )?;
        let provider_sink = Arc::clone(&sink);
        context.on(provider, move |_, event: &SearchDropdownEvent, _| {
            if let SearchDropdownEvent::Select(id) = event {
                emit(&provider_sink, ShellIntent::SelectProvider(id.to_string()));
            }
        })?;
        let provider_field = context.create_detached_component(
            document_id,
            FormField::new("模型服务").control_child(provider.stable_id()),
        )?;
        context.append_child(provider_field, provider)?;
        let settings_card =
            context.create_detached_component(document_id, SettingsCard::new(String::new()))?;
        let product_body =
            context.create_detached_component(document_id, Text::new(String::new()))?;
        let product_error =
            context.create_detached_component(document_id, Text::new(String::new()))?;
        let project_name = context.create_detached_component(
            document_id,
            TextInput::new(settings.project_name.clone()),
        )?;
        let project_name_sink = Arc::clone(&sink);
        context.on(project_name, move |_, event: &TextChanged, _| {
            emit(
                &project_name_sink,
                ShellIntent::ProjectNameChanged(event.value.clone()),
            );
        })?;
        let project_name_field = context.create_detached_component(
            document_id,
            FormField::new("项目名称").control_child(project_name.stable_id()),
        )?;
        context.append_child(project_name_field, project_name)?;
        let project_workspace = context.create_detached_component(
            document_id,
            Text::new(settings.project_workspace.clone()),
        )?;
        let project_workspace_row = context.create_detached_component(
            document_id,
            SettingsRow::new("工作区")
                .stacked(true)
                .control_child(project_workspace.stable_id()),
        )?;
        context.append_child(project_workspace_row, project_workspace)?;
        let clone_parent = context.create_detached_component(
            document_id,
            Text::new(clone_parent_label(&settings.project_clone_parent)),
        )?;
        let clone_parent_row = context.create_detached_component(
            document_id,
            SettingsRow::new("Clone 默认父目录")
                .stacked(true)
                .control_child(clone_parent.stable_id()),
        )?;
        context.append_child(clone_parent_row, clone_parent)?;
        let worktree_parent = context.create_detached_component(
            document_id,
            Text::new(worktree_parent_label(&settings.project_worktree_parent)),
        )?;
        let worktree_parent_row = context.create_detached_component(
            document_id,
            SettingsRow::new("工作树父目录")
                .stacked(true)
                .control_child(worktree_parent.stable_id()),
        )?;
        context.append_child(worktree_parent_row, worktree_parent)?;
        let worktree_mode = context.create_detached_component(
            document_id,
            worktree_mode_dropdown(&settings.project_worktree_mode),
        )?;
        let worktree_sink = Arc::clone(&sink);
        context.on(
            worktree_mode,
            move |_, event: &DropdownEvent<Arc<str>>, _| {
                if let DropdownEvent::Select(value) = event {
                    emit(
                        &worktree_sink,
                        ShellIntent::SetProjectWorktreeMode(value.to_string()),
                    );
                }
            },
        )?;
        let worktree_mode_row = context.create_detached_component(
            document_id,
            SettingsRow::new("工作树默认行为")
                .stacked(true)
                .control_child(worktree_mode.stable_id()),
        )?;
        context.append_child(worktree_mode_row, worktree_mode)?;
        let remote_pair_hint = context.create_detached_component(
            document_id,
            Text::new("请使用 Android 扫码完成配对；过期后重新生成。"),
        )?;
        context.append_child(product_settings, settings_card)?;
        context.append_child(settings_card, card_body)?;
        let sidebar_mode = context.create_detached_component(
            document_id,
            sidebar_mode_dropdown(&settings.sidebar_display_mode),
        )?;
        let sidebar_sink = Arc::clone(&sink);
        context.on(
            sidebar_mode,
            move |_, event: &DropdownEvent<Arc<str>>, _| {
                if let DropdownEvent::Select(value) = event {
                    emit(
                        &sidebar_sink,
                        ShellIntent::SetSidebarDisplayMode(value.to_string()),
                    );
                }
            },
        )?;
        let sidebar_mode_row = context.create_detached_component(
            document_id,
            SettingsRow::new("侧边栏样式")
                .stacked(true)
                .control_child(sidebar_mode.stable_id()),
        )?;
        context.append_child(sidebar_mode_row, sidebar_mode)?;
        let appearance_page =
            context.create_detached_component(document_id, Stack::column(16.0).max_width(760.0))?;
        context.append_child(appearance_page, appearance)?;
        context.append_child(appearance_page, sidebar_mode_row)?;
        let settings_page = context.create_detached_component(
            document_id,
            SettingsPage::new(settings.model.clone(), settings.state.clone())
                .content(appearance_page.stable_id()),
        )?;

        context.assemble_settings_sidebar(settings_sidebar)?;
        context.assemble_appearance_section(appearance)?;
        context.assemble_about_section(about)?;
        context.assemble_settings_page(settings_page)?;
        let extensions = crate::runtime_extensions::ExtensionBrowser::mount(
            context,
            document_id,
            Arc::clone(&sink),
        )?;
        Ok(Self {
            fields: crate::form_view::ProductFields::new(Arc::clone(&sink)),
            sink,
            action_bindings: HashMap::new(),
            settings_sidebar,
            settings_page,
            appearance,
            appearance_page,
            sidebar_mode,
            about,
            product_settings,
            product_body,
            product_error,
            project_name,
            project_name_field,
            project_workspace,
            project_workspace_row,
            clone_parent,
            clone_parent_row,
            worktree_parent,
            worktree_parent_row,
            worktree_mode,
            worktree_mode_row,
            remote_qr: None,
            remote_pair_hint,
            product_actions: HashMap::new(),
            provider,
            provider_field,
            card_body,
            toolbar,
            settings_card,
            quota_chart: None,
            quota_donuts: HashMap::new(),
            form_switches: HashMap::new(),
            extensions,
        })
    }
    pub(crate) fn sync(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        settings: &SettingsSnapshot,
        theme: nana_ui::ThemeMode,
        visible: bool,
        width: f32,
    ) -> Result<(), FrameworkError> {
        if !visible {
            context.update_component(self.provider, |field, _| {
                field.close();
            })?;
            context.update_component(self.sidebar_mode, |field, _| {
                field.opened = false;
            })?;
            context.update_component(self.worktree_mode, |field, _| {
                field.opened = false;
            })?;
            return Ok(());
        }
        context.update_component(self.settings_sidebar, |sidebar, _| {
            sidebar.model = settings.model.clone();
            sidebar.state = settings.state.clone();
        })?;
        context.update_component(self.appearance, |section, _| {
            section.theme = theme;
            section.appearance = settings.appearance.clone();
            section.platform_hint = None;
            section.material_status = Some(Arc::from(settings.material_status.as_str()));
        })?;
        let tab = settings.state.active_tab().as_str();
        let (_heading, body, error, show_project, actions) = settings_tab_copy(settings);
        context.update_component(self.product_body, |text, _| {
            *text = Text::new(body.clone());
        })?;
        context.update_component(self.product_error, |text, _| {
            *text = Text::new(error.clone().unwrap_or_default());
        })?;
        context.update_component(self.project_name, |editor, _| {
            if editor.state.value != settings.project_name {
                editor.state.replace_value(settings.project_name.clone());
            }
            editor.disabled = !show_project;
        })?;
        context.update_component(self.project_workspace, |text, _| {
            *text = Text::new(if show_project {
                settings.project_workspace.clone()
            } else {
                String::new()
            });
        })?;
        context.update_component(self.sidebar_mode, |field, _| {
            field.selection =
                DropdownSelection::Single(Some(Arc::from(settings.sidebar_display_mode.as_str())));
            if tab != "appearance" {
                field.opened = false;
            }
        })?;
        context.update_component(self.worktree_mode, |field, _| {
            field.selection =
                DropdownSelection::Single(Some(Arc::from(settings.project_worktree_mode.as_str())));
            if tab != "project" {
                field.opened = false;
            }
        })?;
        context.update_component(self.clone_parent, |text, _| {
            *text = Text::new(clone_parent_label(&settings.project_clone_parent));
        })?;
        context.update_component(self.worktree_parent, |text, _| {
            *text = Text::new(worktree_parent_label(&settings.project_worktree_parent));
        })?;
        let content = if settings.extensions.is_some() {
            self.extensions.root.stable_id()
        } else {
            match tab {
                "appearance" => self.appearance_page.stable_id(),
                "about" => self.about.stable_id(),
                _ => self.product_settings.stable_id(),
            }
        };
        context.update_component(self.settings_page, |page, _| {
            page.model = settings.model.clone();
            page.state = settings.state.clone();
            page.content = Some(content);
        })?;
        context.assemble_settings_sidebar(self.settings_sidebar)?;
        context.assemble_appearance_section(self.appearance)?;
        context.assemble_about_section(self.about)?;
        context.assemble_settings_page(self.settings_page)?;
        if let Some(extensions) = &settings.extensions {
            return self.extensions.sync(
                context,
                document_id,
                extensions,
                width,
                Arc::clone(&self.sink),
            );
        }

        let mut keep = HashSet::new();
        let mut card_order = Vec::new();
        let mut action_order = Vec::new();
        if show_project {
            card_order.push(self.project_name_field.stable_id());
            card_order.push(self.project_workspace_row.stable_id());
            card_order.push(self.clone_parent_row.stable_id());
            card_order.push(self.worktree_mode_row.stable_id());
            card_order.push(self.worktree_parent_row.stable_id());
        }
        for action in actions {
            let id = format!("action:{}", action.id);
            keep.insert(id.clone());
            let button = self.upsert_action(
                context,
                document_id,
                &id,
                product_action_button(&action.label, action.primary),
                action.intent,
            )?;
            action_order.push(button.stable_id());
        }
        reconcile_children(context, self.toolbar.stable_id(), &action_order)?;
        if !action_order.is_empty() {
            card_order.push(self.toolbar.stable_id());
        }
        context.update_component(self.provider, |field, _| {
            field.value = settings
                .providers
                .iter()
                .find(|provider| provider.selected)
                .map(|provider| Arc::from(provider.id.as_str()));
            field.options = settings
                .providers
                .iter()
                .map(|provider| {
                    SearchDropdownOption::new(provider.id.clone(), provider.label.clone())
                })
                .collect();
            if tab != "provider" {
                field.close();
            }
        })?;
        if tab == "provider" {
            card_order.push(self.provider_field.stable_id());
        }
        self.append_settings_forms(context, document_id, settings, &mut keep, &mut card_order)?;
        if settings.remote_pairing_uri.is_empty() {
            if let Some(chart) = self.remote_qr.take() {
                let _ = context.remove_view(chart);
            }
        }
        let stale: Vec<_> = self
            .product_actions
            .keys()
            .chain(self.form_switches.keys())
            .filter(|key| !keep.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            if let Some(button) = self.product_actions.remove(&key) {
                self.action_bindings.remove(&key);
                let _ = context.remove_view(button);
            }
            if let Some(toggle) = self.form_switches.remove(&key) {
                let _ = context.remove_view(toggle);
            }
        }
        self.fields.retain(context, &keep)?;
        context.update_component(self.settings_card, |card, _| {
            *card = SettingsCard::new(String::new());
        })?;
        reconcile_children(context, self.card_body.stable_id(), &card_order)?;
        let mut product_order = Vec::new();
        if !body.is_empty() {
            product_order.push(self.product_body.stable_id());
        }
        if error.as_ref().is_some_and(|value| !value.is_empty()) {
            product_order.push(self.product_error.stable_id());
        }
        product_order.push(self.settings_card.stable_id());
        reconcile_children(context, self.product_settings.stable_id(), &product_order)
    }

    fn upsert_action(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        id: &str,
        view: Button,
        intent: ShellIntent,
    ) -> Result<Entity<Button>, FrameworkError> {
        if let Some(button) = self.product_actions.get(id).copied() {
            *self
                .action_bindings
                .get(id)
                .expect("action binding")
                .lock()
                .unwrap() = intent;
            context.update_component(button, |button, _| *button = view)?;
            return Ok(button);
        }
        let button = context.create_detached_component(document_id, view)?;
        let binding = Arc::new(Mutex::new(intent));
        let callback_binding = Arc::clone(&binding);
        let sink = Arc::clone(&self.sink);
        context.on(button, move |_, _: &Activate, _| {
            let intent = callback_binding.lock().unwrap().clone();
            emit(&sink, intent);
        })?;
        self.action_bindings.insert(id.to_owned(), binding);
        self.product_actions.insert(id.to_owned(), button);
        Ok(button)
    }

    fn append_settings_forms(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        settings: &SettingsSnapshot,
        keep: &mut HashSet<String>,
        order: &mut Vec<StableNodeId>,
    ) -> Result<(), FrameworkError> {
        match settings.state.active_tab().as_str() {
            "project" => {
                self.upsert_switch(
                    context,
                    document_id,
                    keep,
                    order,
                    "worktree-cleanup",
                    "归档后清理工作树",
                    settings.project_cleanup_on_archive,
                    ShellIntent::ToggleWorktreeCleanup,
                )?;
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "worktree-instructions",
                    &settings.project_worktree_instructions,
                    |value| ShellIntent::ProjectWorktreeInstructionsChanged(value),
                )?;
            }
            "provider" => {
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "provider_secret",
                    &settings.provider_secret,
                    |value| ShellIntent::ProviderSecretChanged(value),
                )?;
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "provider_model",
                    &settings.provider_model,
                    |value| ShellIntent::ProviderModelChanged(value),
                )?;
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "provider_openai",
                    &settings.provider_openai_endpoint,
                    |value| ShellIntent::ProviderOpenAiEndpointChanged(value),
                )?;
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "provider_anthropic",
                    &settings.provider_anthropic_endpoint,
                    |value| ShellIntent::ProviderAnthropicEndpointChanged(value),
                )?;
                for credential in &settings.credentials {
                    let id = format!("revoke-{}", credential.id);
                    keep.insert(id.clone());
                    let label = format!("撤销 {}", credential.label);
                    let button = self.upsert_action(
                        context,
                        document_id,
                        &id,
                        extra_button(&label, ButtonKind::Danger),
                        ShellIntent::RevokeProviderCredential {
                            credential_id: credential.id.clone(),
                            revision: credential.revision,
                        },
                    )?;
                    order.push(button.stable_id());
                }
            }
            "agent" => {
                if settings.custom_agent_editor_open {
                    self.fields.upsert(
                        context,
                        document_id,
                        keep,
                        order,
                        "agent_name",
                        &settings.custom_agent_name,
                        |value| ShellIntent::AgentNameChanged(value),
                    )?;
                    self.fields.upsert(
                        context,
                        document_id,
                        keep,
                        order,
                        "agent_description",
                        &settings.custom_agent_description,
                        |value| ShellIntent::AgentDescriptionChanged(value),
                    )?;
                    self.fields.upsert(
                        context,
                        document_id,
                        keep,
                        order,
                        "agent_instruction",
                        &settings.custom_agent_instruction,
                        |value| ShellIntent::AgentInstructionChanged(value),
                    )?;
                }
                for agent in &settings.custom_agents {
                    for (suffix, label, intent) in [
                        (
                            "edit",
                            format!("编辑 {}", agent.label),
                            ShellIntent::EditCustomAgent(agent.id.clone()),
                        ),
                        (
                            "toggle",
                            if agent.enabled {
                                format!("关闭 {}", agent.label)
                            } else {
                                format!("开启 {}", agent.label)
                            },
                            ShellIntent::ToggleCustomAgent(agent.id.clone()),
                        ),
                        (
                            "delete",
                            format!("删除 {}", agent.label),
                            ShellIntent::DeleteCustomAgent(agent.id.clone()),
                        ),
                    ] {
                        let id = format!("agent-{}-{}", agent.id, suffix);
                        keep.insert(id.clone());
                        let kind = if suffix == "delete" {
                            ButtonKind::Danger
                        } else {
                            ButtonKind::Subtle
                        };
                        let button = self.upsert_action(
                            context,
                            document_id,
                            &id,
                            extra_button(&label, kind),
                            intent,
                        )?;
                        order.push(button.stable_id());
                    }
                }
            }
            "quota" => {
                let chart = if let Some(chart) = self.quota_chart {
                    context.update_component(chart, |view, _| {
                        *view = crate::runtime_shell::quota::trend(&settings.quota_daily, 960.0);
                    })?;
                    chart
                } else {
                    let chart = context.create_detached_component(
                        document_id,
                        crate::runtime_shell::quota::trend(&settings.quota_daily, 960.0),
                    )?;
                    self.quota_chart = Some(chart);
                    chart
                };
                order.push(chart.stable_id());
                for (key, slices) in [
                    ("project", &settings.quota_project_slices),
                    ("conversation", &settings.quota_conversation_slices),
                    ("tool", &settings.quota_tool_slices),
                ] {
                    let donut = if let Some(chart) = self.quota_donuts.get(key).copied() {
                        context.update_component(chart, |view, _| {
                            *view = quota_donut_chart(slices);
                        })?;
                        chart
                    } else {
                        let chart = context
                            .create_detached_component(document_id, quota_donut_chart(slices))?;
                        self.quota_donuts.insert(key.to_owned(), chart);
                        chart
                    };
                    order.push(donut.stable_id());
                }
            }
            "extensions" => {
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "extensions-search",
                    &settings.extensions_search,
                    |value| ShellIntent::ExtensionsSearchChanged(value),
                )?;
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "skill_id",
                    &settings.skill_id,
                    |value| ShellIntent::SkillIdChanged(value),
                )?;
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "skill_description",
                    &settings.skill_description,
                    |value| ShellIntent::SkillDescriptionChanged(value),
                )?;
                for skill in &settings.skills {
                    let id = format!("skill-{}", skill.id);
                    keep.insert(id.clone());
                    let label = if skill.enabled {
                        format!("关闭 {}", skill.label)
                    } else {
                        format!("开启 {}", skill.label)
                    };
                    let button = self.upsert_action(
                        context,
                        document_id,
                        &id,
                        extra_button(&label, ButtonKind::Subtle),
                        ShellIntent::ToggleSkill(skill.id.clone()),
                    )?;
                    order.push(button.stable_id());
                }
                for server in &settings.mcp_servers {
                    for (suffix, label, intent) in [
                        (
                            "edit",
                            format!("编辑 {}", server.label),
                            ShellIntent::EditMcpServer(server.id.clone()),
                        ),
                        (
                            "toggle",
                            if server.enabled {
                                format!("关闭 {}", server.label)
                            } else {
                                format!("开启 {}", server.label)
                            },
                            ShellIntent::ToggleMcpServer(server.id.clone()),
                        ),
                    ] {
                        let id = format!("mcp-{}-{}", server.id, suffix);
                        keep.insert(id.clone());
                        let button = self.upsert_action(
                            context,
                            document_id,
                            &id,
                            extra_button(&label, ButtonKind::Subtle),
                            intent,
                        )?;
                        order.push(button.stable_id());
                    }
                }
                if let Some(editor) = &settings.mcp_editor {
                    self.fields.upsert(
                        context,
                        document_id,
                        keep,
                        order,
                        "mcp_server_id",
                        &editor.server_id,
                        |value| ShellIntent::McpServerIdChanged(value),
                    )?;
                    self.fields.upsert(
                        context,
                        document_id,
                        keep,
                        order,
                        "mcp_location",
                        &editor.location,
                        |value| ShellIntent::McpLocationChanged(value),
                    )?;
                    self.fields.upsert(
                        context,
                        document_id,
                        keep,
                        order,
                        "mcp_args",
                        &editor.args,
                        |value| ShellIntent::McpArgsChanged(value),
                    )?;
                }
            }
            "remote" => {
                self.fields.upsert(
                    context,
                    document_id,
                    keep,
                    order,
                    "remote-name",
                    &settings.remote_pc_name,
                    |value| ShellIntent::RemoteNameChanged(value),
                )?;
                self.upsert_switch(
                    context,
                    document_id,
                    keep,
                    order,
                    "remote_host",
                    "远程主机",
                    settings.remote_host_enabled,
                    ShellIntent::ToggleRemoteHost,
                )?;
                self.upsert_switch(
                    context,
                    document_id,
                    keep,
                    order,
                    "remote_keep_awake",
                    "保持唤醒",
                    settings.remote_keep_awake,
                    ShellIntent::ToggleRemoteKeepAwake,
                )?;
                if !settings.remote_pairing_uri.is_empty() {
                    if let Ok(qr) = QrCode::encode(settings.remote_pairing_uri.as_bytes(), 220.0) {
                        let chart = if let Some(existing) = self.remote_qr {
                            context.update_component(existing, |view, _| {
                                *view = qr;
                            })?;
                            existing
                        } else {
                            let chart = context.create_detached_component(document_id, qr)?;
                            self.remote_qr = Some(chart);
                            chart
                        };
                        order.push(chart.stable_id());
                    }
                    order.push(self.remote_pair_hint.stable_id());
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn upsert_switch(
        &mut self,
        context: &mut AppContext,
        document_id: DocumentId,
        keep: &mut HashSet<String>,
        order: &mut Vec<StableNodeId>,
        id: &str,
        label: &str,
        checked: bool,
        intent: ShellIntent,
    ) -> Result<(), FrameworkError> {
        keep.insert(id.to_owned());
        let toggle = if let Some(toggle) = self.form_switches.get(id).copied() {
            context.update_component(toggle, |view, _| {
                *view = Switch::new(label, checked);
            })?;
            toggle
        } else {
            let toggle =
                context.create_detached_component(document_id, Switch::new(label, checked))?;
            let sink = Arc::clone(&self.sink);
            context.on(toggle, move |_, _event: &ToggleChanged, _| {
                emit(&sink, intent.clone());
            })?;
            self.form_switches.insert(id.to_owned(), toggle);
            toggle
        };
        order.push(toggle.stable_id());
        Ok(())
    }
}
fn product_action_button(label: &str, primary: bool) -> Button {
    Button::new(label)
        .kind(if primary {
            ButtonKind::Primary
        } else {
            ButtonKind::Subtle
        })
        .size(ControlSize::Medium)
}

struct SettingsAction {
    id: String,
    label: String,
    primary: bool,
    intent: ShellIntent,
}

fn quota_donut_chart(slices: &[(String, f64)]) -> DonutChart {
    let mut style = NodeStyle::default();
    let layout = std::sync::Arc::make_mut(&mut style.layout);
    layout.width = Some(LengthSpec::Px(160.0));
    layout.height = Some(LengthSpec::Px(160.0));
    layout.flex_shrink = Some(0.0);
    let total: f64 = slices.iter().map(|(_, value)| *value).sum();
    DonutChart::new(
        slices
            .iter()
            .zip(QUOTA_DONUT_COLORS)
            .map(|((_, value), color)| DonutSlice {
                value: *value,
                color,
            }),
    )
    .labels(slices.iter().map(|(name, _)| name.as_str()))
    .label(
        slices
            .iter()
            .map(|(name, value)| {
                format!(
                    "{name}: {value:.0} ({:.1}%)",
                    if total > 0.0 {
                        value / total * 100.0
                    } else {
                        0.0
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("；"),
    )
    .style(style)
}

const QUOTA_DONUT_COLORS: [SemanticColorRole; 5] = [
    SemanticColorRole::Accent,
    SemanticColorRole::Success,
    SemanticColorRole::Warning,
    SemanticColorRole::Text,
    SemanticColorRole::Muted,
];

fn clone_parent_label(path: &str) -> String {
    if path.trim().is_empty() {
        "未设置（使用家目录）".to_owned()
    } else {
        path.to_owned()
    }
}

fn worktree_parent_label(path: &str) -> String {
    if path.trim().is_empty() {
        "默认".to_owned()
    } else {
        path.to_owned()
    }
}

fn worktree_mode_dropdown(selected: &str) -> Dropdown {
    Dropdown::single(Some(selected.to_owned()))
        .size(ControlSize::Small)
        .options([
            DropdownOption::new("current", "当前工作区"),
            DropdownOption::new("create", "新建工作树"),
            DropdownOption::new("existing", "选择现有工作树"),
        ])
}

fn sidebar_mode_dropdown(selected: &str) -> Dropdown {
    Dropdown::single(Some(selected.to_owned()))
        .size(ControlSize::Small)
        .options([
            DropdownOption::new("grouped", "按项目分组"),
            DropdownOption::new("unified", "统一列表"),
        ])
}

fn settings_tab_copy(
    settings: &SettingsSnapshot,
) -> (String, String, Option<String>, bool, Vec<SettingsAction>) {
    match settings.state.active_tab().as_str() {
        "project" => (
            "项目".to_owned(),
            "保存当前项目名称和工作区路径。".to_owned(),
            settings.project_error.clone(),
            true,
            vec![
                SettingsAction {
                    id: "save-project".into(),
                    label: "保存项目".into(),
                    primary: true,
                    intent: ShellIntent::SaveProjectSettings,
                },
                SettingsAction {
                    id: "pick-workspace".into(),
                    label: "选择工作区".into(),
                    primary: false,
                    intent: ShellIntent::PickProjectWorkspace,
                },
                SettingsAction {
                    id: "project-clone-default-pick".into(),
                    label: "选择 Clone 父目录".into(),
                    primary: false,
                    intent: ShellIntent::PickCloneParent,
                },
                SettingsAction {
                    id: "worktree-parent".into(),
                    label: "选择工作树父目录".into(),
                    primary: false,
                    intent: ShellIntent::PickProjectWorktreeParent,
                },
            ],
        ),
        "provider" => (
            "模型服务".to_owned(),
            settings.provider_status.clone(),
            None,
            false,
            {
                let mut actions = vec![SettingsAction {
                    id: "refresh-provider".into(),
                    label: "刷新服务".into(),
                    primary: false,
                    intent: ShellIntent::RefreshProvider,
                }];
                if settings.can_save_credential {
                    actions.push(SettingsAction {
                        id: "save-credential".into(),
                        label: "保存凭据".into(),
                        primary: true,
                        intent: ShellIntent::SaveProviderCredential,
                    });
                }
                actions.push(SettingsAction {
                    id: "save-runtime".into(),
                    label: "保存运行配置".into(),
                    primary: false,
                    intent: ShellIntent::SaveProviderRuntimeSettings,
                });
                actions.push(SettingsAction {
                    id: "reset-runtime".into(),
                    label: "恢复默认配置".into(),
                    primary: false,
                    intent: ShellIntent::ResetProviderRuntimeSettings,
                });
                actions
            },
        ),
        "agent" => ("Agent".to_owned(), String::new(), None, false, {
            let mut actions: Vec<_> = settings
                .agent_actions
                .iter()
                .map(|action| SettingsAction {
                    id: action.id.clone(),
                    label: action.label.clone(),
                    primary: false,
                    intent: ShellIntent::ToggleAgent(action.id.clone()),
                })
                .collect();
            actions.push(SettingsAction {
                id: "new-agent".into(),
                label: "新建 Agent".into(),
                primary: true,
                intent: ShellIntent::NewCustomAgent,
            });
            if settings.custom_agent_editor_open {
                actions.push(SettingsAction {
                    id: "save-agent".into(),
                    label: "保存 Agent".into(),
                    primary: true,
                    intent: ShellIntent::SaveCustomAgent,
                });
                actions.push(SettingsAction {
                    id: "cancel-agent".into(),
                    label: "取消编辑".into(),
                    primary: false,
                    intent: ShellIntent::CancelCustomAgentEdit,
                });
            }
            actions
        }),
        "quota" => (
            "用量与额度".to_owned(),
            settings.quota_status.clone(),
            None,
            false,
            vec![
                SettingsAction {
                    id: "refresh-quota".into(),
                    label: "刷新用量".into(),
                    primary: false,
                    intent: ShellIntent::RefreshQuota,
                },
                SettingsAction {
                    id: "cycle-quota-days".into(),
                    label: settings.quota_days_label.clone(),
                    primary: false,
                    intent: ShellIntent::CycleQuotaDays,
                },
                SettingsAction {
                    id: "cycle-quota-backend".into(),
                    label: settings.quota_backend_label.clone(),
                    primary: false,
                    intent: ShellIntent::CycleQuotaBackend,
                },
            ],
        ),
        "extensions" => (
            "扩展".to_owned(),
            settings.extensions_status.clone(),
            None,
            false,
            {
                let mut actions = vec![SettingsAction {
                    id: "refresh-extensions".into(),
                    label: "刷新扩展".into(),
                    primary: false,
                    intent: ShellIntent::RefreshExtensions,
                }];
                if settings.can_create_skill {
                    actions.push(SettingsAction {
                        id: "create-skill".into(),
                        label: "创建技能".into(),
                        primary: true,
                        intent: ShellIntent::CreateSkill,
                    });
                }
                actions.push(SettingsAction {
                    id: "new-mcp".into(),
                    label: "新建 MCP".into(),
                    primary: false,
                    intent: ShellIntent::NewMcpServer,
                });
                if let Some(editor) = &settings.mcp_editor {
                    actions.push(SettingsAction {
                        id: "cycle-mcp-transport".into(),
                        label: format!("传输：{}", editor.transport),
                        primary: false,
                        intent: ShellIntent::CycleMcpTransport,
                    });
                    actions.push(SettingsAction {
                        id: "toggle-mcp-enabled".into(),
                        label: if editor.enabled {
                            "MCP：开".into()
                        } else {
                            "MCP：关".into()
                        },
                        primary: false,
                        intent: ShellIntent::ToggleMcpEditorEnabled,
                    });
                    actions.push(SettingsAction {
                        id: "save-mcp".into(),
                        label: "保存 MCP".into(),
                        primary: true,
                        intent: ShellIntent::SaveMcpServer,
                    });
                    actions.push(SettingsAction {
                        id: "cancel-mcp".into(),
                        label: "取消编辑".into(),
                        primary: false,
                        intent: ShellIntent::CancelMcpEditor,
                    });
                }
                actions
            },
        ),
        "remote" => (
            "远程控制".to_owned(),
            settings.remote_status.clone(),
            None,
            false,
            {
                let mut actions = vec![
                    SettingsAction {
                        id: "remote-name-save".into(),
                        label: "保存名称".into(),
                        primary: true,
                        intent: ShellIntent::SaveRemoteName,
                    },
                    SettingsAction {
                        id: "remote-refresh".into(),
                        label: "刷新状态".into(),
                        primary: false,
                        intent: ShellIntent::RefreshRemote,
                    },
                ];
                if settings.remote_pairing_active {
                    actions.push(SettingsAction {
                        id: "remote-copy".into(),
                        label: "复制配对链接".into(),
                        primary: false,
                        intent: ShellIntent::CopyPairingUri,
                    });
                    actions.push(SettingsAction {
                        id: "remote-cancel".into(),
                        label: "取消配对".into(),
                        primary: false,
                        intent: ShellIntent::CancelRemotePairing,
                    });
                } else if settings.remote_host_enabled {
                    actions.push(SettingsAction {
                        id: "remote-pair".into(),
                        label: "生成配对二维码".into(),
                        primary: false,
                        intent: ShellIntent::StartRemotePairing,
                    });
                }
                for (id, name) in &settings.remote_devices {
                    actions.push(SettingsAction {
                        id: format!("remote-revoke-{id}"),
                        label: format!("撤销 {name}"),
                        primary: false,
                        intent: ShellIntent::RevokeRemoteDevice(id.clone()),
                    });
                }
                actions
            },
        ),
        "desktop" => (
            "桌面".to_owned(),
            {
                let binding_state = match settings.github_state.as_str() {
                    "bound" => "已绑定",
                    "unbound" => "未绑定",
                    state => state,
                };
                let github = if settings.github_login.is_empty() {
                    format!("GitHub：{}", binding_state)
                } else {
                    format!("GitHub：{} · {}", binding_state, settings.github_login)
                };
                let shortcut = if settings.shortcut_capturing {
                    "快捷键：正在录制，按下组合键".to_owned()
                } else if settings.shortcut.is_empty() {
                    "快捷键：未设置".to_owned()
                } else {
                    format!(
                        "快捷键：{}{}",
                        settings.shortcut,
                        if settings.shortcut_registered {
                            "（已注册）"
                        } else {
                            ""
                        }
                    )
                };
                format!("{}\n{}\n{}", settings.desktop_status, github, shortcut)
            },
            None,
            false,
            {
                let mut actions = vec![SettingsAction {
                    id: "check-update".into(),
                    label: "检查更新".into(),
                    primary: false,
                    intent: ShellIntent::CheckForUpdate,
                }];
                if settings.github_busy {
                    actions.push(SettingsAction {
                        id: "cancel-github".into(),
                        label: "取消绑定".into(),
                        primary: false,
                        intent: ShellIntent::CancelGitHubBinding,
                    });
                } else if settings.github_can_bind {
                    actions.push(SettingsAction {
                        id: "bind-github".into(),
                        label: "绑定 GitHub".into(),
                        primary: true,
                        intent: ShellIntent::StartGitHubBinding,
                    });
                }
                actions.push(SettingsAction {
                    id: "record-shortcut".into(),
                    label: if settings.shortcut_capturing {
                        "录制中".into()
                    } else {
                        "录制快捷键".into()
                    },
                    primary: false,
                    intent: ShellIntent::BeginShortcutCapture,
                });
                actions.push(SettingsAction {
                    id: "save-shortcut".into(),
                    label: "保存快捷键".into(),
                    primary: false,
                    intent: ShellIntent::SaveShortcut,
                });
                actions.push(SettingsAction {
                    id: "clear-shortcut".into(),
                    label: "清空快捷键".into(),
                    primary: false,
                    intent: ShellIntent::ClearShortcut,
                });
                actions
            },
        ),
        "data" => (
            "数据迁移".to_owned(),
            settings.data_status.clone(),
            None,
            false,
            vec![
                SettingsAction {
                    id: "pick-import".into(),
                    label: "选择导入目录".into(),
                    primary: false,
                    intent: ShellIntent::PickDataImportSource,
                },
                SettingsAction {
                    id: "execute-import".into(),
                    label: "开始导入".into(),
                    primary: true,
                    intent: ShellIntent::ExecuteDataImport,
                },
                SettingsAction {
                    id: "reset-import".into(),
                    label: "重置".into(),
                    primary: false,
                    intent: ShellIntent::ResetDataImport,
                },
            ]
            .into_iter()
            .filter(|action| action.id != "execute-import" || settings.data_can_import)
            .collect(),
        ),
        _ => (String::new(), String::new(), None, false, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::{SettingsTab, ThemeMode};

    fn settings(tab: &str) -> SettingsSnapshot {
        let mut settings = crate::runtime_shell::empty_snapshot().settings;
        settings.model = SettingsModel::new(tab, [SettingsTab::new(tab, tab)]).unwrap();
        settings.state = SettingsState::new(&settings.model);
        settings
    }

    #[test]
    fn extensions_tab_exposes_origin_entry_ids() {
        use crate::module::extensions::ExtensionEntry;
        use crate::runtime_extensions::ExtensionBrowserSnapshot;
        let mut context = AppContext::new();
        let document = DocumentId::new(328).unwrap();
        let mut snapshot = settings("extensions");
        snapshot.extensions = Some(ExtensionBrowserSnapshot {
            tab: "extensions".into(),
            query: String::new(),
            entries: vec![ExtensionEntry {
                key: "skill:user:native-debug-skill".into(),
                label: "native-debug-skill".into(),
                meta: "用户技能".into(),
                enabled: true,
            }],
            selected: Some("skill:user:native-debug-skill".into()),
            title: "native-debug-skill · 用户技能".into(),
            toolbar: vec![],
            detail: vec![],
            detail_actions: vec![],
            editor: None,
        });
        let mut view = SettingsView::mount(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            Arc::new(|_| {}),
        )
        .unwrap();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        let ids: Vec<_> = view
            .extensions
            .debug_nodes()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(
            ids.iter()
                .any(|id| id == "extensions-entry-skill:user:native-debug-skill")
        );
        assert!(ids.iter().any(|id| id == "extensions-search"));
    }

    #[test]
    fn appearance_sidebar_choice_dispatches_the_display_mode() {
        let mut context = AppContext::new();
        let document = DocumentId::new(324).unwrap();
        let snapshot = settings("appearance");
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let view = SettingsView::mount(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        context
            .update_component(view.sidebar_mode, |_, cx| {
                cx.emit(DropdownEvent::<Arc<str>>::Select(Arc::from("unified")))
            })
            .unwrap();
        assert!(matches!(
            received.lock().unwrap().last(),
            Some(ShellIntent::SetSidebarDisplayMode(mode)) if mode == "unified"
        ));
    }

    #[test]
    fn leaving_appearance_closes_the_sidebar_mode_dropdown() {
        let mut context = AppContext::new();
        let document = DocumentId::new(325).unwrap();
        let appearance = settings("appearance");
        let mut view = SettingsView::mount(
            &mut context,
            document,
            &appearance,
            ThemeMode::Light,
            Arc::new(|_| {}),
        )
        .unwrap();
        view.sync(
            &mut context,
            document,
            &appearance,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        context
            .update_component(view.sidebar_mode, |field, _| {
                field.opened = true;
            })
            .unwrap();
        let project = settings("project");
        view.sync(
            &mut context,
            document,
            &project,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        assert!(
            !context
                .read(view.sidebar_mode, |field| field.opened)
                .unwrap()
        );
    }

    #[test]
    fn project_tab_exposes_worktree_and_clone_controls() {
        let mut context = AppContext::new();
        let document = DocumentId::new(326).unwrap();
        let mut snapshot = settings("project");
        snapshot.project_worktree_mode = "create".into();
        snapshot.project_worktree_instructions = "run review".into();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let mut view = SettingsView::mount(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        assert!(
            view.product_actions
                .contains_key("action:project-clone-default-pick")
        );
        assert!(view.product_actions.contains_key("action:worktree-parent"));
        assert!(view.form_switches.contains_key("worktree-cleanup"));
        assert!(view.fields.editors.contains_key("worktree-instructions"));
        context
            .update_component(view.worktree_mode, |_, cx| {
                cx.emit(DropdownEvent::<Arc<str>>::Select(Arc::from("existing")))
            })
            .unwrap();
        assert!(matches!(
            received.lock().unwrap().last(),
            Some(ShellIntent::SetProjectWorktreeMode(mode)) if mode == "existing"
        ));
    }

    #[test]
    fn remote_pairing_ticket_shows_qr_and_copy_action() {
        let mut context = AppContext::new();
        let document = DocumentId::new(327).unwrap();
        let mut snapshot = settings("remote");
        snapshot.remote_host_enabled = true;
        snapshot.remote_pairing_active = true;
        snapshot.remote_pairing_uri = "lilia://pair/native-debug-ticket".into();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let mut view = SettingsView::mount(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        let qr = view.remote_qr.expect("pairing QR");
        assert!(context.world().node(qr.stable_id()).is_some());
        let copy = view.product_actions["action:remote-copy"];
        context
            .update_component(copy, |_, cx| cx.emit(Activate))
            .unwrap();
        assert!(matches!(
            received.lock().unwrap().last(),
            Some(ShellIntent::CopyPairingUri)
        ));
        snapshot.remote_pairing_active = false;
        snapshot.remote_pairing_uri.clear();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        assert!(view.remote_qr.is_none());
        assert!(context.world().node(qr.stable_id()).is_none());
        assert!(!view.product_actions.contains_key("action:remote-copy"));
    }

    #[test]
    fn credential_refresh_updates_the_existing_buttons_revision_and_removal_destroys_it() {
        let mut context = AppContext::new();
        let document = DocumentId::new(321).unwrap();
        let mut snapshot = settings("provider");
        snapshot.credentials.push(CredentialRow {
            id: "credential".into(),
            revision: 4,
            label: "personal".into(),
        });
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let mut view = SettingsView::mount(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        let button = view.product_actions["revoke-credential"];
        snapshot.credentials[0].revision = 8;
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        assert_eq!(view.product_actions["revoke-credential"], button);
        context
            .update_component(button, |_, cx| cx.emit(Activate))
            .unwrap();
        assert!(
            matches!(&received.lock().unwrap()[0], ShellIntent::RevokeProviderCredential { credential_id, revision: 8 } if credential_id == "credential")
        );
        snapshot.credentials.clear();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        assert!(context.world().node(button.stable_id()).is_none());
        assert!(!view.action_bindings.contains_key("revoke-credential"));
    }

    #[test]
    fn changing_tabs_releases_fields_without_touching_another_views_draft() {
        let mut context = AppContext::new();
        let mut first = settings("provider");
        first.provider_secret = "first".into();
        first.providers.push(ProviderRow {
            id: "provider_secret".into(),
            label: "Service".into(),
            selected: true,
        });
        let mut second = first.clone();
        second.provider_secret = "second".into();
        let document_a = DocumentId::new(322).unwrap();
        let document_b = DocumentId::new(323).unwrap();
        let mut a = SettingsView::mount(
            &mut context,
            document_a,
            &first,
            ThemeMode::Light,
            Arc::new(|_| {}),
        )
        .unwrap();
        let mut b = SettingsView::mount(
            &mut context,
            document_b,
            &second,
            ThemeMode::Light,
            Arc::new(|_| {}),
        )
        .unwrap();
        a.sync(
            &mut context,
            document_a,
            &first,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        b.sync(
            &mut context,
            document_b,
            &second,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        let old_field = a.fields.editors["provider_secret"];
        let retained_field = b.fields.editors["provider_secret"];
        first = settings("remote");
        a.sync(
            &mut context,
            document_a,
            &first,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        assert!(context.world().node(old_field.stable_id()).is_none());
        assert!(!context.read(a.provider, |field| field.opened).unwrap());
        assert!(!a.fields.editors.contains_key("provider_secret"));
        assert!(a.fields.editors.contains_key("remote-name"));
        assert_eq!(retained_field.value(&context), "second");
        assert!(a.form_switches.contains_key("remote_host"));
    }

    #[test]
    fn provider_search_survives_refresh_and_selection_routes_the_selected_service() {
        let mut context = AppContext::new();
        let document = DocumentId::new(325).unwrap();
        let mut snapshot = settings("provider");
        snapshot.providers = vec![
            ProviderRow {
                id: "one".into(),
                label: "First service".into(),
                selected: true,
            },
            ProviderRow {
                id: "two".into(),
                label: "Second service".into(),
                selected: false,
            },
        ];
        snapshot.provider_secret = "private input".into();
        let received = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::clone(&received);
        let mut view = SettingsView::mount(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            Arc::new(move |event| events.lock().unwrap().push(event)),
        )
        .unwrap();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        context
            .update_component(view.provider, |field, _| {
                field.toggle_open();
                field.set_query("Second");
            })
            .unwrap();
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            true,
            960.0,
        )
        .unwrap();
        assert_eq!(
            context
                .read(view.provider, |field| field.visible_indices())
                .unwrap(),
            vec![1]
        );
        context
            .update_component(view.provider, |field, cx| {
                if let Some(event) = field
                    .highlighted
                    .and_then(|index| field.select_index(index))
                {
                    cx.emit(event);
                }
            })
            .unwrap();
        assert!(
            matches!(&received.lock().unwrap()[0], ShellIntent::SelectProvider(id) if id == "two")
        );
        let crate::form_view::ProductField::Single(secret) = view.fields.editors["provider_secret"]
        else {
            panic!("credential requires a secure single-line input")
        };
        assert!(context.read(secret, |field| field.secure).unwrap());
        view.sync(
            &mut context,
            document,
            &snapshot,
            ThemeMode::Light,
            false,
            960.0,
        )
        .unwrap();
        assert!(!context.read(view.provider, |field| field.opened).unwrap());
    }
}
