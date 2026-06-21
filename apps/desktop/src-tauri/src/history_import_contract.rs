use std::sync::OnceLock;

use serde::Deserialize;

const HISTORY_IMPORT_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/history-import-contract.json");

static HISTORY_IMPORT_CONTRACT: OnceLock<HistoryImportContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryImportContract {
    commands: HistoryImportCommandsContract,
    providers: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryImportCommandsContract {
    search: String,
    preview: String,
    attach: String,
    runtime_states: String,
    clean_background_terminals: String,
}

fn history_import_contract() -> &'static HistoryImportContract {
    HISTORY_IMPORT_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            HISTORY_IMPORT_CONTRACT_JSON,
            "history-import-contract.json",
        )
    })
}

pub(crate) fn providers() -> &'static [String] {
    &history_import_contract().providers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_import::{
        history_import_attach, history_import_clean_background_terminals, history_import_preview,
        history_import_runtime_states, history_import_search,
    };

    #[test]
    fn history_import_command_names_load_from_contract_manifest() {
        let commands = &history_import_contract().commands;
        let _ = history_import_search;
        let _ = history_import_preview;
        let _ = history_import_attach;
        let _ = history_import_runtime_states;
        let _ = history_import_clean_background_terminals;

        assert_eq!(commands.search, stringify!(history_import_search));
        assert_eq!(commands.preview, stringify!(history_import_preview));
        assert_eq!(commands.attach, stringify!(history_import_attach));
        assert_eq!(
            commands.runtime_states,
            stringify!(history_import_runtime_states)
        );
        assert_eq!(
            commands.clean_background_terminals,
            stringify!(history_import_clean_background_terminals)
        );
    }
}
