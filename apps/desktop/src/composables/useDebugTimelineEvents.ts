import type { AgentTimelineEvent } from "@lilia/contracts";

type DebugTimelineHandler = (event: AgentTimelineEvent) => void;

const handlersByTaskId = new Map<string, Set<DebugTimelineHandler>>();

export function emitDebugTimelineEvent(event: AgentTimelineEvent) {
  for (const handler of handlersByTaskId.get(event.taskId) ?? []) {
    handler(event);
  }
}

export function onDebugTimelineEvent(
  taskId: string,
  handler: DebugTimelineHandler,
): () => void {
  let handlers = handlersByTaskId.get(taskId);
  if (!handlers) {
    handlers = new Set();
    handlersByTaskId.set(taskId, handlers);
  }
  handlers.add(handler);
  return () => {
    handlers?.delete(handler);
    if (handlers?.size === 0) {
      handlersByTaskId.delete(taskId);
    }
  };
}
