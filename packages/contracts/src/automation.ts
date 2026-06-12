import type { ChatBackendKind, ChatComposerState } from "./chat";

export type AutomationNodeKind =
  | "trigger"
  | "agent"
  | "logic"
  | "tool"
  | "human";

export type AutomationTriggerKind =
  | "manual"
  | "task_changed"
  | "timeline_event"
  | "todo_changed"
  | "interaction_request";

export type AutomationRunStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "skipped"
  | "waiting_user";

export interface AutomationScopeFilter {
  projectIds: string[];
  includeInbox: boolean;
  taskStatuses: string[];
  backends: ChatBackendKind[];
  eventKinds: string[];
}

export interface AutomationTrigger {
  kind: AutomationTriggerKind;
  eventKinds?: string[];
}

export interface AutomationNodePosition {
  x: number;
  y: number;
}

export interface AutomationNode {
  id: string;
  kind: AutomationNodeKind;
  title: string;
  position: AutomationNodePosition;
  config: Record<string, unknown>;
}

export interface AutomationEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

export interface AutomationWorkflowDraft {
  nodes: AutomationNode[];
  edges: AutomationEdge[];
  scope: AutomationScopeFilter;
}

export interface AutomationWorkflow {
  id: string;
  name: string;
  enabled: boolean;
  scope: AutomationScopeFilter;
  draft: AutomationWorkflowDraft;
  publishedVersionId: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface AutomationWorkflowVersion {
  id: string;
  workflowId: string;
  version: number;
  snapshot: AutomationWorkflowDraft;
  createdAt: number;
}

export interface AutomationSignalEnvelope {
  id: string;
  kind: AutomationTriggerKind;
  projectId?: string | null;
  taskId?: string | null;
  backend?: ChatBackendKind | null;
  eventKind?: string | null;
  automationRunId?: string | null;
  payload: Record<string, unknown>;
  createdAt: number;
}

export interface AutomationRun {
  id: string;
  workflowId: string;
  workflowVersionId: string;
  status: AutomationRunStatus;
  trigger: AutomationSignalEnvelope;
  scope: AutomationScopeFilter;
  startedAt: number;
  finishedAt: number | null;
  error: string | null;
}

export interface AutomationRunNodeState {
  id: string;
  runId: string;
  nodeId: string;
  status: AutomationRunStatus;
  input: Record<string, unknown>;
  output: Record<string, unknown> | null;
  error: string | null;
  startedAt: number | null;
  finishedAt: number | null;
}

export interface AutomationSaveDraftInput {
  id?: string | null;
  name: string;
  scope: AutomationScopeFilter;
  nodes: AutomationNode[];
  edges: AutomationEdge[];
}

export interface AutomationRunOnceInput {
  payload?: Record<string, unknown> | null;
}

export interface AutomationResumeRunInput {
  nodeId?: string | null;
  payload?: Record<string, unknown> | null;
}

export interface AutomationAgentNodeConfig {
  taskId?: string | null;
  projectId?: string | null;
  title?: string | null;
  prompt?: string | null;
  projectCwd?: string | null;
  composer?: Partial<ChatComposerState> | null;
  createTask?: boolean | null;
}
