import { computed, reactive, type ComputedRef } from "vue";
import type {
  AgentInteractionRequest,
  AgentInteractionResponse,
  McpElicitationPayload,
  McpElicitationResult,
  PermissionApprovalPayload,
  PermissionApprovalResult,
} from "@lilia/contracts";
import { respondAgentInteraction } from "../services/chat";
import {
  clearConversationRequiresAction,
  markConversationRequiresAction,
} from "./useConversationActivity";

interface PendingMcpElicitation {
  kind: "mcp_elicitation";
  taskId: string;
  turnId: string | null;
  requestId: string;
  payload: McpElicitationPayload;
}

interface PendingPermissionApproval {
  kind: "permission_approval";
  taskId: string;
  turnId: string | null;
  requestId: string;
  payload: PermissionApprovalPayload;
}

export type PendingAgentInteraction =
  | PendingMcpElicitation
  | PendingPermissionApproval;

const pending = reactive<Record<string, Record<string, PendingAgentInteraction>>>({});

type TaskIdSource = string | (() => string);

function readTaskId(source: TaskIdSource): string {
  return typeof source === "function" ? source() : source;
}

interface ClearAgentPendingInteractionsForTaskOptions {
  turnId?: string | null;
  keepRequestIds?: Set<string>;
}

function taskBucket(taskId: string): Record<string, PendingAgentInteraction> {
  if (!pending[taskId]) pending[taskId] = {};
  return pending[taskId];
}

function clearPending(taskId: string, requestId: string) {
  if (pending[taskId]?.[requestId]) delete pending[taskId][requestId];
  clearConversationRequiresAction(taskId, requestId);
}

function mcpPayload(value: unknown): McpElicitationPayload | null {
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

function permissionPayload(value: unknown): PermissionApprovalPayload | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const row = value as Record<string, unknown>;
  const providerContext = row.providerContext && typeof row.providerContext === "object" && !Array.isArray(row.providerContext)
    ? row.providerContext as PermissionApprovalPayload["providerContext"]
    : undefined;
  const requestedAccess = row.requestedAccess ?? row.permissions ?? {};
  return {
    reason: typeof row.reason === "string" ? row.reason : null,
    requestedAccess,
    scopeSuggestion: row.scopeSuggestion,
    providerContext,
  };
}

export function handleAgentPendingInteractionRequest(req: AgentInteractionRequest): boolean {
  if (req.kind === "mcp_elicitation") {
    const payload = mcpPayload(req.payload);
    if (!payload) return false;
    hydrateAgentPendingInteraction({
      kind: req.kind,
      taskId: req.taskId,
      turnId: req.turnId || null,
      requestId: req.requestId,
      payload,
    });
    return true;
  }
  if (req.kind !== "permission_approval") return false;
  const payload = permissionPayload(req.payload);
  if (!payload) return false;
  hydrateAgentPendingInteraction({
    kind: req.kind,
    taskId: req.taskId,
    turnId: req.turnId || null,
    requestId: req.requestId,
    payload,
  });
  return true;
}

export function hydrateAgentPendingInteraction(interaction: PendingAgentInteraction) {
  taskBucket(interaction.taskId)[interaction.requestId] = interaction;
  markConversationRequiresAction(interaction.taskId, interaction.requestId);
}

export function clearAgentPendingInteractionsForTask(
  taskId: string,
  options: ClearAgentPendingInteractionsForTaskOptions = {},
) {
  const bucket = pending[taskId];
  if (!bucket) return;
  for (const [requestId, interaction] of Object.entries(bucket)) {
    if (options.turnId !== undefined && interaction.turnId !== options.turnId) continue;
    if (options.keepRequestIds?.has(requestId)) continue;
    delete bucket[requestId];
    clearConversationRequiresAction(taskId, requestId);
  }
  if (Object.keys(bucket).length === 0) delete pending[taskId];
}

export function usePendingAgentInteractionsForTask(
  taskId: TaskIdSource,
): ComputedRef<PendingAgentInteraction[]> {
  return computed(() => {
    const bucket = pending[readTaskId(taskId)];
    return bucket ? Object.values(bucket) : [];
  });
}

export async function respondMcpElicitation(
  taskId: string,
  requestId: string,
  result: McpElicitationResult,
): Promise<void> {
  await respondAgentInteraction({
    taskId,
    requestId,
    kind: "mcp_elicitation",
    result,
  } satisfies AgentInteractionResponse);
  clearPending(taskId, requestId);
}

export async function respondPermissionApproval(
  taskId: string,
  requestId: string,
  result: PermissionApprovalResult,
): Promise<void> {
  await respondAgentInteraction({
    taskId,
    requestId,
    kind: "permission_approval",
    result,
  } satisfies AgentInteractionResponse);
  clearPending(taskId, requestId);
}
