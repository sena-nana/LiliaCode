import { CHAT_BACKENDS, normalizeChatBackendKind, type ChatBackendKind } from "./chat";
import {
  architectureChangeDisplayText,
  type ArchitectureChangeDisplayOptions,
} from "./architectureDisplay.mjs";
import {
  ARCHITECTURE_CLAUDE_TOOL_NAME,
  ARCHITECTURE_MCP_TOOL_NAME,
  ARCHITECTURE_TOOL_NAME,
  ARCHITECTURE_TOOL_NAMES,
  isLiliaArchitectureTool as isLiliaArchitectureToolImpl,
  isProjectArchitectureChangeStatus as isProjectArchitectureChangeStatusImpl,
  isProjectArchitecturePermission as isProjectArchitecturePermissionImpl,
  normalizeProjectArchitectureChangeStatus as normalizeProjectArchitectureChangeStatusImpl,
  normalizeProjectArchitecturePermission as normalizeProjectArchitecturePermissionImpl,
  PROJECT_ARCHITECTURE_APPLIED_INTERACTION_STATUS,
  PROJECT_ARCHITECTURE_APPLY_COMMAND,
  PROJECT_ARCHITECTURE_CHANGE_STATUSES,
  PROJECT_ARCHITECTURE_DEFAULT_CHANGE_STATUS,
  PROJECT_ARCHITECTURE_DEFAULT_EDGE_TYPE,
  PROJECT_ARCHITECTURE_DEFAULT_INTERACTION_STATUS,
  PROJECT_ARCHITECTURE_DEFAULT_NODE_TYPE,
  PROJECT_ARCHITECTURE_DEFAULT_PERMISSION,
  PROJECT_ARCHITECTURE_GET_COMMAND,
  PROJECT_ARCHITECTURE_LIST_CHANGES_COMMAND,
  PROJECT_ARCHITECTURE_PERMISSIONS,
  PROJECT_ARCHITECTURE_READONLY_INTERACTION_STATUS,
  PROJECT_ARCHITECTURE_REJECT_COMMAND,
  PROJECT_ARCHITECTURE_ROLLBACK_COMMAND,
  PROJECT_ARCHITECTURE_ROLLBACK_PERMISSION,
  projectArchitecturePermissionFromRuntimePermission as projectArchitecturePermissionFromRuntimePermissionImpl,
  PROJECT_ARCHITECTURE_RUNTIME_PERMISSION_MAP,
  UPDATE_PROJECT_ARCHITECTURE_INPUT_SCHEMA,
  type ArchitectureToolName as ContractArchitectureToolName,
  type ProjectArchitectureChangeStatus as ContractProjectArchitectureChangeStatus,
  type ProjectArchitecturePermission as ContractProjectArchitecturePermission,
} from "./architectureContract.mjs";

type ProjectArchitectureInteractionStatus = "pending" | "proposed" | "applied";

export type ProjectArchitectureNodeType =
  | "module"
  | "service"
  | "component"
  | "data"
  | "external"
  | "workflow"
  | (string & {});

export type ProjectArchitectureEdgeType =
  | "depends_on"
  | "calls"
  | "reads"
  | "writes"
  | "emits"
  | "renders"
  | "owns"
  | (string & {});

export interface ProjectArchitectureNode {
  id: string;
  label: string;
  type: ProjectArchitectureNodeType;
  summary: string;
  paths: string[];
  tags: string[];
}

export interface ProjectArchitectureEdge {
  id: string;
  from: string;
  to: string;
  type: ProjectArchitectureEdgeType;
  label: string;
  summary: string;
}

export interface ProjectArchitectureGraph {
  projectId: string;
  version: number;
  summary: string;
  nodes: ProjectArchitectureNode[];
  edges: ProjectArchitectureEdge[];
  updatedAt: number;
}

export type ProjectArchitectureChange =
  | {
      type: "upsert_node";
      node: ProjectArchitectureNode;
    }
  | {
      type: "remove_node";
      nodeId: string;
    }
  | {
      type: "upsert_edge";
      edge: ProjectArchitectureEdge;
    }
  | {
      type: "remove_edge";
      edgeId: string;
    }
  | {
      type: "set_summary";
      summary: string;
    };

export type ProjectArchitecturePermission = ContractProjectArchitecturePermission;
export type ArchitectureToolName = ContractArchitectureToolName;

export const PROJECT_ARCHITECTURE_BACKENDS = CHAT_BACKENDS;

export {
  ARCHITECTURE_CLAUDE_TOOL_NAME,
  ARCHITECTURE_MCP_TOOL_NAME,
  ARCHITECTURE_TOOL_NAME,
  ARCHITECTURE_TOOL_NAMES,
  PROJECT_ARCHITECTURE_APPLIED_INTERACTION_STATUS,
  PROJECT_ARCHITECTURE_APPLY_COMMAND,
  PROJECT_ARCHITECTURE_CHANGE_STATUSES,
  PROJECT_ARCHITECTURE_DEFAULT_CHANGE_STATUS,
  PROJECT_ARCHITECTURE_DEFAULT_EDGE_TYPE,
  PROJECT_ARCHITECTURE_DEFAULT_INTERACTION_STATUS,
  PROJECT_ARCHITECTURE_DEFAULT_NODE_TYPE,
  PROJECT_ARCHITECTURE_DEFAULT_PERMISSION,
  PROJECT_ARCHITECTURE_GET_COMMAND,
  PROJECT_ARCHITECTURE_LIST_CHANGES_COMMAND,
  PROJECT_ARCHITECTURE_PERMISSIONS,
  PROJECT_ARCHITECTURE_READONLY_INTERACTION_STATUS,
  PROJECT_ARCHITECTURE_REJECT_COMMAND,
  PROJECT_ARCHITECTURE_ROLLBACK_COMMAND,
  PROJECT_ARCHITECTURE_ROLLBACK_PERMISSION,
  PROJECT_ARCHITECTURE_RUNTIME_PERMISSION_MAP,
  UPDATE_PROJECT_ARCHITECTURE_INPUT_SCHEMA,
};

export const isProjectArchitecturePermission =
  isProjectArchitecturePermissionImpl as (
    value: unknown,
  ) => value is ProjectArchitecturePermission;

export const normalizeProjectArchitecturePermission =
  normalizeProjectArchitecturePermissionImpl as (
    value: unknown,
    fallback?: ProjectArchitecturePermission,
  ) => ProjectArchitecturePermission;

export const projectArchitecturePermissionFromRuntimePermission =
  projectArchitecturePermissionFromRuntimePermissionImpl as (
    value: unknown,
  ) => ProjectArchitecturePermission;

export type ProjectArchitectureChangeStatus = ContractProjectArchitectureChangeStatus;

export const isProjectArchitectureChangeStatus =
  isProjectArchitectureChangeStatusImpl as (
    value: unknown,
  ) => value is ProjectArchitectureChangeStatus;

export const normalizeProjectArchitectureChangeStatus =
  normalizeProjectArchitectureChangeStatusImpl as (
    value: unknown,
    fallback?: ProjectArchitectureChangeStatus,
  ) => ProjectArchitectureChangeStatus;

export const isLiliaArchitectureTool = isLiliaArchitectureToolImpl as (
  toolName: unknown,
) => toolName is ArchitectureToolName;

export interface ProjectArchitectureChangeEvent {
  id?: string;
  projectId: string;
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  permission: ProjectArchitecturePermission;
  status: ProjectArchitectureChangeStatus;
  reason: string;
  changes: ProjectArchitectureChange[];
  beforeVersion: number;
  afterVersion: number | null;
  createdAt?: number;
  resolvedAt?: number | null;
}

export interface ProjectArchitectureApplyInput {
  projectId: string;
  taskId: string;
  turnId?: string | null;
  backend: ChatBackendKind;
  permission: ProjectArchitecturePermission;
  reason: string;
  changes: ProjectArchitectureChange[];
  requestId?: string | null;
}

export interface ProjectArchitectureApplyResult {
  graph: ProjectArchitectureGraph;
  event: ProjectArchitectureChangeEvent;
}

export interface ProjectArchitectureRejectInput {
  projectId: string;
  taskId: string;
  turnId?: string | null;
  backend: ChatBackendKind;
  permission: ProjectArchitecturePermission;
  reason: string;
  changes: ProjectArchitectureChange[];
  requestId?: string | null;
}

export interface ProjectArchitectureRollbackResult {
  graph: ProjectArchitectureGraph;
  event: ProjectArchitectureChangeEvent | null;
}

export interface ProjectArchitectureChangeRecord extends ProjectArchitectureChangeEvent {
  beforeGraph?: ProjectArchitectureGraph | null;
  afterGraph?: ProjectArchitectureGraph | null;
}

export interface ProjectArchitecturePlanImpact {
  reason: string;
  changes: ProjectArchitectureChange[];
}

export interface ProjectArchitectureInteractionPayload extends ProjectArchitectureApplyInput {
  status: ProjectArchitectureInteractionStatus;
  requiresConfirmation: boolean;
}

export interface ProjectArchitectureInteractionEnvelope {
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  requestId: string;
  payload: ProjectArchitectureInteractionPayload;
}

export interface ProjectArchitectureInteractionResult {
  decision: "allow" | "deny";
  graph?: ProjectArchitectureGraph | null;
  event?: ProjectArchitectureChangeEvent | null;
  message?: string | null;
}

export type ProjectArchitectureChangeTextOptions = ArchitectureChangeDisplayOptions;

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function stringOrEmpty(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function stringList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((item): item is string => typeof item === "string");
}

function normalizeProjectArchitectureNode(value: unknown): ProjectArchitectureNode | null {
  if (!isRecord(value)) return null;
  const id = stringOrEmpty(value.id);
  const label = stringOrEmpty(value.label);
  if (!id || !label) return null;
  return {
    id,
    label,
    type: stringOrEmpty(value.type) || "module",
    summary: stringOrEmpty(value.summary),
    paths: stringList(value.paths),
    tags: stringList(value.tags),
  };
}

function normalizeProjectArchitectureEdge(value: unknown): ProjectArchitectureEdge | null {
  if (!isRecord(value)) return null;
  const id = stringOrEmpty(value.id);
  const from = stringOrEmpty(value.from);
  const to = stringOrEmpty(value.to);
  if (!id || !from || !to) return null;
  return {
    id,
    from,
    to,
    type: stringOrEmpty(value.type) || "depends_on",
    label: stringOrEmpty(value.label),
    summary: stringOrEmpty(value.summary),
  };
}

export function normalizeProjectArchitectureChange(
  value: unknown,
): ProjectArchitectureChange | null {
  if (!isRecord(value)) return null;
  if (value.type === "upsert_node") {
    const node = normalizeProjectArchitectureNode(value.node);
    return node ? { type: "upsert_node", node } : null;
  }
  if (value.type === "remove_node") {
    const nodeId = stringOrEmpty(value.nodeId);
    return nodeId ? { type: "remove_node", nodeId } : null;
  }
  if (value.type === "upsert_edge") {
    const edge = normalizeProjectArchitectureEdge(value.edge);
    return edge ? { type: "upsert_edge", edge } : null;
  }
  if (value.type === "remove_edge") {
    const edgeId = stringOrEmpty(value.edgeId);
    return edgeId ? { type: "remove_edge", edgeId } : null;
  }
  if (value.type === "set_summary") {
    return { type: "set_summary", summary: stringOrEmpty(value.summary) };
  }
  return null;
}

export function normalizeProjectArchitectureChanges(value: unknown): ProjectArchitectureChange[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => normalizeProjectArchitectureChange(item))
    .filter((item): item is ProjectArchitectureChange => item !== null);
}

export function projectArchitectureChangeText(
  change: ProjectArchitectureChange,
  options: ProjectArchitectureChangeTextOptions = {},
): string {
  return architectureChangeDisplayText(change, options);
}

export function projectArchitectureChangeTextFromPayload(
  value: unknown,
  options: ProjectArchitectureChangeTextOptions = {},
): string {
  return architectureChangeDisplayText(value, options);
}

export function normalizeProjectArchitectureInteractionPayload(
  value: unknown,
): ProjectArchitectureInteractionPayload | null {
  if (!isRecord(value)) return null;
  const projectId = stringOrEmpty(value.projectId);
  if (!projectId) return null;
  const changes = normalizeProjectArchitectureChanges(value.changes);
  if (changes.length === 0) return null;
  return {
    projectId,
    taskId: stringOrEmpty(value.taskId),
    turnId: stringOrNull(value.turnId),
    backend: normalizeChatBackendKind(value.backend),
    permission: normalizeProjectArchitecturePermission(value.permission),
    reason: stringOrEmpty(value.reason),
    changes,
    requestId: stringOrNull(value.requestId),
    status: value.status === "proposed" || value.status === "applied"
      ? value.status
      : PROJECT_ARCHITECTURE_DEFAULT_INTERACTION_STATUS,
    requiresConfirmation: value.requiresConfirmation !== false,
  };
}

export function projectArchitectureApplyInputFromInteraction(
  request: ProjectArchitectureInteractionEnvelope,
): ProjectArchitectureApplyInput {
  return {
    projectId: request.payload.projectId,
    taskId: request.payload.taskId || request.taskId,
    turnId: request.payload.turnId ?? request.turnId,
    backend: request.payload.backend || request.backend,
    permission: request.payload.permission,
    reason: request.payload.reason,
    changes: request.payload.changes,
    requestId: request.requestId,
  };
}

export function projectArchitectureRejectInputFromInteraction(
  request: ProjectArchitectureInteractionEnvelope,
): ProjectArchitectureRejectInput {
  return projectArchitectureApplyInputFromInteraction(request);
}
