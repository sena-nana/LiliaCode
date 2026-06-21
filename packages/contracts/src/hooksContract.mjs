import hooksContract from "./hooks-contract.json" with { type: "json" };

const manifest = Object.freeze(hooksContract);

export const HOOKS_CONTRACT = manifest;
export const HOOK_BACKENDS = manifest.hookBackends;
export const HOOK_SCOPES = manifest.hookScopes;
export const HOOK_SOURCE_FORMATS = manifest.hookSourceFormats;
export const HOOK_TRUST_STATES = manifest.hookTrustStates;
export const HOOK_SCOPE_LABELS = manifest.hookScopeLabels;
export const HOOK_SOURCE_FORMAT_LABELS = manifest.hookSourceFormatLabels;
export const HOOK_TRUST_STATE_LABELS = manifest.hookTrustStateLabels;
export const HOOK_SOURCE_STATE_LABELS = manifest.hookSourceStateLabels;
export const HOOK_SOURCE_EDIT_LABELS = manifest.hookSourceEditLabels;
