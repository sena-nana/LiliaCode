import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import {
  isAgentTimelineRunningStatus,
  isAgentTimelineTerminalStatus,
  timelineLatestEventTimeMs,
  timelineChatMessageFromEvent,
  timelineProcessEventsSummary,
  timelineRunningSubagentLabel,
  timelineThinkingDurationLabel,
  type AgentTimelineEvent,
  type TimelineChatMessage,
} from "@lilia/contracts";
import {
  mergeAdjacentTimelineGroups,
  processGroupEntries,
  type TimelineEntry,
  type TimelineEventEntry,
} from "./timelineEntries";
import {
  isHiddenTimelineEvent,
  isTimelineInterruptEvent,
  isTimelineFinalReply,
  isTimelineProcessAnchor,
  isTimelineUserMessage,
  timelineInlinePreview,
  type TimelineDisplayContext,
} from "./timelineDisplay";

type StreamableMessage = TimelineChatMessage & { streaming?: boolean };

interface UseAgentTimelineEntriesOptions {
  activePlanApprovalTurnId: ComputedRef<string | null | undefined>;
  events: ComputedRef<AgentTimelineEvent[]>;
  isThinking: ComputedRef<boolean | undefined>;
  projectCwd: ComputedRef<string | null | undefined>;
}

export function useAgentTimelineEntries(options: UseAgentTimelineEntriesOptions) {
  const displayContext = computed<TimelineDisplayContext>(() => ({
    projectCwd: options.projectCwd.value,
    activePlanApprovalTurnId: options.activePlanApprovalTurnId.value,
  }));

  const turnState = computed(() => {
    const completed = new Set<string>();
    const interrupted = new Set<string>();
    for (const event of options.events.value) {
      if (!event.turnId) continue;
      if (isTimelineInterruptEvent(event)) {
        completed.add(event.turnId);
        interrupted.add(event.turnId);
        continue;
      }
      if (event.kind !== "turn") continue;
      if (!isAgentTimelineTerminalStatus(event.status)) continue;
      completed.add(event.turnId);
    }
    return { completed, interrupted };
  });

  const visibleEvents = computed(() =>
    options.events.value.filter((event) => !isHiddenTimelineEvent(event)),
  );

  const chronologicalEntries = computed<TimelineEventEntry[]>(() =>
    timelineEventEntries(visibleEvents.value),
  );

  const orderedEntries = computed<TimelineEntry[]>(() => {
    const entries = chronologicalEntries.value;
    const completed = turnState.value.completed;
    const lastAnchorByTurnId = new Map<string, TimelineEventEntry>();

    for (const entry of entries) {
      const turnId = entry.event.turnId;
      if (!turnId || !completed.has(turnId)) continue;
      if (isTimelineProcessAnchor(entry.event)) lastAnchorByTurnId.set(turnId, entry);
    }

    const processEventsByAnchorId = new Map<string, AgentTimelineEvent[]>();
    const hiddenEventIds = new Set<string>();

    for (const entry of entries) {
      const turnId = entry.event.turnId;
      if (!turnId || !completed.has(turnId)) continue;
      if (isTimelineUserMessage(entry.event)) continue;
      const anchorEntry = lastAnchorByTurnId.get(turnId);
      if (!anchorEntry) continue;
      if (entry.intraTurnOrder >= anchorEntry.intraTurnOrder) continue;
      let list = processEventsByAnchorId.get(anchorEntry.event.id);
      if (!list) {
        list = [];
        processEventsByAnchorId.set(anchorEntry.event.id, list);
      }
      list.push(entry.event);
      hiddenEventIds.add(entry.event.id);
    }

    const output: TimelineEventEntry[] = [];
    for (const entry of entries) {
      if (hiddenEventIds.has(entry.event.id)) continue;
      const processEvents = isTimelineProcessAnchor(entry.event)
        ? processEventsByAnchorId.get(entry.event.id)
        : undefined;
      output.push(processEvents ? { ...entry, processEvents } : entry);
    }

    return mergeAdjacentTimelineGroups(output);
  });

  const eventPreviewCache = computed(() => {
    const cache = new Map<string, string>();
    for (const event of visibleEvents.value) {
      cache.set(event.id, timelineInlinePreview(event, displayContext.value));
    }
    return cache;
  });

  const userMessageCache = computed(() => {
    const cache = new Map<string, StreamableMessage>();
    for (const event of visibleEvents.value) {
      if (isTimelineUserMessage(event)) cache.set(event.id, timelineChatMessageFromEvent(event));
    }
    return cache;
  });

  const showThinkingIndicator = computed(() => {
    if (!options.isThinking.value) return false;
    return !visibleEvents.value.some((event) =>
      isTimelineFinalReply(event) && isAgentTimelineRunningStatus(event.status),
    );
  });

  const nowMs = ref(Date.now());
  let thinkingTimer: ReturnType<typeof setInterval> | null = null;

  function stopThinkingTimer() {
    if (!thinkingTimer) return;
    clearInterval(thinkingTimer);
    thinkingTimer = null;
  }

  watch(
    showThinkingIndicator,
    (visible) => {
      stopThinkingTimer();
      if (!visible) return;
      nowMs.value = Date.now();
      thinkingTimer = setInterval(() => {
        nowMs.value = Date.now();
      }, 1000);
    },
    { immediate: true },
  );

  onBeforeUnmount(stopThinkingTimer);

  const thinkingContext = computed(() => {
    const events = chronologicalEntries.value.map((entry) => entry.event);
    return {
      previousAt: timelineLatestEventTimeMs(events),
      subagent: timelineRunningSubagentLabel(events),
    };
  });

  const thinkingIndicatorLabel = computed(() => {
    const context = thinkingContext.value;
    const parts = [timelineThinkingDurationLabel(context.previousAt, nowMs.value)];
    if (context.subagent) parts.push(context.subagent);
    return parts.join("，");
  });

  function previewText(event: AgentTimelineEvent): string {
    return eventPreviewCache.value.get(event.id) ?? "";
  }

  function userMessage(event: AgentTimelineEvent): StreamableMessage {
    return userMessageCache.value.get(event.id) ?? timelineChatMessageFromEvent(event);
  }

  function processGroupRunning(entry: TimelineEventEntry): boolean {
    if (isAgentTimelineTerminalStatus(entry.event.status)) return false;
    return hasRunningEvent(entry.processEvents ?? []);
  }

  function processGroupLabel(entry: TimelineEventEntry): string {
    return timelineProcessEventsSummary(entry.processEvents ?? [], entry.event);
  }

  return {
    displayContext,
    isTimelineUserMessage,
    orderedEntries,
    previewText,
    processGroupLabel,
    processGroupRunning,
    showThinkingIndicator,
    thinkingIndicatorLabel,
    timelineGroupEntryIds,
    turnState,
    userMessage,
    visibleEvents,
  };
}

function timelineGroupEntryIds(entries: TimelineEntry[]): Set<string> {
  const ids = new Set<string>();
  for (const entry of entries) {
    if (entry.type === "group") {
      ids.add(entry.id);
      continue;
    }
    if (entry.processEvents?.length) {
      for (const processEntry of processGroupEntries(entry)) {
        if (processEntry.type === "group") ids.add(processEntry.id);
      }
    }
  }
  return ids;
}

function hasRunningEvent(events: AgentTimelineEvent[]): boolean {
  return events.some((event) => isAgentTimelineRunningStatus(event.status));
}

function timelineEventEntries(events: AgentTimelineEvent[]): TimelineEventEntry[] {
  const entries = events.map((event): TimelineEventEntry => ({
    type: "event",
    id: `event:${event.id}`,
    createdAt: event.createdAt,
    turnSeq: event.turnSeq,
    intraTurnOrder: event.intraTurnOrder,
    event,
  }));
  if (!entriesAreChronological(entries)) entries.sort(compareTimelineEntries);
  return entries;
}

function entriesAreChronological(entries: TimelineEventEntry[]): boolean {
  for (let i = 1; i < entries.length; i += 1) {
    if (compareTimelineEntries(entries[i - 1], entries[i]) > 0) return false;
  }
  return true;
}

function compareTimelineEntries(a: TimelineEventEntry, b: TimelineEventEntry): number {
  return a.turnSeq - b.turnSeq ||
    a.intraTurnOrder - b.intraTurnOrder ||
    a.createdAt - b.createdAt ||
    a.id.localeCompare(b.id);
}

