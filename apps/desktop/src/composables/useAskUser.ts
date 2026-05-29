import { reactive } from "vue";
import type { AskUserResult, AskUserSpec } from "@lilia/contracts";

interface PendingAsk {
  id: number;
  spec: AskUserSpec;
  taskId: string | null;
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
): Promise<AskUserResult> {
  return new Promise((resolve) => {
    state.queue.push({ id: askSeq++, spec, taskId, resolve });
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

export function useAskUser() {
  return { state };
}
