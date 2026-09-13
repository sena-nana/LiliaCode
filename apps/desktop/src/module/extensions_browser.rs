use crate::module::extensions::{
    ExtensionsModule, mcp_credential_draft_key, parse_hook_handlers_draft,
};
use crate::runtime_extensions::ExtensionBrowserSnapshot;
use crate::runtime_shell::ShellIntent;
use crate::runtime_surface::SurfaceControl;
use crate::shell::{ExtensionsMessage, HookHandlerDraftField};

pub(crate) fn snapshot(module: &ExtensionsModule, tab: &str) -> ExtensionBrowserSnapshot {
    use crate::module::extensions::ExtensionEditor;
    let mut toolbar = Vec::new();
    append_extension_panel(module, tab, &mut toolbar, ExtensionPart::Toolbar);
    if tab == "extensions" {
        toolbar.push(
            SurfaceControl::action(
                "skill-create-open",
                "新建技能",
                ShellIntent::ExtensionsCommand(ExtensionsMessage::OpenSkillEditor),
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
    detail.retain(|control| !matches!(control, SurfaceControl::Section { .. }));
    let (mut detail_actions, mut detail): (Vec<_>, Vec<_>) =
        detail.into_iter().partition(|control| {
            matches!(
                control,
                SurfaceControl::Action {
                    intent: ShellIntent::ExtensionsCommand(
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
                } | SurfaceControl::Action {
                    intent: ShellIntent::OpenPath(_),
                    ..
                }
            )
        });
    if selected.is_none() {
        detail.push(SurfaceControl::text(
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
            controls.push(SurfaceControl::field(
                "skill_id",
                "名称",
                name,
                false,
                |value| ShellIntent::ExtensionsCommand(ExtensionsMessage::SkillIdChanged(value)),
            ));
            controls.push(SurfaceControl::field(
                "skill_description",
                "描述",
                description,
                true,
                |value| {
                    ShellIntent::ExtensionsCommand(ExtensionsMessage::SkillDescriptionChanged(
                        value,
                    ))
                },
            ));
            controls.push(
                SurfaceControl::action(
                    "create-skill",
                    "创建技能",
                    ShellIntent::ExtensionsCommand(ExtensionsMessage::CreateSkill),
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
            controls.push(SurfaceControl::text(
                "extension-editor-readonly",
                "此来源已不可编辑。草稿已保留，可以取消后查看最新配置。",
            ));
        }
        let (mut actions, controls): (Vec<_>, Vec<_>) = controls.into_iter().partition(|control| {
            matches!(
                control,
                SurfaceControl::Action {
                    intent: ShellIntent::ExtensionsCommand(
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
                SurfaceControl::Action {
                    intent: ShellIntent::ExtensionsCommand(ExtensionsMessage::CancelMcpEditor),
                    ..
                }
            )
        });
        actions.insert(
            0,
            SurfaceControl::action(
                "extension-editor-cancel",
                "取消",
                ShellIntent::ExtensionsCommand(ExtensionsMessage::CancelEditor),
            ),
        );
        if !can_save {
            for action in actions.iter_mut().skip(1) {
                if let SurfaceControl::Action { enabled, .. } = action {
                    *enabled = false;
                }
            }
        }
        if module.busy() {
            for action in &mut actions {
                if let SurfaceControl::Action { enabled, .. } = action {
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
            if let SurfaceControl::Action { enabled, .. } = action {
                *enabled = false;
            }
        }
    }
    if module.busy() {
        for action in &mut detail_actions {
            if let SurfaceControl::Action { enabled, .. } = action {
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
fn append_hook_details(
    source: &crate::application::DesktopHookSourceView,
    document: Option<&crate::application::DesktopHookDocumentView>,
    rows: &mut Vec<SurfaceControl>,
) {
    let trust = match source.trust_state.as_str() {
        "managed" => "已允许执行",
        "required" => "尚未允许执行",
        _ => "尚未创建配置",
    };
    rows.push(SurfaceControl::text(
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
            rows.push(SurfaceControl::text(
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
            rows.push(SurfaceControl::text(
                format!("hook-document-{}", source.id),
                format!("配置原文\n{raw}"),
            ));
        }
    }
    for (index, notice) in notices.into_iter().enumerate() {
        rows.push(SurfaceControl::text(
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
fn append_extension_panel(
    module: &crate::module::extensions::ExtensionsModule,
    tab: &str,
    rows: &mut Vec<SurfaceControl>,
    part: ExtensionPart,
) {
    let toolbar = matches!(part, ExtensionPart::All | ExtensionPart::Toolbar);
    let detail = matches!(part, ExtensionPart::All | ExtensionPart::Detail);
    let editor = matches!(part, ExtensionPart::All | ExtensionPart::Editor);
    let selected = module.selected_entry(tab).map(|entry| entry.key);
    let action = |message| ShellIntent::ExtensionsCommand(message);
    if toolbar {
        rows.push(SurfaceControl::action(
            "extensions-refresh",
            "刷新",
            action(ExtensionsMessage::Refresh),
        ));
    }
    if let Some(error) = module.error() {
        rows.push(SurfaceControl::text("extensions-error", error));
    }
    if part == ExtensionPart::All
        && matches!(
            tab,
            "extensions" | "plugin-packages" | "plugin-hooks" | "plugin-mcp"
        )
    {
        rows.push(SurfaceControl::field(
            "extensions-search",
            "搜索",
            module.query(),
            false,
            |value| ShellIntent::ExtensionsCommand(ExtensionsMessage::SearchChanged(value)),
        ));
    }
    if tab == "extensions" {
        if editor {
            rows.push(SurfaceControl::toggle(
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
                    rows.push(SurfaceControl::text(
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
                        rows.push(SurfaceControl::text(
                            format!("skill-readonly-{}", skill.skill_id),
                            if skill.scope == "plugin" {
                                "请在所属插件中管理此技能。"
                            } else {
                                "此来源的技能为只读。"
                            },
                        ));
                    }
                    if skill.editable {
                        rows.push(SurfaceControl::action(
                            format!("skill-{}", skill.skill_id),
                            if skill.enabled {
                                "停用技能"
                            } else {
                                "启用技能"
                            },
                            action(ExtensionsMessage::ToggleSkill(skill.skill_id.clone())),
                        ));
                        rows.push(SurfaceControl::action(
                            format!("skill-open-{}", skill.skill_id),
                            "打开技能文件",
                            ShellIntent::OpenPath(skill.path.clone()),
                        ));
                        rows.push(
                            SurfaceControl::action(
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
            rows.push(SurfaceControl::field(
                "plugin-source",
                "插件目录",
                module.plugin_source_input(),
                false,
                |v| ShellIntent::ExtensionsCommand(ExtensionsMessage::PluginSourceChanged(v)),
            ));
            rows.push(SurfaceControl::action(
                "plugin-pick",
                "选择插件目录",
                action(ExtensionsMessage::PickPluginDirectory),
            ));
            rows.push(
                SurfaceControl::action(
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
                    rows.push(SurfaceControl::section(
                        format!("plugin-section-{}", plugin.plugin_id),
                        &plugin.name,
                    ));
                    rows.push(SurfaceControl::text(
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
                        SurfaceControl::action(
                            format!("plugin-toggle-{}", plugin.plugin_id),
                            if plugin.enabled { "停用" } else { "启用" },
                            action(ExtensionsMessage::TogglePlugin(plugin.plugin_id.clone())),
                        )
                        .enabled(plugin.editable),
                    );
                    if !plugin.editable {
                        rows.push(SurfaceControl::text(
                            format!("plugin-readonly-{}", plugin.plugin_id),
                            "此来源的插件为只读。",
                        ));
                    }
                    if plugin.editable {
                        rows.push(
                            SurfaceControl::action(
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
                        rows.push(SurfaceControl::text(
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
                    rows.push(SurfaceControl::section(
                        format!("hook-section-{}", source.id),
                        if source.scope == crate::application::DesktopHookScope::User {
                            "用户 Hooks"
                        } else {
                            "项目 Hooks"
                        },
                    ));
                    rows.push(SurfaceControl::text(
                        format!("hook-source-{}", source.id),
                        format!("{}\n{}", source.id, source.path),
                    ));
                    append_hook_details(source, module.hook_document(&source.id), rows);
                }
                if !source.editable {
                    continue;
                }
                if !source.exists {
                    rows.push(SurfaceControl::action(
                        format!("hook-create-{}", source.id),
                        "创建配置",
                        action(ExtensionsMessage::CreateHookSource(source.id.clone())),
                    ));
                    continue;
                }
                if detail {
                    rows.push(SurfaceControl::action(
                        format!("hook-toggle-{}", source.id),
                        if source.enabled { "停用" } else { "启用" },
                        action(ExtensionsMessage::ToggleHookSource(source.id.clone())),
                    ));
                    rows.push(SurfaceControl::action(
                        format!("hook-edit-{}", source.id),
                        "编辑配置",
                        action(ExtensionsMessage::OpenHookEditor(source.id.clone())),
                    ));
                    rows.push(
                        SurfaceControl::action(
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
                                rows.push(SurfaceControl::field(
                                    format!("hook-{}-{index}-{}", source.id, field.target_key()),
                                    label,
                                    value,
                                    matches!(
                                        field,
                                        HookHandlerDraftField::Command
                                            | HookHandlerDraftField::CommandWindows
                                    ),
                                    move |value| {
                                        ShellIntent::ExtensionsCommand(
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
                                SurfaceControl::action(
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
                rows.push(SurfaceControl::action(
                    format!("hook-add-{}", source.id),
                    "添加处理器",
                    action(ExtensionsMessage::AddHookHandler(source.id.clone())),
                ));
                rows.push(SurfaceControl::action(
                    format!("hook-save-{}", source.id),
                    "保存配置",
                    action(ExtensionsMessage::SaveHookSource(source.id.clone())),
                ));
            }
        }
    }
    if tab == "plugin-mcp" {
        if toolbar {
            rows.push(SurfaceControl::action(
                "mcp-new",
                "添加 MCP 服务",
                action(ExtensionsMessage::NewMcpServer),
            ));
            rows.push(SurfaceControl::action(
                "mcp-activate",
                "连接已启用服务",
                action(ExtensionsMessage::ActivateRegisteredMcp),
            ));
        }
        if editor {
            if let Some(editor) = module.mcp_editor() {
                if let Some(id) = &editor.editing_server_id {
                    rows.push(SurfaceControl::text(
                        "mcp-id-readonly",
                        format!("服务名称：{id}"),
                    ));
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
                    rows.push(SurfaceControl::field(id, label, value, false, move |v| {
                        ShellIntent::ExtensionsCommand(constructor(v))
                    }));
                }
                rows.push(SurfaceControl::choice(
                    "mcp-transport",
                    "传输方式",
                    editor.transport.as_registry(),
                    options(&[
                        ("stdio", "本地命令"),
                        ("streamable_http", "HTTP"),
                        ("sse", "SSE"),
                    ]),
                    |value| {
                        ShellIntent::ExtensionsCommand(ExtensionsMessage::McpTransportChanged(
                            value,
                        ))
                    },
                ));
                rows.push(SurfaceControl::toggle(
                    "mcp-enabled",
                    "启用服务",
                    editor.enabled,
                    action(ExtensionsMessage::ToggleMcpEditorEnabled),
                ));
                rows.push(SurfaceControl::action(
                    "mcp-save",
                    "保存服务",
                    action(ExtensionsMessage::SaveMcpServer),
                ));
                rows.push(SurfaceControl::action(
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
                    rows.push(SurfaceControl::section(
                        format!("mcp-section-{}", server.server_id),
                        &server.server_id,
                    ));
                    rows.push(SurfaceControl::text(
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
                        rows.push(SurfaceControl::action(
                            format!("mcp-edit-{}", server.server_id),
                            "编辑",
                            action(ExtensionsMessage::EditMcpServer(server.server_id.clone())),
                        ));
                        rows.push(SurfaceControl::action(
                            format!("mcp-toggle-{}", server.server_id),
                            if server.enabled { "停用" } else { "启用" },
                            action(ExtensionsMessage::ToggleMcpServer(server.server_id.clone())),
                        ));
                        rows.push(
                            SurfaceControl::action(
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
                        rows.push(SurfaceControl::secret(
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
                                ShellIntent::ExtensionsCommand(
                                    ExtensionsMessage::McpCredentialChanged {
                                        server_id: server_id.clone(),
                                        kind,
                                        name: name.clone(),
                                        value,
                                    },
                                )
                            },
                        ));
                        rows.push(SurfaceControl::action(
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
                                SurfaceControl::action(
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
                        rows.push(SurfaceControl::field(
                            format!("mcp-prompt-{name}"),
                            format!("{}参数", prompt.name),
                            module.prompt_draft(&name),
                            true,
                            move |value| {
                                ShellIntent::ExtensionsCommand(
                                    ExtensionsMessage::McpPromptArgumentsChanged {
                                        namespaced_name: name.clone(),
                                        value,
                                    },
                                )
                            },
                        ));
                        rows.push(SurfaceControl::action(
                            format!("mcp-get-{}", prompt.namespaced_name),
                            format!("获取 {}", prompt.name),
                            action(ExtensionsMessage::GetMcpPrompt(
                                prompt.namespaced_name.clone(),
                            )),
                        ));
                    }
                    for resource in &server.resources {
                        rows.push(SurfaceControl::action(
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
                rows.push(SurfaceControl::text(
                    "mcp-preview",
                    format!("{}\n{}", preview.title, preview.text),
                ));
            }
        }
    }
}
fn options(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(id, label)| ((*id).into(), (*label).into()))
        .collect()
}
