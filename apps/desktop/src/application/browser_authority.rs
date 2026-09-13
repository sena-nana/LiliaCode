use lilia_agent::{BrowserError, BrowserScopeAuthority};
use lilia_contracts::{BrowserScope, ProjectArchiveState};
use lilia_feature_task::ProjectTaskService;

pub struct ProductBrowserScopeAuthority {
    projects: ProjectTaskService,
}

impl ProductBrowserScopeAuthority {
    pub fn new(projects: ProjectTaskService) -> Self {
        Self { projects }
    }
}

impl BrowserScopeAuthority for ProductBrowserScopeAuthority {
    fn validate(&self, scope: &BrowserScope) -> Result<(), BrowserError> {
        let task = self
            .projects
            .get_task(&scope.task_id)
            .map_err(|_| BrowserError::WrongScope)?;
        if task.archived || task.project_id.as_ref() != Some(&scope.project_id) {
            return Err(BrowserError::WrongScope);
        }
        let project = self
            .projects
            .get_project(&scope.project_id)
            .map_err(|_| BrowserError::WrongScope)?;
        if project.archive != ProjectArchiveState::Active {
            return Err(BrowserError::WrongScope);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::*;
    use std::sync::Arc;

    #[test]
    fn browser_authority_rereads_task_ownership_and_archive_state() {
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
        let home = tempfile::tempdir().unwrap();
        let app = DesktopApplication::from_authority(
            DesktopApplicationConfig::new(home.path(), &id).unwrap(),
            lilia_service::ServiceAuthority::bootstrap_in_memory_named(&id, &id).unwrap(),
            Arc::new(Host),
        )
        .unwrap();
        let first = app
            .create_project(DesktopProjectCreate::new("first"))
            .unwrap();
        let second = app
            .create_project(DesktopProjectCreate::new("second"))
            .unwrap();
        let task = app
            .create_task(DesktopTaskCreate::new(Some(first.id.clone()), "task"))
            .unwrap();
        let mut scope = BrowserScope {
            project_id: first.id,
            task_id: task.id.clone(),
            tab_id: format!("browser:{}", task.id.as_str()),
        };
        let authority = ProductBrowserScopeAuthority::new(app.project_task_services().0);
        assert_eq!(authority.validate(&scope), Ok(()));
        app.move_task(
            &task.id,
            DesktopTaskMove {
                target_project_id: Some(second.id.clone()),
                target_parent_id: None,
            },
        )
        .unwrap();
        assert_eq!(authority.validate(&scope), Err(BrowserError::WrongScope));
        scope.project_id = second.id;
        assert_eq!(authority.validate(&scope), Ok(()));
        app.set_task_archived(&task.id, true).unwrap();
        assert_eq!(authority.validate(&scope), Err(BrowserError::WrongScope));
        app.set_task_archived(&task.id, false).unwrap();
        assert_eq!(authority.validate(&scope), Ok(()));
        app.update_project(
            &scope.project_id,
            DesktopProjectPatch {
                archived: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!app.get_task(&task.id).unwrap().archived);
        assert_eq!(authority.validate(&scope), Err(BrowserError::WrongScope));
    }
}
