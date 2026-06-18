function uniqueTaskIds(taskIds: string[]): string[] {
  return Array.from(new Set(taskIds.filter(Boolean)));
}

export interface ConversationActivityStagePlan {
  initialTaskIds: string[];
  initialPriorityTaskIds: string[];
  deferredTaskIds: string[];
  deferredPriorityTaskIds: string[];
}

export function createConversationActivityStagePlan(options: {
  taskIds: string[];
  initialTaskIds: string[];
  priorityTaskIds?: string[];
}): ConversationActivityStagePlan {
  const taskIds = uniqueTaskIds(options.taskIds);
  const taskIdSet = new Set(taskIds);
  const requestedInitialTaskIds = uniqueTaskIds(options.initialTaskIds)
    .filter((taskId) => taskIdSet.has(taskId));
  const priorityTaskIds = uniqueTaskIds(options.priorityTaskIds ?? requestedInitialTaskIds)
    .filter((taskId) => taskIdSet.has(taskId));
  const initialSet = new Set(requestedInitialTaskIds);
  const initialPriorityTaskIds = priorityTaskIds.filter((taskId) => initialSet.has(taskId));
  const initialTaskIds = uniqueTaskIds([
    ...initialPriorityTaskIds,
    ...requestedInitialTaskIds,
  ]);
  const normalizedInitialSet = new Set(initialTaskIds);
  const deferredTaskIds = taskIds.filter((taskId) => !normalizedInitialSet.has(taskId));
  const deferredPriorityTaskIds = priorityTaskIds.filter((taskId) => !normalizedInitialSet.has(taskId));
  return {
    initialTaskIds,
    initialPriorityTaskIds,
    deferredTaskIds,
    deferredPriorityTaskIds,
  };
}
