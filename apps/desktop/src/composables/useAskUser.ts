import { computed, reactive, type ComputedRef } from "vue";
import type { AskUserResult, AskUserSpec } from "@lilia/contracts";

export interface PendingAsk {
  id: number;
  requestId?: string | null;
  spec: AskUserSpec;
  taskId: string | null;
  turnId: string | null;
  resolve: (result: AskUserResult) => void;
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

export function askUser(spec: AskUserSpec): Promise<AskUserResult> {
  return askUserForTask(null, spec);
}

export function askUserForTask(
  taskId: string | null,
  spec: AskUserSpec,
  turnId: string | null = null,
  requestId: string | null = null,
): Promise<AskUserResult> {
  return new Promise((resolve) => {
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

export function resolveAskUser(result: AskUserResult) {
  const current = state.current;
  if (!current) return;
  resolveAskUserById(current.id, result);
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
  ask.resolve(result);
  pumpNext();
  return true;
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

export function useAskUser() {
  return { state };
}
