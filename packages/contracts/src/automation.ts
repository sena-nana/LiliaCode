import {
  AUTOMATION_CHANGED_EVENT_NAME,
  AUTOMATION_DELETE_WORKFLOW_COMMAND,
  AUTOMATION_GET_RUN_COMMAND,
  AUTOMATION_LIST_RUNS_COMMAND,
  AUTOMATION_LIST_WORKFLOWS_COMMAND,
  AUTOMATION_LOGIC_KINDS,
  AUTOMATION_LOGIC_KIND_LABELS,
  AUTOMATION_PUBLISH_COMMAND,
  AUTOMATION_RESUME_RUN_COMMAND,
  AUTOMATION_RUN_EVENT_NAMES,
  AUTOMATION_RUN_FINISHED_EVENT_NAME,
  AUTOMATION_RUN_ONCE_COMMAND,
  AUTOMATION_RUN_STARTED_EVENT_NAME,
  AUTOMATION_RUN_STATUSES,
  AUTOMATION_RUN_STATUS_TONES,
  AUTOMATION_RUN_UPDATED_EVENT_NAME,
  AUTOMATION_SAVE_DRAFT_COMMAND,
  AUTOMATION_SCOPE_EVENT_KINDS,
  AUTOMATION_SCOPE_TASK_STATUSES,
  AUTOMATION_SET_ENABLED_COMMAND,
  AUTOMATION_TOOL_ACTIONS,
  AUTOMATION_TOOL_ACTION_FIELDS,
  AUTOMATION_TOOL_ACTION_LABELS,
  AUTOMATION_TOOL_PRIORITIES,
  AUTOMATION_TRIGGER_KINDS,
  AUTOMATION_TRIGGER_KIND_LABELS,
  AUTOMATION_WAITING_USER_STATUS,
  DEFAULT_AUTOMATION_AGENT_PROMPT,
  DEFAULT_AUTOMATION_HUMAN_PROMPT,
  DEFAULT_AUTOMATION_LOGIC_KIND,
  DEFAULT_AUTOMATION_LOGIC_PATH,
  DEFAULT_AUTOMATION_RUN_STATUS,
  DEFAULT_AUTOMATION_TOOL_ACTION,
  DEFAULT_AUTOMATION_TOOL_PRIORITY,
  DEFAULT_AUTOMATION_TRIGGER_KIND,
} from "./automationContract.mjs";
import { isChatBackendKind, type ChatBackendKind, type ChatComposerState } from "./chat";
import type { TaskStatus } from "./task";

export type AutomationNodeKind =
  | "trigger"
  | "agent"
  | "logic"
  | "tool"
  | "human";

export const AUTOMATION_NODE_KIND_LABELS: Record<AutomationNodeKind, string> = {
  trigger: "事件触发",
  agent: "Agent 调用",
  logic: "逻辑",
  tool: "工具",
  human: "人工确认",
};

export function automationNodeKindLabel(kind: AutomationNodeKind): string {
  return AUTOMATION_NODE_KIND_LABELS[kind];
}

export type AutomationTriggerKind = typeof AUTOMATION_TRIGGER_KINDS[number];
export type AutomationScopeEventKind = typeof AUTOMATION_SCOPE_EVENT_KINDS[number];
export type AutomationScopeTaskStatus = typeof AUTOMATION_SCOPE_TASK_STATUSES[number] & TaskStatus;
export type AutomationLogicKind = typeof AUTOMATION_LOGIC_KINDS[number];
export type AutomationRunStatus = typeof AUTOMATION_RUN_STATUSES[number];
type AutomationStatusTone = (typeof AUTOMATION_RUN_STATUS_TONES)[AutomationRunStatus];
export type AutomationToolAction = typeof AUTOMATION_TOOL_ACTIONS[number];
export type AutomationToolConfigField =
  (typeof AUTOMATION_TOOL_ACTION_FIELDS)[AutomationToolAction][number];
export type AutomationToolPriority = typeof AUTOMATION_TOOL_PRIORITIES[number];

export {
  AUTOMATION_CHANGED_EVENT_NAME,
  AUTOMATION_DELETE_WORKFLOW_COMMAND,
  AUTOMATION_GET_RUN_COMMAND,
  AUTOMATION_LIST_RUNS_COMMAND,
  AUTOMATION_LIST_WORKFLOWS_COMMAND,
  AUTOMATION_LOGIC_KINDS,
  AUTOMATION_LOGIC_KIND_LABELS,
  AUTOMATION_PUBLISH_COMMAND,
  AUTOMATION_RESUME_RUN_COMMAND,
  AUTOMATION_RUN_EVENT_NAMES,
  AUTOMATION_RUN_FINISHED_EVENT_NAME,
  AUTOMATION_RUN_ONCE_COMMAND,
  AUTOMATION_RUN_STARTED_EVENT_NAME,
  AUTOMATION_RUN_STATUSES,
  AUTOMATION_RUN_STATUS_TONES,
  AUTOMATION_RUN_UPDATED_EVENT_NAME,
  AUTOMATION_SAVE_DRAFT_COMMAND,
  AUTOMATION_SCOPE_EVENT_KINDS,
  AUTOMATION_SCOPE_TASK_STATUSES,
  AUTOMATION_SET_ENABLED_COMMAND,
  AUTOMATION_TOOL_ACTIONS,
  AUTOMATION_TOOL_ACTION_FIELDS,
  AUTOMATION_TOOL_ACTION_LABELS,
  AUTOMATION_TOOL_PRIORITIES,
  AUTOMATION_TRIGGER_KINDS,
  AUTOMATION_TRIGGER_KIND_LABELS,
  AUTOMATION_WAITING_USER_STATUS,
  DEFAULT_AUTOMATION_AGENT_PROMPT,
  DEFAULT_AUTOMATION_HUMAN_PROMPT,
  DEFAULT_AUTOMATION_LOGIC_KIND,
  DEFAULT_AUTOMATION_LOGIC_PATH,
  DEFAULT_AUTOMATION_RUN_STATUS,
  DEFAULT_AUTOMATION_TOOL_ACTION,
  DEFAULT_AUTOMATION_TOOL_PRIORITY,
  DEFAULT_AUTOMATION_TRIGGER_KIND,
};

const AUTOMATION_TOOL_ACTION_SET = new Set<string>(AUTOMATION_TOOL_ACTIONS);
const AUTOMATION_TOOL_PRIORITY_SET = new Set<string>(AUTOMATION_TOOL_PRIORITIES);
const AUTOMATION_LOGIC_KIND_SET = new Set<string>(AUTOMATION_LOGIC_KINDS);
const AUTOMATION_RUN_STATUS_SET = new Set<string>(AUTOMATION_RUN_STATUSES);

export function automationTriggerKindLabel(kind: AutomationTriggerKind): string {
  return AUTOMATION_TRIGGER_KIND_LABELS[kind];
}

export function isAutomationLogicKind(value: unknown): value is AutomationLogicKind {
  return typeof value === "string" && AUTOMATION_LOGIC_KIND_SET.has(value);
}

export function normalizeAutomationLogicKind(
  value: unknown,
  fallback: AutomationLogicKind = DEFAULT_AUTOMATION_LOGIC_KIND,
): AutomationLogicKind {
  return isAutomationLogicKind(value) ? value : fallback;
}

export function automationLogicKindLabel(kind: AutomationLogicKind): string {
  return AUTOMATION_LOGIC_KIND_LABELS[kind];
}

export function isAutomationRunStatus(value: unknown): value is AutomationRunStatus {
  return typeof value === "string" && AUTOMATION_RUN_STATUS_SET.has(value);
}

export function normalizeAutomationRunStatus(
  value: unknown,
  fallback: AutomationRunStatus = DEFAULT_AUTOMATION_RUN_STATUS,
): AutomationRunStatus {
  return isAutomationRunStatus(value) ? value : fallback;
}

export function automationRunStatusTone(status: AutomationRunStatus): AutomationStatusTone {
  return AUTOMATION_RUN_STATUS_TONES[status];
}

export function isAutomationToolAction(value: unknown): value is AutomationToolAction {
  return typeof value === "string" && AUTOMATION_TOOL_ACTION_SET.has(value);
}

export function normalizeAutomationToolAction(
  value: unknown,
  fallback: AutomationToolAction = DEFAULT_AUTOMATION_TOOL_ACTION,
): AutomationToolAction {
  return isAutomationToolAction(value) ? value : fallback;
}

export function automationToolActionLabel(action: AutomationToolAction): string {
  return AUTOMATION_TOOL_ACTION_LABELS[action];
}

export function automationToolActionUsesField(
  action: AutomationToolAction,
  field: AutomationToolConfigField,
): boolean {
  return AUTOMATION_TOOL_ACTION_FIELDS[action].includes(field);
}

export function isAutomationToolPriority(value: unknown): value is AutomationToolPriority {
  return typeof value === "string" && AUTOMATION_TOOL_PRIORITY_SET.has(value);
}

export function normalizeAutomationToolPriority(
  value: unknown,
  fallback: AutomationToolPriority = DEFAULT_AUTOMATION_TOOL_PRIORITY,
): AutomationToolPriority {
  return isAutomationToolPriority(value) ? value : fallback;
}

export interface AutomationScopeFilter {
  projectIds: string[];
  includeInbox: boolean;
  taskStatuses: string[];
  backends: ChatBackendKind[];
  eventKinds: string[];
}

export const DEFAULT_AUTOMATION_SCOPE = {
  projectIds: [],
  includeInbox: true,
  taskStatuses: [],
  backends: [],
  eventKinds: [],
} satisfies AutomationScopeFilter;

function normalizeStringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const items = value
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean);
  return [...new Set(items)];
}

function normalizeBackendList(value: unknown): ChatBackendKind[] {
  return normalizeStringList(value).filter(isChatBackendKind);
}

function normalizeAutomationTaskStatusList(value: unknown): AutomationScopeTaskStatus[] {
  return normalizeStringList(value).filter((status): status is AutomationScopeTaskStatus =>
    AUTOMATION_SCOPE_TASK_STATUSES.includes(status as AutomationScopeTaskStatus)
  );
}

function normalizeAutomationEventKindList(value: unknown): AutomationScopeEventKind[] {
  return normalizeStringList(value).filter((kind): kind is AutomationScopeEventKind =>
    AUTOMATION_SCOPE_EVENT_KINDS.includes(kind as AutomationScopeEventKind)
  );
}

export function normalizeAutomationScope(
  input: Partial<AutomationScopeFilter> | null | undefined,
  base: AutomationScopeFilter = DEFAULT_AUTOMATION_SCOPE,
): AutomationScopeFilter {
  return {
    projectIds: input?.projectIds === undefined
      ? [...base.projectIds]
      : normalizeStringList(input.projectIds),
    includeInbox: typeof input?.includeInbox === "boolean" ? input.includeInbox : base.includeInbox,
    taskStatuses: input?.taskStatuses === undefined
      ? [...base.taskStatuses]
      : normalizeAutomationTaskStatusList(input.taskStatuses),
    backends: input?.backends === undefined
      ? [...base.backends]
      : normalizeBackendList(input.backends),
    eventKinds: input?.eventKinds === undefined
      ? [...base.eventKinds]
      : normalizeAutomationEventKindList(input.eventKinds),
  };
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

export interface AutomationChangedEvent {
  workflowId: string | null;
}

export type AutomationRunEventName = typeof AUTOMATION_RUN_EVENT_NAMES[number];

export interface AutomationRunEvent {
  run: AutomationRun;
}

export function createAutomationChangedEvent(
  workflowId: string | null | undefined,
): AutomationChangedEvent {
  return { workflowId: workflowId ?? null };
}

export function createAutomationRunEvent(run: AutomationRun): AutomationRunEvent {
  return { run };
}

export interface AutomationRunSummary {
  id: string;
  workflowId: string;
  workflowVersionId: string;
  status: AutomationRunStatus;
  triggerKind: AutomationTriggerKind;
  projectId?: string | null;
  taskId?: string | null;
  backend?: ChatBackendKind | null;
  eventKind?: string | null;
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
