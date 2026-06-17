import { computed, shallowRef } from "vue";
import { onDebugTimelineEvent } from "../../composables/useDebugTimelineEvents";
import {
  listAgentTimeline,
} from "../../services/chat";
import {
  conversationReferencesToPayload,
  readConversationReferences,
} from "../../services/chatConversationReferences";
import type {
  AgentTimelineEvent,
  AgentTimelinePayload,
  ChatAttachment,
  ChatBackendKind,
  ChatConversationReference,
} from "@lilia/contracts";

export interface TimelineRetryContext {
  content: string;
  attachments: ChatAttachment[];
  conversationReferences?: ChatConversationReference[];
}

export interface CreateMessageTimelineEventInput {
  id: string;
  taskId: string;
  backend: ChatBackendKind;
  content: string;
  attachments?: ChatAttachment[];
  conversationReferences?: ChatConversationReference[];
  createdAt: number;
  queued?: boolean;
}

export interface CreateErrorTimelineEventInput {
  id: string;
  taskId: string;
  backend: ChatBackendKind;
  message: string;
  createdAt: number;
  retryContext?: TimelineRetryContext;
}

export function attachmentsToTimelinePayload(
  attachments: ChatAttachment[],
): AgentTimelinePayload[] {
  return attachments.map((attachment) => ({
    id: attachment.id,
    name: attachment.name,
    path: attachment.path,
    kind: attachment.kind,
    size: attachment.size,
    exists: attachment.exists ?? null,
    mime: attachment.mime ?? null,
    directory: attachment.directory
      ? {
        fileCount: attachment.directory.fileCount,
        directoryCount: attachment.directory.directoryCount,
        totalSize: attachment.directory.totalSize,
        truncated: attachment.directory.truncated,
        unreadableCount: attachment.directory.unreadableCount,
      }
      : null,
  }));
}

export function createMessageTimelineEvent(
  input: CreateMessageTimelineEventInput,
): AgentTimelineEvent {
  return {
    id: input.id,
    taskId: input.taskId,
    turnId: null,
    backend: input.backend,
    kind: "message",
    status: input.queued ? "pending" : "success",
    title: "用户输入",
    summary: input.content,
    payload: {
      role: "user",
      content: input.content,
      attachments: attachmentsToTimelinePayload(input.attachments ?? []),
      conversationReferences: conversationReferencesToPayload(input.conversationReferences ?? []),
      queued: input.queued === true,
    },
    createdAt: input.createdAt,
    updatedAt: input.createdAt,
    turnSeq: Number.MAX_SAFE_INTEGER,
    intraTurnOrder: 0,
  };
}

export function createErrorTimelineEvent(
  input: CreateErrorTimelineEventInput,
): AgentTimelineEvent {
  const payload: Record<string, AgentTimelinePayload> = {
    message: input.message,
  };
  if (input.retryContext) {
    payload.retryContext = {
      content: input.retryContext.content,
      attachments: attachmentsToTimelinePayload(input.retryContext.attachments),
      conversationReferences: conversationReferencesToPayload(
        input.retryContext.conversationReferences ?? [],
      ),
    };
  }
  return {
    id: input.id,
    taskId: input.taskId,
    turnId: null,
    backend: input.backend,
    kind: "error",
    status: "error",
    title: "错误",
    summary: input.message,
    payload,
    createdAt: input.createdAt,
    updatedAt: input.createdAt,
    turnSeq: Number.MAX_SAFE_INTEGER,
    intraTurnOrder: Number.MAX_SAFE_INTEGER,
  };
}

export function upsertTimelineEventById(
  events: AgentTimelineEvent[],
  event: AgentTimelineEvent,
): AgentTimelineEvent[] {
  const existingIndex = events.findIndex((item) => item.id === event.id);
  if (existingIndex < 0) return [...events, event];
  const next = events.slice();
  next[existingIndex] = event;
  return next;
}

function compareTimelineEvents(a: AgentTimelineEvent, b: AgentTimelineEvent): number {
  return a.turnSeq - b.turnSeq ||
    a.intraTurnOrder - b.intraTurnOrder ||
    a.createdAt - b.createdAt ||
    a.id.localeCompare(b.id);
}

export function upsertTimelineEventsById(
  events: AgentTimelineEvent[],
  nextEvents: AgentTimelineEvent[],
): AgentTimelineEvent[] {
  if (nextEvents.length === 0) return events;
  const byId = new Map<string, AgentTimelineEvent>();
  for (const event of events) byId.set(event.id, event);
  for (const event of nextEvents) byId.set(event.id, event);
  return [...byId.values()].sort(compareTimelineEvents);
}

export function mergeTimelineEvents(
  events: AgentTimelineEvent[],
  current: AgentTimelineEvent[],
): AgentTimelineEvent[] {
  const byId = new Map<string, AgentTimelineEvent>();
  for (const event of events) byId.set(event.id, event);
  for (const event of current) {
    if (!byId.has(event.id)) byId.set(event.id, event);
  }
  return [...byId.values()].sort(compareTimelineEvents);
}

export function mergeLoadedTimelineEvents(
  loaded: AgentTimelineEvent[],
  current: AgentTimelineEvent[],
  preserveEventIds: Set<string> = new Set(),
): AgentTimelineEvent[] {
  const loadedKeys = new Set(
    loaded
      .filter(isUserMessageEvent)
      .map(userMessageIdentityKey),
  );
  const preservedEvents = current.filter((event) =>
    preserveEventIds.has(event.id) ||
    (isQueuedUserMessageEvent(event) && !loadedKeys.has(userMessageIdentityKey(event)))
  );
  return mergeTimelineEvents(loaded, preservedEvents);
}

export function markFirstQueuedUserMessageSuccessful(
  events: AgentTimelineEvent[],
  now = Date.now(),
): AgentTimelineEvent[] {
  let cleared = false;
  return events.map((event) => {
    const payload = readTimelineEventPayloadRecord(event);
    if (!cleared && event.kind === "message" && payload.queued === true) {
      cleared = true;
      return {
        ...event,
        status: "success",
        payload: { ...payload, queued: false },
        updatedAt: now,
      };
    }
    return event;
  });
}

export function retryContextForTimelineEvent(
  event: AgentTimelineEvent,
  timelineEvents: AgentTimelineEvent[],
): TimelineRetryContext | null {
  if (event.kind !== "error") return null;
  const payload = readTimelineEventPayloadRecord(event);
  const embedded = readRetryContext(payload.retryContext);
  if (embedded) return embedded;
  if (!event.turnId) return null;
  const source = timelineEvents.find((candidate) => {
    if (candidate.kind !== "message" || candidate.turnId !== event.turnId) return false;
    return readTimelineEventPayloadRecord(candidate).role === "user";
  });
  if (!source) return null;
  const sourcePayload = readTimelineEventPayloadRecord(source);
  return readRetryContext({
    content: typeof sourcePayload.content === "string" ? sourcePayload.content : source.summary ?? "",
    attachments: sourcePayload.attachments,
    conversationReferences: sourcePayload.conversationReferences,
  });
}

export function readRetryContext(value: unknown): TimelineRetryContext | null {
  const payload = readPayloadRecord(value);
  const content = typeof payload.content === "string" ? payload.content : "";
  const attachments = Array.isArray(payload.attachments)
    ? payload.attachments.filter(isChatAttachment)
    : [];
  const conversationReferences = readConversationReferences(payload.conversationReferences);
  if (!content.trim() && attachments.length === 0 && conversationReferences.length === 0) return null;
  return { content, attachments, conversationReferences };
}

export function readTimelineEventPayloadRecord(
  event: AgentTimelineEvent,
): Record<string, unknown> {
  return readPayloadRecord(event.payload);
}

export function readPayloadRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

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

function isUserMessageEvent(event: AgentTimelineEvent): boolean {
  if (event.kind !== "message") return false;
  const payload = readTimelineEventPayloadRecord(event);
  return payload.role === "user" || payload.role === "system";
}

function isQueuedUserMessageEvent(event: AgentTimelineEvent): boolean {
  return isUserMessageEvent(event) &&
    readTimelineEventPayloadRecord(event).queued === true;
}

function userMessageIdentityKey(event: AgentTimelineEvent): string {
  const payload = readTimelineEventPayloadRecord(event);
  const content = typeof payload.content === "string" ? payload.content : event.summary ?? "";
  const attachments = Array.isArray(payload.attachments)
    ? payload.attachments
      .map((attachment) => {
        const row = readPayloadRecord(attachment);
        return typeof row.path === "string" ? row.path : "";
      })
      .filter(Boolean)
      .join("\u001f")
    : "";
  const conversationReferences = readConversationReferences(payload.conversationReferences)
    .map((reference) => reference.taskId)
    .join("\u001f");
  return `${payload.role ?? "user"}\u001f${content}\u001f${attachments}\u001f${conversationReferences}`;
}

function isChatAttachment(value: unknown): value is ChatAttachment {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const row = value as Record<string, unknown>;
  return typeof row.id === "string" &&
    typeof row.name === "string" &&
    typeof row.path === "string" &&
    (row.kind === "file" || row.kind === "directory" || row.kind === "unknown") &&
    (typeof row.size === "number" || row.size === null);
}
