use std::sync::OnceLock;

use serde::Deserialize;

const APP_EVENTS_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/app-events-contract.json");

static APP_EVENTS_CONTRACT: OnceLock<AppEventsContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppEventsContract {
    main_navigate_event_name: String,
    popup_navigate_event_name: String,
    cli_project_open_event_name: String,
    #[allow(dead_code)]
    cli_project_open_consume_pending_command: String,
}

fn app_events_contract() -> &'static AppEventsContract {
    APP_EVENTS_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            APP_EVENTS_CONTRACT_JSON,
            "app-events-contract.json",
        )
    })
}

pub(crate) fn main_navigate_event_name() -> &'static str {
    &app_events_contract().main_navigate_event_name
}

pub(crate) fn popup_navigate_event_name() -> &'static str {
    &app_events_contract().popup_navigate_event_name
}

pub(crate) fn cli_project_open_event_name() -> &'static str {
    &app_events_contract().cli_project_open_event_name
}

#[cfg(test)]
fn cli_project_open_consume_pending_command() -> &'static str {
    &app_events_contract().cli_project_open_consume_pending_command
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_project::cli_project_open_consume_pending;

    #[test]
    fn app_event_names_load_from_contract_manifest() {
        assert_eq!(main_navigate_event_name(), "lilia:main:navigate");
        assert_eq!(popup_navigate_event_name(), "lilia:popup:navigate");
        assert_eq!(cli_project_open_event_name(), "lilia:cli-project-open");
    }

    #[test]
    fn app_command_names_load_from_contract_manifest() {
        let _ = cli_project_open_consume_pending;
        assert_eq!(
            cli_project_open_consume_pending_command(),
            stringify!(cli_project_open_consume_pending)
        );
    }
}
