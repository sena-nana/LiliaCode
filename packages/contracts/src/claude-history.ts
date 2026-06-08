import type { AgentTimelineEvent } from "./timeline";
import type { Task } from "./task";

export interface ClaudeSessionSearchInput {
  searchTerm?: string | null;
  cursor?: string | null;
  limit?: number | null;
  archived?: boolean | null;
}

export interface ClaudeSessionSummary {
  id: string;
  title: string;
  status: string | null;
  model: string | null;
  sourceKind: string | null;
  createdAt: number | null;
  updatedAt: number | null;
  archived: boolean;
  preview: string | null;
  cwd: string | null;
  project: string | null;
}

export interface ClaudeSessionSearchResult {
  sessions: ClaudeSessionSummary[];
  nextCursor: string | null;
}

export interface ClaudeSessionPreviewMessage {
  id: string;
  role: "user" | "assistant";
  summary: string | null;
}

export interface ClaudeSessionPreviewInput {
  sessionId: string;
  detail?: "lite" | "full" | null;
}

export interface ClaudeSessionPreview {
  session: ClaudeSessionSummary;
  events: AgentTimelineEvent[];
  eventCount: number;
  messages?: ClaudeSessionPreviewMessage[];
  hasFullPreview?: boolean;
}

export interface ClaudeSessionAttachInput {
  sessionId: string;
  taskId?: string | null;
  projectId?: string | null;
  session?: ClaudeSessionSummary | null;
  mode: "current" | "new";
}

export interface ClaudeSessionAttachResult {
  taskId: string;
  projectId: string | null;
  sessionId: string;
  task: Task | null;
  eventCount: number;
  historySync?: "queued" | null;
}
