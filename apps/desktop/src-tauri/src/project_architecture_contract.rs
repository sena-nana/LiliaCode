use std::sync::OnceLock;

use serde::Deserialize;

const ARCHITECTURE_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/architecture-contract.json");

static ARCHITECTURE_CONTRACT: OnceLock<ProjectArchitectureContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectArchitectureContract {
    #[cfg(test)]
    commands: ProjectArchitectureCommandsContract,
    project_architecture_permissions: Vec<String>,
    project_architecture_rollback_permission: String,
    project_architecture_change_statuses: Vec<String>,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectArchitectureCommandsContract {
    get: String,
    list_changes: String,
    apply: String,
    reject: String,
    rollback: String,
}

fn architecture_contract() -> &'static ProjectArchitectureContract {
    ARCHITECTURE_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            ARCHITECTURE_CONTRACT_JSON,
            "architecture-contract.json",
        )
    })
}

pub(crate) fn project_architecture_permissions() -> &'static [String] {
    &architecture_contract().project_architecture_permissions
}

pub(crate) fn project_architecture_rollback_permission() -> &'static str {
    &architecture_contract().project_architecture_rollback_permission
}

pub(crate) fn project_architecture_change_statuses() -> &'static [String] {
    &architecture_contract().project_architecture_change_statuses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects_tasks::{
        project_architecture_apply, project_architecture_get, project_architecture_list_changes,
        project_architecture_reject, project_architecture_rollback,
    };

    #[test]
    fn project_architecture_command_names_load_from_contract_manifest() {
        let commands = &architecture_contract().commands;
        let _ = project_architecture_get;
        let _ = project_architecture_list_changes;
        let _ = project_architecture_apply;
        let _ = project_architecture_reject;
        let _ = project_architecture_rollback;

        assert_eq!(commands.get, stringify!(project_architecture_get));
        assert_eq!(
            commands.list_changes,
            stringify!(project_architecture_list_changes)
        );
        assert_eq!(commands.apply, stringify!(project_architecture_apply));
        assert_eq!(commands.reject, stringify!(project_architecture_reject));
        assert_eq!(commands.rollback, stringify!(project_architecture_rollback));
    }
}
