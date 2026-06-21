use std::sync::OnceLock;

use serde::Deserialize;

const MILESTONE_COMMAND_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/milestone-command-contract.json");

static MILESTONE_COMMAND_CONTRACT: OnceLock<MilestoneCommandContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct MilestoneCommandContract {
    commands: MilestoneCommandsContract,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MilestoneCommandsContract {
    list: String,
    create: String,
    update: String,
    delete: String,
    reorder: String,
    set_tasks: String,
}

fn milestone_command_contract() -> &'static MilestoneCommandContract {
    MILESTONE_COMMAND_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            MILESTONE_COMMAND_CONTRACT_JSON,
            "milestone-command-contract.json",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects_tasks::{
        milestone_create, milestone_delete, milestone_list, milestone_reorder, milestone_set_tasks,
        milestone_update,
    };

    #[test]
    fn milestone_command_names_load_from_contract_manifest() {
        let commands = &milestone_command_contract().commands;
        let _ = milestone_list;
        let _ = milestone_create;
        let _ = milestone_update;
        let _ = milestone_delete;
        let _ = milestone_reorder;
        let _ = milestone_set_tasks;

        assert_eq!(commands.list, stringify!(milestone_list));
        assert_eq!(commands.create, stringify!(milestone_create));
        assert_eq!(commands.update, stringify!(milestone_update));
        assert_eq!(commands.delete, stringify!(milestone_delete));
        assert_eq!(commands.reorder, stringify!(milestone_reorder));
        assert_eq!(commands.set_tasks, stringify!(milestone_set_tasks));
    }
}
