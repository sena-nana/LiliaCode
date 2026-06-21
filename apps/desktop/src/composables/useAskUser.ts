import { computed, reactive, type ComputedRef } from "vue";
import type { AskUserResult, AskUserSpec } from "@lilia/contracts";
import { askUserSpecKey, isPlanApprovalAskUserSpec } from "@lilia/contracts";
import {
  clearConversationRequiresAction,
  markConversationRequiresAction,
} from "./useConversationActivity";
import {
  shouldClearPendingInteraction,
  type ClearPendingInteractionsOptions,
} from "./pendingInteractionClearOptions";

export interface PendingAsk {
  id: number;
  requestId?: string | null;
  spec: AskUserSpec;
  taskId: string | null;
  turnId: string | null;
  resolve: (result: AskUserResult) => void;
}

export function pendingAskInteractionKey(ask: PendingAsk): string {
  return `ask:${ask.requestId ?? ask.id}`;
}

interface AskUserState {
  current: PendingAsk | null;
  queue: PendingAsk[];
  pending: PendingAsk[];
}

const state = reactive<AskUserState>({
  current: null,
  queue: [],
  pending: [],
});
let askSeq = 1;

type TaskIdSource = string | (() => string);

function readTaskId(source: TaskIdSource): string {
  return typeof source === "function" ? source() : source;
}

function pumpNext() {
  if (state.current) return;
  const next = state.queue.shift();
  if (next) state.current = next;
}

function findHydratedAsk(
  taskId: string,
  spec: AskUserSpec,
  turnId: string | null,
  requestId: string | null,
): PendingAsk | null {
  if (requestId) {
    return state.pending.find((ask) =>
      ask.taskId === taskId &&
      ask.requestId === requestId
    ) ?? null;
  }
  return state.pending.find((ask) =>
    ask.taskId === taskId &&
    ask.turnId === turnId &&
    !ask.requestId &&
    askUserSpecKey(ask.spec) === askUserSpecKey(spec)
  ) ?? null;
}

export function askUserForTask(
  taskId: string | null,
  spec: AskUserSpec,
  turnId: string | null = null,
  requestId: string | null = null,
): Promise<AskUserResult> {
  return new Promise((resolve) => {
    if (taskId && requestId) markConversationRequiresAction(taskId, requestId);
    const ask = {
      id: askSeq++,
      requestId,
      spec,
      taskId,
      turnId,
      resolve,
    };
    state.pending.push(ask);
    state.queue.push(ask);
    pumpNext();
  });
}

export function resolveAskUserById(id: number, result: AskUserResult): boolean {
  const current = state.current;
  const queueIndex = state.queue.findIndex((ask) => ask.id === id);
  const pendingIndex = state.pending.findIndex((ask) => ask.id === id);
  const ask = current?.id === id
    ? current
    : queueIndex >= 0
      ? state.queue[queueIndex]
      : pendingIndex >= 0
        ? state.pending[pendingIndex]
        : null;
  if (!ask) return false;

  if (current?.id === id) {
    state.current = null;
  }
  if (queueIndex >= 0) {
    state.queue.splice(queueIndex, 1);
  }
  if (pendingIndex >= 0) {
    state.pending.splice(pendingIndex, 1);
  }
  if (ask.taskId) clearConversationRequiresAction(ask.taskId, ask.requestId);
  ask.resolve(result);
  pumpNext();
  return true;
}

export function hydrateAskUserForTask(
  taskId: string,
  spec: AskUserSpec,
  turnId: string | null = null,
  requestId: string | null = null,
  resolve: (result: AskUserResult) => void = () => {},
) {
  const existing = findHydratedAsk(taskId, spec, turnId, requestId);
  if (existing) {
    existing.spec = spec;
    existing.turnId = turnId;
    existing.resolve = resolve;
    if (taskId && requestId) markConversationRequiresAction(taskId, requestId);
    return existing;
  }
  if (taskId && requestId) markConversationRequiresAction(taskId, requestId);
  const ask: PendingAsk = {
    id: askSeq++,
    requestId,
    spec,
    taskId,
    turnId,
    resolve,
  };
  state.pending.push(ask);
  state.queue.push(ask);
  pumpNext();
  return ask;
}

export function clearAskUsersForTask(
  taskId: string,
  options: ClearPendingInteractionsOptions = {},
) {
  const removedRequestIds = new Set<string>();
  if (state.current && shouldClearPendingInteraction(state.current, taskId, options)) {
    if (state.current.requestId) removedRequestIds.add(state.current.requestId);
    state.current = null;
  }

  for (let index = state.queue.length - 1; index >= 0; index -= 1) {
    const ask = state.queue[index];
    if (!ask || !shouldClearPendingInteraction(ask, taskId, options)) continue;
    if (ask.requestId) removedRequestIds.add(ask.requestId);
    state.queue.splice(index, 1);
  }

  for (let index = state.pending.length - 1; index >= 0; index -= 1) {
    const ask = state.pending[index];
    if (!ask || !shouldClearPendingInteraction(ask, taskId, options)) continue;
    if (ask.requestId) removedRequestIds.add(ask.requestId);
    state.pending.splice(index, 1);
  }

  for (const requestId of removedRequestIds) {
    clearConversationRequiresAction(taskId, requestId);
  }
  pumpNext();
}

export function useAskUserForTask(
  taskId: TaskIdSource,
): ComputedRef<PendingAsk | null> {
  return computed(() => {
    const ask = state.current;
    if (!ask) return null;
    const currentTaskId = readTaskId(taskId);
    if (ask.taskId == null || ask.taskId === currentTaskId) return ask;
    return null;
  });
}

export function usePendingAsksForTask(
  taskId: TaskIdSource,
): ComputedRef<PendingAsk[]> {
  return computed(() => {
    const currentTaskId = readTaskId(taskId);
    return state.pending.filter((ask) => ask.taskId == null || ask.taskId === currentTaskId);
  });
}

export function isPlanApprovalAsk(ask: Pick<PendingAsk, "spec">): boolean {
  return isPlanApprovalAskUserSpec(ask.spec);
}

export function findPlanApprovalAsk(
  asks: readonly PendingAsk[],
): PendingAsk | null {
  return asks.find(isPlanApprovalAsk) ?? null;
}

export function useAskUser() {
  return { state };
}
