import historyImportContract from "./history-import-contract.json" with { type: "json" };

const manifest = deepFreeze(historyImportContract);

export const HISTORY_IMPORT_CONTRACT = manifest;
export const HISTORY_IMPORT_PROVIDERS = manifest.providers;
export const HISTORY_IMPORT_SEARCH_COMMAND = manifest.commands.search;
export const HISTORY_IMPORT_PREVIEW_COMMAND = manifest.commands.preview;
export const HISTORY_IMPORT_ATTACH_COMMAND = manifest.commands.attach;
export const HISTORY_IMPORT_RUNTIME_STATES_COMMAND = manifest.commands.runtimeStates;
export const HISTORY_IMPORT_CLEAN_BACKGROUND_TERMINALS_COMMAND =
  manifest.commands.cleanBackgroundTerminals;
export const HISTORY_IMPORT_DEFAULT_SEARCH_LIMIT = manifest.limits.defaultSearchLimit;
export const HISTORY_IMPORT_MAX_SEARCH_LIMIT = manifest.limits.maxSearchLimit;
export const HISTORY_IMPORT_MAX_SESSION_MANAGEMENT_LIMIT =
  manifest.limits.maxSessionManagementLimit;
export const HISTORY_IMPORT_DEFAULT_SYNC_LIMIT = manifest.limits.defaultSyncLimit;
export const HISTORY_IMPORT_MAX_SYNC_LIMIT = manifest.limits.maxSyncLimit;
export const CODEX_HISTORY_DEFAULT_TURN_LIMIT = manifest.limits.codexDefaultTurnLimit;
export const CODEX_HISTORY_PREVIEW_TURN_LIMIT = manifest.limits.codexPreviewTurnLimit;
export const HISTORY_IMPORT_PREVIEW_MESSAGE_LIMIT = manifest.limits.previewMessageLimit;
export const HISTORY_IMPORT_TITLE_TEXT_LIMIT = manifest.textLimits.title;
export const HISTORY_IMPORT_PREVIEW_TEXT_LIMIT = manifest.textLimits.preview;
export const HISTORY_IMPORT_INLINE_PREVIEW_TEXT_LIMIT = manifest.textLimits.inlinePreview;
export const HISTORY_IMPORT_MESSAGE_SUMMARY_TEXT_LIMIT = manifest.textLimits.messageSummary;
export const HISTORY_IMPORT_ERROR_SUMMARY_TEXT_LIMIT = manifest.textLimits.errorSummary;
export const HISTORY_IMPORT_PROVIDER_DISPLAY = manifest.display;
export const HISTORY_IMPORT_PROVIDER_UI_LABEL_TEMPLATES = manifest.uiLabelTemplates;

const providerSet = new Set(HISTORY_IMPORT_PROVIDERS);

export function isHistoryImportProvider(value) {
  return typeof value === "string" && providerSet.has(value);
}

export function historyImportProviderDisplay(provider) {
  return HISTORY_IMPORT_PROVIDER_DISPLAY[provider];
}

export function historyImportProviderUiLabels(provider) {
  const display = historyImportProviderDisplay(provider);
  return Object.fromEntries(
    Object.entries(HISTORY_IMPORT_PROVIDER_UI_LABEL_TEMPLATES).map(([key, template]) => [
      key,
      template
        .replaceAll("{name}", display.name)
        .replaceAll("{entity}", display.entity),
    ]),
  );
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
