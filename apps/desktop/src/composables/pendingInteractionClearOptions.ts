export interface ClearPendingInteractionsOptions {
  turnId?: string | null;
  requestId?: string;
  keepRequestIds?: Set<string>;
}

export interface PendingInteractionClearCandidate {
  taskId: string | null;
  turnId: string | null;
  requestId?: string | null;
}

export function shouldClearPendingInteraction(
  candidate: PendingInteractionClearCandidate,
  taskId: string,
  options: ClearPendingInteractionsOptions = {},
): boolean {
  if (candidate.taskId !== taskId) return false;
  if (options.turnId !== undefined && candidate.turnId !== options.turnId) return false;
  if (options.requestId !== undefined && candidate.requestId !== options.requestId) return false;
  if (candidate.requestId && options.keepRequestIds?.has(candidate.requestId)) return false;
  return true;
}
