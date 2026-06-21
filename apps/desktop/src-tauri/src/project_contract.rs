use std::sync::OnceLock;

use serde::Deserialize;

const PROJECT_COMMAND_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/project-command-contract.json");

static PROJECT_COMMAND_CONTRACT: OnceLock<ProjectCommandContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct ProjectCommandContract {
    commands: ProjectCommandsContract,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectCommandsContract {
    list: String,
    dashboard_list: String,
    get: String,
    create: String,
    rename: String,
    remove: String,
    toggle_pin: String,
    reorder: String,
    get_settings: String,
    set_settings: String,
}

fn project_command_contract() -> &'static ProjectCommandContract {
    PROJECT_COMMAND_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            PROJECT_COMMAND_CONTRACT_JSON,
            "project-command-contract.json",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_shell::{project_get_settings, project_set_settings};
    use crate::projects_tasks::{
        project_create, project_dashboard_list, project_get, project_list, project_remove,
        project_rename, project_reorder, project_toggle_pin,
    };

    #[test]
    fn project_command_names_load_from_contract_manifest() {
        let commands = &project_command_contract().commands;
        let _ = project_list;
        let _ = project_dashboard_list;
        let _ = project_get;
        let _ = project_create;
        let _ = project_rename;
        let _ = project_remove;
        let _ = project_toggle_pin;
        let _ = project_reorder;
        let _ = project_get_settings;
        let _ = project_set_settings;

        assert_eq!(commands.list, stringify!(project_list));
        assert_eq!(commands.dashboard_list, stringify!(project_dashboard_list));
        assert_eq!(commands.get, stringify!(project_get));
        assert_eq!(commands.create, stringify!(project_create));
        assert_eq!(commands.rename, stringify!(project_rename));
        assert_eq!(commands.remove, stringify!(project_remove));
        assert_eq!(commands.toggle_pin, stringify!(project_toggle_pin));
        assert_eq!(commands.reorder, stringify!(project_reorder));
        assert_eq!(commands.get_settings, stringify!(project_get_settings));
        assert_eq!(commands.set_settings, stringify!(project_set_settings));
    }
}
