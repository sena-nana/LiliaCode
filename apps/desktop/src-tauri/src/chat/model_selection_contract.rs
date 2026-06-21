use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;

const MODEL_SELECTION_DEFAULTS_JSON: &str =
    include_str!("../../../../../packages/contracts/src/model-selection-defaults.json");

static MODEL_SELECTION_DEFAULTS: OnceLock<ModelSelectionDefaults> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelSelectionDefaults {
    auto_models: BTreeMap<String, BTreeMap<String, String>>,
    auto_reasoning_efforts: BTreeMap<String, String>,
}

fn model_selection_defaults() -> &'static ModelSelectionDefaults {
    MODEL_SELECTION_DEFAULTS.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            MODEL_SELECTION_DEFAULTS_JSON,
            "model-selection-defaults.json",
        )
    })
}

pub(super) fn auto_model_for_tier(backend: &str, tier: &str) -> Option<&'static str> {
    model_selection_defaults()
        .auto_models
        .get(backend)
        .and_then(|tiers| tiers.get(tier))
        .map(String::as_str)
}

pub(super) fn tier_for_model(backend: &str, model: &str) -> Option<&'static str> {
    model_selection_defaults()
        .auto_models
        .get(backend)?
        .iter()
        .find_map(|(tier, tier_model)| (tier_model == model).then_some(tier.as_str()))
}

pub(super) fn auto_reasoning_effort_for_tier(tier: &str) -> Option<&'static str> {
    model_selection_defaults()
        .auto_reasoning_efforts
        .get(tier)
        .map(String::as_str)
}
