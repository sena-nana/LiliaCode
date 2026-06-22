use std::sync::OnceLock;

use serde::Deserialize;

const SYSTEM_COMMAND_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/system-command-contract.json");
const POPUP_COMMAND_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/popup-command-contract.json");

static SYSTEM_COMMAND_CONTRACT: OnceLock<SystemCommandContract> = OnceLock::new();
static POPUP_COMMAND_CONTRACT: OnceLock<PopupCommandContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct SystemCommandContract {
    commands: SystemCommandsContract,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemCommandsContract {
    open_path: String,
    open_url: String,
    #[serde(rename = "openInVSCode")]
    open_in_vs_code: String,
}

#[derive(Debug, Deserialize)]
struct PopupCommandContract {
    commands: PopupCommandsContract,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PopupCommandsContract {
    get_window_settings: String,
    set_window_settings: String,
    remember_last_project: String,
    open_new_chat: String,
    open_task: String,
    open_child_question: String,
    open_conversation_status: String,
    toggle_conversation_status: String,
    focus_main: String,
}

fn system_command_contract() -> &'static SystemCommandContract {
    SYSTEM_COMMAND_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            SYSTEM_COMMAND_CONTRACT_JSON,
            "system-command-contract.json",
        )
    })
}

fn popup_command_contract() -> &'static PopupCommandContract {
    POPUP_COMMAND_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            POPUP_COMMAND_CONTRACT_JSON,
            "popup-command-contract.json",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::popup_windows::{
        popup_focus_main, popup_get_window_settings, popup_open_child_question,
        popup_open_conversation_status, popup_open_new_chat, popup_open_task,
        popup_remember_last_project, popup_set_window_settings, popup_toggle_conversation_status,
    };
    use crate::project_shell::{system_open_in_vscode, system_open_path, system_open_url};

    #[test]
    fn system_command_names_load_from_contract_manifest() {
        let commands = &system_command_contract().commands;
        let _ = system_open_path;
        let _ = system_open_url;
        let _ = system_open_in_vscode;

        assert_eq!(commands.open_path, stringify!(system_open_path));
        assert_eq!(commands.open_url, stringify!(system_open_url));
        assert_eq!(commands.open_in_vs_code, stringify!(system_open_in_vscode));
    }

    #[test]
    fn popup_command_names_load_from_contract_manifest() {
        let commands = &popup_command_contract().commands;
        let _ = popup_get_window_settings;
        let _ = popup_set_window_settings;
        let _ = popup_remember_last_project;
        let _ = popup_open_new_chat;
        let _ = popup_open_task;
        let _ = popup_open_child_question;
        let _ = popup_open_conversation_status;
        let _ = popup_toggle_conversation_status;
        let _ = popup_focus_main;

        assert_eq!(
            commands.get_window_settings,
            stringify!(popup_get_window_settings)
        );
        assert_eq!(
            commands.set_window_settings,
            stringify!(popup_set_window_settings)
        );
        assert_eq!(
            commands.remember_last_project,
            stringify!(popup_remember_last_project)
        );
        assert_eq!(commands.open_new_chat, stringify!(popup_open_new_chat));
        assert_eq!(commands.open_task, stringify!(popup_open_task));
        assert_eq!(
            commands.open_child_question,
            stringify!(popup_open_child_question)
        );
        assert_eq!(
            commands.open_conversation_status,
            stringify!(popup_open_conversation_status)
        );
        assert_eq!(
            commands.toggle_conversation_status,
            stringify!(popup_toggle_conversation_status)
        );
        assert_eq!(commands.focus_main, stringify!(popup_focus_main));
    }
}
