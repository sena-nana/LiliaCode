import type { AutomationNodeKind, AutomationScopeFilter, ChatBackendKind } from "@lilia/contracts";

export const DEFAULT_SCOPE: AutomationScopeFilter = {
  projectIds: [],
  includeInbox: true,
  taskStatuses: [],
  backends: [],
  eventKinds: [],
};

export const NODE_KIND_LABELS: Record<AutomationNodeKind, string> = {
  trigger: "事件触发",
  agent: "Agent 调用",
  logic: "逻辑",
  tool: "工具",
  human: "人工确认",
};

export const TASK_STATUS_OPTIONS = ["waiting", "running", "done", "blocked"] as const;
export const BACKEND_OPTIONS = ["claude", "codex"] as const satisfies readonly ChatBackendKind[];
export const TRIGGER_EVENT_KIND_OPTIONS = [
  "task_created",
  "task_status_changed",
  "task_updated",
  "timeline_event",
  "todo_changed",
  "interaction_request",
] as const;
export const PRIORITY_OPTIONS = ["low", "normal", "high"] as const;
