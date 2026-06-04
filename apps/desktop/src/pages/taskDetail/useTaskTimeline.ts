import { computed, shallowRef } from "vue";
import { onDebugTimelineEvent } from "../../composables/useDebugTimelineEvents";
import {
  listAgentTimeline,
} from "../../services/chat";
import type {
  AgentTimelineEvent,
  AgentTimelinePayload,
  ChatAttachment,
  ChatBackendKind,
} from "@lilia/contracts";

export interface TimelineRetryContext {
  content: string;
  attachments: ChatAttachment[];
}

export interface CreateMessageTimelineEventInput {
  id: string;
  taskId: string;
  backend: ChatBackendKind;
  content: string;
  attachments?: ChatAttachment[];
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

export function mergeTimelineEvents(
  events: AgentTimelineEvent[],
  current: AgentTimelineEvent[],
): AgentTimelineEvent[] {
  const byId = new Map<string, AgentTimelineEvent>();
  for (const event of events) byId.set(event.id, event);
  for (const event of current) {
    if (!byId.has(event.id)) byId.set(event.id, event);
  }
  return [...byId.values()].sort((a, b) =>
    a.turnSeq - b.turnSeq ||
    a.intraTurnOrder - b.intraTurnOrder ||
    a.createdAt - b.createdAt ||
    a.id.localeCompare(b.id)
  );
}

export function mergeLoadedTimelineEvents(
  loaded: AgentTimelineEvent[],
  current: AgentTimelineEvent[],
): AgentTimelineEvent[] {
  const loadedKeys = new Set(
    loaded
      .filter(isUserMessageEvent)
      .map(userMessageIdentityKey),
  );
  const optimisticEvents = current.filter((event) =>
    isQueuedUserMessageEvent(event) && !loadedKeys.has(userMessageIdentityKey(event))
  );
  return mergeTimelineEvents(loaded, optimisticEvents);
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

export function canRetryTimelineEvent(event: AgentTimelineEvent): boolean {
  return retryContextForTimelineEvent(event, []) !== null;
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
  });
}

export function readRetryContext(value: unknown): TimelineRetryContext | null {
  const payload = readPayloadRecord(value);
  const content = typeof payload.content === "string" ? payload.content : "";
  const attachments = Array.isArray(payload.attachments)
    ? payload.attachments.filter(isChatAttachment)
    : [];
  if (!content.trim() && attachments.length === 0) return null;
  return { content, attachments };
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

  function upsertTimelineEvent(event: AgentTimelineEvent) {
    persistedTimelineEvents.value = upsertTimelineEventById(
      persistedTimelineEvents.value,
      event,
    );
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
    createdAt?: number;
  }): AgentTimelineEvent {
    const createdAt = input.createdAt ?? Date.now();
    return createMessageTimelineEvent({
      id: nextOptimisticMessageId(),
      taskId: options.taskId(),
      backend: options.backend(),
      content: input.content,
      attachments: input.attachments,
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

  function applyLoadedTimelineEvents(events: AgentTimelineEvent[]) {
    persistedTimelineEvents.value = mergeLoadedTimelineEvents(
      events,
      persistedTimelineEvents.value,
    );
  }

  function markQueuedUserMessageSuccessful() {
    persistedTimelineEvents.value = markFirstQueuedUserMessageSuccessful(
      persistedTimelineEvents.value,
    );
  }

  function resetTimeline() {
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
    unsubscribeDebugTimeline?.();
    unsubscribeDebugTimeline = null;
  }

  resubscribeDebugTimeline();

  return {
    timelineEvents,
    persistedTimelineEvents,
    overlayTimelineEvents,
    upsertTimelineEvent,
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
  return `${payload.role ?? "user"}\u001f${content}\u001f${attachments}`;
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
