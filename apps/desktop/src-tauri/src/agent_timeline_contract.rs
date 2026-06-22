use std::sync::OnceLock;

use serde::Deserialize;

const TIMELINE_CONTRACT_JSON: &str =
    include_str!("../../../../packages/contracts/src/timeline-contract.json");

static TIMELINE_CONTRACT: OnceLock<TimelineContract> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineContract {
    default_timeline_status: String,
    agent_timeline_action_kind_by_event_kind: AgentTimelineActionKindByEventKind,
    agent_timeline_tool_window_kinds: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AgentTimelineActionKindByEventKind {
    title_update: String,
}

fn timeline_contract() -> &'static TimelineContract {
    TIMELINE_CONTRACT.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            TIMELINE_CONTRACT_JSON,
            "packages/contracts/src/timeline-contract.json",
        )
    })
}

pub(crate) fn default_timeline_status() -> &'static str {
    &timeline_contract().default_timeline_status
}

pub(crate) fn title_update_action_kind() -> &'static str {
    &timeline_contract()
        .agent_timeline_action_kind_by_event_kind
        .title_update
}

pub(crate) fn agent_timeline_tool_window_kinds() -> &'static [String] {
    &timeline_contract().agent_timeline_tool_window_kinds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_timeline_contract_values() {
        assert_eq!(default_timeline_status(), "info");
        assert_eq!(title_update_action_kind(), "title_update");
        assert!(agent_timeline_tool_window_kinds()
            .iter()
            .any(|kind| kind == "tool"));
    }
}
