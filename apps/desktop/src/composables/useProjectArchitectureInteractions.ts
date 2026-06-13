import { computed, reactive, type ComputedRef } from "vue";
import type {
  ChatBackendKind,
  ProjectArchitectureInteractionPayload,
  ProjectArchitectureInteractionResult,
} from "@lilia/contracts";
import {
  applyProjectArchitecture,
  rejectProjectArchitecture,
  respondAgentInteraction,
} from "../services/chat";
import {
  clearConversationRequiresAction,
  markConversationRequiresAction,
} from "./useConversationActivity";

export interface PendingArchitectureChange {
  kind: "architecture_change";
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  requestId: string;
  payload: ProjectArchitectureInteractionPayload;
}

type ArchitectureInteractionRequest = {
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  requestId: string;
  payload: ProjectArchitectureInteractionPayload;
};

const state = reactive<{ pending: PendingArchitectureChange[] }>({
  pending: [],
});

function sameRequest(left: PendingArchitectureChange, right: PendingArchitectureChange): boolean {
  return left.taskId === right.taskId && left.requestId === right.requestId;
}

export function handleProjectArchitectureInteractionRequest(req: {
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  requestId: string;
  payload: ProjectArchitectureInteractionPayload;
}) {
  if (req.payload.requiresConfirmation === false) {
    void autoApplyProjectArchitectureInteraction(req);
    return;
  }
  hydrateProjectArchitectureInteraction(req);
}

export function hydrateProjectArchitectureInteraction(req: {
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  requestId: string;
  payload: ProjectArchitectureInteractionPayload;
}) {
  const next = pendingArchitectureChangeFromRequest(req);
  const existing = state.pending.findIndex((item) => sameRequest(item, next));
  if (existing >= 0) {
    state.pending.splice(existing, 1, next);
  } else {
    state.pending.push(next);
  }
  markConversationRequiresAction(req.taskId, req.requestId);
}

function pendingArchitectureChangeFromRequest(req: ArchitectureInteractionRequest): PendingArchitectureChange {
  return {
    kind: "architecture_change",
    taskId: req.taskId,
    turnId: req.turnId,
    backend: req.backend,
    requestId: req.requestId,
    payload: {
      ...req.payload,
      requestId: req.payload.requestId ?? req.requestId,
      taskId: req.payload.taskId || req.taskId,
      turnId: req.payload.turnId ?? req.turnId,
      backend: req.payload.backend || req.backend,
    },
  };
}

async function autoApplyProjectArchitectureInteraction(req: ArchitectureInteractionRequest) {
  const request = pendingArchitectureChangeFromRequest(req);
  let result: ProjectArchitectureInteractionResult;
  try {
    result = await applyArchitectureChange(request);
  } catch (err) {
    result = {
      decision: "deny",
      event: null,
      message: `架构图自动应用失败：${String(err)}`,
    };
  }

  try {
    await respondAgentInteraction({
      taskId: request.taskId,
      requestId: request.requestId,
      kind: "architecture_change",
      result,
    });
  } catch (err) {
    console.error("[project-architecture] auto response failed", err);
  }
}

export async function respondProjectArchitectureChange(
  request: PendingArchitectureChange,
  decision: "allow" | "deny",
) {
  let result: ProjectArchitectureInteractionResult;
  if (decision === "allow") {
    result = await applyArchitectureChange(request);
  } else {
    const event = await rejectProjectArchitecture({
      projectId: request.payload.projectId,
      taskId: request.payload.taskId || request.taskId,
      turnId: request.payload.turnId ?? request.turnId,
      backend: request.payload.backend || request.backend,
      permission: request.payload.permission,
      reason: request.payload.reason,
      changes: request.payload.changes,
      requestId: request.requestId,
    });
    result = {
      decision: "deny",
      event,
      message: "用户拒绝了架构图变更",
    };
  }

  await respondAgentInteraction({
    taskId: request.taskId,
    requestId: request.requestId,
    kind: "architecture_change",
    result,
  });
  clearProjectArchitectureInteraction(request.taskId, request.requestId);
}

async function applyArchitectureChange(
  request: PendingArchitectureChange,
): Promise<ProjectArchitectureInteractionResult> {
  const applied = await applyProjectArchitecture({
    ...request.payload,
    requestId: request.requestId,
    taskId: request.payload.taskId || request.taskId,
    turnId: request.payload.turnId ?? request.turnId,
    backend: request.payload.backend || request.backend,
  });
  return {
    decision: "allow",
    graph: applied.graph,
    event: applied.event,
    message: "架构图已更新",
  };
}

export function clearProjectArchitectureInteraction(taskId: string, requestId: string) {
  const index = state.pending.findIndex((item) =>
    item.taskId === taskId && item.requestId === requestId
  );
  if (index >= 0) state.pending.splice(index, 1);
  clearConversationRequiresAction(taskId, requestId);
}

export function clearProjectArchitectureInteractionsForTask(
  taskId: string,
  options: { turnId?: string | null; keepRequestIds?: Set<string> } = {},
) {
  for (let index = state.pending.length - 1; index >= 0; index -= 1) {
    const item = state.pending[index];
    if (!item || item.taskId !== taskId) continue;
    if (options.turnId !== undefined && item.turnId !== options.turnId) continue;
    if (options.keepRequestIds?.has(item.requestId)) continue;
    clearConversationRequiresAction(taskId, item.requestId);
    state.pending.splice(index, 1);
  }
}

export function usePendingProjectArchitectureChangesForTask(
  taskId: string | (() => string),
): ComputedRef<PendingArchitectureChange[]> {
  return computed(() => {
    const currentTaskId = typeof taskId === "function" ? taskId() : taskId;
    return state.pending.filter((item) => item.taskId === currentTaskId);
  });
}
