import type { AgentTimelineEvent } from "@lilia/contracts";

export interface TaskTimelineLoadMergeInput {
  loadedEvents: readonly AgentTimelineEvent[];
  currentEvents: readonly AgentTimelineEvent[];
  eventIdsBeforeLoad: ReadonlySet<string>;
  liveEventsDuringLoad: ReadonlyMap<string, AgentTimelineEvent>;
}

export interface TaskTimelineLoadMergePlan {
  preserveEventIds: Set<string>;
  liveEventsToReplay: AgentTimelineEvent[];
}

function currentEventIsFreshEnough(
  current: AgentTimelineEvent,
  loadedById: ReadonlyMap<string, AgentTimelineEvent>,
): boolean {
  const loaded = loadedById.get(current.id);
  return !loaded || current.updatedAt >= loaded.updatedAt;
}

export function taskTimelineLoadMergePlan(
  input: TaskTimelineLoadMergeInput,
): TaskTimelineLoadMergePlan {
  const loadedById = new Map(input.loadedEvents.map((event) => [event.id, event]));
  const preserveEventIds = new Set<string>();

  for (const event of input.currentEvents) {
    const existedBeforeLoad = input.eventIdsBeforeLoad.has(event.id);
    const changedDuringLoad = input.liveEventsDuringLoad.has(event.id);
    if (existedBeforeLoad && !changedDuringLoad) continue;
    if (currentEventIsFreshEnough(event, loadedById)) {
      preserveEventIds.add(event.id);
    }
  }

  const liveEventsToReplay = [...input.liveEventsDuringLoad.values()].filter((event) =>
    currentEventIsFreshEnough(event, loadedById)
  );

  return { preserveEventIds, liveEventsToReplay };
}
