import type { AgentTimelineEvent } from "./timeline";
import type { Task } from "./task";

export interface CodexThreadSearchInput {
  searchTerm?: string | null;
  cursor?: string | null;
  limit?: number | null;
  archived?: boolean | null;
}

export interface CodexThreadSummary {
  id: string;
  title: string;
  status: string | null;
  model: string | null;
  sourceKind: string | null;
  createdAt: number | null;
  updatedAt: number | null;
  archived: boolean;
  preview: string | null;
}

export interface CodexThreadSearchResult {
  threads: CodexThreadSummary[];
  nextCursor: string | null;
}

export interface CodexThreadPreviewMessage {
  id: string;
  role: "user" | "assistant";
  summary: string | null;
}

export interface CodexThreadPreviewLite {
  thread: CodexThreadSummary;
  eventCount: number;
  messages: CodexThreadPreviewMessage[];
  hasFullPreview: boolean;
}

export interface CodexThreadPreviewInput {
  threadId: string;
  detail?: "lite" | "full" | null;
}

export interface CodexThreadPreview {
  thread: CodexThreadSummary;
  events: AgentTimelineEvent[];
  eventCount: number;
  messages?: CodexThreadPreviewMessage[];
  hasFullPreview?: boolean;
}

export interface CodexThreadAttachInput {
  threadId: string;
  taskId?: string | null;
  projectId?: string | null;
  thread?: CodexThreadSummary | null;
  mode: "current" | "new";
}

export interface CodexThreadAttachResult {
  taskId: string;
  projectId: string | null;
  threadId: string;
  task: Task | null;
  eventCount: number;
  historySync?: "queued" | null;
}
