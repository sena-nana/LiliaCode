import { computed } from "vue";
import type { ComputedRef } from "vue";
import type {
  AgentTimelineEvent,
  AgentTimelineEventStatus,
  ChatAttachment,
  ChatMessage,
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
  timelineInlinePreview,
  timelineDeclaredGroupUnit,
  type TimelineDisplayContext,
} from "./timelineDisplay";

type StreamableMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

interface UseAgentTimelineEntriesOptions {
  activePlanApprovalTurnId: ComputedRef<string | null | undefined>;
  events: ComputedRef<AgentTimelineEvent[]>;
  isThinking: ComputedRef<boolean | undefined>;
  projectCwd: ComputedRef<string | null | undefined>;
}

const TERMINAL_STATUSES = new Set<AgentTimelineEventStatus>([
  "success",
  "completed",
  "done",
  "error",
  "failed",
  "cancelled",
]);

const PROCESS_CATEGORY_LABELS: Record<string, string> = {
  command: "命令执行",
  file: "文件处理",
  mcp: "MCP 调用",
  plan: "计划更新",
  search: "搜索",
  subagent: "子代理任务",
  todo: "待办更新",
  tool: "工具调用",
  ask_user: "用户提问",
};

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
      if (!TERMINAL_STATUSES.has(event.status)) continue;
      completed.add(event.turnId);
    }
    return { completed, interrupted };
  });

  const visibleEvents = computed(() =>
    options.events.value.filter((event) => !isHiddenTimelineEvent(event)),
  );

  const chronologicalEntries = computed<TimelineEventEntry[]>(() =>
    visibleEvents.value
      .map((event): TimelineEventEntry => ({
        type: "event",
        id: `event:${event.id}`,
        createdAt: event.createdAt,
        turnSeq: event.turnSeq,
        intraTurnOrder: event.intraTurnOrder,
        event,
      }))
      .sort((a, b) =>
        a.turnSeq - b.turnSeq ||
        a.intraTurnOrder - b.intraTurnOrder ||
        a.createdAt - b.createdAt ||
        a.id.localeCompare(b.id)
      ),
  );

  const orderedEntries = computed<TimelineEntry[]>(() => {
    const entries = chronologicalEntries.value;
    const completed = turnState.value.completed;
    const lastAnchorByTurnId = new Map<string, TimelineEventEntry>();

    for (const entry of entries) {
      const turnId = entry.event.turnId;
      if (!turnId || !completed.has(turnId)) continue;
      if (isProcessAnchor(entry.event)) lastAnchorByTurnId.set(turnId, entry);
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
      const processEvents = isProcessAnchor(entry.event)
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
      if (isTimelineUserMessage(event)) cache.set(event.id, messageFromEvent(event));
    }
    return cache;
  });

  const showThinkingIndicator = computed(() => {
    if (!options.isThinking.value) return false;
    return !visibleEvents.value.some((event) =>
      isTimelineFinalReply(event) && isRunningStatus(event.status),
    );
  });

  function previewText(event: AgentTimelineEvent): string {
    return eventPreviewCache.value.get(event.id) ?? "";
  }

  function userMessage(event: AgentTimelineEvent): StreamableMessage {
    return userMessageCache.value.get(event.id) ?? messageFromEvent(event);
  }

  function processGroupRunning(entry: TimelineEventEntry): boolean {
    if (TERMINAL_STATUSES.has(entry.event.status)) return false;
    return hasRunningEvent(entry.processEvents ?? []);
  }

  function processGroupLabel(entry: TimelineEventEntry): string {
    return processEventsSummary(entry.processEvents ?? [], entry.event);
  }

  return {
    displayContext,
    isTimelineUserMessage,
    orderedEntries,
    previewText,
    processGroupLabel,
    processGroupRunning,
    showThinkingIndicator,
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

function processEventsSummary(
  events: AgentTimelineEvent[],
  finalEvent: AgentTimelineEvent,
): string {
  const labels: string[] = [];
  const seen = new Set<string>();
  for (const event of events) {
    const label = processEventCategoryLabel(event);
    if (!label || seen.has(label)) continue;
    seen.add(label);
    labels.push(label);
  }
  const duration = formatProcessDuration(processEventsElapsedMs(events, finalEvent));
  if (labels.length > 0) return [labels.join("、"), duration].filter(Boolean).join(" · ");
  if (duration) return `已处理 ${duration}`;
  return "处理中";
}

function processEventCategoryLabel(event: AgentTimelineEvent): string {
  const declared = timelineDeclaredGroupUnit(event);
  if (!declared) return "";
  const key = declared?.key ?? event.kind;
  return PROCESS_CATEGORY_LABELS[key] ?? PROCESS_CATEGORY_LABELS[event.kind] ?? "";
}

function processEventsElapsedMs(
  events: AgentTimelineEvent[],
  finalEvent: AgentTimelineEvent,
): number | null {
  let start = Number.POSITIVE_INFINITY;
  let processEnd = Number.NEGATIVE_INFINITY;
  let hasProcessDuration = false;
  for (const event of events) {
    if (!Number.isFinite(event.createdAt)) continue;
    const updatedAt = Number.isFinite(event.updatedAt) ? event.updatedAt : event.createdAt;
    start = Math.min(start, event.createdAt);
    processEnd = Math.max(processEnd, updatedAt, event.createdAt);
    if (updatedAt > event.createdAt) hasProcessDuration = true;
  }
  const end = hasProcessDuration || !Number.isFinite(finalEvent.createdAt)
    ? processEnd
    : Math.max(processEnd, finalEvent.createdAt);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return null;
  return end - start;
}

function formatProcessDuration(elapsedMs: number | null): string {
  if (elapsedMs === null) return "";
  return `${Math.max(1, Math.ceil(elapsedMs / 1000))} 秒`;
}

function hasRunningEvent(events: AgentTimelineEvent[]): boolean {
  return events.some((event) => isRunningStatus(event.status));
}

function isRunningStatus(status: AgentTimelineEventStatus): boolean {
  return status === "pending" ||
    status === "started" ||
    status === "running" ||
    status === "in_progress";
}

function isProcessAnchor(event: AgentTimelineEvent): boolean {
  return isTimelineFinalReply(event) || isTimelineInterruptEvent(event);
}

function isTimelineUserMessage(event: AgentTimelineEvent): boolean {
  return event.kind === "message" && !isTimelineFinalReply(event);
}

function messageFromEvent(event: AgentTimelineEvent): StreamableMessage {
  const payload = event.payload && typeof event.payload === "object" && !Array.isArray(event.payload)
    ? event.payload as Record<string, unknown>
    : {};
  const role = payload.role === "system" ? "system" : "user";
  const content = typeof payload.content === "string"
    ? payload.content
    : event.summary ?? "";
  const attachments = Array.isArray(payload.attachments)
    ? payload.attachments.filter(isChatAttachment)
    : [];

  return {
    id: event.id,
    taskId: event.taskId,
    role,
    content,
    attachments,
    createdAt: event.createdAt,
    queued: payload.queued === true,
  };
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
