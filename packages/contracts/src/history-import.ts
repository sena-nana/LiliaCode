import type { AgentTimelineEvent } from "./timeline";
import type { Task } from "./task";
import {
  CODEX_HISTORY_DEFAULT_TURN_LIMIT,
  CODEX_HISTORY_PREVIEW_TURN_LIMIT,
  historyImportProviderDisplay as historyImportProviderDisplayImpl,
  historyImportProviderUiLabels as historyImportProviderUiLabelsImpl,
  HISTORY_IMPORT_ATTACH_COMMAND,
  HISTORY_IMPORT_CLEAN_BACKGROUND_TERMINALS_COMMAND,
  HISTORY_IMPORT_DEFAULT_SEARCH_LIMIT,
  HISTORY_IMPORT_DEFAULT_SYNC_LIMIT,
  HISTORY_IMPORT_ERROR_SUMMARY_TEXT_LIMIT,
  HISTORY_IMPORT_INLINE_PREVIEW_TEXT_LIMIT,
  HISTORY_IMPORT_MAX_SEARCH_LIMIT,
  HISTORY_IMPORT_MAX_SESSION_MANAGEMENT_LIMIT,
  HISTORY_IMPORT_MAX_SYNC_LIMIT,
  HISTORY_IMPORT_MESSAGE_SUMMARY_TEXT_LIMIT,
  HISTORY_IMPORT_PREVIEW_COMMAND,
  HISTORY_IMPORT_PREVIEW_MESSAGE_LIMIT,
  HISTORY_IMPORT_PREVIEW_TEXT_LIMIT,
  HISTORY_IMPORT_PROVIDER_DISPLAY,
  HISTORY_IMPORT_PROVIDERS,
  HISTORY_IMPORT_RUNTIME_STATES_COMMAND,
  HISTORY_IMPORT_SEARCH_COMMAND,
  HISTORY_IMPORT_TITLE_TEXT_LIMIT,
  isHistoryImportProvider as isHistoryImportProviderImpl,
  type HistoryImportProvider as ContractHistoryImportProvider,
  type HistoryImportProviderDisplay as ContractHistoryImportProviderDisplay,
  type HistoryImportProviderUiLabels as ContractHistoryImportProviderUiLabels,
} from "./historyImportContract.mjs";

/**
 * Lilia desktop application protocol for importing provider history.
 * This is not an agent workflow or runtime command; UI calls the
 * history_import_* Tauri facade directly and provider-specific shapes stay
 * behind that facade.
 */
export type HistoryImportProvider = ContractHistoryImportProvider;
export type HistoryImportProviderDisplay = ContractHistoryImportProviderDisplay;
export type HistoryImportProviderUiLabels = ContractHistoryImportProviderUiLabels;

export {
  CODEX_HISTORY_DEFAULT_TURN_LIMIT,
  CODEX_HISTORY_PREVIEW_TURN_LIMIT,
  HISTORY_IMPORT_ATTACH_COMMAND,
  HISTORY_IMPORT_CLEAN_BACKGROUND_TERMINALS_COMMAND,
  HISTORY_IMPORT_DEFAULT_SEARCH_LIMIT,
  HISTORY_IMPORT_DEFAULT_SYNC_LIMIT,
  HISTORY_IMPORT_ERROR_SUMMARY_TEXT_LIMIT,
  HISTORY_IMPORT_INLINE_PREVIEW_TEXT_LIMIT,
  HISTORY_IMPORT_MAX_SEARCH_LIMIT,
  HISTORY_IMPORT_MAX_SESSION_MANAGEMENT_LIMIT,
  HISTORY_IMPORT_MAX_SYNC_LIMIT,
  HISTORY_IMPORT_MESSAGE_SUMMARY_TEXT_LIMIT,
  HISTORY_IMPORT_PREVIEW_COMMAND,
  HISTORY_IMPORT_PREVIEW_MESSAGE_LIMIT,
  HISTORY_IMPORT_PREVIEW_TEXT_LIMIT,
  HISTORY_IMPORT_PROVIDER_DISPLAY,
  HISTORY_IMPORT_PROVIDERS,
  HISTORY_IMPORT_RUNTIME_STATES_COMMAND,
  HISTORY_IMPORT_SEARCH_COMMAND,
  HISTORY_IMPORT_TITLE_TEXT_LIMIT,
};

export const isHistoryImportProvider = isHistoryImportProviderImpl as (
  value: unknown,
) => value is HistoryImportProvider;

export const historyImportProviderDisplay = historyImportProviderDisplayImpl as (
  provider: HistoryImportProvider,
) => HistoryImportProviderDisplay;

export const historyImportProviderUiLabels = historyImportProviderUiLabelsImpl as (
  provider: HistoryImportProvider,
) => HistoryImportProviderUiLabels;

export interface HistoryImportSearchInput {
  provider: HistoryImportProvider;
  searchTerm?: string | null;
  cursor?: string | null;
  limit?: number | null;
  archived?: boolean | null;
}

export interface HistoryImportRuntimeState {
  itemId: string;
  taskId: string;
  taskTitle: string;
  projectId: string | null;
  running: boolean;
  queued: boolean;
  pending: boolean;
  queuedCount: number;
}

export interface HistoryImportItem {
  id: string;
  provider: HistoryImportProvider;
  title: string;
  status: string | null;
  model: string | null;
  sourceKind: string | null;
  createdAt: number | null;
  updatedAt: number | null;
  archived: boolean;
  preview: string | null;
  cwd?: string | null;
  project?: string | null;
  runtime?: HistoryImportRuntimeState | null;
}

export interface HistoryImportSearchResult {
  items: HistoryImportItem[];
  nextCursor: string | null;
}

export interface HistoryImportPreviewMessage {
  id: string;
  role: "user" | "assistant";
  summary: string | null;
}

export interface HistoryImportPreviewInput {
  provider: HistoryImportProvider;
  itemId: string;
  detail?: "lite" | "full" | null;
}

export interface HistoryImportPreview {
  item: HistoryImportItem;
  events: AgentTimelineEvent[];
  eventCount: number;
  messages?: HistoryImportPreviewMessage[];
  hasFullPreview?: boolean;
}

export interface HistoryImportAttachInput {
  provider: HistoryImportProvider;
  itemId: string;
  taskId?: string | null;
  projectId?: string | null;
  item?: HistoryImportItem | null;
  mode: "current" | "new";
}

export interface HistoryImportAttachResult {
  taskId: string;
  projectId: string | null;
  itemId: string;
  task: Task | null;
  eventCount: number;
  historySync?: "queued" | null;
}
