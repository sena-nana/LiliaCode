export type ConversationContextInvalidationReason =
  | "popup-close"
  | "route-change"
  | "unmount";

type ConversationContextInvalidationListener = (
  version: number,
  reason: ConversationContextInvalidationReason,
) => void;

let contextSnapshotVersion = 0;
const listeners = new Set<ConversationContextInvalidationListener>();

export function getConversationContextSnapshotVersion(): number {
  return contextSnapshotVersion;
}

export function isConversationContextSnapshotCurrent(version: number): boolean {
  return version === contextSnapshotVersion;
}

export function invalidateConversationContextSnapshot(
  reason: ConversationContextInvalidationReason,
): number {
  contextSnapshotVersion += 1;
  for (const listener of listeners) {
    listener(contextSnapshotVersion, reason);
  }
  return contextSnapshotVersion;
}

export function onConversationContextSnapshotInvalidated(
  listener: ConversationContextInvalidationListener,
): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
