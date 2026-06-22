export type HistoryImportProvider = "codex" | "claude";

export interface HistoryImportProviderDisplay {
  name: string;
  entity: string;
  supportsArchived: boolean;
}

export interface HistoryImportProviderUiLabels {
  list: string;
  preview: string;
  searchPlaceholder: string;
  loading: string;
  emptyList: string;
  emptyPreview: string;
  choosePreview: string;
}

export const HISTORY_IMPORT_CONTRACT: Record<string, unknown>;
export const HISTORY_IMPORT_PROVIDERS: readonly HistoryImportProvider[];
export const HISTORY_IMPORT_SEARCH_COMMAND: "history_import_search";
export const HISTORY_IMPORT_PREVIEW_COMMAND: "history_import_preview";
export const HISTORY_IMPORT_ATTACH_COMMAND: "history_import_attach";
export const HISTORY_IMPORT_RUNTIME_STATES_COMMAND: "history_import_runtime_states";
export const HISTORY_IMPORT_CLEAN_BACKGROUND_TERMINALS_COMMAND: "history_import_clean_background_terminals";
export const HISTORY_IMPORT_DEFAULT_SEARCH_LIMIT: number;
export const HISTORY_IMPORT_MAX_SEARCH_LIMIT: number;
export const HISTORY_IMPORT_MAX_SESSION_MANAGEMENT_LIMIT: number;
export const HISTORY_IMPORT_DEFAULT_SYNC_LIMIT: number;
export const HISTORY_IMPORT_MAX_SYNC_LIMIT: number;
export const CODEX_HISTORY_DEFAULT_TURN_LIMIT: number;
export const CODEX_HISTORY_PREVIEW_TURN_LIMIT: number;
export const HISTORY_IMPORT_PREVIEW_MESSAGE_LIMIT: number;
export const HISTORY_IMPORT_TITLE_TEXT_LIMIT: number;
export const HISTORY_IMPORT_PREVIEW_TEXT_LIMIT: number;
export const HISTORY_IMPORT_INLINE_PREVIEW_TEXT_LIMIT: number;
export const HISTORY_IMPORT_MESSAGE_SUMMARY_TEXT_LIMIT: number;
export const HISTORY_IMPORT_ERROR_SUMMARY_TEXT_LIMIT: number;
export const HISTORY_IMPORT_PROVIDER_DISPLAY: Readonly<
  Record<HistoryImportProvider, HistoryImportProviderDisplay>
>;
export const HISTORY_IMPORT_PROVIDER_UI_LABEL_TEMPLATES: Readonly<
  Record<keyof HistoryImportProviderUiLabels, string>
>;
export function isHistoryImportProvider(
  value: unknown,
): value is HistoryImportProvider;
export function historyImportProviderDisplay(
  provider: HistoryImportProvider,
): HistoryImportProviderDisplay;
export function historyImportProviderUiLabels(
  provider: HistoryImportProvider,
): HistoryImportProviderUiLabels;
