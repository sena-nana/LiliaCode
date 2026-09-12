use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryDisplayContract {
    memory_scope_titles: BTreeMap<String, String>,
    memory_scope_empty_labels: BTreeMap<String, String>,
}

pub struct MemoryScopeDisplay {
    pub title: &'static str,
    pub empty: &'static str,
}

pub fn memory_scope_display(scope: &str) -> Option<MemoryScopeDisplay> {
    static CONTRACT: OnceLock<MemoryDisplayContract> = OnceLock::new();
    let contract = CONTRACT.get_or_init(|| {
        serde_json::from_str(include_str!("../contracts/task-statuses.json"))
            .expect("valid embedded task status contract")
    });
    Some(MemoryScopeDisplay {
        title: contract.memory_scope_titles.get(scope)?.as_str(),
        empty: contract.memory_scope_empty_labels.get(scope)?.as_str(),
    })
}

pub const MEMORY_TURN_INJECTION_CONTRACT: &str =
    include_str!("../contracts/memory-turn-injection.json");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTurnInjection {
    pub task_id: String,
    pub turn_id: String,
    pub turn_sequence: i64,
    pub memory_ids: Vec<String>,
    pub baseline: Option<String>,
}
