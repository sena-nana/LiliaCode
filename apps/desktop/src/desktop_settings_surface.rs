use super::*;
use crate::runtime_shell::ShellIntent as Intent;
use crate::runtime_surface::SurfaceControl as Control;

#[derive(Clone, Debug)]
pub(crate) enum ProjectPreferenceChange {
    Mode(String),
    Cleanup(bool),
    Instructions(String),
}

fn save_project_preference_change(
    application: &DesktopApplication,
    current: &DesktopProjectSettings,
    change: ProjectPreferenceChange,
) -> Result<DesktopProjectSettings, crate::application::DesktopApplicationError> {
    let mut settings = current.clone();
    match change {
        ProjectPreferenceChange::Mode(mode) => {
            settings.worktree.default_mode = match mode.as_str() {
                "create" => DesktopWorktreeSelectionMode::Create,
                "existing" => DesktopWorktreeSelectionMode::Existing,
                _ => DesktopWorktreeSelectionMode::Current,
            }
        }
        ProjectPreferenceChange::Cleanup(enabled) => settings.worktree.cleanup_on_archive = enabled,
        ProjectPreferenceChange::Instructions(value) => settings.worktree.auto_instructions = value,
    }
    application.save_project_settings(settings)
}

#[derive(Clone, Debug)]
pub(crate) enum SettingsSurfaceAction {
    SidebarDisplayMode(NativeSidebarDisplayMode),
    SaveAssistant,
    ClearAssistantSecret,
    FetchModels,
    TestAssistant,
    SaveModels,
    PresetEffortValue { id: String, value: String },
    RemovePreset(String),
    Suggestions(bool),
    ProjectPreference(ProjectPreferenceChange),
    OpenPath(String),
    RemoteName(String),
    RefreshRemote,
    ZoomAutomation(f32),
    FitAutomation,
    SelectRunNode(String),
    AutomationInspector(String),
    PickCloneParent,
    PickWorktreeParent,
    GitHubCopy,
    GitHubOpen,
    GitHubUnbind,
    AgentToggle(AgentInteractionToggle),
    DeleteAutomation,
    RevokeRemote(String),
    AgentPrompt(String),
    AgentMode(String),
    AgentPermission(String),
    AgentForwardText,
    AgentProgress,
    SaveAgent,
}

impl DesktopProgram {
    pub(super) fn apply_automation_inspector_draft(&mut self) {
        let Some(id) = self.selected_automation.as_deref() else {
            return;
        };
        let Some(workflow) = self
            .automations
            .iter_mut()
            .find(|workflow| workflow.id == id)
        else {
            return;
        };
        if update_inspected_node(workflow, &self.automation_node_inspector) {
            self.automation_dirty.insert(workflow.id.clone());
            self.rebuild_automation_graph(false);
        }
    }

    pub(super) fn merge_automation_refresh(&mut self, mut incoming: Vec<AutomationWorkflow>) {
        for workflow in &mut incoming {
            if self.automation_dirty.contains(&workflow.id) {
                if let Some(local) = self
                    .automations
                    .iter()
                    .find(|local| local.id == workflow.id)
                {
                    workflow.name = local.name.clone();
                    workflow.scope = local.scope.clone();
                    workflow.draft = local.draft.clone();
                }
            }
        }
        self.automation_dirty
            .retain(|id| incoming.iter().any(|workflow| workflow.id == *id));
        self.automations = incoming;
    }

    pub(super) fn apply_settings_surface_action(&mut self, action: SettingsSurfaceAction) {
        use SettingsSurfaceAction::*;
        match action {
            SidebarDisplayMode(mode) => {
                self.apply_settings_message(SettingsMessage::SetSidebarDisplayMode(mode));
            }
            DeleteAutomation | RevokeRemote(_) => self.settings_confirmation = Some(action),
            SaveAssistant => self.save_assistant_ai_configuration(),
            ClearAssistantSecret => self.clear_assistant_ai_secret(),
            FetchModels => self.start_assistant_ai_models_fetch(),
            TestAssistant => self.start_assistant_ai_connection_test(),
            SaveModels => self.save_model_feature_settings(),
            PresetEffortValue { id, value } => {
                self.provider_ai_settings.set_preset_effort(&id, &value)
            }
            OpenPath(path) => {
                if let Err(error) = self
                    .kernel
                    .session()
                    .execute_host(DesktopHostAction::OpenPath(PathBuf::from(path)))
                {
                    self.provider_error = Some(error.to_string());
                }
            }
            ProjectPreference(change) => {
                if let ProjectPreferenceChange::Instructions(value) = &change {
                    self.project_worktree_instructions.set_text(value);
                }
                match save_project_preference_change(
                    self.kernel.session(),
                    &self.project_settings,
                    change,
                ) {
                    Ok(saved) => {
                        self.project_settings = saved;
                        self.project_settings_error = None;
                    }
                    Err(error) => {
                        eprintln!("failed to save project preference: {error}");
                        self.project_settings_error = Some("无法保存项目偏好，请重试。".into());
                    }
                }
            }
            RemovePreset(id) => self.provider_ai_settings.remove_custom_preset(&id),
            Suggestions(enabled) => self.set_conversation_suggestions_enabled(enabled),
            RemoteName(value) => self.remote_pc_name = value,
            RefreshRemote => self.start_remote_operation(RemoteRequest::Refresh),
            ZoomAutomation(factor) => self.adjust_automation_view(Some(factor)),
            FitAutomation => self.adjust_automation_view(None),
            SelectRunNode(id) => self.automation_run_node = Some(id),
            AutomationInspector(panel) => {
                if matches!(panel.as_str(), "node" | "workflow" | "scope" | "runs") {
                    self.automation_inspector_panel = panel;
                }
            }
            PickCloneParent => self.pick_project_clone_parent(),
            PickWorktreeParent => self.pick_project_worktree_parent(),
            GitHubCopy => self.copy_github_user_code(),
            GitHubOpen => self.open_github_verification(),
            GitHubUnbind => self.unbind_github(),
            AgentToggle(toggle) => self.toggle_agent_interaction_setting(toggle),
            AgentPrompt(value) => self.agent_interaction_settings.main_agent_custom_prompt = value,
            AgentMode(mode) => {
                self.agent_interaction_settings.main_agent_prompt_mode = mode;
                self.save_agent_surface_settings();
            }
            AgentPermission(mode) => {
                let enabled = self
                    .agent_interaction_settings
                    .permission_mode_availability
                    .entry(mode)
                    .or_insert(false);
                *enabled = !*enabled;
                self.save_agent_surface_settings();
            }
            AgentForwardText => {
                self.agent_interaction_settings
                    .subagent_mode
                    .forward_subagent_text = !self
                    .agent_interaction_settings
                    .subagent_mode
                    .forward_subagent_text;
                self.save_agent_surface_settings();
            }
            AgentProgress => {
                self.agent_interaction_settings
                    .subagent_mode
                    .agent_progress_summaries = !self
                    .agent_interaction_settings
                    .subagent_mode
                    .agent_progress_summaries;
                self.save_agent_surface_settings();
            }
            SaveAgent => self.save_agent_surface_settings(),
        }
    }

    pub(super) fn settings_surface_controls(&self) -> Vec<Control> {
        use SettingsSurfaceAction as Action;
        let mut rows = Vec::new();
        let tab = self.settings_state.active_tab().as_str();
        if !matches!(
            tab,
            "appearance"
                | "assistant"
                | "model-config"
                | "agent"
                | "remote"
                | "preferences"
                | "plugin-packages"
                | "plugin-hooks"
                | "plugin-mcp"
                | "quota"
                | "credentials"
                | "extensions"
        ) {
            return rows;
        }
        if tab == "appearance" {
            return appearance_controls(
                self.sidebar_display_mode,
                self.agent_interaction_settings.non_interrupt_mode,
                self.agent_interaction_settings.debug,
                self.agent_interaction_error.as_deref(),
            );
        }
        rows.push(Control::section(
            format!("{tab}-section"),
            match tab {
                "assistant" => "连接配置",
                "model-config" => "模型预设",
                "agent" => "权限与策略",
                "remote" => "设备配对",
                "preferences" => "项目偏好",
                "plugin-packages" => "插件管理",
                "plugin-hooks" => "Hooks",
                "plugin-mcp" => "MCP 服务",
                "quota" => "统计摘要",
                "credentials" => "凭据",
                _ => "详情",
            },
        ));
        match tab {
            "credentials" => {
                rows.push(
                    Control::secret(
                        "provider-secret",
                        "API 密钥",
                        &self.provider_secret,
                        |v| Intent::ProviderCommand(ProviderMessage::SecretChanged(v)),
                    )
                    .binding_identity(self.selected_provider.clone().unwrap_or_default()),
                );
                rows.push(
                    Control::action(
                        "provider-credential-save",
                        "保存凭据",
                        Intent::ProviderCommand(ProviderMessage::SaveCredential),
                    )
                    .enabled(!self.provider_busy() && !self.provider_secret.trim().is_empty()),
                );
                for credential in &self.provider.credentials {
                    if self.selected_provider.as_deref() != Some(credential.provider_id.as_str()) {
                        continue;
                    }
                    rows.push(
                        Control::action(
                            format!("credential-revoke-{}", credential.credential_id),
                            "撤销凭据",
                            Intent::ProviderCommand(ProviderMessage::RevokeCredential {
                                credential_id: credential.credential_id.clone(),
                                revision: credential.revision,
                            }),
                        )
                        .danger(),
                    );
                }
            }
            "assistant" => {
                let state = &self.provider_ai_settings;
                rows.push(Control::field(
                    "assistant-url",
                    "基础 URL",
                    &state.assistant_base_url,
                    false,
                    |v| Intent::ProviderCommand(ProviderMessage::AssistantBaseUrlChanged(v)),
                ));
                rows.push(Control::secret(
                    "assistant-secret",
                    "API 密钥",
                    &state.assistant_secret,
                    |v| Intent::ProviderCommand(ProviderMessage::AssistantSecretChanged(v)),
                ));
                rows.push(Control::field(
                    "assistant-model",
                    "默认模型",
                    &state.assistant_model,
                    false,
                    |v| Intent::ProviderCommand(ProviderMessage::AssistantModelChanged(v)),
                ));
                rows.push(Control::section("assistant-pool", "模型池"));
                for model in state.assistant_model_pool() {
                    let id = model.id.clone();
                    rows.push(Control::field(
                        format!("assistant-model-{}", id),
                        id.clone(),
                        &model.label,
                        false,
                        move |value| {
                            Intent::ProviderCommand(ProviderMessage::RenameAssistantModel {
                                model_id: id.clone(),
                                value,
                            })
                        },
                    ));
                }
                rows.push(Control::field(
                    "assistant-new-id",
                    "模型 ID",
                    &self.assistant_ai_new_model_id,
                    false,
                    |v| Intent::ProviderCommand(ProviderMessage::AssistantNewModelIdChanged(v)),
                ));
                rows.push(Control::field(
                    "assistant-new-label",
                    "模型名称",
                    &self.assistant_ai_new_model_label,
                    false,
                    |v| Intent::ProviderCommand(ProviderMessage::AssistantNewModelLabelChanged(v)),
                ));
                rows.push(
                    Control::action(
                        "assistant-add",
                        "添加模型",
                        Intent::ProviderCommand(ProviderMessage::AddAssistantModel),
                    )
                    .enabled(!self.assistant_ai_new_model_id.trim().is_empty()),
                );
                for (id, label, action) in [
                    ("assistant-fetch", "获取模型", Action::FetchModels),
                    ("assistant-test", "测试连接", Action::TestAssistant),
                    ("assistant-save", "保存配置", Action::SaveAssistant),
                ] {
                    rows.push(
                        Control::action(id, label, Intent::SettingsCommand(action))
                            .enabled(!self.provider_busy() && !self.assistant_ai_probe_busy()),
                    );
                }
                if state.assistant_secret_configured {
                    rows.push(
                        Control::action(
                            "assistant-clear",
                            "清除已保存密钥",
                            Intent::SettingsCommand(Action::ClearAssistantSecret),
                        )
                        .danger(),
                    );
                }
                if let Some((_, message)) = &self.assistant_ai_probe_notice {
                    rows.push(Control::text("assistant-notice", message));
                }
                if let Some(error) = &self.provider_error {
                    rows.push(Control::text("assistant-error", error));
                }
            }
            "model-config" => {
                let state = &self.provider_ai_settings;
                for preset in state.builtin_presets().chain(state.custom_presets()) {
                    let id = preset.id.clone();
                    rows.push(Control::choice(
                        format!("preset-model-{id}"),
                        &preset.label,
                        preset.model.clone().unwrap_or_default(),
                        state.composer_model_options(None),
                        move |value| {
                            Intent::ProviderCommand(ProviderMessage::FeaturePresetModelChanged {
                                preset_id: id.clone(),
                                value,
                            })
                        },
                    ));
                    let id = preset.id.clone();
                    rows.push(Control::choice(
                        format!("preset-effort-{id}"),
                        "推理强度",
                        preset.reasoning_effort.clone().unwrap_or_default(),
                        options(&[
                            ("", "默认"),
                            ("low", "低"),
                            ("medium", "中"),
                            ("high", "高"),
                            ("xhigh", "更高"),
                            ("max", "最高"),
                        ]),
                        move |value| {
                            Intent::SettingsCommand(Action::PresetEffortValue {
                                id: id.clone(),
                                value,
                            })
                        },
                    ));
                    if preset.kind == "custom" {
                        let id = preset.id.clone();
                        rows.push(Control::field(
                            format!("preset-label-{id}"),
                            "预设名称",
                            &preset.label,
                            false,
                            move |value| {
                                Intent::ProviderCommand(ProviderMessage::RenameCustomPreset {
                                    preset_id: id.clone(),
                                    value,
                                })
                            },
                        ));
                        rows.push(
                            Control::action(
                                format!("preset-remove-{}", preset.id),
                                "删除预设",
                                Intent::SettingsCommand(Action::RemovePreset(preset.id.clone())),
                            )
                            .danger(),
                        );
                    }
                }
                rows.push(Control::field(
                    "preset-new",
                    "新预设名称",
                    &self.custom_preset_draft,
                    false,
                    |v| Intent::ProviderCommand(ProviderMessage::CustomPresetDraftChanged(v)),
                ));
                rows.push(
                    Control::action(
                        "preset-add",
                        "添加预设",
                        Intent::ProviderCommand(ProviderMessage::AddCustomPreset),
                    )
                    .enabled(!self.custom_preset_draft.trim().is_empty()),
                );
                rows.push(Control::section("feature-models", "功能模型"));
                for (id, label, value, constructor) in [
                    (
                        "feature-title",
                        "标题生成",
                        &state.title_model,
                        ProviderMessage::TitleModelChanged as fn(String) -> ProviderMessage,
                    ),
                    (
                        "feature-suggestion",
                        "对话建议",
                        &state.suggestion_model,
                        ProviderMessage::SuggestionModelChanged,
                    ),
                    (
                        "feature-router",
                        "提示词路由",
                        &state.prompt_router_model,
                        ProviderMessage::PromptRouterModelChanged,
                    ),
                    (
                        "feature-optimize",
                        "提示词优化",
                        &state.prompt_optimize_model,
                        ProviderMessage::PromptOptimizeModelChanged,
                    ),
                    (
                        "feature-auto",
                        "自动轮次决策",
                        &state.auto_turn_decision_model,
                        ProviderMessage::AutoTurnDecisionModelChanged,
                    ),
                ] {
                    rows.push(Control::choice(
                        id,
                        label,
                        value,
                        state.composer_model_options(None),
                        move |v| Intent::ProviderCommand(constructor(v)),
                    ));
                }
                rows.push(Control::toggle(
                    "suggestions-toggle",
                    "对话建议",
                    state.conversation_suggestions_enabled(),
                    Intent::SettingsCommand(Action::Suggestions(
                        !state.conversation_suggestions_enabled(),
                    )),
                ));
                rows.push(
                    Control::action(
                        "models-save",
                        "保存模型配置",
                        Intent::SettingsCommand(Action::SaveModels),
                    )
                    .enabled(!self.provider_busy()),
                );
                if let Some(error) = &self.provider_error {
                    rows.push(Control::text("models-error", error));
                }
            }
            "agent" => {
                let settings = &self.agent_interaction_settings;
                rows.push(Control::text("agent-permissions", "权限选项"));
                for (mode, enabled) in &settings.permission_mode_availability {
                    rows.push(Control::toggle(
                        format!("agent-permission-{mode}"),
                        mode,
                        *enabled,
                        Intent::SettingsCommand(Action::AgentPermission(mode.clone())),
                    ));
                }
                rows.push(Control::text("agent-strategy", "主 Agent 策略"));
                rows.push(Control::choice(
                    "agent-strategy-mode",
                    "策略模式",
                    &settings.main_agent_prompt_mode,
                    options(&[
                        ("conservative", "保守"),
                        ("aggressive", "积极"),
                        ("custom", "自定义"),
                    ]),
                    |v| Intent::SettingsCommand(Action::AgentMode(v)),
                ));
                rows.push(Control::field(
                    "agent-custom-prompt",
                    "自定义工作流提示词",
                    &settings.main_agent_custom_prompt,
                    true,
                    |v| Intent::SettingsCommand(Action::AgentPrompt(v)),
                ));
                rows.push(Control::action(
                    "agent-prompt-save",
                    "保存提示词",
                    Intent::SettingsCommand(Action::SaveAgent),
                ));
                rows.push(Control::section("agent-auto-options", "自动轮次策略"));
                let auto = &settings.auto_turn_decision;
                for (id, label, enabled, toggle) in [
                    (
                        "model",
                        "自动选择模型",
                        auto.allow_model_tier,
                        AgentInteractionToggle::AutoModelTier,
                    ),
                    (
                        "effort",
                        "自动选择推理强度",
                        auto.allow_reasoning_effort,
                        AgentInteractionToggle::AutoReasoningEffort,
                    ),
                    (
                        "plan",
                        "自动选择计划模式",
                        auto.allow_plan_mode,
                        AgentInteractionToggle::AutoPlanMode,
                    ),
                    (
                        "goal",
                        "自动选择目标模式",
                        auto.allow_goal_mode,
                        AgentInteractionToggle::AutoGoalMode,
                    ),
                    (
                        "fork",
                        "自动分支会话",
                        auto.allow_session_fork,
                        AgentInteractionToggle::AutoSessionFork,
                    ),
                ] {
                    rows.push(Control::toggle(
                        format!("agent-auto-{id}"),
                        label,
                        enabled,
                        Intent::SettingsCommand(Action::AgentToggle(toggle)),
                    ));
                }
                rows.push(Control::toggle(
                    "agent-forward",
                    "转发子代理文本",
                    settings.subagent_mode.forward_subagent_text,
                    Intent::SettingsCommand(Action::AgentForwardText),
                ));
                rows.push(Control::toggle(
                    "agent-progress",
                    "子代理进度摘要",
                    settings.subagent_mode.agent_progress_summaries,
                    Intent::SettingsCommand(Action::AgentProgress),
                ));
                if let Some(error) = &self.agent_interaction_error {
                    rows.push(Control::text("agent-error", error));
                }
            }
            "quota" => {
                if let Some(stats) = &self.quota_usage {
                    rows.push(Control::text(
                        "quota-totals",
                        format!(
                            "总量 {} · 输入 {} · 输出 {}\n缓存读取 {} · 缓存写入 {}",
                            stats.totals.total_tokens,
                            stats.totals.input_tokens,
                            stats.totals.output_tokens,
                            stats.totals.cache_read_tokens,
                            stats.totals.cache_creation_tokens
                        ),
                    ));
                    rows.push(Control::text(
                        "quota-cost",
                        format!(
                            "已知费用 {} · 费用覆盖 {}/{} 条记录",
                            stats
                                .cost
                                .known_cost_usd
                                .map(|v| format!("${v:.4}"))
                                .unwrap_or_else(|| "未知".into()),
                            stats.cost.cost_record_count,
                            stats.cost.total_record_count
                        ),
                    ));
                    let total = stats.totals.total_tokens.max(1) as f64;
                    rows.push(Control::section("quota-token-types", "Token 分布"));
                    for (id, label, value) in [
                        ("input", "输入", stats.totals.input_tokens),
                        ("output", "输出", stats.totals.output_tokens),
                        ("cache-read", "缓存读取", stats.totals.cache_read_tokens),
                        (
                            "cache-write",
                            "缓存写入",
                            stats.totals.cache_creation_tokens,
                        ),
                    ] {
                        rows.push(Control::bar(
                            format!("quota-category-{id}"),
                            format!("{label} · {value}"),
                            value as f64,
                            total,
                        ));
                    }
                    rows.push(Control::section("quota-backends", "后端消耗"));
                    for backend in &stats.backends {
                        rows.push(Control::bar(
                            format!("quota-backend-{}", backend.backend),
                            format!("{} · {} tokens", backend.backend, backend.total_tokens),
                            backend.total_tokens as f64,
                            total,
                        ));
                    }
                    rows.push(Control::section("quota-projects", "项目消耗"));
                    rows.push(quota_donut(
                        "quota-project-chart",
                        stats
                            .projects
                            .iter()
                            .map(|item| (item.project_name.clone(), item.total_tokens as f64)),
                    ));
                    rows.push(Control::section("quota-conversations", "对话消耗"));
                    rows.push(quota_donut(
                        "quota-conversation-chart",
                        stats
                            .conversations
                            .iter()
                            .map(|item| (item.task_title.clone(), item.total_tokens as f64)),
                    ));
                    rows.push(Control::section("quota-tools", "工具活跃度"));
                    rows.push(quota_donut(
                        "quota-tool-chart",
                        stats
                            .tools
                            .iter()
                            .map(|item| (item.label.clone(), item.call_count as f64)),
                    ));
                    rows.push(Control::section("quota-recent", "最近记录"));
                    for record in &stats.recent {
                        rows.push(Control::text(
                            format!("quota-record-{}", record.event_id),
                            format!(
                                "{} · {} · {} tokens",
                                settings_time(record.created_at),
                                record.backend,
                                record.total_tokens
                            ),
                        ));
                    }
                }
            }
            "remote" => {
                let busy = self.remote_busy();
                rows.push(Control::field(
                    "remote-name",
                    "电脑名称",
                    &self.remote_pc_name,
                    false,
                    |v| Intent::SettingsCommand(Action::RemoteName(v)),
                ));
                rows.push(
                    Control::action(
                        "remote-name-save",
                        "保存名称",
                        Intent::RemoteCommand(RemoteMessage::SavePcName),
                    )
                    .enabled(!busy),
                );
                rows.push(
                    Control::action(
                        "remote-refresh",
                        "刷新状态",
                        Intent::SettingsCommand(Action::RefreshRemote),
                    )
                    .enabled(!busy),
                );
                rows.push(
                    Control::action(
                        "remote-pair",
                        "生成配对二维码",
                        Intent::RemoteCommand(RemoteMessage::StartPairing),
                    )
                    .enabled(!busy && self.remote.as_ref().is_some_and(|s| s.host_enabled)),
                );
                if let Some(status) = &self.remote {
                    if let Some(ticket) = &status.active_ticket {
                        rows.push(Control::Qr {
                            id: "remote-qr".into(),
                            payload: ticket.pairing_uri.clone(),
                        });
                        rows.push(Control::text(
                            "remote-expiry",
                            "请使用 Android 扫码完成配对；过期后重新生成。",
                        ));
                        rows.push(Control::action(
                            "remote-copy",
                            "复制配对链接",
                            Intent::RemoteCommand(RemoteMessage::CopyPairingUri),
                        ));
                        rows.push(
                            Control::action(
                                "remote-cancel",
                                "取消配对",
                                Intent::RemoteCommand(RemoteMessage::CancelPairing),
                            )
                            .enabled(!busy),
                        );
                    }
                    rows.push(Control::section("remote-devices", "可信设备"));
                    let devices: Vec<_> = status
                        .trusted_devices
                        .iter()
                        .filter(|d| d.trusted && d.revoked_at.is_none())
                        .collect();
                    if devices.is_empty() {
                        rows.push(Control::text("remote-empty", "还没有配对的设备。"));
                    }
                    for device in devices {
                        rows.push(
                            Control::action(
                                format!("remote-revoke-{}", device.id),
                                format!("撤销 {}", device.display_name),
                                Intent::SettingsCommand(Action::RevokeRemote(device.id.clone())),
                            )
                            .danger()
                            .enabled(!busy),
                        );
                    }
                }
            }
            "preferences" => {
                append_project_preference_controls(
                    &self.project_settings,
                    &self.project_clone_parent,
                    &self.project_worktree_instructions.text(),
                    &mut rows,
                );
                rows.push(Control::section("github-section", "GitHub 绑定"));
                append_github_status(
                    &self.github_binding,
                    self.github_error.as_deref(),
                    &mut rows,
                );
                if self.github_binding.binding.is_some() {
                    rows.push(
                        Control::action(
                            "github-unbind",
                            "解除 GitHub 绑定",
                            Intent::SettingsCommand(Action::GitHubUnbind),
                        )
                        .danger(),
                    );
                } else if self.github_binding_busy() {
                    if let Some(flow) = &self.github_device_flow {
                        rows.push(Control::text(
                            "github-code",
                            format!("验证码：{}", flow.user_code),
                        ));
                    }
                    rows.push(Control::action(
                        "github-copy",
                        "复制验证码",
                        Intent::SettingsCommand(Action::GitHubCopy),
                    ));
                    rows.push(Control::action(
                        "github-open",
                        "打开 GitHub 验证",
                        Intent::SettingsCommand(Action::GitHubOpen),
                    ));
                    rows.push(Control::action(
                        "github-cancel",
                        "取消绑定",
                        Intent::CancelGitHubBinding,
                    ));
                } else if self.github_binding.client_id_configured {
                    rows.push(Control::action(
                        "github-bind",
                        "绑定 GitHub",
                        Intent::StartGitHubBinding,
                    ));
                }
                if let Some(error) = &self.project_settings_error {
                    rows.push(Control::text("preferences-error", error));
                }
            }
            "plugin-packages" | "plugin-hooks" | "plugin-mcp" | "extensions" => {}
            _ => {}
        }
        rows
    }

    pub(super) fn automation_surface_controls(&self) -> Vec<Control> {
        let mut rows = Vec::new();
        if !self.automations_open {
            return rows;
        }
        let Some(workflow) = self.selected_automation_workflow() else {
            return rows;
        };
        let action = |message| Intent::AutomationCommand(message);
        rows.push(Control::choice(
            "auto-inspector-panel",
            "检查器",
            &self.automation_inspector_panel,
            options(&[
                ("node", "节点"),
                ("workflow", "工作流"),
                ("scope", "触发范围"),
                ("runs", "运行记录"),
            ]),
            |panel| Intent::SettingsCommand(SettingsSurfaceAction::AutomationInspector(panel)),
        ));
        match self.automation_inspector_panel.as_str() {
            "node" => {
                if let Some(node) = self.selected_automation_node() {
                    rows.push(Control::section("auto-node", "节点检查器"));
                    rows.push(Control::field(
                        "auto-node-title",
                        "节点标题",
                        &self.automation_node_inspector.title,
                        false,
                        |v| Intent::AutomationCommand(AutomationMessage::NodeTitle(v)),
                    ));
                    for field in automation_node_config_fields(
                        &node.kind,
                        &self.automation_node_inspector.config,
                    ) {
                        let label = automation_field_label(field);
                        let value =
                            automation_config_string(&self.automation_node_inspector.config, field);
                        if matches!(
                            field,
                            "triggerKind"
                                | "permission"
                                | "logic"
                                | "action"
                                | "priority"
                                | "backend"
                                | "status"
                        ) {
                            rows.push(Control::choice(
                                format!("auto-node-{field}"),
                                label,
                                value,
                                automation_options(field),
                                move |value| {
                                    Intent::AutomationCommand(AutomationMessage::NodeConfig {
                                        field: field.into(),
                                        value: Value::String(value),
                                    })
                                },
                            ));
                        } else if field == "createTask" {
                            rows.push(Control::toggle(
                                "auto-node-create-task",
                                "创建任务",
                                automation_config_boolean(
                                    &self.automation_node_inspector.config,
                                    field,
                                ),
                                action(AutomationMessage::ToggleConfig(field.into())),
                            ));
                        } else {
                            rows.push(Control::field(
                                format!("auto-node-{field}"),
                                label,
                                value,
                                matches!(field, "prompt" | "text" | "summary"),
                                move |value| {
                                    Intent::AutomationCommand(AutomationMessage::NodeConfig {
                                        field: field.into(),
                                        value: Value::String(value),
                                    })
                                },
                            ));
                        }
                    }
                    rows.push(Control::action(
                        "auto-node-save",
                        "保存节点",
                        action(AutomationMessage::SaveNode),
                    ));
                }
                if self.selected_automation_node().is_none() {
                    rows.push(Control::text(
                        "auto-node-empty",
                        "在画布中选择节点，或前往工作流添加节点。",
                    ));
                }
                rows.push(Control::action(
                    "auto-publish",
                    "发布",
                    action(AutomationMessage::Publish),
                ));
            }
            "scope" => {
                rows.push(Control::section("auto-scope", "触发范围"));
                rows.push(Control::toggle(
                    "auto-inbox",
                    "包括收件箱",
                    workflow.scope.include_inbox,
                    action(AutomationMessage::ToggleInbox),
                ));
                for project in &self.projects {
                    rows.push(Control::toggle(
                        format!("auto-project-{}", project.id.as_str()),
                        &project.name,
                        workflow
                            .scope
                            .project_ids
                            .iter()
                            .any(|id| id == project.id.as_str()),
                        action(AutomationMessage::ToggleScope {
                            field: "project".into(),
                            value: project.id.as_str().into(),
                        }),
                    ));
                }
                for (field, values, selected) in [
                    (
                        "task-status",
                        options(&[
                            ("waiting", "等待"),
                            ("running", "运行中"),
                            ("blocked", "阻塞"),
                            ("done", "完成"),
                        ]),
                        &workflow.scope.task_statuses,
                    ),
                    (
                        "backend",
                        options(&[("native-agentkit", "Native")]),
                        &workflow.scope.backends,
                    ),
                    (
                        "event-kind",
                        options(&[
                            ("task_created", "新建任务"),
                            ("task_status_changed", "任务状态"),
                            ("task_updated", "任务更新"),
                            ("timeline_event", "对话事件"),
                            ("todo_changed", "待办更新"),
                            ("interaction_request", "交互请求"),
                        ]),
                        &workflow.scope.event_kinds,
                    ),
                ] {
                    for (value, label) in values {
                        rows.push(Control::toggle(
                            format!("auto-scope-{field}-{value}"),
                            label,
                            selected.contains(&value),
                            action(AutomationMessage::ToggleScope {
                                field: field.into(),
                                value,
                            }),
                        ));
                    }
                }
            }
            "runs" => {
                rows.push(Control::choice(
                    "auto-run-selection",
                    "运行记录",
                    self.selected_automation_run.clone().unwrap_or_default(),
                    self.automation_runs
                        .iter()
                        .map(|run| {
                            (
                                run.id.clone(),
                                format!(
                                    "{} · {}",
                                    settings_time(run.started_at),
                                    run_status(run.status)
                                ),
                            )
                        })
                        .collect(),
                    |id| Intent::AutomationCommand(AutomationMessage::SelectRun(id)),
                ));
                if let Some(detail) = &self.automation_run_detail {
                    rows.push(Control::text(
                        "auto-run-context",
                        format!(
                            "{} · {}{}",
                            detail.run.trigger.project_id.as_deref().unwrap_or("收件箱"),
                            detail
                                .run
                                .trigger
                                .event_kind
                                .as_deref()
                                .unwrap_or(&detail.run.trigger.kind),
                            detail
                                .run
                                .trigger
                                .task_id
                                .as_ref()
                                .map(|id| format!(" · 任务 {id}"))
                                .unwrap_or_default()
                        ),
                    ));
                    if let Some(error) = &detail.run.error {
                        rows.push(Control::text("auto-run-error", error));
                    }
                    if let Some(waiting) = waiting_human_node(detail) {
                        rows.push(Control::section("auto-human-waiting", "自动化等待确认"));
                        rows.push(Control::text(
                            "auto-human-question",
                            waiting
                                .output
                                .as_ref()
                                .and_then(|output| output.get("prompt"))
                                .and_then(Value::as_str)
                                .filter(|prompt| !prompt.trim().is_empty())
                                .unwrap_or("确认后继续运行。"),
                        ));
                        rows.push(Control::field(
                            "auto-response",
                            "回复",
                            &self.automation_human_response,
                            true,
                            |v| Intent::AutomationCommand(AutomationMessage::HumanResponse(v)),
                        ));
                        rows.push(Control::action(
                            "auto-resume",
                            "确认并继续",
                            action(AutomationMessage::Resume),
                        ));
                    }
                    if matches!(
                        detail.run.status,
                        AutomationRunStatus::Running | AutomationRunStatus::WaitingUser
                    ) {
                        rows.push(
                            Control::action(
                                "auto-cancel",
                                "停止运行",
                                action(AutomationMessage::CancelRun),
                            )
                            .danger(),
                        );
                    }
                    let selected = self
                        .automation_run_node
                        .as_deref()
                        .and_then(|id| detail.nodes.iter().find(|node| node.node_id == id))
                        .or_else(|| detail.nodes.first());
                    if let Some(node) = selected {
                        let node_title = |id: &str| {
                            workflow
                                .draft
                                .nodes
                                .iter()
                                .find(|node| node.id == id)
                                .map(|node| node.title.clone())
                                .unwrap_or_else(|| id.to_owned())
                        };
                        rows.push(Control::choice(
                            "auto-run-node",
                            "运行节点",
                            &node.node_id,
                            detail
                                .nodes
                                .iter()
                                .map(|node| {
                                    (
                                        node.node_id.clone(),
                                        format!(
                                            "{} · {}",
                                            node_title(&node.node_id),
                                            run_status(node.status)
                                        ),
                                    )
                                })
                                .collect(),
                            |id| Intent::SettingsCommand(SettingsSurfaceAction::SelectRunNode(id)),
                        ));
                        rows.push(Control::section(
                            "auto-run-node-detail",
                            node_title(&node.node_id),
                        ));
                        rows.push(Control::text(
                            "auto-node-status",
                            format!(
                                "状态：{} · 耗时 {}",
                                run_status(node.status),
                                node.started_at
                                    .zip(node.finished_at)
                                    .map(|(start, end)| format!(
                                        "{} ms",
                                        end.saturating_sub(start).max(0)
                                    ))
                                    .unwrap_or_else(|| "—".into())
                            ),
                        ));
                        rows.push(Control::text(
                            "auto-node-input",
                            format!(
                                "输入\n{}",
                                serde_json::to_string_pretty(&node.input).unwrap_or_default()
                            ),
                        ));
                        rows.push(Control::text(
                            "auto-node-output",
                            format!(
                                "输出\n{}",
                                node.output
                                    .as_ref()
                                    .map(|output| serde_json::to_string_pretty(output)
                                        .unwrap_or_default())
                                    .unwrap_or_else(|| "暂无输出".into())
                            ),
                        ));
                        if let Some(error) = &node.error {
                            rows.push(Control::text("auto-node-error", error));
                        }
                    }
                }
            }
            _ => {
                rows.push(Control::section("auto-palette", "节点库"));
                for (kind, label) in [
                    ("agent", "Agent"),
                    ("tool", "工具"),
                    ("logic", "条件分支"),
                    ("human", "人工确认"),
                ] {
                    rows.push(Control::action(
                        format!("auto-add-{kind}"),
                        label,
                        action(AutomationMessage::AddNode(kind.into())),
                    ));
                }
                rows.push(
                    Control::action(
                        "auto-delete-selection",
                        "删除选中节点或连线",
                        action(AutomationMessage::DeleteSelection),
                    )
                    .danger()
                    .enabled(self.automation_selection.is_some()),
                );
                rows.push(Control::section("auto-workflow", "工作流"));
                rows.push(Control::text(
                    "auto-draft-status",
                    if self.automation_dirty.contains(&workflow.id) {
                        "草稿未保存"
                    } else if workflow.published_version_id.is_some() {
                        "已有发布版本"
                    } else {
                        "尚未发布"
                    },
                ));
                rows.push(Control::field(
                    "auto-name",
                    "自动化名称",
                    &workflow.name,
                    false,
                    |v| Intent::AutomationCommand(AutomationMessage::AutomationNameChanged(v)),
                ));
                rows.push(Control::action(
                    "auto-publish",
                    "发布",
                    action(AutomationMessage::Publish),
                ));
                rows.push(
                    Control::action(
                        "auto-toggle",
                        if workflow.enabled { "停用" } else { "启用" },
                        action(AutomationMessage::ToggleEnabled),
                    )
                    .enabled(workflow.enabled || workflow.published_version_id.is_some()),
                );
                rows.push(
                    Control::action(
                        "auto-delete",
                        "删除自动化",
                        Intent::SettingsCommand(SettingsSurfaceAction::DeleteAutomation),
                    )
                    .danger(),
                );
                rows.push(Control::section("auto-view", "画布"));
                rows.push(Control::action(
                    "auto-zoom-in",
                    "放大",
                    Intent::SettingsCommand(SettingsSurfaceAction::ZoomAutomation(1.25)),
                ));
                rows.push(Control::action(
                    "auto-zoom-out",
                    "缩小",
                    Intent::SettingsCommand(SettingsSurfaceAction::ZoomAutomation(0.8)),
                ));
                rows.push(Control::action(
                    "auto-fit",
                    "适应视图",
                    Intent::SettingsCommand(SettingsSurfaceAction::FitAutomation),
                ));
            }
        }
        if let Some(error) = &self.automation_error {
            rows.push(Control::text("auto-error", error));
        }
        let identity = match self.automation_inspector_panel.as_str() {
            "node" => format!(
                "{}:node:{}",
                workflow.id,
                self.selected_automation_node()
                    .map(|node| node.id.as_str())
                    .unwrap_or_default()
            ),
            "runs" => format!(
                "{}:run:{}",
                workflow.id,
                self.selected_automation_run.as_deref().unwrap_or_default()
            ),
            panel => format!("{}:{panel}", workflow.id),
        };
        let rows = rows
            .into_iter()
            .map(|control| control.binding_identity(identity.clone()))
            .collect();

        rows
    }
}

fn run_status(status: AutomationRunStatus) -> &'static str {
    match status {
        AutomationRunStatus::Pending => "等待",
        AutomationRunStatus::Running => "运行中",
        AutomationRunStatus::Succeeded => "成功",
        AutomationRunStatus::Failed => "失败",
        AutomationRunStatus::Cancelled => "已取消",
        AutomationRunStatus::WaitingUser => "等待确认",
        _ => "已结束",
    }
}

fn settings_time(timestamp: i64) -> String {
    let seconds = timestamp.div_euclid(1000).rem_euclid(86_400);
    format!(
        "{} {:02}:{:02}:{:02} UTC",
        format_civil_date(timestamp),
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod automation_editor_tests {
    use super::*;

    #[test]
    fn inspector_edits_are_in_the_draft_published_without_a_separate_node_save() {
        let service = lilia_feature_automation::DesktopAutomationService::in_memory(Arc::new(
            lilia_feature_automation::SilentAutomationEvents,
        ))
        .unwrap();
        let mut workflow = service
            .save_draft(AutomationSaveDraftInput {
                id: None,
                name: "节点编辑".into(),
                scope: AutomationScopeFilter::default(),
                nodes: vec![
                    AutomationNode {
                        id: "trigger".into(),
                        kind: "trigger".into(),
                        title: "触发".into(),
                        position: AutomationNodePosition { x: 0.0, y: 0.0 },
                        config: json!({ "triggerKind": "manual" }),
                    },
                    AutomationNode {
                        id: "human".into(),
                        kind: "human".into(),
                        title: "旧名称".into(),
                        position: AutomationNodePosition { x: 220.0, y: 0.0 },
                        config: json!({ "prompt": "旧问题" }),
                    },
                ],
                edges: vec![],
            })
            .unwrap();
        let inspector = AutomationNodeInspectorDraft {
            node_id: Some("human".into()),
            title: "新名称".into(),
            config: json!({ "prompt": "发布前的新问题" }).to_string(),
        };
        assert!(update_inspected_node(&mut workflow, &inspector));
        service
            .save_draft(AutomationSaveDraftInput {
                id: Some(workflow.id.clone()),
                name: workflow.name,
                scope: workflow.scope,
                nodes: workflow.draft.nodes,
                edges: workflow.draft.edges,
            })
            .unwrap();
        let published = service.publish(&workflow.id).unwrap();
        let node = published
            .snapshot
            .nodes
            .iter()
            .find(|node| node.id == "human")
            .unwrap();
        assert_eq!(node.title, "新名称");
        assert_eq!(node.config["prompt"], "发布前的新问题");
    }

    #[test]
    fn new_logic_nodes_offer_executable_branch_ports_before_any_edges_exist() {
        let mut node = AutomationNode {
            id: "logic".into(),
            kind: "logic".into(),
            title: "分支".into(),
            position: AutomationNodePosition { x: 0.0, y: 0.0 },
            config: json!({ "logic": "condition" }),
        };
        assert_eq!(
            automation_output_handles(&node),
            BTreeSet::from(["true".into(), "false".into()])
        );
        node.config = json!({ "logic": "switch", "cases": "done, needs review, ,blocked" });
        assert_eq!(
            automation_output_handles(&node),
            BTreeSet::from([
                "done".into(),
                "needs_review".into(),
                "blocked".into(),
                "default".into()
            ])
        );
        let outgoing = automation_output_handles(&node);
        let matching =
            lilia_feature_automation::automation_json_value_to_port(&json!("needs review"));
        assert!(outgoing.contains(&matching));
    }
}

fn update_inspected_node(
    workflow: &mut AutomationWorkflow,
    inspector: &AutomationNodeInspectorDraft,
) -> bool {
    let Some(node) = workflow
        .draft
        .nodes
        .iter_mut()
        .find(|node| inspector.node_id.as_deref() == Some(node.id.as_str()))
    else {
        return false;
    };
    let Ok(config) = parse_automation_node_config(&inspector.config) else {
        return false;
    };
    let changed = node.title != inspector.title || node.config != config;
    node.title = inspector.title.clone();
    node.config = config;
    changed
}

pub(super) fn automation_output_handles(node: &AutomationNode) -> BTreeSet<String> {
    let mut handles = BTreeSet::new();
    if node.kind == "logic" {
        match node
            .config
            .get("logic")
            .and_then(Value::as_str)
            .unwrap_or("condition")
        {
            "condition" => handles.extend(["true".to_owned(), "false".to_owned()]),
            "switch" => {
                handles.insert("default".to_owned());
                handles.extend(
                    node.config
                        .get("cases")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .split(',')
                        .map(str::trim)
                        .filter(|case| !case.is_empty())
                        .map(|case| {
                            lilia_feature_automation::automation_json_value_to_port(&json!(case))
                        })
                        .filter(|case| !case.is_empty()),
                );
            }
            _ => {
                handles.insert("output".to_owned());
            }
        }
    } else {
        handles.insert("output".to_owned());
    }
    handles
}

fn automation_field_label(field: &str) -> &str {
    match field {
        "triggerKind" => "触发条件",
        "taskId" => "任务",
        "projectId" => "项目",
        "title" => "标题",
        "model" => "模型",
        "projectCwd" => "项目目录",
        "prompt" => "提示词",
        "permission" => "权限",
        "logic" => "分支类型",
        "path" => "匹配字段",
        "equals" => "匹配值",
        "cases" => "分支",
        "action" => "操作",
        "status" => "状态",
        "text" => "内容",
        "priority" => "优先级",
        "summary" => "摘要",
        "backend" => "运行后端",
        _ => field,
    }
}

impl DesktopProgram {
    fn save_agent_surface_settings(&mut self) {
        let update =
            DesktopAgentInteractionSettingsUpdate::from_settings(&self.agent_interaction_settings);
        match self
            .kernel
            .session()
            .save_agent_interaction_settings(update)
        {
            Ok(_) => self.refresh_agent_interaction(),
            Err(error) => {
                self.agent_interaction_error = Some(agent_interaction_error_message(&error))
            }
        }
    }

    pub(super) fn extension_browser_snapshot(
        &self,
    ) -> Option<crate::runtime_extensions::ExtensionBrowserSnapshot> {
        let tab = self.settings_state.active_tab().as_str();
        (self.settings_open
            && matches!(
                tab,
                "extensions" | "plugin-packages" | "plugin-hooks" | "plugin-mcp"
            ))
        .then(|| extension_browser(self.extensions_module(), tab))
    }
}

fn extension_browser(
    module: &crate::module::extensions::ExtensionsModule,
    tab: &str,
) -> crate::runtime_extensions::ExtensionBrowserSnapshot {
    use crate::module::extensions::ExtensionEditor;
    let mut toolbar = Vec::new();
    append_extension_panel(module, tab, &mut toolbar, ExtensionPart::Toolbar);
    if tab == "extensions" {
        toolbar.push(
            Control::action(
                "skill-create-open",
                "新建技能",
                Intent::ExtensionsCommand(ExtensionsMessage::OpenSkillEditor),
            )
            .enabled(!module.busy()),
        );
    }
    let mut detail = Vec::new();
    append_extension_panel(module, tab, &mut detail, ExtensionPart::Detail);
    let selected = module.selected_entry(tab);
    let title = selected
        .as_ref()
        .map(|entry| format!("{} · {}", entry.label, entry.meta))
        .unwrap_or_else(|| "选择一项".into());
    detail.retain(|control| !matches!(control, Control::Section { .. }));
    let (mut detail_actions, mut detail): (Vec<_>, Vec<_>) =
        detail.into_iter().partition(|control| {
            matches!(
                control,
                Control::Action {
                    intent: Intent::ExtensionsCommand(
                        ExtensionsMessage::ToggleSkill(_)
                            | ExtensionsMessage::TogglePlugin(_)
                            | ExtensionsMessage::ToggleHookSource(_)
                            | ExtensionsMessage::ToggleMcpServer(_)
                            | ExtensionsMessage::RequestDeleteSkill(_)
                            | ExtensionsMessage::RequestDeletePlugin(_)
                            | ExtensionsMessage::RequestDeleteHookSource(_)
                            | ExtensionsMessage::RequestDeleteMcpServer(_)
                            | ExtensionsMessage::OpenHookEditor(_)
                            | ExtensionsMessage::EditMcpServer(_)
                            | ExtensionsMessage::CreateHookSource(_)
                    ),
                    ..
                } | Control::Action {
                    intent: Intent::SettingsCommand(SettingsSurfaceAction::OpenPath(_)),
                    ..
                }
            )
        });
    if selected.is_none() {
        detail.push(Control::text(
            "extensions-empty-detail",
            "选择一项以查看详情",
        ));
    }
    let editor = module.editor().map(|editor| {
        let (title, editor_tab) = match editor {
            ExtensionEditor::Skill => ("新建技能", "extensions"),
            ExtensionEditor::Hook { .. } => ("编辑 Hooks 配置", "plugin-hooks"),
            ExtensionEditor::Mcp => (
                if module
                    .mcp_editor()
                    .is_some_and(|editor| editor.editing_server_id.is_some())
                {
                    "编辑 MCP 服务"
                } else {
                    "添加 MCP 服务"
                },
                "plugin-mcp",
            ),
        };
        let mut controls = Vec::new();
        if matches!(editor, ExtensionEditor::Skill) {
            let (name, description) = module.skill_draft();
            controls.push(Control::field("skill_id", "名称", name, false, |value| {
                Intent::ExtensionsCommand(ExtensionsMessage::SkillIdChanged(value))
            }));
            controls.push(Control::field(
                "skill_description",
                "描述",
                description,
                true,
                |value| {
                    Intent::ExtensionsCommand(ExtensionsMessage::SkillDescriptionChanged(value))
                },
            ));
            controls.push(
                Control::action(
                    "create-skill",
                    "创建技能",
                    Intent::ExtensionsCommand(ExtensionsMessage::CreateSkill),
                )
                .enabled(!name.trim().is_empty()),
            );
        }
        append_extension_panel(module, editor_tab, &mut controls, ExtensionPart::Editor);
        let can_save = match editor {
            ExtensionEditor::Skill => true,
            ExtensionEditor::Hook { source_id, .. } => module.hooks().is_some_and(|hooks| {
                hooks
                    .sources
                    .iter()
                    .any(|source| &source.id == source_id && source.editable && source.exists)
            }),
            ExtensionEditor::Mcp => module.mcp_editor().is_some_and(|editor| {
                editor.editing_server_id.as_ref().is_none_or(|id| {
                    module.snapshot().is_some_and(|snapshot| {
                        snapshot
                            .mcp_servers
                            .iter()
                            .any(|server| &server.server_id == id && server.editable)
                    })
                })
            }),
        };
        if !can_save {
            controls.push(Control::text(
                "extension-editor-readonly",
                "此来源已不可编辑。草稿已保留，可以取消后查看最新配置。",
            ));
        }
        let (mut actions, controls): (Vec<_>, Vec<_>) = controls.into_iter().partition(|control| {
            matches!(
                control,
                Control::Action {
                    intent: Intent::ExtensionsCommand(
                        ExtensionsMessage::CreateSkill
                            | ExtensionsMessage::SaveHookSource(_)
                            | ExtensionsMessage::SaveMcpServer
                            | ExtensionsMessage::CancelMcpEditor
                    ),
                    ..
                }
            )
        });
        actions.retain(|control| {
            !matches!(
                control,
                Control::Action {
                    intent: Intent::ExtensionsCommand(ExtensionsMessage::CancelMcpEditor),
                    ..
                }
            )
        });
        actions.insert(
            0,
            Control::action(
                "extension-editor-cancel",
                "取消",
                Intent::ExtensionsCommand(ExtensionsMessage::CancelEditor),
            ),
        );
        if !can_save {
            for action in actions.iter_mut().skip(1) {
                if let Control::Action { enabled, .. } = action {
                    *enabled = false;
                }
            }
        }
        if module.busy() {
            for action in &mut actions {
                if let Control::Action { enabled, .. } = action {
                    *enabled = false;
                }
            }
        }
        crate::runtime_extensions::ExtensionEditorSnapshot {
            title: title.into(),
            controls,
            actions,
            busy: module.busy(),
        }
    });
    if module.busy() {
        for action in &mut toolbar {
            if let Control::Action { enabled, .. } = action {
                *enabled = false;
            }
        }
    }
    if module.busy() {
        for action in &mut detail_actions {
            if let Control::Action { enabled, .. } = action {
                *enabled = false;
            }
        }
    }
    crate::runtime_extensions::ExtensionBrowserSnapshot {
        tab: tab.into(),
        query: module.query().into(),
        entries: module.entries(tab),
        selected: selected.map(|entry| entry.key),
        title,
        toolbar,
        detail,
        detail_actions,
        editor,
    }
}

fn append_project_preference_controls(
    settings: &DesktopProjectSettings,
    clone_parent: &str,
    instructions: &str,
    rows: &mut Vec<Control>,
) {
    rows.push(Control::text(
        "project-clone-default",
        format!(
            "Clone 默认父目录：{}",
            if clone_parent.is_empty() {
                "未设置（使用家目录）"
            } else {
                &clone_parent
            }
        ),
    ));
    rows.push(Control::action(
        "project-clone-default-pick",
        "选择 Clone 父目录",
        Intent::SettingsCommand(SettingsSurfaceAction::PickCloneParent),
    ));
    rows.push(Control::choice(
        "worktree-mode",
        "工作树默认行为",
        match settings.worktree.default_mode {
            DesktopWorktreeSelectionMode::Current => "current",
            DesktopWorktreeSelectionMode::Create => "create",
            DesktopWorktreeSelectionMode::Existing => "existing",
        },
        options(&[
            ("current", "当前工作区"),
            ("create", "新建工作树"),
            ("existing", "选择现有工作树"),
        ]),
        |v| {
            Intent::SettingsCommand(SettingsSurfaceAction::ProjectPreference(
                ProjectPreferenceChange::Mode(v),
            ))
        },
    ));
    rows.push(Control::toggle(
        "worktree-cleanup",
        "归档后清理工作树",
        settings.worktree.cleanup_on_archive,
        Intent::SettingsCommand(SettingsSurfaceAction::ProjectPreference(
            ProjectPreferenceChange::Cleanup(!settings.worktree.cleanup_on_archive),
        )),
    ));
    rows.push(Control::action(
        "worktree-parent",
        format!(
            "工作树父目录：{}",
            settings.worktree.parent_dir.as_deref().unwrap_or("默认")
        ),
        Intent::SettingsCommand(SettingsSurfaceAction::PickWorktreeParent),
    ));
    rows.push(Control::field(
        "worktree-instructions",
        "创建后自动指令",
        instructions,
        true,
        |v| {
            Intent::SettingsCommand(SettingsSurfaceAction::ProjectPreference(
                ProjectPreferenceChange::Instructions(v),
            ))
        },
    ));
}

fn append_github_status(
    status: &DesktopGitHubBindingStatus,
    error: Option<&str>,
    rows: &mut Vec<Control>,
) {
    rows.push(Control::text(
        "github-status",
        match &status.binding {
            Some(binding) => format!("已绑定 GitHub：{}", binding.login),
            None if !status.client_id_configured => "当前版本暂不支持 GitHub 绑定。".into(),
            None => "尚未绑定 GitHub".into(),
        },
    ));
    if let Some(error) = error {
        rows.push(Control::text("github-error", error));
    }
}

fn append_hook_details(
    source: &crate::application::DesktopHookSourceView,
    document: Option<&crate::application::DesktopHookDocumentView>,
    rows: &mut Vec<Control>,
) {
    let trust = match source.trust_state.as_str() {
        "managed" => "已允许执行",
        "required" => "尚未允许执行",
        _ => "尚未创建配置",
    };
    rows.push(Control::text(
        format!("hook-state-{}", source.id),
        format!(
            "{} · {} · {}{}",
            if source.enabled {
                "已启用"
            } else {
                "已停用"
            },
            if source.editable {
                "可编辑"
            } else {
                "只读"
            },
            trust,
            source
                .project_cwd
                .as_ref()
                .map(|cwd| format!("\n生效项目：{cwd}"))
                .unwrap_or_default(),
        ),
    ));
    let mut notices = std::collections::BTreeSet::new();
    notices.extend(source.warnings.iter());
    notices.extend(source.limitations.iter());
    if let Some(document) = document {
        notices.extend(document.warnings.iter());
        notices.extend(document.limitations.iter());
        for (index, handler) in document.handlers.iter().enumerate() {
            let state = if !handler.supported {
                "当前暂不支持此处理器，配置会保留但不会执行"
            } else if handler.executable {
                "可执行"
            } else {
                "当前不会执行，请检查来源状态和处理器配置"
            };
            rows.push(Control::text(
                format!("hook-handler-status-{}-{index}", source.id),
                format!(
                    "{} · {}\n{}{}{}",
                    handler.event,
                    handler.handler_type,
                    state,
                    handler
                        .command
                        .as_ref()
                        .map(|v| format!("\n命令：{v}"))
                        .unwrap_or_default(),
                    handler
                        .command_windows
                        .as_ref()
                        .map(|v| format!("\nWindows 命令：{v}"))
                        .unwrap_or_default()
                ),
            ));
            notices.extend(handler.warnings.iter());
        }
        if let Some(raw) = &document.raw_document {
            rows.push(Control::text(
                format!("hook-document-{}", source.id),
                format!("配置原文\n{raw}"),
            ));
        }
    }
    for (index, notice) in notices.into_iter().enumerate() {
        rows.push(Control::text(
            format!("hook-notice-{}-{index}", source.id),
            notice,
        ));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExtensionPart {
    All,
    Toolbar,
    Detail,
    Editor,
}

#[cfg(test)]
fn append_extension_controls(
    module: &crate::module::extensions::ExtensionsModule,
    tab: &str,
    rows: &mut Vec<Control>,
) {
    append_extension_panel(module, tab, rows, ExtensionPart::All);
}

fn append_extension_panel(
    module: &crate::module::extensions::ExtensionsModule,
    tab: &str,
    rows: &mut Vec<Control>,
    part: ExtensionPart,
) {
    let toolbar = matches!(part, ExtensionPart::All | ExtensionPart::Toolbar);
    let detail = matches!(part, ExtensionPart::All | ExtensionPart::Detail);
    let editor = matches!(part, ExtensionPart::All | ExtensionPart::Editor);
    let selected = module.selected_entry(tab).map(|entry| entry.key);
    let action = |message| Intent::ExtensionsCommand(message);
    if toolbar {
        rows.push(Control::action(
            "extensions-refresh",
            "刷新",
            action(ExtensionsMessage::Refresh),
        ));
    }
    if let Some(error) = module.error() {
        rows.push(Control::text("extensions-error", error));
    }
    if part == ExtensionPart::All
        && matches!(
            tab,
            "extensions" | "plugin-packages" | "plugin-hooks" | "plugin-mcp"
        )
    {
        rows.push(Control::field(
            "extensions-search",
            "搜索",
            module.query(),
            false,
            |value| Intent::ExtensionsCommand(ExtensionsMessage::SearchChanged(value)),
        ));
    }
    if tab == "extensions" {
        if editor {
            rows.push(Control::toggle(
                "skill-scope",
                "创建到当前项目",
                module.skill_project_scope(),
                action(ExtensionsMessage::ToggleSkillScope),
            ));
        }
        if detail {
            if let Some(snapshot) = module.snapshot() {
                for skill in snapshot
                    .skills
                    .iter()
                    .filter(|s| {
                        module.matches_search(&format!(
                            "{} {} {} {} {}",
                            s.skill_id, s.description, s.path, s.scope, s.registered_from
                        ))
                    })
                    .filter(|skill| {
                        selected.as_deref()
                            == Some(format!("skill:{}:{}", skill.scope, skill.path).as_str())
                    })
                {
                    rows.push(Control::text(
                        format!("skill-detail-{}", skill.skill_id),
                        format!(
                            "{} · {}\n{}\n{}",
                            skill.skill_id,
                            match skill.scope.as_str() {
                                "project" => "项目技能",
                                "plugin" => "插件技能",
                                _ => "用户技能",
                            },
                            skill.description,
                            skill.path,
                        ),
                    ));
                    if !skill.editable {
                        rows.push(Control::text(
                            format!("skill-readonly-{}", skill.skill_id),
                            if skill.scope == "plugin" {
                                "请在所属插件中管理此技能。"
                            } else {
                                "此来源的技能为只读。"
                            },
                        ));
                    }
                    if skill.editable {
                        rows.push(Control::action(
                            format!("skill-{}", skill.skill_id),
                            if skill.enabled {
                                "停用技能"
                            } else {
                                "启用技能"
                            },
                            action(ExtensionsMessage::ToggleSkill(skill.skill_id.clone())),
                        ));
                        rows.push(Control::action(
                            format!("skill-open-{}", skill.skill_id),
                            "打开技能文件",
                            Intent::SettingsCommand(SettingsSurfaceAction::OpenPath(
                                skill.path.clone(),
                            )),
                        ));
                        rows.push(
                            Control::action(
                                format!("skill-delete-{}", skill.skill_id),
                                "删除技能",
                                action(ExtensionsMessage::RequestDeleteSkill(
                                    skill.skill_id.clone(),
                                )),
                            )
                            .danger(),
                        );
                    }
                }
            }
        }
    }
    if tab == "plugin-packages" {
        if toolbar {
            rows.push(Control::field(
                "plugin-source",
                "插件目录",
                module.plugin_source_input(),
                false,
                |v| Intent::ExtensionsCommand(ExtensionsMessage::PluginSourceChanged(v)),
            ));
            rows.push(Control::action(
                "plugin-pick",
                "选择插件目录",
                action(ExtensionsMessage::PickPluginDirectory),
            ));
            rows.push(
                Control::action(
                    "plugin-install",
                    "安装插件",
                    action(ExtensionsMessage::InstallPlugin),
                )
                .enabled(!module.plugin_source_input().trim().is_empty()),
            );
        }
        if detail {
            if let Some(snapshot) = module.snapshot() {
                for plugin in snapshot
                    .plugins
                    .iter()
                    .filter(|p| {
                        module.matches_search(&format!(
                            "{} {} {} {} {}",
                            p.plugin_id, p.name, p.description, p.version, p.path
                        ))
                    })
                    .filter(|plugin| {
                        selected.as_deref() == Some(format!("plugin:{}", plugin.path).as_str())
                    })
                {
                    rows.push(Control::section(
                        format!("plugin-section-{}", plugin.plugin_id),
                        &plugin.name,
                    ));
                    rows.push(Control::text(
                        format!("plugin-detail-{}", plugin.plugin_id),
                        format!(
                            "{} {}\n{}\n技能 {} · Hooks {} · MCP {}\n{}",
                            plugin.name,
                            plugin.version,
                            plugin.description,
                            plugin.skill_count,
                            plugin.hook_count,
                            plugin.mcp_server_count,
                            plugin.path,
                        ),
                    ));
                    rows.push(
                        Control::action(
                            format!("plugin-toggle-{}", plugin.plugin_id),
                            if plugin.enabled { "停用" } else { "启用" },
                            action(ExtensionsMessage::TogglePlugin(plugin.plugin_id.clone())),
                        )
                        .enabled(plugin.editable),
                    );
                    if !plugin.editable {
                        rows.push(Control::text(
                            format!("plugin-readonly-{}", plugin.plugin_id),
                            "此来源的插件为只读。",
                        ));
                    }
                    if plugin.editable {
                        rows.push(
                            Control::action(
                                format!("plugin-delete-{}", plugin.plugin_id),
                                "卸载",
                                action(ExtensionsMessage::RequestDeletePlugin(
                                    plugin.plugin_id.clone(),
                                )),
                            )
                            .danger(),
                        );
                    }
                    for (index, warning) in plugin.warnings.iter().enumerate() {
                        rows.push(Control::text(
                            format!("plugin-warning-{}-{index}", plugin.plugin_id),
                            warning,
                        ));
                    }
                }
            }
        }
    }
    if tab == "plugin-hooks" && (detail || editor) {
        if let Some(overview) = module.hooks() {
            for source in overview
                .sources
                .iter()
                .filter(|source| {
                    module.matches_search(&format!(
                        "{} {} {} {:?} {}",
                        source.id,
                        source.path,
                        source.project_cwd.as_deref().unwrap_or_default(),
                        source.scope,
                        module
                            .hook_document(&source.id)
                            .and_then(|doc| doc.raw_document.as_deref())
                            .unwrap_or_default()
                    ))
                })
                .filter(|source| match module.editor() {
                    Some(crate::module::extensions::ExtensionEditor::Hook {
                        source_id, ..
                    }) if part == ExtensionPart::Editor => &source.id == source_id,
                    _ => {
                        selected.as_deref()
                            == Some(format!("hook:{}:{}", source.id, source.path).as_str())
                    }
                })
            {
                if detail {
                    rows.push(Control::section(
                        format!("hook-section-{}", source.id),
                        if source.scope == crate::application::DesktopHookScope::User {
                            "用户 Hooks"
                        } else {
                            "项目 Hooks"
                        },
                    ));
                    rows.push(Control::text(
                        format!("hook-source-{}", source.id),
                        format!("{}\n{}", source.id, source.path),
                    ));
                    append_hook_details(source, module.hook_document(&source.id), rows);
                }
                if !source.editable {
                    continue;
                }
                if !source.exists {
                    rows.push(Control::action(
                        format!("hook-create-{}", source.id),
                        "创建配置",
                        action(ExtensionsMessage::CreateHookSource(source.id.clone())),
                    ));
                    continue;
                }
                if detail {
                    rows.push(Control::action(
                        format!("hook-toggle-{}", source.id),
                        if source.enabled { "停用" } else { "启用" },
                        action(ExtensionsMessage::ToggleHookSource(source.id.clone())),
                    ));
                    rows.push(Control::action(
                        format!("hook-edit-{}", source.id),
                        "编辑配置",
                        action(ExtensionsMessage::OpenHookEditor(source.id.clone())),
                    ));
                    rows.push(
                        Control::action(
                            format!("hook-delete-{}", source.id),
                            "删除配置",
                            action(ExtensionsMessage::RequestDeleteHookSource(
                                source.id.clone(),
                            )),
                        )
                        .danger(),
                    );
                }
                if !editor {
                    continue;
                }
                if let Some(draft) = module.hook_drafts().get(&source.id) {
                    if let Ok(handlers) = parse_hook_handlers_draft(draft) {
                        for (index, handler) in handlers.iter().enumerate() {
                            for (field, label, value) in [
                                (HookHandlerDraftField::Event, "事件", handler.event.clone()),
                                (
                                    HookHandlerDraftField::Matcher,
                                    "匹配条件",
                                    handler.matcher.clone().unwrap_or_default(),
                                ),
                                (
                                    HookHandlerDraftField::Type,
                                    "类型",
                                    handler.handler_type.clone(),
                                ),
                                (
                                    HookHandlerDraftField::TimeoutSeconds,
                                    "超时（秒）",
                                    handler
                                        .timeout_seconds
                                        .map(|v| v.to_string())
                                        .unwrap_or_default(),
                                ),
                                (
                                    HookHandlerDraftField::Command,
                                    "命令",
                                    handler.command.clone().unwrap_or_default(),
                                ),
                                (
                                    HookHandlerDraftField::CommandWindows,
                                    "Windows 命令",
                                    handler.command_windows.clone().unwrap_or_default(),
                                ),
                                (
                                    HookHandlerDraftField::StatusMessage,
                                    "状态说明",
                                    handler.status_message.clone().unwrap_or_default(),
                                ),
                            ] {
                                let source_id = source.id.clone();
                                rows.push(Control::field(
                                    format!("hook-{}-{index}-{}", source.id, field.target_key()),
                                    label,
                                    value,
                                    matches!(
                                        field,
                                        HookHandlerDraftField::Command
                                            | HookHandlerDraftField::CommandWindows
                                    ),
                                    move |value| {
                                        Intent::ExtensionsCommand(
                                            ExtensionsMessage::HookHandlerDraftChanged {
                                                source_id: source_id.clone(),
                                                index,
                                                field,
                                                value,
                                            },
                                        )
                                    },
                                ));
                            }
                            rows.push(
                                Control::action(
                                    format!("hook-remove-{}-{index}", source.id),
                                    "移除处理器",
                                    action(ExtensionsMessage::RemoveHookHandler {
                                        source_id: source.id.clone(),
                                        index,
                                    }),
                                )
                                .danger(),
                            );
                        }
                    }
                }
                rows.push(Control::action(
                    format!("hook-add-{}", source.id),
                    "添加处理器",
                    action(ExtensionsMessage::AddHookHandler(source.id.clone())),
                ));
                rows.push(Control::action(
                    format!("hook-save-{}", source.id),
                    "保存配置",
                    action(ExtensionsMessage::SaveHookSource(source.id.clone())),
                ));
            }
        }
    }
    if tab == "plugin-mcp" {
        if toolbar {
            rows.push(Control::action(
                "mcp-new",
                "添加 MCP 服务",
                action(ExtensionsMessage::NewMcpServer),
            ));
            rows.push(Control::action(
                "mcp-activate",
                "连接已启用服务",
                action(ExtensionsMessage::ActivateRegisteredMcp),
            ));
        }
        if editor {
            if let Some(editor) = module.mcp_editor() {
                if let Some(id) = &editor.editing_server_id {
                    rows.push(Control::text("mcp-id-readonly", format!("服务名称：{id}")));
                }
                for (id, label, value, constructor) in [
                    (
                        "mcp-id",
                        "服务名称",
                        &editor.server_id,
                        ExtensionsMessage::McpServerIdChanged as fn(String) -> ExtensionsMessage,
                    ),
                    (
                        "mcp-location",
                        "命令或地址",
                        &editor.location,
                        ExtensionsMessage::McpLocationChanged,
                    ),
                    (
                        "mcp-args",
                        "启动参数",
                        &editor.args_json,
                        ExtensionsMessage::McpArgsChanged,
                    ),
                    (
                        "mcp-credentials",
                        "凭据名称",
                        &editor.credential_names_json,
                        ExtensionsMessage::McpCredentialNamesChanged,
                    ),
                ] {
                    if id == "mcp-id" && editor.editing_server_id.is_some() {
                        continue;
                    }
                    rows.push(Control::field(id, label, value, false, move |v| {
                        Intent::ExtensionsCommand(constructor(v))
                    }));
                }
                rows.push(Control::choice(
                    "mcp-transport",
                    "传输方式",
                    editor.transport.as_registry(),
                    options(&[
                        ("stdio", "本地命令"),
                        ("streamable_http", "HTTP"),
                        ("sse", "SSE"),
                    ]),
                    |value| {
                        Intent::ExtensionsCommand(ExtensionsMessage::McpTransportChanged(value))
                    },
                ));
                rows.push(Control::toggle(
                    "mcp-enabled",
                    "启用服务",
                    editor.enabled,
                    action(ExtensionsMessage::ToggleMcpEditorEnabled),
                ));
                rows.push(Control::action(
                    "mcp-save",
                    "保存服务",
                    action(ExtensionsMessage::SaveMcpServer),
                ));
                rows.push(Control::action(
                    "mcp-cancel",
                    "取消编辑",
                    action(ExtensionsMessage::CancelMcpEditor),
                ));
            }
        }
        if detail {
            if let Some(snapshot) = module.snapshot() {
                for server in snapshot
                    .mcp_servers
                    .iter()
                    .filter(|server| {
                        module.matches_search(&format!(
                            "{} {} {} {} {} {}",
                            server.server_id,
                            server.source,
                            server.transport,
                            server.location.as_deref().unwrap_or_default(),
                            server
                                .command
                                .as_deref()
                                .or(server.url.as_deref())
                                .unwrap_or_default(),
                            server.args.join(" ")
                        ))
                    })
                    .filter(|server| {
                        selected.as_deref()
                            == Some(format!("mcp:{}:{}", server.source, server.server_id).as_str())
                    })
                {
                    rows.push(Control::section(
                        format!("mcp-section-{}", server.server_id),
                        &server.server_id,
                    ));
                    rows.push(Control::text(
                        format!("mcp-status-{}", server.server_id),
                        format!(
                            "{} · {}\n工具 {} · 资源 {} · 提示词 {}{}",
                            server.server_id,
                            server.runtime_state.as_deref().unwrap_or("未连接"),
                            server.tool_count,
                            server.resource_count,
                            server.prompt_count,
                            server
                                .last_error
                                .as_ref()
                                .map(|e| format!("\n{e}"))
                                .unwrap_or_default()
                        ),
                    ));
                    if server.editable {
                        rows.push(Control::action(
                            format!("mcp-edit-{}", server.server_id),
                            "编辑",
                            action(ExtensionsMessage::EditMcpServer(server.server_id.clone())),
                        ));
                        rows.push(Control::action(
                            format!("mcp-toggle-{}", server.server_id),
                            if server.enabled { "停用" } else { "启用" },
                            action(ExtensionsMessage::ToggleMcpServer(server.server_id.clone())),
                        ));
                        rows.push(
                            Control::action(
                                format!("mcp-delete-{}", server.server_id),
                                "删除服务",
                                action(ExtensionsMessage::RequestDeleteMcpServer(
                                    server.server_id.clone(),
                                )),
                            )
                            .danger(),
                        );
                    }
                    for credential in &server.credentials {
                        let server_id = server.server_id.clone();
                        let name = credential.name.clone();
                        let kind = credential.kind;
                        let key = mcp_credential_draft_key(&server_id, kind, &name);
                        rows.push(Control::secret(
                            format!("mcp-secret-{key}"),
                            format!(
                                "{}{}",
                                name,
                                if credential.present {
                                    "（已保存）"
                                } else {
                                    ""
                                }
                            ),
                            module
                                .mcp_credential_drafts()
                                .get(&key)
                                .cloned()
                                .unwrap_or_default(),
                            move |value| {
                                Intent::ExtensionsCommand(ExtensionsMessage::McpCredentialChanged {
                                    server_id: server_id.clone(),
                                    kind,
                                    name: name.clone(),
                                    value,
                                })
                            },
                        ));
                        rows.push(Control::action(
                            format!("mcp-secret-save-{key}"),
                            "保存凭据",
                            action(ExtensionsMessage::SaveMcpCredential {
                                server_id: server.server_id.clone(),
                                kind,
                                name: credential.name.clone(),
                            }),
                        ));
                        if credential.present {
                            rows.push(
                                Control::action(
                                    format!("mcp-secret-delete-{key}"),
                                    "清除凭据",
                                    action(ExtensionsMessage::DeleteMcpCredential {
                                        server_id: server.server_id.clone(),
                                        kind,
                                        name: credential.name.clone(),
                                    }),
                                )
                                .danger(),
                            );
                        }
                    }
                    for prompt in &server.prompts {
                        let name = prompt.namespaced_name.clone();
                        rows.push(Control::field(
                            format!("mcp-prompt-{name}"),
                            format!("{}参数", prompt.name),
                            module.prompt_draft(&name),
                            true,
                            move |value| {
                                Intent::ExtensionsCommand(
                                    ExtensionsMessage::McpPromptArgumentsChanged {
                                        namespaced_name: name.clone(),
                                        value,
                                    },
                                )
                            },
                        ));
                        rows.push(Control::action(
                            format!("mcp-get-{}", prompt.namespaced_name),
                            format!("获取 {}", prompt.name),
                            action(ExtensionsMessage::GetMcpPrompt(
                                prompt.namespaced_name.clone(),
                            )),
                        ));
                    }
                    for resource in &server.resources {
                        rows.push(Control::action(
                            format!("mcp-resource-{}-{}", server.server_id, resource.uri),
                            &resource.name,
                            action(ExtensionsMessage::ReadMcpResource {
                                server_id: server.server_id.clone(),
                                uri: resource.uri.clone(),
                            }),
                        ));
                    }
                }
            }
            if let Some(preview) = module.content_preview() {
                rows.push(Control::text(
                    "mcp-preview",
                    format!("{}\n{}", preview.title, preview.text),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "desktop_settings_extensions_tests.rs"]
mod extensions_equivalence_tests;

impl DesktopProgram {
    pub(super) fn settings_surface_confirm(&self) -> Option<crate::runtime_shell::ShellConfirm> {
        let module = self.extensions_module();
        let name = module
            .skill_delete_confirmation()
            .or(module.plugin_delete_confirmation())
            .or(module.hook_delete_confirmation())
            .or(module.mcp_delete_confirmation())
            .map(str::to_owned)
            .or_else(|| match &self.settings_confirmation {
                Some(SettingsSurfaceAction::DeleteAutomation) => {
                    self.selected_automation_workflow().map(|w| w.name.clone())
                }
                Some(SettingsSurfaceAction::RevokeRemote(id)) => self
                    .remote
                    .as_ref()
                    .and_then(|s| s.trusted_devices.iter().find(|d| &d.id == id))
                    .map(|d| d.display_name.clone()),
                _ => None,
            })?;
        Some(crate::runtime_shell::ShellConfirm {
            kind: crate::runtime_shell::ShellConfirmKind::Settings,
            title: "确认移除".into(),
            message: format!("确定移除 {name}？"),
            confirm_label: "移除".into(),
            cancel_label: "取消".into(),
            danger: true,
            busy: false,
        })
    }

    pub(super) fn confirm_settings_surface(&mut self, confirm: bool) -> Option<Message> {
        let module = self.extensions_module();
        for (pending, accept, cancel) in [
            (
                module.skill_delete_confirmation().is_some(),
                ExtensionsMessage::ConfirmDeleteSkill,
                ExtensionsMessage::CancelDeleteSkill,
            ),
            (
                module.plugin_delete_confirmation().is_some(),
                ExtensionsMessage::ConfirmDeletePlugin,
                ExtensionsMessage::CancelDeletePlugin,
            ),
            (
                module.hook_delete_confirmation().is_some(),
                ExtensionsMessage::ConfirmDeleteHookSource,
                ExtensionsMessage::CancelDeleteHookSource,
            ),
            (
                module.mcp_delete_confirmation().is_some(),
                ExtensionsMessage::ConfirmDeleteMcpServer,
                ExtensionsMessage::CancelDeleteMcpServer,
            ),
        ] {
            if pending {
                return Some(Message::Extensions(if confirm { accept } else { cancel }));
            }
        }
        let action = self.settings_confirmation.take()?;
        if confirm {
            match action {
                SettingsSurfaceAction::DeleteAutomation => {
                    return Some(Message::Automation(AutomationMessage::Delete));
                }
                SettingsSurfaceAction::RevokeRemote(id) => {
                    return Some(Message::Remote(RemoteMessage::RevokeDevice(id)));
                }
                _ => {}
            }
        }
        Some(Message::Settings(SettingsMessage::SelectSettingsTab(
            self.settings_state.active_tab().clone(),
        )))
    }
}

fn options(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(id, label)| ((*id).into(), (*label).into()))
        .collect()
}
fn automation_options(field: &str) -> Vec<(String, String)> {
    options(match field {
        "triggerKind" => &[
            ("manual", "手动运行"),
            ("task_changed", "任务变化"),
            ("timeline_event", "对话事件"),
            ("todo_changed", "待办变化"),
            ("interaction_request", "交互请求"),
        ],
        "permission" => &[
            ("ask", "需要确认"),
            ("full", "完全访问"),
            ("readonly", "只读"),
        ],
        "logic" => &[
            ("condition", "条件"),
            ("switch", "多路分支"),
            ("stop", "停止"),
        ],
        "action" => &[
            ("record_timeline", "记录事件"),
            ("create_task", "新建任务"),
            ("update_task_status", "更新任务状态"),
            ("add_todo", "添加待办"),
            ("send_guide", "发送指导"),
        ],
        "priority" => &[("low", "低"), ("normal", "普通"), ("high", "高")],
        "backend" => &[("native-agentkit", "Native")],
        "status" => &[
            ("waiting", "等待"),
            ("running", "运行中"),
            ("blocked", "阻塞"),
            ("done", "完成"),
        ],
        _ => &[],
    })
}

impl DesktopProgram {
    pub(super) fn adjust_automation_view(&mut self, factor: Option<f32>) {
        let size = self
            .runtime_shell
            .as_ref()
            .zip(self.documents.get(&HostedWindowId::PRIMARY))
            .and_then(|(shell, document)| shell.automation_canvas_size(document));
        let Some(size) = size.filter(|size| size.width > 0.0 && size.height > 0.0) else {
            return;
        };
        self.automation_viewport = match factor {
            Some(factor) => self
                .automation_viewport
                .zoom_at(GraphPoint::new(size.width / 2.0, size.height / 2.0), factor),
            None => self
                .automation_graph
                .bounds()
                .map(|bounds| GraphViewport::fit(bounds, size, 32.0))
                .unwrap_or_default(),
        };
    }

    pub(super) fn remote_refresh_deadline(&self) -> Option<Instant> {
        if !self.settings_open
            || self.settings_state.active_tab().as_str() != "remote"
            || self.remote_busy()
        {
            return None;
        }
        self.remote_next_refresh
    }

    pub(super) fn refresh_remote_if_due(&mut self, now: Instant) -> bool {
        if self
            .remote_refresh_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            self.start_remote_operation(RemoteRequest::Refresh);
            return true;
        }
        false
    }
}

fn quota_donut(id: &str, values: impl IntoIterator<Item = (String, f64)>) -> Control {
    let mut entries: Vec<_> = values
        .into_iter()
        .filter(|(_, value)| value.is_finite() && *value > 0.0)
        .collect();
    entries.sort_by(|a, b| b.1.total_cmp(&a.1));
    entries.truncate(5);
    Control::Donut {
        id: id.into(),
        entries,
    }
}

#[cfg(test)]
mod quota_chart_tests {
    use super::*;
    #[test]
    fn quota_chart_uses_positive_top_five_with_stable_ties() {
        let Control::Donut { entries, .. } = quota_donut(
            "test",
            [
                ("zero", 0.0),
                ("invalid", f64::NAN),
                ("negative", -4.0),
                ("one", 1.0),
                ("two", 2.0),
                ("three", 3.0),
                ("four", 4.0),
                ("five", 5.0),
                ("six", 6.0),
            ]
            .map(|(name, value)| (name.to_string(), value)),
        ) else {
            panic!("expected donut");
        };
        assert_eq!(
            entries,
            [
                ("six", 6.0),
                ("five", 5.0),
                ("four", 4.0),
                ("three", 3.0),
                ("two", 2.0)
            ]
            .map(|(name, value)| (name.to_string(), value))
        );
    }
}

fn appearance_controls(
    mode: NativeSidebarDisplayMode,
    non_interrupt: bool,
    debug: bool,
    error: Option<&str>,
) -> Vec<Control> {
    let mut rows = vec![
        Control::section("appearance-product", "界面"),
        Control::text("appearance-language", "语言：简体中文"),
        Control::choice(
            "appearance-sidebar",
            "侧边栏样式",
            if mode == NativeSidebarDisplayMode::Unified {
                "unified"
            } else {
                "grouped"
            },
            options(&[("grouped", "按项目分组"), ("unified", "统一列表")]),
            |value| {
                Intent::SettingsCommand(SettingsSurfaceAction::SidebarDisplayMode(
                    if value == "unified" {
                        NativeSidebarDisplayMode::Unified
                    } else {
                        NativeSidebarDisplayMode::Grouped
                    },
                ))
            },
        ),
        Control::section("appearance-runtime", "运行配置"),
        Control::toggle(
            "appearance-non-interrupt",
            "非打断模式",
            non_interrupt,
            Intent::SettingsCommand(SettingsSurfaceAction::AgentToggle(
                AgentInteractionToggle::NonInterrupt,
            )),
        ),
        Control::toggle(
            "appearance-debug",
            "Debug 面板",
            debug,
            Intent::SettingsCommand(SettingsSurfaceAction::AgentToggle(
                AgentInteractionToggle::Debug,
            )),
        ),
    ];
    if let Some(error) = error {
        rows.push(Control::text("appearance-runtime-error", error));
    }
    rows
}

#[cfg(test)]
mod appearance_entry_tests {
    use super::*;
    use nana_ui::runtime::{AppContext, DocumentId, LayoutViewport, Stack};
    use std::sync::{Arc, Mutex};

    #[test]
    fn appearance_sidebar_choice_dispatches_and_persists_both_display_modes() {
        let home = tempfile::tempdir().unwrap();
        let doc = DocumentId::new(797).unwrap();
        let mut context = AppContext::new();
        let root = context
            .create_component(doc, Stack::fill_column(0.0))
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = events.clone();
        let sink: Arc<dyn Fn(Intent) + Send + Sync> =
            Arc::new(move |intent| output.lock().unwrap().push(intent));
        let mut surface = crate::runtime_surface::SurfaceHandles::default();
        let mut mode = load_sidebar_display_mode(home.path());
        for (delta, expected) in [
            (1, NativeSidebarDisplayMode::Unified),
            (-1, NativeSidebarDisplayMode::Grouped),
        ] {
            let controls = appearance_controls(mode, false, false, None);
            let children = surface
                .sync(&mut context, doc, &controls, sink.clone())
                .unwrap();
            context
                .reconcile_children(root.stable_id(), &children)
                .unwrap();
            context
                .layout_document(doc, LayoutViewport::new(960.0, 800.0))
                .unwrap();
            let selector = surface
                .debug_nodes()
                .into_iter()
                .find(|(id, _, _)| id == "appearance-sidebar")
                .unwrap()
                .1;
            context.focus_node(doc, selector).unwrap();
            assert_eq!(context.world().focused(doc), Some(selector));
            assert!(context.adjust_focused_dropdown(doc, delta).unwrap());
            assert!(context.adjust_focused_dropdown(doc, delta).unwrap());
            assert!(context.commit_focused_dropdown(doc).unwrap());
            let Some(Intent::SettingsCommand(SettingsSurfaceAction::SidebarDisplayMode(selected))) =
                events.lock().unwrap().pop()
            else {
                panic!("normal dropdown selection must dispatch the settings action");
            };
            assert_eq!(selected, expected);
            save_sidebar_display_mode(home.path(), selected).unwrap();
            mode = load_sidebar_display_mode(home.path());
            assert_eq!(mode, expected);
        }
    }
}
