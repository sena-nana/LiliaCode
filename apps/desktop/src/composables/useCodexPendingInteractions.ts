import { computed, reactive, type ComputedRef } from "vue";
import type {
  AgentInteractionRequest,
  AgentInteractionResponse,
  CodexMcpElicitationPayload,
  CodexMcpElicitationResult,
  CodexPermissionApprovalPayload,
  CodexPermissionApprovalResult,
} from "@lilia/contracts";
import { respondAgentInteraction } from "../services/chat";
import {
  clearConversationRequiresAction,
  markConversationRequiresAction,
} from "./useConversationActivity";

export interface PendingCodexMcpElicitation {
  kind: "mcp_elicitation";
  taskId: string;
  turnId: string | null;
  requestId: string;
  payload: CodexMcpElicitationPayload;
}

export interface PendingCodexPermissionApproval {
  kind: "permission_approval";
  taskId: string;
  turnId: string | null;
  requestId: string;
  payload: CodexPermissionApprovalPayload;
}

export type PendingCodexInteraction =
  | PendingCodexMcpElicitation
  | PendingCodexPermissionApproval;

const pending = reactive<Record<string, Record<string, PendingCodexInteraction>>>({});

type TaskIdSource = string | (() => string);

function readTaskId(source: TaskIdSource): string {
  return typeof source === "function" ? source() : source;
}

function taskBucket(taskId: string): Record<string, PendingCodexInteraction> {
  if (!pending[taskId]) pending[taskId] = {};
  return pending[taskId];
}

function clearPending(taskId: string, requestId: string) {
  if (pending[taskId]?.[requestId]) delete pending[taskId][requestId];
  clearConversationRequiresAction(taskId, requestId);
}

function mcpPayload(value: unknown): CodexMcpElicitationPayload | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const row = value as Record<string, unknown>;
  const mode = row.mode === "url" ? "url" : row.mode === "form" ? "form" : null;
  const threadId = typeof row.threadId === "string" ? row.threadId : "";
  const serverName = typeof row.serverName === "string" ? row.serverName : "";
  const message = typeof row.message === "string" ? row.message : "";
  if (!mode || !threadId || !serverName) return null;
  return {
    threadId,
    turnId: typeof row.turnId === "string" ? row.turnId : null,
    serverName,
    mode,
    message,
    requestedSchema: row.requestedSchema,
    url: typeof row.url === "string" ? row.url : undefined,
    elicitationId: typeof row.elicitationId === "string" ? row.elicitationId : undefined,
    _meta: row._meta,
  };
}

function permissionPayload(value: unknown): CodexPermissionApprovalPayload | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const row = value as Record<string, unknown>;
  const threadId = typeof row.threadId === "string" ? row.threadId : "";
  const turnId = typeof row.turnId === "string" ? row.turnId : "";
  const itemId = typeof row.itemId === "string" ? row.itemId : "";
  const cwd = typeof row.cwd === "string" ? row.cwd : "";
  if (!threadId || !turnId || !itemId || !cwd) return null;
  return {
    threadId,
    turnId,
    itemId,
    startedAtMs: typeof row.startedAtMs === "number" && Number.isFinite(row.startedAtMs)
      ? row.startedAtMs
      : 0,
    cwd,
    reason: typeof row.reason === "string" ? row.reason : null,
    permissions: row.permissions,
  };
}

export function handleCodexPendingInteractionRequest(req: AgentInteractionRequest): boolean {
  if (req.kind !== "mcp_elicitation" && req.kind !== "permission_approval") return false;
  const payload = req.kind === "mcp_elicitation"
    ? mcpPayload(req.payload)
    : permissionPayload(req.payload);
  if (!payload) return false;
  taskBucket(req.taskId)[req.requestId] = {
    kind: req.kind,
    taskId: req.taskId,
    turnId: req.turnId || null,
    requestId: req.requestId,
    payload,
  } as PendingCodexInteraction;
  markConversationRequiresAction(req.taskId, req.requestId);
  return true;
}

export function usePendingCodexInteractionsForTask(
  taskId: TaskIdSource,
): ComputedRef<PendingCodexInteraction[]> {
  return computed(() => {
    const bucket = pending[readTaskId(taskId)];
    return bucket ? Object.values(bucket) : [];
  });
}

export async function respondCodexMcpElicitation(
  taskId: string,
  requestId: string,
  result: CodexMcpElicitationResult,
): Promise<void> {
  clearPending(taskId, requestId);
  await respondAgentInteraction({
    taskId,
    requestId,
    kind: "mcp_elicitation",
    result,
  } satisfies AgentInteractionResponse);
}

export async function respondCodexPermissionApproval(
  taskId: string,
  requestId: string,
  result: CodexPermissionApprovalResult,
): Promise<void> {
  clearPending(taskId, requestId);
  await respondAgentInteraction({
    taskId,
    requestId,
    kind: "permission_approval",
    result,
  } satisfies AgentInteractionResponse);
}
