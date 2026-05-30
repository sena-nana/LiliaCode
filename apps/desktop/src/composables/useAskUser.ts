import { computed, reactive, type ComputedRef } from "vue";
import type { AskUserResult, AskUserSpec } from "@lilia/contracts";

export interface PendingAsk {
  id: number;
  spec: AskUserSpec;
  taskId: string | null;
  turnId: string | null;
  resolve: (result: AskUserResult) => void;
}

interface AskUserState {
  current: PendingAsk | null;
  queue: PendingAsk[];
}

const state = reactive<AskUserState>({
  current: null,
  queue: [],
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
): Promise<AskUserResult> {
  return new Promise((resolve) => {
    state.queue.push({
      id: askSeq++,
      spec,
      taskId,
      turnId,
      resolve,
    });
    pumpNext();
  });
}

export function resolveAskUser(result: AskUserResult) {
  const current = state.current;
  if (!current) return;
  state.current = null;
  current.resolve(result);
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

export function useAskUser() {
  return { state };
}
