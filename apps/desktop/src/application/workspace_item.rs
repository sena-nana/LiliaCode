use lilia_contracts::{
    BrowserTabRestoration, ProductError, ProductTask, Project, ProjectArchiveState, ProjectId,
    TaskId,
};

use crate::application::{
    DesktopApplication, DesktopApplicationError, DocumentId, DocumentSnapshot, WorkspaceItemId,
    document_resource_key, path_from_document_resource_key,
};

pub use lilia_feature_workspace::{
    ARCHITECTURE_WORKSPACE_ITEM_KIND, AUTOMATION_WORKSPACE_ITEM_KIND, ApplicationWorkspaceSurface,
    DOCUMENT_WORKSPACE_ITEM_KIND, MEMORY_WORKSPACE_ITEM_KIND, PROJECT_FILES_WORKSPACE_ITEM_KIND,
    ProjectWorkspaceSurface, ROADMAP_WORKSPACE_ITEM_KIND, SETTINGS_WORKSPACE_ITEM_KIND,
    TASK_WORKSPACE_ITEM_KIND, TERMINAL_WORKSPACE_ITEM_KIND, WorkspaceFocusTarget, WorkspaceItem,
    WorkspaceItemCapabilities, WorkspaceItemError, WorkspaceItemKind, WorkspaceItemRestoration,
    WorkspaceResourceId,
};

pub const BROWSER_WORKSPACE_ITEM_KIND: &str = "task-browser";

pub fn browser_tab_restoration(
    restoration: &WorkspaceItemRestoration,
) -> Result<Option<BrowserTabRestoration>, WorkspaceItemError> {
    if restoration.kind.as_str() != BROWSER_WORKSPACE_ITEM_KIND {
        return Ok(None);
    }
    let invalid = || WorkspaceItemError::InvalidRestorationIdentity {
        item_id: restoration.id.as_str().into(),
        kind: BROWSER_WORKSPACE_ITEM_KIND.into(),
    };
    let state: BrowserTabRestoration =
        serde_json::from_value(restoration.serialized_state.clone().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
    let base = format!("browser:{}", state.scope.task_id.as_str());
    let valid_tab = state.scope.tab_id == base
        || state
            .scope
            .tab_id
            .strip_prefix(&format!("{base}:"))
            .is_some_and(|suffix| uuid::Uuid::parse_str(suffix).is_ok());
    if !valid_tab
        || restoration.id.as_str() != state.scope.tab_id
        || restoration
            .resource_id
            .as_ref()
            .map(WorkspaceResourceId::as_str)
            != Some(state.scope.tab_id.as_str())
        || !(state.url == "about:blank"
            || state.url.starts_with("http://")
            || state.url.starts_with("https://"))
    {
        return Err(invalid());
    }
    Ok(Some(state))
}

pub fn browser_workspace_item(
    state: &BrowserTabRestoration,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    let item = WorkspaceItem::new(
        WorkspaceItemId::new(state.scope.tab_id.clone()).map_err(|_| {
            WorkspaceItemError::InvalidRestorationIdentity {
                item_id: state.scope.tab_id.clone(),
                kind: BROWSER_WORKSPACE_ITEM_KIND.into(),
            }
        })?,
        WorkspaceResourceId::new(state.scope.tab_id.clone())?,
        WorkspaceItemKind::new(BROWSER_WORKSPACE_ITEM_KIND)?,
        "浏览器",
        WorkspaceFocusTarget::new("browser-page")?,
        WorkspaceItemCapabilities {
            closable: true,
            splittable: false,
            movable_across_windows: true,
            persistent: true,
        },
    )?
    .with_serialized_state(Some(serde_json::json!(state)));
    browser_tab_restoration(&item.restoration().expect("browser items are persistent"))?;
    Ok(item)
}

pub trait WorkspaceItemResolve {
    fn browser_restoration(&self) -> Result<Option<BrowserTabRestoration>, WorkspaceItemError>;
    fn document_path(&self) -> Result<Option<std::path::PathBuf>, WorkspaceItemError>;
    fn terminal_session_id(
        &self,
    ) -> Result<Option<crate::application::DesktopTerminalSessionId>, WorkspaceItemError>;
}

impl WorkspaceItemResolve for WorkspaceItem {
    fn browser_restoration(&self) -> Result<Option<BrowserTabRestoration>, WorkspaceItemError> {
        browser_tab_restoration(&WorkspaceItemRestoration {
            id: self.id.clone(),
            resource_id: Some(self.resource_id.clone()),
            kind: self.kind.clone(),
            serialized_state: self.serialized_state.clone(),
        })
    }

    fn document_path(&self) -> Result<Option<std::path::PathBuf>, WorkspaceItemError> {
        if self.kind.as_str() != DOCUMENT_WORKSPACE_ITEM_KIND {
            return Ok(None);
        }
        path_from_document_resource_key(self.resource_id.as_str())
            .map(Some)
            .map_err(|_| WorkspaceItemError::InvalidRestorationIdentity {
                item_id: self.resource_id.as_str().to_owned(),
                kind: DOCUMENT_WORKSPACE_ITEM_KIND.to_owned(),
            })
    }

    fn terminal_session_id(
        &self,
    ) -> Result<Option<crate::application::DesktopTerminalSessionId>, WorkspaceItemError> {
        if self.kind.as_str() != TERMINAL_WORKSPACE_ITEM_KIND {
            return Ok(None);
        }
        terminal_session_id_from_resource_identity(&self.resource_id).map(Some)
    }
}

impl DesktopApplication {
    pub fn task_workspace_item(
        &self,
        task_id: &TaskId,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        let task = self.get_task(task_id)?;
        if task.archived {
            return Err(DesktopApplicationError::InvalidInput {
                field: "taskId",
                message: format!("task `{}` is archived", task_id.as_str()),
            });
        }
        task_item(task).map_err(DesktopApplicationError::from)
    }

    pub fn task_workspace_item_view(
        &self,
        task_id: &TaskId,
        instance_id: WorkspaceItemId,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        let task = self.get_task(task_id)?;
        if task.archived {
            return Err(DesktopApplicationError::InvalidInput {
                field: "taskId",
                message: format!("task `{}` is archived", task_id.as_str()),
            });
        }
        task_item_with_instance_id(task, instance_id).map_err(DesktopApplicationError::from)
    }

    pub fn project_workspace_item(
        &self,
        project_id: &ProjectId,
        surface: ProjectWorkspaceSurface,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        let project = self.get_project(project_id)?;
        if project.archive == ProjectArchiveState::Archived {
            return Err(DesktopApplicationError::InvalidInput {
                field: "projectId",
                message: format!("project `{}` is archived", project_id.as_str()),
            });
        }
        project_item(project, surface).map_err(DesktopApplicationError::from)
    }

    pub fn project_workspace_item_view(
        &self,
        project_id: &ProjectId,
        surface: ProjectWorkspaceSurface,
        instance_id: WorkspaceItemId,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        let project = self.get_project(project_id)?;
        if project.archive == ProjectArchiveState::Archived {
            return Err(DesktopApplicationError::InvalidInput {
                field: "projectId",
                message: format!("project `{}` is archived", project_id.as_str()),
            });
        }
        project_item_with_instance_id(project, instance_id, surface)
            .map_err(DesktopApplicationError::from)
    }

    pub fn application_workspace_item(
        &self,
        surface: ApplicationWorkspaceSurface,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        application_item(surface).map_err(DesktopApplicationError::from)
    }

    pub fn document_workspace_item(
        &self,
        document_id: DocumentId,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        let snapshot = self.document_snapshot(document_id)?;
        document_item(&snapshot).map_err(DesktopApplicationError::from)
    }

    pub fn document_workspace_item_view(
        &self,
        document_id: DocumentId,
        instance_id: WorkspaceItemId,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        let snapshot = self.document_snapshot(document_id)?;
        document_item_with_instance_id(&snapshot, instance_id)
            .map_err(DesktopApplicationError::from)
    }

    pub fn terminal_workspace_item(
        &self,
        snapshot: &crate::application::DesktopTerminalSnapshot,
    ) -> Result<WorkspaceItem, DesktopApplicationError> {
        terminal_item(snapshot).map_err(DesktopApplicationError::from)
    }

    pub fn open_project_document_workspace_item(
        &self,
        project_id: &ProjectId,
        relative_path: &str,
    ) -> Result<(DocumentSnapshot, WorkspaceItem), DesktopApplicationError> {
        let snapshot = self.open_project_file(project_id, relative_path)?;
        let item = self.document_workspace_item(snapshot.id)?;
        Ok((snapshot, item))
    }

    pub(crate) fn restore_workspace_item(
        &self,
        restoration: &WorkspaceItemRestoration,
    ) -> Result<Option<WorkspaceItem>, DesktopApplicationError> {
        if let Some(state) = browser_tab_restoration(restoration)? {
            let task = match self.get_task(&state.scope.task_id) {
                Ok(task) => task,
                Err(DesktopApplicationError::Product(ProductError::NotFound { .. })) => {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            if task.project_id.as_ref() != Some(&state.scope.project_id) {
                return Err(WorkspaceItemError::InvalidRestorationIdentity {
                    item_id: restoration.id.as_str().into(),
                    kind: BROWSER_WORKSPACE_ITEM_KIND.into(),
                }
                .into());
            }
            if task.archived {
                return Ok(None);
            }
            let project = match self.get_project(&state.scope.project_id) {
                Ok(project) => project,
                Err(DesktopApplicationError::Product(ProductError::NotFound { .. })) => {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            if project.archive == ProjectArchiveState::Archived {
                return Ok(None);
            }
            return browser_workspace_item(&state).map(Some).map_err(Into::into);
        }

        if restoration.kind.as_str() == DOCUMENT_WORKSPACE_ITEM_KIND {
            let resource_id = restoration.resource_id.clone().unwrap_or_else(|| {
                WorkspaceResourceId::new(restoration.id.as_str().to_owned())
                    .expect("restored workspace item ids are already validated")
            });
            let path = match path_from_document_resource_key(resource_id.as_str()) {
                Ok(path) => path,
                Err(_) => {
                    return Err(WorkspaceItemError::InvalidRestorationIdentity {
                        item_id: resource_id.as_str().to_owned(),
                        kind: DOCUMENT_WORKSPACE_ITEM_KIND.to_owned(),
                    }
                    .into());
                }
            };
            let (snapshot, _) = match self.open_document_at_path(&path) {
                Ok(opened) => opened,
                Err(DesktopApplicationError::Document(crate::application::DocumentError::Io {
                    ..
                }))
                | Err(DesktopApplicationError::Document(
                    crate::application::DocumentError::NotAFile(_),
                )) => {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            return document_item_with_instance_id(&snapshot, restoration.id.clone())
                .map(|item| item.with_serialized_state(restoration.serialized_state.clone()))
                .map(Some)
                .map_err(DesktopApplicationError::from);
        }

        if restoration.kind.as_str() == TERMINAL_WORKSPACE_ITEM_KIND {
            let resource_id = restoration.resource_id.clone().unwrap_or_else(|| {
                WorkspaceResourceId::new(restoration.id.as_str().to_owned())
                    .expect("restored workspace item ids are already validated")
            });
            let session_id = terminal_session_id_from_resource_identity(&resource_id)?;
            let state = restoration.serialized_state.clone().ok_or_else(|| {
                WorkspaceItemError::InvalidRestorationIdentity {
                    item_id: resource_id.as_str().to_owned(),
                    kind: TERMINAL_WORKSPACE_ITEM_KIND.to_owned(),
                }
            })?;
            let terminal: crate::application::DesktopTerminalRestoration =
                serde_json::from_value(state).map_err(|_| {
                    WorkspaceItemError::InvalidRestorationIdentity {
                        item_id: resource_id.as_str().to_owned(),
                        kind: TERMINAL_WORKSPACE_ITEM_KIND.to_owned(),
                    }
                })?;
            if terminal.id != session_id {
                return Err(WorkspaceItemError::InvalidRestorationIdentity {
                    item_id: resource_id.as_str().to_owned(),
                    kind: TERMINAL_WORKSPACE_ITEM_KIND.to_owned(),
                }
                .into());
            }
            return terminal_item_with_instance_id(&terminal.snapshot(), restoration.id.clone())
                .map(Some)
                .map_err(DesktopApplicationError::from);
        }

        if restoration.kind.as_str() == TASK_WORKSPACE_ITEM_KIND {
            let resource_id = restoration.resource_id.clone().unwrap_or_else(|| {
                WorkspaceResourceId::new(restoration.id.as_str().to_owned())
                    .expect("restored workspace item ids are already validated")
            });
            let task_id = task_id_from_resource_identity(&resource_id)?;
            let task = match self.get_task(&task_id) {
                Ok(task) => task,
                Err(DesktopApplicationError::Product(ProductError::NotFound { .. })) => {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            if task.archived {
                return Ok(None);
            }
            return task_item_with_instance_id(task, restoration.id.clone())
                .map(|item| item.with_serialized_state(restoration.serialized_state.clone()))
                .map(Some)
                .map_err(DesktopApplicationError::from);
        }

        if let Some(surface) = ApplicationWorkspaceSurface::from_kind(&restoration.kind) {
            let resource_id = restoration.resource_id.clone().unwrap_or_else(|| {
                WorkspaceResourceId::new(restoration.id.as_str().to_owned())
                    .expect("restored workspace item ids are already validated")
            });
            if resource_id.as_str() != surface.resource_id() {
                return Err(WorkspaceItemError::InvalidRestorationIdentity {
                    item_id: resource_id.as_str().to_owned(),
                    kind: surface.kind().to_owned(),
                }
                .into());
            }
            return application_item_with_instance_id(surface, restoration.id.clone())
                .map(|item| item.with_serialized_state(restoration.serialized_state.clone()))
                .map(Some)
                .map_err(DesktopApplicationError::from);
        }

        let Some(surface) = ProjectWorkspaceSurface::from_kind(&restoration.kind) else {
            return Ok(None);
        };
        let resource_id = restoration.resource_id.clone().unwrap_or_else(|| {
            WorkspaceResourceId::new(restoration.id.as_str().to_owned())
                .expect("restored workspace item ids are already validated")
        });
        let project_id = project_id_from_resource_identity(&resource_id, surface)?;
        let project = match self.get_project(&project_id) {
            Ok(project) => project,
            Err(DesktopApplicationError::Product(ProductError::NotFound { .. })) => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        if project.archive == ProjectArchiveState::Archived {
            return Ok(None);
        }
        project_item_with_instance_id(project, restoration.id.clone(), surface)
            .map(|item| item.with_serialized_state(restoration.serialized_state.clone()))
            .map(Some)
            .map_err(DesktopApplicationError::from)
    }
}

fn terminal_session_id_from_resource_identity(
    id: &WorkspaceResourceId,
) -> Result<crate::application::DesktopTerminalSessionId, WorkspaceItemError> {
    let Some(session_id) = id
        .as_str()
        .strip_prefix("terminal:")
        .filter(|value| !value.is_empty())
    else {
        return Err(WorkspaceItemError::InvalidRestorationIdentity {
            item_id: id.as_str().to_owned(),
            kind: TERMINAL_WORKSPACE_ITEM_KIND.to_owned(),
        });
    };
    serde_json::from_value(serde_json::Value::String(session_id.to_owned())).map_err(|_| {
        WorkspaceItemError::InvalidRestorationIdentity {
            item_id: id.as_str().to_owned(),
            kind: TERMINAL_WORKSPACE_ITEM_KIND.to_owned(),
        }
    })
}

fn task_id_from_resource_identity(id: &WorkspaceResourceId) -> Result<TaskId, WorkspaceItemError> {
    let Some(task_id) = id.as_str().strip_prefix("task:") else {
        return Err(WorkspaceItemError::InvalidRestorationIdentity {
            item_id: id.as_str().to_owned(),
            kind: TASK_WORKSPACE_ITEM_KIND.to_owned(),
        });
    };
    TaskId::new(task_id.to_owned()).map_err(|_| WorkspaceItemError::InvalidRestorationIdentity {
        item_id: id.as_str().to_owned(),
        kind: TASK_WORKSPACE_ITEM_KIND.to_owned(),
    })
}

fn project_id_from_resource_identity(
    id: &WorkspaceResourceId,
    surface: ProjectWorkspaceSurface,
) -> Result<ProjectId, WorkspaceItemError> {
    let Some(project_id) = id
        .as_str()
        .strip_prefix(surface.resource_prefix())
        .filter(|value| !value.is_empty())
    else {
        return Err(WorkspaceItemError::InvalidRestorationIdentity {
            item_id: id.as_str().to_owned(),
            kind: surface.kind().to_owned(),
        });
    };
    ProjectId::new(project_id.to_owned()).map_err(|_| {
        WorkspaceItemError::InvalidRestorationIdentity {
            item_id: id.as_str().to_owned(),
            kind: surface.kind().to_owned(),
        }
    })
}

fn task_item(task: ProductTask) -> Result<WorkspaceItem, WorkspaceItemError> {
    let instance_id = WorkspaceItemId::new(format!("task:{}", task.id.as_str())).map_err(|_| {
        WorkspaceItemError::InvalidRestorationIdentity {
            item_id: task.id.as_str().to_owned(),
            kind: TASK_WORKSPACE_ITEM_KIND.to_owned(),
        }
    })?;
    task_item_with_instance_id(task, instance_id)
}

fn task_item_with_instance_id(
    task: ProductTask,
    instance_id: WorkspaceItemId,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    WorkspaceItem::new(
        instance_id,
        WorkspaceResourceId::new(format!("task:{}", task.id.as_str()))?,
        WorkspaceItemKind::new(TASK_WORKSPACE_ITEM_KIND)?,
        task.title,
        WorkspaceFocusTarget::new("composer")?,
        WorkspaceItemCapabilities::dockable(),
    )?
    .with_icon("task")
}

fn project_item(
    project: Project,
    surface: ProjectWorkspaceSurface,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    let instance_id = WorkspaceItemId::new(format!(
        "{}{}",
        surface.resource_prefix(),
        project.id.as_str()
    ))
    .map_err(|_| WorkspaceItemError::InvalidRestorationIdentity {
        item_id: project.id.as_str().to_owned(),
        kind: surface.kind().to_owned(),
    })?;
    project_item_with_instance_id(project, instance_id, surface)
}

fn project_item_with_instance_id(
    project: Project,
    instance_id: WorkspaceItemId,
    surface: ProjectWorkspaceSurface,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    WorkspaceItem::new(
        instance_id,
        WorkspaceResourceId::new(format!(
            "{}{}",
            surface.resource_prefix(),
            project.id.as_str()
        ))?,
        WorkspaceItemKind::new(surface.kind())?,
        format!("{} · {}", project.name, surface.title_suffix()),
        WorkspaceFocusTarget::new(surface.focus_target())?,
        WorkspaceItemCapabilities {
            closable: true,
            splittable: true,
            movable_across_windows: true,
            persistent: true,
        },
    )?
    .with_icon(surface.icon())
}

fn application_item(
    surface: ApplicationWorkspaceSurface,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    let instance_id = WorkspaceItemId::new(surface.resource_id()).map_err(|_| {
        WorkspaceItemError::InvalidRestorationIdentity {
            item_id: surface.resource_id().to_owned(),
            kind: surface.kind().to_owned(),
        }
    })?;
    application_item_with_instance_id(surface, instance_id)
}

fn application_item_with_instance_id(
    surface: ApplicationWorkspaceSurface,
    instance_id: WorkspaceItemId,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    WorkspaceItem::new(
        instance_id,
        WorkspaceResourceId::new(surface.resource_id())?,
        WorkspaceItemKind::new(surface.kind())?,
        surface.title(),
        WorkspaceFocusTarget::new(surface.focus_target())?,
        WorkspaceItemCapabilities {
            closable: true,
            splittable: true,
            movable_across_windows: true,
            persistent: true,
        },
    )?
    .with_icon(surface.icon())
}

fn document_item(snapshot: &DocumentSnapshot) -> Result<WorkspaceItem, WorkspaceItemError> {
    let resource = document_resource_key(&snapshot.canonical_path).map_err(|_| {
        WorkspaceItemError::InvalidRestorationIdentity {
            item_id: snapshot.canonical_path.display().to_string(),
            kind: DOCUMENT_WORKSPACE_ITEM_KIND.to_owned(),
        }
    })?;
    let instance_id = WorkspaceItemId::new(resource.clone()).map_err(|_| {
        WorkspaceItemError::InvalidRestorationIdentity {
            item_id: resource.clone(),
            kind: DOCUMENT_WORKSPACE_ITEM_KIND.to_owned(),
        }
    })?;
    document_item_with_resource(snapshot, instance_id, resource)
}

fn document_item_with_instance_id(
    snapshot: &DocumentSnapshot,
    instance_id: WorkspaceItemId,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    let resource = document_resource_key(&snapshot.canonical_path).map_err(|_| {
        WorkspaceItemError::InvalidRestorationIdentity {
            item_id: snapshot.canonical_path.display().to_string(),
            kind: DOCUMENT_WORKSPACE_ITEM_KIND.to_owned(),
        }
    })?;
    document_item_with_resource(snapshot, instance_id, resource)
}

fn document_item_with_resource(
    snapshot: &DocumentSnapshot,
    instance_id: WorkspaceItemId,
    resource: String,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    let title = snapshot
        .canonical_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("未命名文档")
        .to_owned();
    WorkspaceItem::new(
        instance_id,
        WorkspaceResourceId::new(resource)?,
        WorkspaceItemKind::new(DOCUMENT_WORKSPACE_ITEM_KIND)?,
        title,
        WorkspaceFocusTarget::new("editor")?,
        WorkspaceItemCapabilities {
            closable: true,
            splittable: true,
            movable_across_windows: true,
            persistent: true,
        },
    )?
    .with_icon("document")
}

fn terminal_item(
    snapshot: &crate::application::DesktopTerminalSnapshot,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    let instance_id =
        WorkspaceItemId::new(format!("terminal:{}", snapshot.id.as_str())).map_err(|_| {
            WorkspaceItemError::InvalidRestorationIdentity {
                item_id: snapshot.id.as_str().to_owned(),
                kind: TERMINAL_WORKSPACE_ITEM_KIND.to_owned(),
            }
        })?;
    terminal_item_with_instance_id(snapshot, instance_id)
}

fn terminal_item_with_instance_id(
    snapshot: &crate::application::DesktopTerminalSnapshot,
    instance_id: WorkspaceItemId,
) -> Result<WorkspaceItem, WorkspaceItemError> {
    let title = snapshot
        .cwd
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map_or_else(|| "终端".to_owned(), |name| format!("终端 · {name}"));
    WorkspaceItem::new(
        instance_id,
        WorkspaceResourceId::new(format!("terminal:{}", snapshot.id.as_str()))?,
        WorkspaceItemKind::new(TERMINAL_WORKSPACE_ITEM_KIND)?,
        title,
        WorkspaceFocusTarget::new("terminal-input")?,
        WorkspaceItemCapabilities::dockable(),
    )?
    .with_icon("terminal")
    .map(|item| {
        item.with_serialized_state(
            serde_json::to_value(
                crate::application::DesktopTerminalRestoration::from_snapshot(snapshot),
            )
            .ok(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::WorkspaceItemResolve;

    fn browser_application() -> DesktopApplication {
        use crate::application::*;
        struct Host;
        impl DesktopHost for Host {
            fn execute(
                &self,
                _: &DesktopHostContext,
                _: DesktopHostAction,
            ) -> Result<DesktopHostResult, DesktopHostError> {
                Ok(DesktopHostResult::Completed)
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        DesktopApplication::from_authority(
            DesktopApplicationConfig::new(std::env::temp_dir().join(&id), &id).unwrap(),
            lilia_service::ServiceAuthority::bootstrap_in_memory_named(&id, &id).unwrap(),
            std::sync::Arc::new(Host),
        )
        .unwrap()
    }

    #[test]
    fn browser_restoration_preserves_scope_url_and_rejects_corrupted_identity() {
        let state = BrowserTabRestoration {
            scope: lilia_contracts::BrowserScope {
                project_id: ProjectId::new("project").unwrap(),
                task_id: TaskId::new("task").unwrap(),
                tab_id: "browser:task".into(),
            },
            url: "https://example.test/path".into(),
        };
        for tab in [
            "browser:task".to_owned(),
            format!("browser:task:{}", uuid::Uuid::new_v4()),
        ] {
            let mut state = state.clone();
            state.scope.tab_id = tab;
            let item = browser_workspace_item(&state).unwrap();
            assert!(item.capabilities.persistent);
            assert_eq!(item.browser_restoration().unwrap(), Some(state));
        }
        let record = browser_workspace_item(&state)
            .unwrap()
            .restoration()
            .unwrap();
        let original = serde_json::to_value(&record).unwrap();
        for mutation in 0..6 {
            let mut changed = record.clone();
            match mutation {
                0 => changed.id = WorkspaceItemId::new("browser:other").unwrap(),
                1 => changed.resource_id = Some(WorkspaceResourceId::new("browser:other").unwrap()),
                2 => {
                    changed.serialized_state.as_mut().unwrap()["scope"]["taskId"] =
                        serde_json::json!("other")
                }
                3 => {
                    changed.serialized_state.as_mut().unwrap()["scope"]["tabId"] =
                        serde_json::json!("browser:task:invalid")
                }
                4 => changed.serialized_state = None,
                _ => {
                    changed.serialized_state.as_mut().unwrap()["url"] =
                        serde_json::json!("file:///secret")
                }
            }
            let before = serde_json::to_value(&changed).unwrap();
            assert!(browser_tab_restoration(&changed).is_err());
            assert_eq!(serde_json::to_value(&changed).unwrap(), before);
        }
        assert_eq!(serde_json::to_value(&record).unwrap(), original);
    }

    #[test]
    fn browser_restore_requires_live_task_in_the_recorded_project_and_preserves_record() {
        let app = browser_application();
        let first = app
            .create_project(crate::application::DesktopProjectCreate::new("first"))
            .unwrap();
        let second = app
            .create_project(crate::application::DesktopProjectCreate::new("second"))
            .unwrap();
        let task = app
            .create_task(crate::application::DesktopTaskCreate::new(
                Some(first.id.clone()),
                "task",
            ))
            .unwrap();
        let state = BrowserTabRestoration {
            scope: lilia_contracts::BrowserScope {
                project_id: first.id,
                task_id: task.id.clone(),
                tab_id: format!("browser:{}", task.id.as_str()),
            },
            url: "about:blank".into(),
        };
        let record = browser_workspace_item(&state)
            .unwrap()
            .restoration()
            .unwrap();
        let original = serde_json::to_value(&record).unwrap();
        let restored = app.restore_workspace_item(&record).unwrap().unwrap();
        assert_eq!(restored.browser_restoration().unwrap(), Some(state.clone()));
        let mut crossed = record.clone();
        crossed.serialized_state.as_mut().unwrap()["scope"]["projectId"] =
            serde_json::json!(second.id);
        let crossed_original = serde_json::to_value(&crossed).unwrap();
        assert!(app.restore_workspace_item(&crossed).is_err());
        assert_eq!(serde_json::to_value(&crossed).unwrap(), crossed_original);
        let mut absent = state;
        absent.scope.task_id = TaskId::new("missing-task").unwrap();
        absent.scope.tab_id = "browser:missing-task".into();
        assert!(
            app.restore_workspace_item(
                &browser_workspace_item(&absent)
                    .unwrap()
                    .restoration()
                    .unwrap()
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(serde_json::to_value(&record).unwrap(), original);
    }

    #[test]
    fn browser_restore_omits_archived_tasks_and_projects_without_mutating_records() {
        let app = browser_application();
        let client = app.authority().client().unwrap();
        for archived_project in [false, true] {
            let project_id = ProjectId::new(format!("project-{archived_project}")).unwrap();
            let mut project = Project::new(project_id.clone(), "project").unwrap();
            if archived_project {
                project.archive = ProjectArchiveState::Archived;
            }
            client
                .products()
                .create_entity(lilia_contracts::ProductEntity::Project(project))
                .unwrap();
            let task_id = TaskId::new(format!("task-{archived_project}")).unwrap();
            let mut task =
                ProductTask::new(task_id.clone(), Some(project_id.clone()), "task").unwrap();
            task.archived = !archived_project;
            client
                .products()
                .create_entity(lilia_contracts::ProductEntity::Task(task))
                .unwrap();
            let state = BrowserTabRestoration {
                scope: lilia_contracts::BrowserScope {
                    project_id,
                    tab_id: format!("browser:{}", task_id.as_str()),
                    task_id,
                },
                url: "about:blank".into(),
            };
            let record = browser_workspace_item(&state)
                .unwrap()
                .restoration()
                .unwrap();
            let original = record.clone();
            assert!(app.restore_workspace_item(&record).unwrap().is_none());
            assert_eq!(record, original);
        }
    }

    #[test]
    fn restoration_does_not_duplicate_presentation_or_product_facts() {
        let item = WorkspaceItem::new(
            WorkspaceItemId::new("task-view:one").unwrap(),
            WorkspaceResourceId::new("task:one").unwrap(),
            WorkspaceItemKind::new(TASK_WORKSPACE_ITEM_KIND).unwrap(),
            "Current title",
            WorkspaceFocusTarget::new("composer").unwrap(),
            WorkspaceItemCapabilities::dockable(),
        )
        .unwrap()
        .with_icon("task")
        .unwrap()
        .with_serialized_state(Some(serde_json::json!({ "scrollOffset": 12 })));

        assert_eq!(
            serde_json::to_value(item.restoration().unwrap()).unwrap(),
            serde_json::json!({
                "id": "task-view:one",
                "resourceId": "task:one",
                "kind": "task",
                "serializedState": { "scrollOffset": 12 }
            })
        );
    }

    #[test]
    fn transient_items_are_not_serialized_for_session_restore() {
        let item = WorkspaceItem::new(
            WorkspaceItemId::new("search:temporary").unwrap(),
            WorkspaceResourceId::new("search:query").unwrap(),
            WorkspaceItemKind::new("search").unwrap(),
            "Search",
            WorkspaceFocusTarget::new("query").unwrap(),
            WorkspaceItemCapabilities {
                closable: true,
                splittable: true,
                movable_across_windows: true,
                persistent: false,
            },
        )
        .unwrap();

        assert_eq!(item.restoration(), None);
    }

    #[test]
    fn project_surfaces_keep_product_identity_out_of_serialized_view_state() {
        let project = Project::new(ProjectId::new("native-project").unwrap(), "Native").unwrap();
        for surface in ProjectWorkspaceSurface::ALL {
            let item = project_item(project.clone(), surface).unwrap();
            assert_eq!(item.kind.as_str(), surface.kind());
            assert_eq!(
                item.project_surface().unwrap(),
                Some((project.id.clone(), surface))
            );
            assert!(item.capabilities.closable);
            assert!(item.capabilities.splittable);
            assert!(item.capabilities.movable_across_windows);
            assert!(item.capabilities.persistent);
            assert_eq!(item.serialized_state, None);
        }
    }

    #[test]
    fn project_surface_rejects_a_kind_resource_mismatch() {
        let item = WorkspaceItem::new(
            WorkspaceItemId::new("project-roadmap:view").unwrap(),
            WorkspaceResourceId::new("project-memory:native-project").unwrap(),
            WorkspaceItemKind::new(ROADMAP_WORKSPACE_ITEM_KIND).unwrap(),
            "Native · 路线图",
            WorkspaceFocusTarget::new("roadmap").unwrap(),
            WorkspaceItemCapabilities::dockable(),
        )
        .unwrap();

        assert!(matches!(
            item.project_surface(),
            Err(WorkspaceItemError::InvalidRestorationIdentity { kind, .. })
                if kind == ROADMAP_WORKSPACE_ITEM_KIND
        ));
    }

    #[test]
    fn application_surface_has_stable_identity_and_cross_window_capabilities() {
        for (surface, resource_id) in [
            (
                ApplicationWorkspaceSurface::Projects,
                "application:projects",
            ),
            (
                ApplicationWorkspaceSurface::Automations,
                "application:automations",
            ),
            (
                ApplicationWorkspaceSurface::Settings,
                "application:settings",
            ),
        ] {
            let item = application_item(surface).unwrap();
            assert_eq!(item.id.as_str(), resource_id);
            assert_eq!(item.resource_id.as_str(), resource_id);
            assert_eq!(item.kind.as_str(), surface.kind());
            assert_eq!(item.application_surface().unwrap(), Some(surface));
            assert!(item.capabilities.closable);
            assert!(item.capabilities.splittable);
            assert!(item.capabilities.movable_across_windows);
            assert!(item.capabilities.persistent);
        }
    }

    #[test]
    fn application_surface_rejects_a_kind_resource_mismatch() {
        let item = WorkspaceItem::new(
            WorkspaceItemId::new("application:automations").unwrap(),
            WorkspaceResourceId::new("application:settings").unwrap(),
            WorkspaceItemKind::new(AUTOMATION_WORKSPACE_ITEM_KIND).unwrap(),
            "自动化",
            WorkspaceFocusTarget::new("automation-canvas").unwrap(),
            WorkspaceItemCapabilities::dockable(),
        )
        .unwrap();

        assert!(matches!(
            item.application_surface(),
            Err(WorkspaceItemError::InvalidRestorationIdentity { kind, .. })
                if kind == AUTOMATION_WORKSPACE_ITEM_KIND
        ));
    }

    #[test]
    fn document_editor_items_share_canonical_resource_identity() {
        let snapshot = DocumentSnapshot {
            id: DocumentId::new(3),
            canonical_path: std::env::current_dir().unwrap().join("src/lib.rs"),
            language: None,
            read_only: false,
            buffer: crate::application::BufferSnapshot {
                id: crate::application::BufferId::new(1),
                text: "fn ok() {}".into(),
                revision: crate::application::BufferRevision::INITIAL,
                saved_revision: crate::application::BufferRevision::INITIAL,
            },
            disk_fingerprint: 1,
        };
        let first = document_item(&snapshot).unwrap();
        let second = document_item_with_instance_id(
            &snapshot,
            WorkspaceItemId::new("document-view:lib:second").unwrap(),
        )
        .unwrap();
        assert_eq!(first.kind.as_str(), DOCUMENT_WORKSPACE_ITEM_KIND);
        assert_eq!(first.resource_id, second.resource_id);
        assert_ne!(first.id, second.id);
        // Windows 的资源键按大小写不敏感规范化，所以取回的路径不会逐字节等于原路径；
        // 真正要守的是它仍指向同一个文档。
        let recovered = first.document_path().unwrap().unwrap();
        assert_eq!(
            document_resource_key(&recovered).unwrap(),
            document_resource_key(&snapshot.canonical_path).unwrap()
        );
        assert_eq!(first.serialized_state, None);
        assert!(first.capabilities.persistent);
    }
}
