use std::sync::OnceLock;

use serde::Deserialize;

const PROVIDER_CODEX_JSON: &str =
    include_str!("../../../../../packages/contracts/src/provider-codex.json");
const PERMISSION_MODES_JSON: &str =
    include_str!("../../../../../packages/contracts/src/permission-modes.json");

static PROVIDER_CODEX: OnceLock<ProviderCodexManifest> = OnceLock::new();
static PERMISSION_MODES: OnceLock<PermissionModesManifest> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderCodexManifest {
    codex_settings_profiles: Vec<String>,
    default_codex_settings_profile: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionModesManifest {
    permission_modes: Vec<String>,
    default_permission_mode: String,
}

fn provider_codex_manifest() -> &'static ProviderCodexManifest {
    PROVIDER_CODEX.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(PROVIDER_CODEX_JSON, "provider-codex.json")
    })
}

fn permission_modes_manifest() -> &'static PermissionModesManifest {
    PERMISSION_MODES.get_or_init(|| {
        crate::contract_manifest::parse_contract_json(
            PERMISSION_MODES_JSON,
            "permission-modes.json",
        )
    })
}

pub(super) fn codex_settings_profiles() -> &'static [String] {
    &provider_codex_manifest().codex_settings_profiles
}

pub(super) fn default_codex_settings_profile() -> &'static str {
    &provider_codex_manifest().default_codex_settings_profile
}

pub(super) fn permission_modes() -> &'static [String] {
    &permission_modes_manifest().permission_modes
}

pub(super) fn default_permission_mode() -> &'static str {
    &permission_modes_manifest().default_permission_mode
}
