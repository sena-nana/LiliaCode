import type { AgentTimelineEvent } from "./timeline";
import type { Task } from "./task";

export type HistoryImportProvider = "codex" | "claude";

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
