use super::*;
use crate::module::extensions::{
    ExtensionsCommand, ExtensionsModule, ExtensionsModuleMessage, ExtensionsOutcome,
    McpRegistryOperation, NativeHooksSnapshot,
};
use crate::ui_module::{UiModule, UiModuleContext};

fn snapshot(editable: bool) -> crate::application::DesktopExtensionsSnapshot {
    serde_json::from_value(serde_json::json!({
        "dataSource":"native", "sharedIdentityOk":true,
        "skillsRegistryPath":"/skills.json", "skillsRegistryRevision":7,
        "mcpRegistryPath":"/mcp.json", "mcpRegistryRevision":9,
        "pluginsRegistryPath":"/plugins.json", "pluginsRegistryRevision":11,
        "skillRoots":[], "runtimeServices":[],
        "legacyPluginManagerAvailable":false, "legacyHooksManagerAvailable":false,
        "skills":[{"skillId":"review", "path":"/Acme/skills/review/SKILL.md",
            "registeredFrom":"package", "scope":"plugin", "description":"Review changes",
            "enabled":true, "editable":editable, "runtimeAvailable":true}],
        "plugins":[{"pluginId":"review-package", "name":"Reviewer", "version":"1.2",
            "description":"Code review", "path":"/Acme/plugins/reviewer", "enabled":true,
            "editable":editable, "runtimeAvailable":true, "packageSha256":"fixture",
            "skillCount":1, "hookCount":1, "mcpServerCount":1, "warnings":[]}],
        "mcpServers":[{"serverId":"tools", "source":"user", "transport":"stdio",
            "location":"/Acme/bin/mcp", "registered":true, "editable":editable, "enabled":true,
            "command":"/Acme/bin/mcp", "args":[], "url":null, "registeredFrom":"/mcp.json",
            "runtimeState":null, "toolCount":0, "resourceCount":0, "promptCount":0,
            "restartCount":0, "lastError":null, "tools":[], "resources":[], "prompts":[],
            "credentials":[]}]
    }))
    .unwrap()
}

fn hooks(editable: bool) -> NativeHooksSnapshot {
    let source = crate::application::DesktopHookSourceView {
        id: "project".into(),
        scope: crate::application::DesktopHookScope::Project,
        project_cwd: Some("/Acme/project".into()),
        path: "/Acme/project/hooks.json".into(),
        exists: true,
        editable,
        enabled: false,
        revision: 13,
        handler_count: 1,
        trust_state: "required".into(),
        warnings: vec!["来源待确认".into()],
        limitations: vec![],
    };
    let document = crate::application::DesktopHookDocumentView {
        source: source.clone(),
        handlers: vec![crate::application::DesktopHookHandlerView {
            id: "notification".into(),
            event: "Notification".into(),
            matcher: None,
            handler_type: "command".into(),
            command: Some("notify-review".into()),
            command_windows: None,
            timeout_seconds: None,
            status_message: None,
            supported: false,
            executable: false,
            warnings: vec!["此事件暂不支持".into()],
        }],
        raw_document: Some("{\"event\":\"Notification\",\"command\":\"notify-review\"}".into()),
        warnings: vec![],
        limitations: vec![],
    };
    NativeHooksSnapshot {
        overview: crate::application::DesktopHooksOverview {
            sources: vec![source],
            warnings: vec![],
        },
        documents: [("project".into(), document)].into_iter().collect(),
    }
}

fn reduce(module: &mut ExtensionsModule, message: ExtensionsModuleMessage) {
    let kernel = lilia_kernel::Kernel::new();
    module.reduce(
        message,
        &UiModuleContext::new(&kernel, nana_ui_platform::WindowId::PRIMARY),
    );
}

fn ui(module: &mut ExtensionsModule, message: ExtensionsMessage) {
    reduce(module, ExtensionsModuleMessage::Ui(message));
}

fn refresh(module: &mut ExtensionsModule, editable: bool) {
    reduce(
        module,
        ExtensionsModuleMessage::ApplyOutcome(ExtensionsOutcome::Refresh(
            snapshot(editable),
            hooks(editable),
        )),
    );
}

fn rows(module: &ExtensionsModule, tab: &str) -> Vec<Control> {
    let mut rows = Vec::new();
    append_extension_controls(module, tab, &mut rows);
    rows
}

fn control<'a>(rows: &'a [Control], target: &str) -> Option<&'a Control> {
    rows.iter().find(|row| match row {
        Control::Text { id, .. }
        | Control::Section { id, .. }
        | Control::Action { id, .. }
        | Control::Field { id, .. }
        | Control::Choice { id, .. }
        | Control::Toggle { id, .. } => id == target,
        _ => false,
    })
}

fn activate(module: &mut ExtensionsModule, rows: &[Control], target: &str) {
    let Some(Control::Action {
        intent: Intent::ExtensionsCommand(message),
        enabled: true,
        ..
    }) = control(rows, target)
    else {
        panic!("{target} must be an enabled user action");
    };
    ui(module, message.clone());
}

fn input(module: &mut ExtensionsModule, rows: &[Control], target: &str, value: &str) {
    let Some(Control::Field { edit, .. }) = control(rows, target) else {
        panic!("missing field {target}");
    };
    let Intent::ExtensionsCommand(message) = edit(value.into()) else {
        panic!("wrong message route");
    };
    ui(module, message);
}

#[test]
fn mcp_edit_preserves_identity_and_emits_updated_registry_command() {
    let mut module = ExtensionsModule::default();
    refresh(&mut module, true);
    let controls = rows(&module, "plugin-mcp");
    activate(&mut module, &controls, "mcp-edit-tools");
    let controls = rows(&module, "plugin-mcp");
    assert!(control(&controls, "mcp-id").is_none());
    assert!(control(&controls, "mcp-id-readonly").is_some());
    ui(
        &mut module,
        ExtensionsMessage::McpServerIdChanged("renamed".into()),
    );
    input(&mut module, &controls, "mcp-location", "/new/bin/mcp");
    input(&mut module, &controls, "mcp-args", "[\"--safe\"]");
    activate(&mut module, &controls, "mcp-save");
    let Some(ExtensionsCommand::McpRegistry(McpRegistryOperation::Upsert(command))) =
        module.take_pending_submit()
    else {
        panic!("missing save");
    };
    assert_eq!(command.server_id, "tools");
    assert_eq!(command.command.as_deref(), Some("/new/bin/mcp"));
    assert_eq!(command.args, ["--safe"]);
    assert_eq!(command.expected_registry_revision, 9);
    reduce(
        &mut module,
        ExtensionsModuleMessage::JobFailed("retry".into()),
    );
    ui(&mut module, ExtensionsMessage::CancelMcpEditor);
    ui(&mut module, ExtensionsMessage::NewMcpServer);
    let controls = rows(&module, "plugin-mcp");
    input(&mut module, &controls, "mcp-id", "fresh");
    assert_eq!(module.mcp_editor().unwrap().server_id, "fresh");
    ui(
        &mut module,
        ExtensionsMessage::EditMcpServer("tools".into()),
    );
    refresh(&mut module, false);
    ui(&mut module, ExtensionsMessage::SaveMcpServer);
    assert!(module.take_pending_submit().is_none());
    assert!(
        module.mcp_editor().is_some(),
        "a rejected save preserves the draft"
    );
    assert!(module.error().is_some());
    let editor = extension_browser(&module, "plugin-mcp").editor.unwrap();
    assert!(matches!(
        control(&editor.actions, "mcp-save"),
        Some(Control::Action { enabled: false, .. })
    ));
    assert!(matches!(
        control(&editor.actions, "extension-editor-cancel"),
        Some(Control::Action { enabled: true, .. })
    ));
}

#[test]
fn readonly_sources_hide_mutations_and_reject_stale_delete_confirmation() {
    let mut module = ExtensionsModule::default();
    refresh(&mut module, false);
    let controls = rows(&module, "plugin-packages");
    assert!(matches!(
        control(&controls, "plugin-toggle-review-package"),
        Some(Control::Action { enabled: false, .. })
    ));
    assert!(control(&controls, "plugin-delete-review-package").is_none());
    let controls = rows(&module, "extensions");
    assert!(control(&controls, "skill-delete-review").is_none());
    assert!(
        matches!(control(&controls, "skill-detail-review"), Some(Control::Text {value, ..}) if value.contains("插件技能"))
    );
    for message in [
        ExtensionsMessage::ToggleSkill("review".into()),
        ExtensionsMessage::TogglePlugin("review-package".into()),
        ExtensionsMessage::ToggleMcpServer("tools".into()),
        ExtensionsMessage::ToggleHookSource("project".into()),
    ] {
        ui(&mut module, message);
        assert!(module.take_pending_submit().is_none());
    }
    for (request, confirm) in [
        (
            ExtensionsMessage::RequestDeleteSkill("review".into()),
            ExtensionsMessage::ConfirmDeleteSkill,
        ),
        (
            ExtensionsMessage::RequestDeletePlugin("review-package".into()),
            ExtensionsMessage::ConfirmDeletePlugin,
        ),
        (
            ExtensionsMessage::RequestDeleteMcpServer("tools".into()),
            ExtensionsMessage::ConfirmDeleteMcpServer,
        ),
        (
            ExtensionsMessage::RequestDeleteHookSource("project".into()),
            ExtensionsMessage::ConfirmDeleteHookSource,
        ),
    ] {
        refresh(&mut module, true);
        ui(&mut module, request);
        assert!(
            module.take_pending_submit().is_none(),
            "request must wait for confirmation"
        );
        refresh(&mut module, false);
        ui(&mut module, confirm);
        assert!(
            module.take_pending_submit().is_none(),
            "permission changed before confirmation"
        );
    }
}

#[test]
fn search_field_filters_every_extension_source_by_location() {
    let mut module = ExtensionsModule::default();
    refresh(&mut module, true);
    for (tab, target) in [
        ("extensions", "skill-detail-review"),
        ("plugin-packages", "plugin-detail-review-package"),
        ("plugin-hooks", "hook-source-project"),
        ("plugin-mcp", "mcp-status-tools"),
    ] {
        let controls = rows(&module, tab);
        input(&mut module, &controls, "extensions-search", "  ACME  ");
        assert!(control(&rows(&module, tab), target).is_some());
        input(
            &mut module,
            &controls,
            "extensions-search",
            "no-matching-source",
        );
        assert!(control(&rows(&module, tab), target).is_none());
    }
    let controls = rows(&module, "plugin-hooks");
    input(&mut module, &controls, "extensions-search", "notify-review");
    assert!(control(&rows(&module, "plugin-hooks"), "hook-source-project").is_some());
}

#[test]
fn hook_details_keep_unsupported_readonly_documents_visible_without_edit_actions() {
    let mut module = ExtensionsModule::default();
    refresh(&mut module, false);
    let controls = rows(&module, "plugin-hooks");
    assert!(
        matches!(control(&controls, "hook-handler-status-project-0"), Some(Control::Text { value, .. }) if value.contains("不会执行") && value.contains("notify-review"))
    );
    assert!(
        matches!(control(&controls, "hook-state-project"), Some(Control::Text { value, .. }) if value.contains("/Acme/project") && value.contains("尚未允许执行"))
    );
    assert!(
        matches!(control(&controls, "hook-document-project"), Some(Control::Text { value, .. }) if value.contains(module.hook_document("project").unwrap().raw_document.as_deref().unwrap()))
    );
    for action in [
        "hook-save-project",
        "hook-toggle-project",
        "hook-delete-project",
        "hook-add-project",
    ] {
        assert!(control(&controls, action).is_none());
    }
    refresh(&mut module, true);
    let controls = rows(&module, "plugin-hooks");
    activate(&mut module, &controls, "hook-toggle-project");
    assert!(matches!(
        module.take_pending_submit(),
        Some(ExtensionsCommand::Hook {
            operation: crate::module::extensions::HookSourceOperation::SetEnabled {
                enabled: true,
                expected_revision: 13,
                ..
            },
            ..
        })
    ));
}

#[test]
fn github_binding_feedback_preserves_account_when_action_fails() {
    let mut status = crate::application::DesktopGitHubBindingStatus {
        state: "bound".into(),
        client_id_configured: true,
        client_id_source: crate::application::DesktopGitHubClientIdSource::Bundled,
        binding: Some(crate::application::DesktopGitHubBindingMetadata {
            login: "octocat".into(),
            avatar_url: None,
            bound_at: 123,
            scopes: vec![],
            client_id_source: crate::application::DesktopGitHubClientIdSource::Bundled,
        }),
    };
    let mut controls = Vec::new();
    append_github_status(&status, Some("解除绑定失败，请重试。"), &mut controls);
    assert!(
        matches!(control(&controls, "github-status"), Some(Control::Text { value, .. }) if value.contains("octocat"))
    );
    assert!(control(&controls, "github-error").is_some());
    status.binding = None;
    controls.clear();
    append_github_status(&status, Some("授权已过期，请重新绑定。"), &mut controls);
    assert!(control(&controls, "github-error").is_some());
    controls.clear();
    append_github_status(&status, None, &mut controls);
    assert!(control(&controls, "github-error").is_none());
}

#[test]
fn settings_replacement_blocks_fold_actions_but_keeps_other_workspace_changes() {
    let mut workspace =
        super::super::initial_workspace(&crate::storage::NativeSidebarTreeState::default());
    let before = workspace.model().clone();
    for action in [
        WorkspaceAction::ToggleRegion(RegionId::Resources),
        WorkspaceAction::SetRegionCollapsed(RegionId::Resources, true),
    ] {
        assert!(!super::super::update_workspace_for_surface(
            &mut workspace,
            true,
            action
        ));
        assert_eq!(workspace.model(), &before);
    }
    assert!(super::super::update_workspace_for_surface(
        &mut workspace,
        true,
        WorkspaceAction::SetRegionSize(RegionId::Resources, 400.0)
    ));
    assert!(super::super::update_workspace_for_surface(
        &mut workspace,
        false,
        WorkspaceAction::ToggleRegion(RegionId::Resources)
    ));
    assert!(workspace
        .layout()
        .region(&RegionId::Resources)
        .unwrap()
        .collapsed_value());
}

#[test]
fn project_preferences_controls_persist_changes_without_explicit_save() {
    use crate::application::{
        DesktopApplicationConfig, DesktopHost, DesktopHostAction, DesktopHostContext,
        DesktopHostError, DesktopHostResult,
    };
    struct NoopHost;
    impl DesktopHost for NoopHost {
        fn execute(
            &self,
            _: &DesktopHostContext,
            _: DesktopHostAction,
        ) -> Result<DesktopHostResult, DesktopHostError> {
            Ok(DesktopHostResult::Completed)
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let identity = format!(
        "preferences-ui-{}",
        dir.path().file_name().unwrap().to_string_lossy()
    );
    let config = DesktopApplicationConfig::new(dir.path(), identity.clone()).unwrap();
    let authority = lilia_service::ServiceAuthority::bootstrap_in_memory_named(
        format!("test:{identity}"),
        identity,
    )
    .unwrap();
    let application =
        DesktopApplication::from_authority(config, authority, std::sync::Arc::new(NoopHost))
            .unwrap();
    let mut settings = application.project_settings().unwrap();
    settings.clone_parent_dir = Some("/clone-root".into());
    settings.worktree.parent_dir = Some("/worktree-root".into());
    application.save_project_settings(settings.clone()).unwrap();
    for (id, value) in [
        ("worktree-mode", "create"),
        ("worktree-cleanup", ""),
        ("worktree-instructions", "run review before archive"),
    ] {
        let mut controls = Vec::new();
        append_project_preference_controls(
            &settings,
            "/clone-root",
            &settings.worktree.auto_instructions,
            &mut controls,
        );
        assert!(control(&controls, "preferences-save").is_none());
        assert!(matches!(
            control(&controls, "project-clone-default-pick"),
            Some(Control::Action {
                intent: Intent::SettingsCommand(SettingsSurfaceAction::PickCloneParent),
                enabled: true,
                ..
            })
        ));
        let intent = match control(&controls, id).unwrap() {
            Control::Choice { edit, .. } | Control::Field { edit, .. } => edit(value.into()),
            Control::Toggle { intent, .. } => intent.clone(),
            _ => panic!("unexpected preference control"),
        };
        let Intent::SettingsCommand(SettingsSurfaceAction::ProjectPreference(change)) = intent
        else {
            panic!("wrong preference route");
        };
        settings = save_project_preference_change(&application, &settings, change).unwrap();
        assert_eq!(
            application.project_settings().unwrap(),
            settings,
            "normal control persists immediately"
        );
        assert_eq!(settings.clone_parent_dir.as_deref(), Some("/clone-root"));
        assert_eq!(
            settings.worktree.parent_dir.as_deref(),
            Some("/worktree-root")
        );
    }
    let stored = application.project_settings().unwrap();
    assert_eq!(
        stored.worktree.default_mode,
        DesktopWorktreeSelectionMode::Create
    );
    assert!(!stored.worktree.cleanup_on_archive);
    assert_eq!(
        stored.worktree.auto_instructions,
        "run review before archive"
    );
}

#[test]
fn extensions_browser_selects_one_detail_and_recovers_selection_after_search() {
    let mut module = ExtensionsModule::default();
    let mut data = snapshot(true);
    let mut second = data.skills[0].clone();
    second.skill_id = "other".into();
    second.path = "/Other/SKILL.md".into();
    data.skills.push(second);
    reduce(
        &mut module,
        ExtensionsModuleMessage::ApplyOutcome(ExtensionsOutcome::Refresh(
            data.clone(),
            hooks(true),
        )),
    );
    let view = extension_browser(&module, "extensions");
    assert_eq!(view.entries.len(), 2);
    assert!(control(&view.detail, "skill-detail-review").is_some());
    assert!(control(&view.detail, "skill-detail-other").is_none());
    ui(
        &mut module,
        ExtensionsMessage::SelectEntry {
            tab: "extensions".into(),
            key: view.entries[1].key.clone(),
        },
    );
    let selected = extension_browser(&module, "extensions");
    assert!(control(&selected.detail, "skill-detail-review").is_none());
    assert!(control(&selected.detail, "skill-detail-other").is_some());
    ui(
        &mut module,
        ExtensionsMessage::SearchChanged("missing".into()),
    );
    assert!(extension_browser(&module, "extensions").selected.is_none());
    ui(&mut module, ExtensionsMessage::SearchChanged(String::new()));
    assert_eq!(
        extension_browser(&module, "extensions").selected,
        selected.selected
    );
    assert!(module.take_pending_submit().is_none());
    assert_eq!(module.snapshot().unwrap(), &data);
}

#[test]
fn extensions_skill_editor_cancel_never_submits_and_failure_preserves_draft() {
    let mut module = ExtensionsModule::default();
    refresh(&mut module, true);
    let original = module.snapshot().unwrap().clone();
    let view = extension_browser(&module, "extensions");
    assert!(control(&view.detail, "skill_id").is_none());
    activate(&mut module, &view.toolbar, "skill-create-open");
    let editor = extension_browser(&module, "extensions").editor.unwrap();
    input(&mut module, &editor.controls, "skill_id", "new-skill");
    activate(&mut module, &editor.actions, "extension-editor-cancel");
    assert!(module.editor().is_none());
    assert!(module.take_pending_submit().is_none());
    assert_eq!(module.snapshot().unwrap(), &original);
    ui(&mut module, ExtensionsMessage::OpenSkillEditor);
    let editor = extension_browser(&module, "extensions").editor.unwrap();
    input(&mut module, &editor.controls, "skill_id", "new-skill");
    let editor = extension_browser(&module, "extensions").editor.unwrap();
    activate(&mut module, &editor.actions, "create-skill");
    ui(&mut module, ExtensionsMessage::CancelEditor);
    assert!(
        module.editor().is_some(),
        "cannot dismiss an already submitted save"
    );
    assert!(matches!(
        module.take_pending_submit(),
        Some(ExtensionsCommand::Skill(
            crate::module::extensions::SkillRegistryOperation::Create(_)
        ))
    ));
    reduce(
        &mut module,
        ExtensionsModuleMessage::JobFailed("保存失败".into()),
    );
    let editor = extension_browser(&module, "extensions").editor.unwrap();
    assert!(
        matches!(control(&editor.controls, "skill_id"), Some(Control::Field {value,..}) if value == "new-skill")
    );
    assert!(control(&editor.controls, "extensions-error").is_some());
    activate(&mut module, &editor.actions, "create-skill");
    assert!(module.take_pending_submit().is_some());
    assert!(module.editor().is_some());
    reduce(
        &mut module,
        ExtensionsModuleMessage::ApplyOutcome(ExtensionsOutcome::Skill(original)),
    );
    assert!(module.editor().is_none());
}

#[test]
fn extensions_hook_editor_cancel_restores_draft_and_save_waits_for_authority() {
    let mut module = ExtensionsModule::default();
    refresh(&mut module, true);
    let original = module.hook_drafts()["project"].clone();
    let browser = extension_browser(&module, "plugin-hooks");
    assert!(control(&browser.detail, "hook-project-0-command").is_none());
    activate(&mut module, &browser.detail_actions, "hook-edit-project");
    let editor = extension_browser(&module, "plugin-hooks").editor.unwrap();
    input(
        &mut module,
        &editor.controls,
        "hook-project-0-command",
        "changed-command",
    );
    assert_ne!(module.hook_drafts()["project"], original);
    activate(&mut module, &editor.actions, "extension-editor-cancel");
    assert_eq!(module.hook_drafts()["project"], original);
    assert!(module.take_pending_submit().is_none());
    ui(
        &mut module,
        ExtensionsMessage::OpenHookEditor("project".into()),
    );
    let editor = extension_browser(&module, "plugin-hooks").editor.unwrap();
    input(
        &mut module,
        &editor.controls,
        "hook-project-0-event",
        "UserPromptSubmit",
    );
    input(
        &mut module,
        &editor.controls,
        "hook-project-0-command",
        "changed-command",
    );
    activate(&mut module, &editor.actions, "hook-save-project");
    assert!(matches!(
        module.take_pending_submit(),
        Some(ExtensionsCommand::Hook { .. })
    ));
    assert!(module.editor().is_some());
    let mut response = hooks(true);
    response.documents.get_mut("project").unwrap().handlers[0].command =
        Some("changed-command".into());
    reduce(
        &mut module,
        ExtensionsModuleMessage::ApplyOutcome(ExtensionsOutcome::Hook(response)),
    );
    assert!(module.editor().is_none());
    assert!(module.hook_drafts()["project"].contains("changed-command"));
}

#[test]
fn plugin_source_input_survives_refresh_through_normal_events_and_snapshot_sync() {
    use crate::module::extensions::PluginRegistryOperation;
    use crate::runtime_extensions::ExtensionBrowser;
    use nana_ui::runtime::{AppContext, DocumentId, Entity, LayoutViewport, Stack, TextInput};
    use std::sync::{Arc, Mutex};

    let mut module = ExtensionsModule::default();
    refresh(&mut module, true);
    reduce(&mut module, ExtensionsModuleMessage::SetBusy(true));
    let doc = DocumentId::new(798).unwrap();
    let mut cx = AppContext::new();
    let root = cx.create_component(doc, Stack::fill_column(0.0)).unwrap();
    let messages = Arc::new(Mutex::new(Vec::new()));
    let pending = messages.clone();
    let sink: Arc<dyn Fn(Intent) + Send + Sync> =
        Arc::new(move |intent| pending.lock().unwrap().push(intent));
    let mut browser = ExtensionBrowser::mount(&mut cx, doc, sink.clone()).unwrap();
    cx.append_child(root, browser.root).unwrap();
    browser
        .sync(
            &mut cx,
            doc,
            &extension_browser(&module, "plugin-packages"),
            960.0,
            sink.clone(),
        )
        .unwrap();
    cx.layout_document(doc, LayoutViewport::new(960.0, 760.0))
        .unwrap();
    let source = browser
        .debug_nodes()
        .into_iter()
        .find(|(id, _)| id == "plugin-source")
        .unwrap()
        .1;
    assert!(cx.focus_node(doc, source).unwrap());
    cx.select_all_focused_text(doc).unwrap();
    let path = "/tmp/native-plugin-source";
    assert!(cx.replace_focused_text(doc, path).unwrap());
    for intent in messages.lock().unwrap().drain(..) {
        let Intent::ExtensionsCommand(message) = intent else {
            panic!("extension input event");
        };
        ui(&mut module, message);
    }
    assert_eq!(module.plugin_source_input(), path);
    assert!(
        module.take_pending_submit().is_none(),
        "editing a draft submits no authority command"
    );
    let during_refresh = extension_browser(&module, "plugin-packages");
    assert!(during_refresh.toolbar.iter().any(|control| matches!(control, Control::Action { id, enabled: false, .. } if id == "plugin-install")));
    browser
        .sync(&mut cx, doc, &during_refresh, 960.0, sink.clone())
        .unwrap();
    assert_eq!(
        cx.read(Entity::<TextInput>::from_stable_id(source), |input| input
            .state
            .value
            .clone())
            .unwrap(),
        path
    );

    refresh(&mut module, true);
    reduce(&mut module, ExtensionsModuleMessage::SetBusy(false));
    let ready = extension_browser(&module, "plugin-packages");
    assert!(ready.toolbar.iter().any(|control| matches!(control, Control::Action { id, enabled: true, .. } if id == "plugin-install")));
    browser.sync(&mut cx, doc, &ready, 960.0, sink).unwrap();
    assert_eq!(
        cx.read(Entity::<TextInput>::from_stable_id(source), |input| input
            .state
            .value
            .clone())
            .unwrap(),
        path
    );
    let install = browser
        .debug_nodes()
        .into_iter()
        .find(|(id, _)| id == "plugin-install")
        .unwrap()
        .1;
    assert!(cx.activate_node(install).unwrap());
    for intent in messages.lock().unwrap().drain(..) {
        let Intent::ExtensionsCommand(message) = intent else {
            panic!("extension install event");
        };
        ui(&mut module, message);
    }
    let Some(ExtensionsCommand::Plugin(PluginRegistryOperation::Install(request))) =
        module.take_pending_submit()
    else {
        panic!("normal install activation must submit the authoritative operation");
    };
    assert_eq!(request.source_path, path);
    assert_eq!(request.expected_registry_revision, 11);
}
