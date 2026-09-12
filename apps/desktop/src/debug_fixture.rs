use crate::application::{
    ArchitectureBackend, ArchitecturePermission, DesktopApplication, ProjectArchitectureApplyInput,
    ProjectArchitectureChange, ProjectArchitectureEdge, ProjectArchitectureNode,
    RemoteEndpointAddress, RemotePairDeviceInput,
};
use lilia_agent::ProductCredentialLoginInput;
use lilia_contracts::{
    AgentSessionRef, ExpectedRevision, ProductEntity, ProjectId, ProjectionEventId, TaskId,
    TimelineProjectionCommand, TimelineProjectionEvent,
};
use lilia_storage::{
    mcp_registry_path, save_mcp_registry, skills_registry_path, AgentkitMcpRegistry,
    AgentkitMcpRegistryEntry, AgentkitSkillPackageRef, AgentkitSkillsRegistry,
};
use mutsuki_agent_contracts::{
    CredentialKind, InteractionKind, InteractionRequest, OPENAI_CREDENTIAL_PROVIDER_ID,
};
use serde_json::json;

pub const PROJECT_ID: &str = "native-agent-debug-project";
pub const TASK_ID: &str = "native-agent-debug-task";
pub const PLAN_REPLAY_TASK_ID: &str = "native-agent-debug-plan-replay-task";
pub const PLAN_CANCEL_TASK_ID: &str = "native-agent-debug-plan-cancel-task";
pub const QUESTION_REPLAY_TASK_ID: &str = "native-agent-debug-question-replay-task";
pub const MCP_ELICITATION_TASK_ID: &str = "native-agent-debug-mcp-elicitation-task";
pub const ARCHITECTURE_APPROVAL_TASK_ID: &str = "native-agent-debug-architecture-approval-task";

pub fn prepare(application: &DesktopApplication) -> Result<(), String> {
    if std::env::var("LILIA_AGENT_DEBUG_SEED").as_deref() != Ok("1") {
        return Ok(());
    }
    if std::env::var("LILIA_AGENT_DEBUG_RESUME").as_deref() == Ok("1") {
        return prepare_model_credential(application);
    }
    if std::env::var("LILIA_AGENT_DEBUG_PAGINATION").as_deref() == Ok("1") {
        prepare_pagination(application)?;
    }
    let project_id = ProjectId::new(PROJECT_ID).map_err(|error| error.to_string())?;
    let task_id = TaskId::new(TASK_ID).map_err(|error| error.to_string())?;
    let client = application
        .authority()
        .client()
        .map_err(|error| error.to_string())?;
    let mut project = match client.products().get_project(&project_id) {
        Ok(project) => project,
        Err(_) => client
            .create_project(project_id.clone(), "Native Agent Debug")
            .map_err(|error| error.to_string())?,
    };
    if let Ok(workspace) = std::env::var("LILIA_AGENT_DEBUG_WORKSPACE") {
        if project.workspace_path.as_deref() != Some(workspace.as_str()) {
            let expected =
                ExpectedRevision::new(project.revision.get()).map_err(|error| error.to_string())?;
            project.workspace_path = Some(workspace);
            match client
                .products()
                .update_entity(ProductEntity::Project(project), expected)
                .map_err(|error| error.to_string())?
            {
                ProductEntity::Project(_) => {}
                _ => return Err("debug project update returned a non-project entity".to_owned()),
            }
        }
    }
    if client.products().get_task(&task_id).is_err() {
        client
            .create_task(
                task_id.clone(),
                Some(project_id.clone()),
                "验证 Native Composer 与时间线",
            )
            .map_err(|error| error.to_string())?;
    }
    application
        .authority()
        .apply_projection(TimelineProjectionCommand::UpsertTimelineEvent {
            event: TimelineProjectionEvent {
                id: ProjectionEventId::new("native-agent-debug-markdown-image"),
                task_id: task_id.clone(),
                agent_session: AgentSessionRef::new("native-agent-debug-media-session")
                    .map_err(|error| error.to_string())?,
                sequence: 1,
                turn_id: Some("native-agent-debug-media-turn".to_owned()),
                kind: "message".to_owned(),
                status: "completed".to_owned(),
                title: "图片回复".to_owned(),
                summary: None,
                payload: json!({
                    "role": "assistant",
                    "content": concat!(
                        "Native Markdown 图片\n\n",
                        "![Native 图片](data:image/png;base64,",
                        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=)"
                    )
                }),
                projected: true,
            },
        })
        .map_err(|error| error.to_string())?;
    for (id, title) in [
        (PLAN_REPLAY_TASK_ID, "验证 Native 计划重启回放"),
        (PLAN_CANCEL_TASK_ID, "验证 Native 计划取消"),
        (QUESTION_REPLAY_TASK_ID, "验证 Native 提问重启回放"),
        (MCP_ELICITATION_TASK_ID, "验证 Native MCP 表单交互"),
        (ARCHITECTURE_APPROVAL_TASK_ID, "验证 Native 架构审批"),
    ] {
        let seeded_task_id = TaskId::new(id).map_err(|error| error.to_string())?;
        if client.products().get_task(&seeded_task_id).is_err() {
            client
                .products()
                .create_task(seeded_task_id, Some(project_id.clone()), title)
                .map_err(|error| error.to_string())?;
        }
    }
    if application
        .project_architecture(&project_id)
        .map_err(|error| error.to_string())?
        .version
        == 0
    {
        let node = |id: &str, label: &str, path: &str| ProjectArchitectureNode {
            id: id.to_owned(),
            label: label.to_owned(),
            node_type: "module".to_owned(),
            summary: format!("{label} 的原生实现边界"),
            paths: vec![path.to_owned()],
            tags: vec!["native".to_owned()],
        };
        application
            .apply_project_architecture(ProjectArchitectureApplyInput {
                project_id: project_id.as_str().to_owned(),
                task_id: task_id.as_str().to_owned(),
                turn_id: Some("native-agent-debug-architecture".to_owned()),
                backend: ArchitectureBackend::NativeAgentkit,
                permission: ArchitecturePermission::Full,
                reason: "初始化原生架构回放".to_owned(),
                changes: vec![
                    ProjectArchitectureChange::UpsertNode {
                        node: node("native-ui", "NanaUI Workspace", "apps/desktop"),
                    },
                    ProjectArchitectureChange::UpsertNode {
                        node: node("desktop-application", "DesktopApplication", "apps/desktop"),
                    },
                    ProjectArchitectureChange::UpsertEdge {
                        edge: ProjectArchitectureEdge {
                            id: "native-ui-application".to_owned(),
                            from: "native-ui".to_owned(),
                            to: "desktop-application".to_owned(),
                            edge_type: "uses".to_owned(),
                            label: "typed intent".to_owned(),
                            summary: "UI 只通过类型化应用合同访问产品事实".to_owned(),
                        },
                    },
                    ProjectArchitectureChange::SetSummary {
                        summary: "Native UI 与产品服务保持单向依赖。".to_owned(),
                    },
                ],
                request_id: Some("native-agent-debug-architecture-v1".to_owned()),
                expected_version: Some(0),
            })
            .map_err(|error| error.to_string())?;
    }
    seed_corrupt_architecture_snapshot(application, &project_id)?;
    seed_usage(application, &project_id)?;

    prepare_model_credential(application)?;
    let runtime = application.authority().shared_runtime();
    let mcp_task_id = TaskId::new(MCP_ELICITATION_TASK_ID).map_err(|error| error.to_string())?;
    runtime
        .inner()
        .seed_debug_interaction(
            &mcp_task_id,
            "native-agent-debug-mcp-session",
            "native-agent-debug-mcp-turn",
            InteractionRequest {
                session_id: "native-agent-debug-mcp-session".to_owned(),
                turn_id: "native-agent-debug-mcp-turn".to_owned(),
                version: 1,
                interaction_id: "native-agent-debug-mcp-request".to_owned(),
                kind: InteractionKind::Custom,
                source_tool: None,
                permission_mode: mutsuki_agent_contracts::AgentPermissionMode::Ask,
                prompt: "为 Native 发布填写 MCP 交接信息".to_owned(),
                options: json!({
                    "interaction": "mcp_elicitation",
                    "threadId": MCP_ELICITATION_TASK_ID,
                    "turnId": "native-agent-debug-mcp-turn",
                    "serverName": "native-debug-mcp",
                    "mode": "form",
                    "message": "选择项目与评审人，并补充发布摘要。",
                    "requestedSchema": {
                        "type": "object",
                        "required": ["project", "reviewers", "summary"],
                        "properties": {
                            "confirm": {
                                "type": "boolean",
                                "title": "确认同步",
                                "default": false
                            },
                            "project": {
                                "type": "string",
                                "title": "项目",
                                "enum": ["Alpha", "Beta"]
                            },
                            "reviewers": {
                                "type": "array",
                                "title": "评审人",
                                "items": {
                                    "anyOf": [
                                        {"const": "alice", "title": "Alice"},
                                        {"const": "bob", "title": "Bob"}
                                    ]
                                }
                            },
                            "summary": {
                                "type": "string",
                                "title": "发布摘要"
                            },
                            "tags": {
                                "type": "array",
                                "title": "标签"
                            }
                        }
                    },
                    "elicitationId": "native-agent-debug-mcp-elicitation",
                    "_meta": {"source": "native-agent-debug"}
                }),
                context: None,
                details: None,
            },
        )
        .map_err(|error| error.to_string())?;
    seed_extensions(application)?;
    seed_remote_control(application)?;
    Ok(())
}

fn prepare_model_credential(application: &DesktopApplication) -> Result<(), String> {
    let endpoint = std::env::var("LILIA_AGENT_DEBUG_MODEL_ENDPOINT")
        .map_err(|_| "LILIA_AGENT_DEBUG_MODEL_ENDPOINT is required for seeded replay")?;
    let runtime = application.authority().shared_runtime();
    if runtime
        .inner()
        .credentials()
        .primary_usable_credential()
        .is_none()
    {
        runtime
            .inner()
            .credentials()
            .login(ProductCredentialLoginInput {
                provider_id: OPENAI_CREDENTIAL_PROVIDER_ID.into(),
                kind: CredentialKind::ApiKey,
                secret_material: "sk-native-agent-debug-fixture".into(),
                account_label: Some("Native Agent Debug".into()),
                source: Some("agent_debug_fixture".into()),
            })
            .map_err(|error| error.to_string())?;
    }
    runtime.inner().set_model_endpoint_override(Some(endpoint));
    runtime
        .inner()
        .refresh_product_profile(None)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn seed_usage(application: &DesktopApplication, primary_project: &ProjectId) -> Result<(), String> {
    let client = application
        .authority()
        .client()
        .map_err(|error| error.to_string())?;
    let secondary_project =
        ProjectId::new("native-agent-debug-usage-project").map_err(|error| error.to_string())?;
    if client.products().get_project(&secondary_project).is_err() {
        client
            .create_project(secondary_project.clone(), "用量图表验收")
            .map_err(|error| error.to_string())?;
    }
    let today = lilia_feature_usage::day_start(lilia_feature_usage::now_millis());
    for index in 0..3_i64 {
        let task_id = TaskId::new(format!("native-agent-debug-usage-task-{index}"))
            .map_err(|error| error.to_string())?;
        if client.products().get_task(&task_id).is_err() {
            client
                .create_task(
                    task_id.clone(),
                    Some(if index == 0 {
                        primary_project.clone()
                    } else {
                        secondary_project.clone()
                    }),
                    format!("用量会话 {}", index + 1),
                )
                .map_err(|error| error.to_string())?;
        }
        for day in 0..7_i64 {
            let factor = (index + 1) * (day + 1);
            let created_at = today - day * 86_400_000 + 12 * 3_600_000;
            for kind in ["usage", "tool"] {
                let payload = if kind == "usage" {
                    json!({"inputTokens": 1000 * factor, "outputTokens": 200 * factor,
                        "cacheReadTokens": 300 * factor, "cacheCreationTokens": 100 * factor,
                        "totalTokens": 1600 * factor, "createdAt": created_at})
                } else {
                    json!({"toolName": (["read_file", "apply_patch", "run_tests"][index as usize]),
                        "createdAt": created_at})
                };
                application
                    .authority()
                    .apply_projection(TimelineProjectionCommand::UpsertTimelineEvent {
                        event: TimelineProjectionEvent {
                            id: ProjectionEventId::new(format!(
                                "native-agent-debug-{kind}-{index}-{day}"
                            )),
                            task_id: task_id.clone(),
                            agent_session: AgentSessionRef::new(format!(
                                "native-agent-debug-usage-{index}"
                            ))
                            .map_err(|error| error.to_string())?,
                            sequence: day as u64 * 2 + if kind == "usage" { 1 } else { 2 },
                            turn_id: Some(format!("native-agent-debug-usage-{index}-{day}")),
                            kind: kind.to_owned(),
                            status: "completed".to_owned(),
                            title: if kind == "usage" {
                                "用量"
                            } else {
                                "工具调用"
                            }
                            .to_owned(),
                            summary: None,
                            payload,
                            projected: true,
                        },
                    })
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn seed_corrupt_architecture_snapshot(
    application: &DesktopApplication,
    project_id: &ProjectId,
) -> Result<(), String> {
    if std::env::var("LILIA_AGENT_DEBUG_CORRUPT_ARCHITECTURE").as_deref() != Ok("1") {
        return Ok(());
    }
    let connection = rusqlite::Connection::open(application.config().domain_database_path())
        .map_err(|error| format!("open debug architecture database: {error}"))?;
    let changed = connection
        .execute(
            r#"UPDATE project_architecture_graphs
               SET version = 99, graph_json = '{broken', updated_at = 99
               WHERE project_id = ?1"#,
            rusqlite::params![project_id.as_str()],
        )
        .map_err(|error| format!("corrupt debug architecture snapshot: {error}"))?;
    if changed == 1 {
        Ok(())
    } else {
        Err("debug architecture snapshot was not seeded before corruption".to_owned())
    }
}

fn seed_remote_control(application: &DesktopApplication) -> Result<(), String> {
    let status = application
        .set_remote_control_enabled(true)
        .map_err(|error| error.to_string())?;
    if status
        .trusted_devices
        .iter()
        .any(|device| device.endpoint_id == "native-agent-debug-android" && device.trusted)
    {
        return Ok(());
    }
    let ticket = application
        .start_remote_pairing()
        .map_err(|error| error.to_string())?;
    application
        .pair_remote_device(RemotePairDeviceInput {
            ticket_id: ticket.id,
            challenge: ticket.challenge,
            device_name: "Agent Debug Android".to_owned(),
            android_endpoint: RemoteEndpointAddress {
                endpoint_id: "native-agent-debug-android".to_owned(),
                relay_url: None,
                direct_addresses: Vec::new(),
            },
            protocol_version: 1,
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn seed_extensions(application: &DesktopApplication) -> Result<(), String> {
    let paths = application.config().data_paths();
    let mcp_path = mcp_registry_path(&paths);
    let skills_path = skills_registry_path(&paths);
    let fixture_root = application.config().home().join("agent-debug-fixtures");
    let skill_root = application.config().home().join("skills");
    let skill_path = skill_root.join("native-debug-skill");
    std::fs::create_dir_all(&skill_path).map_err(|error| error.to_string())?;
    std::fs::write(skill_path.join("SKILL.md"),
        "---\nname: native-debug-skill\ndescription: Local Native UI acceptance fixture\n---\nDescribe the selected code. Do not invoke tools.\n")
        .map_err(|error| error.to_string())?;
    let plugin_path = fixture_root.join("empty-plugin");
    let plugin_skill = plugin_path.join("skills/native-ui-fixture");
    std::fs::create_dir_all(&plugin_skill).map_err(|error| error.to_string())?;
    std::fs::write(plugin_skill.join("SKILL.md"),
        "---\nname: native-ui-fixture\ndescription: Inert local acceptance documentation\n---\nThis package contains only local documentation.\n")
        .map_err(|error| error.to_string())?;
    std::fs::write(plugin_path.join("lilia-plugin.json"), serde_json::to_vec_pretty(&json!({
        "schemaVersion": 1, "pluginId": "native-debug-empty-plugin", "name": "Native Local Fixture",
        "pluginVersion": "1.0.0", "description": "Local package without executable contributions",
        "contributions": { "skills": ["skills/native-ui-fixture"] }
    })).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    lilia_storage::load_plugin_manifest(&plugin_path)
        .map_err(|error| format!("validate local debug Plugin: {error}"))?;
    let hooks_path = lilia_storage::user_hooks_document_path(&paths);
    if !hooks_path.exists() {
        lilia_storage::save_hooks_document(
            &hooks_path,
            &lilia_storage::AgentkitHooksDocument {
                version: 1,
                revision: 0,
                enabled: false,
                handlers: vec![lilia_storage::AgentkitHookHandler {
                    id: "native-debug-disabled-hook".to_owned(),
                    event: lilia_feature_hooks::HookEvent::UserPromptSubmit
                        .as_str()
                        .to_owned(),
                    matcher: None,
                    handler_type: "command".to_owned(),
                    command: Some("native-debug-disabled-hook".to_owned()),
                    command_windows: None,
                    timeout_seconds: Some(10),
                    status_message: None,
                }],
            },
        )
        .map_err(|error| format!("write disabled debug Hooks: {error}"))?;
    }
    if !mcp_path.exists() {
        let registry = AgentkitMcpRegistry {
            version: 1,
            revision: 1,
            secret_free: true,
            servers: vec![AgentkitMcpRegistryEntry {
                server_id: "native-debug-invalid".to_owned(),
                source: "agent-debug".to_owned(),
                transport: "unsupported".to_owned(),
                command: None,
                args: Vec::new(),
                env_allowlist: Vec::new(),
                env_secret_names: Vec::new(),
                url: None,
                header_secret_names: Vec::new(),
                registered_from: "agent-debug".to_owned(),
                enabled: true,
            }],
        };
        save_mcp_registry(&paths, &registry)
            .map_err(|error| format!("write debug MCP registry: {error}"))?;
    }
    if !skills_path.exists() {
        let registry = AgentkitSkillsRegistry {
            version: 1,
            revision: 0,
            secret_free: true,
            user_skill_roots: vec![skill_root.display().to_string()],
            packages: vec![AgentkitSkillPackageRef {
                skill_id: "native-debug-skill".to_owned(),
                path: skill_path.display().to_string(),
                registered_from: "lilia.desktop.skill-manager".to_owned(),
                scope: "user".to_owned(),
                description: "Native debug Skill".to_owned(),
                enabled: true,
            }],
        };
        lilia_storage::save_skills_registry(&paths, &registry)
            .map_err(|error| format!("write debug Skills registry: {error}"))?;
    }
    Ok(())
}

fn prepare_pagination(application: &DesktopApplication) -> Result<(), String> {
    let client = application
        .authority()
        .client()
        .map_err(|error| error.to_string())?;
    let project_id =
        ProjectId::new("native-agent-debug-zz-pagination").map_err(|error| error.to_string())?;
    if client.products().get_project(&project_id).is_err() {
        let mut project = lilia_contracts::Project::new(project_id.clone(), "ZZ 会话分页验收")
            .map_err(|error| error.to_string())?;
        project.sort_order = 10_000;
        client
            .products()
            .create_entity(ProductEntity::Project(project))
            .map_err(|error| error.to_string())?;
    }
    for index in 1..=105 {
        let task_id = TaskId::new(format!("native-pagination-{index:03}"))
            .map_err(|error| error.to_string())?;
        if client.products().get_task(&task_id).is_err() {
            let mut task = lilia_contracts::ProductTask::new(
                task_id,
                Some(project_id.clone()),
                format!("分页会话 {index:03}"),
            )
            .map_err(|error| error.to_string())?;
            task.sort_order = index;
            client
                .products()
                .create_entity(ProductEntity::Task(task))
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
