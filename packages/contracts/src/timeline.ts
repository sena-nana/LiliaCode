import type { ChatBackendKind } from "./chat";

export type AgentTimelineKnownEventKind =
  | "message"
  | "reasoning"
  | "plan"
  | "todo_list"
  | "tool"
  | "ask_user"
  | "command"
  | "subagent"
  | "file_change"
  | "file_read"
  | "search"
  | "web_fetch"
  | "mcp"
  | "web_search"
  | "error"
  | "turn";

export type AgentTimelineEventKind = AgentTimelineKnownEventKind | (string & {});

export type AgentTimelineEventStatus =
  | "pending"
  | "started"
  | "running"
  | "in_progress"
  | "completed"
  | "done"
  | "success"
  | "failed"
  | "error"
  | "cancelled"
  | "skipped"
  | "info"
  | "requires_action";

export type AgentTimelinePayload =
  | null
  | boolean
  | number
  | string
  | AgentTimelinePayload[]
  | { [key: string]: AgentTimelinePayload };

export type AgentTimelineDisplayIcon = string;

export type AgentTimelineDisplayBucket =
  | "command"
  | "file"
  | "plan"
  | "todo"
  | "tool"
  | "ask_user"
  | "mcp"
  | "search"
  | "web_search"
  | "subagent"
  | "error"
  | "other"
  | (string & {});

export interface AgentTimelineDisplayGroup {
  key: string;
  bucket?: AgentTimelineDisplayBucket | null;
  unit?: string | null;
  count?: number | null;
}

export interface AgentTimelineDisplayField {
  label: string;
  value: string;
}

export interface AgentTimelineDisplayListItem {
  text: string;
  tone?: "default" | "muted" | "success" | "warning" | "error" | null;
}

export type AgentTimelineDisplayDetail =
  | {
      type: "line";
      text: string;
      tone?: "default" | "muted" | null;
    }
  | {
      type: "fields";
      fields: AgentTimelineDisplayField[];
    }
  | {
      type: "code";
      label?: string | null;
      content: string;
      language?: string | null;
    }
  | {
      type: "markdown";
      content: string;
      tone?: "default" | "muted" | null;
      singleLine?: boolean | null;
    }
  | {
      type: "list";
      items: AgentTimelineDisplayListItem[];
      ordered?: boolean | null;
    };

export interface AgentTimelineDisplay {
  icon?: AgentTimelineDisplayIcon | null;
  label?: string | null;
  action?: string | null;
  object?: string | null;
  objectInLabel?: boolean | null;
  preview?: string | null;
  details?: AgentTimelineDisplayDetail[] | null;
  group?: AgentTimelineDisplayGroup | null;
  defaultExpanded?: boolean | null;
}

export interface AgentTimelineEvent {
  id: string;
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  kind: AgentTimelineEventKind;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string | null;
  payload: AgentTimelinePayload;
  createdAt: number;
  updatedAt: number;
  turnSeq: number;
  intraTurnOrder: number;
}
