use crate::application::{ApplicationWorkspaceSurface, WorkspaceItemId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WindowRoute {
    #[default]
    Workspace,
    Settings,
    Automations,
    Projects,
}

impl WindowRoute {
    pub(crate) fn from_surface(surface: Option<ApplicationWorkspaceSurface>) -> Self {
        match surface {
            Some(ApplicationWorkspaceSurface::Settings) => Self::Settings,
            Some(ApplicationWorkspaceSurface::Automations) => Self::Automations,
            Some(ApplicationWorkspaceSurface::Projects) => Self::Projects,
            None => Self::Workspace,
        }
    }

    pub(crate) fn surface(self) -> Option<ApplicationWorkspaceSurface> {
        match self {
            Self::Settings => Some(ApplicationWorkspaceSurface::Settings),
            Self::Automations => Some(ApplicationWorkspaceSurface::Automations),
            Self::Projects => Some(ApplicationWorkspaceSurface::Projects),
            Self::Workspace => None,
        }
    }

    pub(crate) fn is_settings(self) -> bool {
        self == Self::Settings
    }

    pub(crate) fn is_automations(self) -> bool {
        self == Self::Automations
    }

    pub(crate) fn is_projects(self) -> bool {
        self == Self::Projects
    }

    pub(crate) fn is_management(self) -> bool {
        matches!(self, Self::Settings | Self::Automations)
    }

    pub(crate) fn return_item_for(
        self,
        destination: ApplicationWorkspaceSurface,
        active_item: Option<WorkspaceItemId>,
    ) -> Option<Option<WorkspaceItemId>> {
        let destination = Self::from_surface(Some(destination));
        (destination.is_management() && self != destination).then_some(active_item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> WorkspaceItemId {
        WorkspaceItemId::new(id).unwrap()
    }

    #[test]
    fn entering_each_management_page_records_the_active_resource() {
        for destination in [
            ApplicationWorkspaceSurface::Settings,
            ApplicationWorkspaceSurface::Automations,
        ] {
            assert_eq!(
                WindowRoute::Workspace.return_item_for(destination, Some(item("task-view"))),
                Some(Some(item("task-view"))),
            );
        }
    }

    #[test]
    fn reopening_management_does_not_replace_its_existing_return_context() {
        for route in [WindowRoute::Settings, WindowRoute::Automations] {
            assert_eq!(
                route.return_item_for(route.surface().unwrap(), Some(item("management-view"))),
                None,
            );
        }
    }

    #[test]
    fn switching_management_pages_returns_to_the_previous_page() {
        assert_eq!(
            WindowRoute::Settings.return_item_for(
                ApplicationWorkspaceSurface::Automations,
                Some(item("settings-view")),
            ),
            Some(Some(item("settings-view"))),
        );
        assert_eq!(
            WindowRoute::Automations.return_item_for(
                ApplicationWorkspaceSurface::Settings,
                Some(item("automations-view")),
            ),
            Some(Some(item("automations-view"))),
        );
    }

    #[test]
    fn entering_management_from_task_list_clears_an_old_return_item() {
        assert_eq!(
            WindowRoute::Workspace.return_item_for(ApplicationWorkspaceSurface::Settings, None),
            Some(None),
        );
        assert_eq!(
            WindowRoute::Workspace.return_item_for(
                ApplicationWorkspaceSurface::Projects,
                Some(item("task-view")),
            ),
            None,
        );
    }
}
