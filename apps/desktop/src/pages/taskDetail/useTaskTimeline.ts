import { computed, shallowRef } from "vue";
import { onDebugTimelineEvent } from "../../composables/useDebugTimelineEvents";
import {
  listAgentTimeline,
} from "../../services/chat";
import {
  createErrorTimelineEvent,
  createMessageTimelineEvent,
  markFirstQueuedUserMessageSuccessful,
  mergeLoadedTimelineEvents,
  mergeTimelineEvents,
  retryContextForTimelineEvent,
  upsertTimelineEventById,
  upsertTimelineEventsById,
} from "@lilia/contracts";
import type {
  AgentTimelineEvent,
  ChatAttachment,
  ChatBackendKind,
  ChatConversationReference,
  TimelineRetryContext,
} from "@lilia/contracts";

export function useTaskTimeline(options: {
  taskId: () => string;
  backend: () => ChatBackendKind;
}) {
  const persistedTimelineEvents = shallowRef<AgentTimelineEvent[]>([]);
  const overlayTimelineEvents = shallowRef<AgentTimelineEvent[]>([]);
  const timelineEvents = computed(() =>
    mergeTimelineEvents(persistedTimelineEvents.value, overlayTimelineEvents.value),
  );
  let optimisticMessageSeq = 0;
  let localErrorSeq = 0;
  let unsubscribeDebugTimeline: (() => void) | null = null;
  let queuedTimelineEvents: AgentTimelineEvent[] = [];
  let timelineBatchFrame: number | null = null;

  function scheduleTimelineBatchFlush() {
    if (timelineBatchFrame !== null) return;
    const flush = () => {
      timelineBatchFrame = null;
      flushQueuedTimelineEvents();
    };
    if (typeof window !== "undefined" && typeof window.requestAnimationFrame === "function") {
      timelineBatchFrame = window.requestAnimationFrame(flush);
      return;
    }
    timelineBatchFrame = 0;
    queueMicrotask(flush);
  }

  function flushQueuedTimelineEvents() {
    if (queuedTimelineEvents.length === 0) return;
    const events = queuedTimelineEvents;
    queuedTimelineEvents = [];
    persistedTimelineEvents.value = upsertTimelineEventsById(
      persistedTimelineEvents.value,
      events,
    );
  }

  function clearQueuedTimelineBatch() {
    if (timelineBatchFrame !== null && typeof window !== "undefined" && timelineBatchFrame > 0) {
      window.cancelAnimationFrame(timelineBatchFrame);
    }
    timelineBatchFrame = null;
    queuedTimelineEvents = [];
  }

  function upsertTimelineEvent(event: AgentTimelineEvent) {
    persistedTimelineEvents.value = upsertTimelineEventById(
      persistedTimelineEvents.value,
      event,
    );
  }

  function upsertTimelineEvents(events: AgentTimelineEvent[]) {
    persistedTimelineEvents.value = upsertTimelineEventsById(
      persistedTimelineEvents.value,
      events,
    );
  }

  function queueTimelineEvents(events: AgentTimelineEvent[]) {
    if (events.length === 0) return;
    queuedTimelineEvents.push(...events);
    scheduleTimelineBatchFlush();
  }

  function upsertOverlayTimelineEvent(event: AgentTimelineEvent) {
    overlayTimelineEvents.value = upsertTimelineEventById(
      overlayTimelineEvents.value,
      event,
    );
  }

  function removeTimelineEvent(eventId: string) {
    persistedTimelineEvents.value = persistedTimelineEvents.value.filter((item) =>
      item.id !== eventId
    );
  }

  function nextOptimisticMessageId(): string {
    optimisticMessageSeq += 1;
    return `pending-${Date.now()}-${optimisticMessageSeq}`;
  }

  function nextLocalErrorId(now = Date.now()): string {
    localErrorSeq += 1;
    return `error-${now}-${localErrorSeq}`;
  }

  function createOptimisticMessageEvent(input: {
    content: string;
    attachments: ChatAttachment[];
    conversationReferences?: ChatConversationReference[];
    createdAt?: number;
  }): AgentTimelineEvent {
    const createdAt = input.createdAt ?? Date.now();
    return createMessageTimelineEvent({
      id: nextOptimisticMessageId(),
      taskId: options.taskId(),
      backend: options.backend(),
      content: input.content,
      attachments: input.attachments,
      conversationReferences: input.conversationReferences ?? [],
      createdAt,
      queued: true,
    });
  }

  function createLocalErrorTimelineEvent(
    message: string,
    retryContext?: TimelineRetryContext,
  ): AgentTimelineEvent {
    const now = Date.now();
    return createErrorTimelineEvent({
      id: nextLocalErrorId(now),
      taskId: options.taskId(),
      backend: options.backend(),
      message,
      retryContext,
      createdAt: now,
    });
  }

  async function loadTimelineEvents(taskId: string): Promise<AgentTimelineEvent[]> {
    try {
      return await listAgentTimeline(taskId);
    } catch (err) {
      console.error("[agent-timeline] list failed", err);
      return [];
    }
  }

  function applyLoadedTimelineEvents(
    events: AgentTimelineEvent[],
    preserveEventIds: Set<string> = new Set(),
  ) {
    persistedTimelineEvents.value = mergeLoadedTimelineEvents(
      events,
      persistedTimelineEvents.value,
      preserveEventIds,
    );
  }

  function markQueuedUserMessageSuccessful() {
    persistedTimelineEvents.value = markFirstQueuedUserMessageSuccessful(
      persistedTimelineEvents.value,
    );
  }

  function resetTimeline() {
    clearQueuedTimelineBatch();
    persistedTimelineEvents.value = [];
    overlayTimelineEvents.value = [];
  }

  function canRetryEvent(event: AgentTimelineEvent): boolean {
    return retryContextForTimelineEvent(event, timelineEvents.value) !== null;
  }

  function retryContextForEvent(event: AgentTimelineEvent): TimelineRetryContext | null {
    return retryContextForTimelineEvent(event, timelineEvents.value);
  }

  function resubscribeDebugTimeline() {
    unsubscribeDebugTimeline?.();
    unsubscribeDebugTimeline = onDebugTimelineEvent(
      options.taskId(),
      upsertOverlayTimelineEvent,
    );
  }

  function disposeTimeline() {
    clearQueuedTimelineBatch();
    unsubscribeDebugTimeline?.();
    unsubscribeDebugTimeline = null;
  }

  resubscribeDebugTimeline();

  return {
    timelineEvents,
    persistedTimelineEvents,
    overlayTimelineEvents,
    upsertTimelineEvent,
    upsertTimelineEvents,
    queueTimelineEvents,
    upsertOverlayTimelineEvent,
    removeTimelineEvent,
    createOptimisticMessageEvent,
    createLocalErrorTimelineEvent,
    loadTimelineEvents,
    applyLoadedTimelineEvents,
    markQueuedUserMessageSuccessful,
    resetTimeline,
    canRetryEvent,
    retryContextForEvent,
    resubscribeDebugTimeline,
    disposeTimeline,
  };
}
