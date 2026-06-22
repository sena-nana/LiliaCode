use std::sync::OnceLock;

use serde::Deserialize;

const GITHUB_COMMAND_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/github-command-contract.json");

static GITHUB_COMMAND_CONTRACT: OnceLock<GitHubCommandContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct GitHubCommandContract {
    commands: GitHubCommandsContract,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubCommandsContract {
    git_clone_repo: String,
    get_binding_status: String,
    start_device_flow: String,
    poll_device_flow: String,
    unbind: String,
    list_repos: String,
    clone_repo: String,
}

fn github_command_contract() -> &'static GitHubCommandContract {
    GITHUB_COMMAND_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            GITHUB_COMMAND_CONTRACT_JSON,
            "github-command-contract.json",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{
        github_clone_repo, github_get_binding_status, github_list_repos, github_poll_device_flow,
        github_start_device_flow, github_unbind,
    };
    use crate::project_shell::git_clone_repo;

    #[test]
    fn github_command_names_load_from_contract_manifest() {
        let commands = &github_command_contract().commands;
        let _ = git_clone_repo;
        let _ = github_get_binding_status;
        let _ = github_start_device_flow;
        let _ = github_poll_device_flow;
        let _ = github_unbind;
        let _ = github_list_repos;
        let _ = github_clone_repo;

        assert_eq!(commands.git_clone_repo, stringify!(git_clone_repo));
        assert_eq!(
            commands.get_binding_status,
            stringify!(github_get_binding_status)
        );
        assert_eq!(
            commands.start_device_flow,
            stringify!(github_start_device_flow)
        );
        assert_eq!(
            commands.poll_device_flow,
            stringify!(github_poll_device_flow)
        );
        assert_eq!(commands.unbind, stringify!(github_unbind));
        assert_eq!(commands.list_repos, stringify!(github_list_repos));
        assert_eq!(commands.clone_repo, stringify!(github_clone_repo));
    }
}
