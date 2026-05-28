<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type {
  AgentTimelineEvent,
  AgentTimelineEventStatus,
  ChatAttachment,
  ChatMessage,
} from "@lilia/contracts";
import ChatBubble from "./ChatBubble.vue";
import TimelineEntryItem from "./TimelineEntryItem.vue";
import TimelineNodeIcon from "./TimelineNodeIcon.vue";
import {
  mergeAdjacentTimelineGroups,
  processGroupEntries,
  type TimelineEntry,
  type TimelineEventEntry,
  type TimelineGroupEntry,
} from "./timelineEntries";
import {
  isHiddenTimelineEvent,
  isTimelineExpanded,
  isTimelineFinalReply,
  timelineInlinePreview,
  pruneTimelineExpandedIds,
  timelineDeclaredGroupUnit,
  toggleTimelineExpandedId,
} from "./timelineDisplay";

type StreamableMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

const props = defineProps<{
  events: AgentTimelineEvent[];
  isThinking?: boolean;
}>();

const toggledIds = ref<Set<string>>(new Set());
const expandedGroupIds = ref<Set<string>>(new Set());
const expandedProcessGroupIds = ref<Set<string>>(new Set());

const TERMINAL_TURN_STATUSES = new Set<AgentTimelineEventStatus>([
  "success",
  "completed",
  "done",
  "error",
  "failed",
  "cancelled",
]);

const completedTurnIds = computed<Set<string>>(() => {
  const set = new Set<string>();
  for (const event of props.events) {
    if (event.kind !== "turn") continue;
    if (!event.turnId) continue;
    if (!TERMINAL_TURN_STATUSES.has(event.status)) continue;
    set.add(event.turnId);
  }
  return set;
});

const visibleEvents = computed(() =>
  props.events.filter((event) => !isHiddenTimelineEvent(event)),
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
  const completed = completedTurnIds.value;
  const lastFinalByTurnId = new Map<string, TimelineEventEntry>();

  for (const entry of entries) {
    const turnId = entry.event.turnId;
    if (!turnId || !completed.has(turnId)) continue;
    if (isTimelineFinalReply(entry.event)) lastFinalByTurnId.set(turnId, entry);
  }

  const processEventsByFinalId = new Map<string, AgentTimelineEvent[]>();
  const hiddenEventIds = new Set<string>();

  for (const entry of entries) {
    const turnId = entry.event.turnId;
    if (!turnId || !completed.has(turnId)) continue;
    if (isTimelineUserMessage(entry.event)) continue;
    const finalEntry = lastFinalByTurnId.get(turnId);
    if (!finalEntry) continue;
    if (entry.intraTurnOrder >= finalEntry.intraTurnOrder) continue;
    let list = processEventsByFinalId.get(finalEntry.event.id);
    if (!list) {
      list = [];
      processEventsByFinalId.set(finalEntry.event.id, list);
    }
    list.push(entry.event);
    hiddenEventIds.add(entry.event.id);
  }

  const output: TimelineEventEntry[] = [];
  for (const entry of entries) {
    if (hiddenEventIds.has(entry.event.id)) continue;
    const processEvents = isTimelineFinalReply(entry.event)
      ? processEventsByFinalId.get(entry.event.id)
      : undefined;
    output.push(processEvents ? { ...entry, processEvents } : entry);
  }

  return mergeAdjacentTimelineGroups(output);
});

const eventPreviewCache = computed(() => {
  const cache = new Map<string, string>();
  for (const event of visibleEvents.value) {
    cache.set(event.id, timelineInlinePreview(event));
  }
  return cache;
});

const showThinkingIndicator = computed(() => {
  if (!props.isThinking) return false;
  return !visibleEvents.value.some((event) =>
    isTimelineFinalReply(event) && isRunningStatus(event.status),
  );
});

watch(
  () => visibleEvents.value.map((event) => event.id).join("|"),
  () => {
    toggledIds.value = pruneTimelineExpandedIds(toggledIds.value, visibleEvents.value);
  },
);

watch(
  orderedEntries,
  (entries) => {
    const valid = timelineGroupEntryIds(entries);
    expandedGroupIds.value = new Set(
      [...expandedGroupIds.value].filter((id) => valid.has(id)),
    );
  },
);

watch(
  () => visibleEvents.value.filter(isTimelineFinalReply).map((e) => e.id).join("|"),
  () => {
    const valid = new Set(
      visibleEvents.value.filter(isTimelineFinalReply).map((e) => e.id),
    );
    expandedProcessGroupIds.value = new Set(
      [...expandedProcessGroupIds.value].filter((id) => valid.has(id)),
    );
  },
);

function expanded(event: AgentTimelineEvent): boolean {
  return isTimelineExpanded(event, toggledIds.value);
}

function toggleEvent(event: AgentTimelineEvent) {
  if (isTimelineMessage(event)) return;
  toggledIds.value = toggleTimelineExpandedId(toggledIds.value, event.id);
}

function processGroupExpanded(event: AgentTimelineEvent): boolean {
  return expandedProcessGroupIds.value.has(event.id);
}

function toggleProcessGroup(event: AgentTimelineEvent) {
  const next = new Set(expandedProcessGroupIds.value);
  if (next.has(event.id)) next.delete(event.id);
  else next.add(event.id);
  expandedProcessGroupIds.value = next;
}

function processGroupRunning(entry: TimelineEventEntry): boolean {
  return hasRunningEvent(entry.processEvents ?? []);
}

function processGroupLabel(entry: TimelineEventEntry): string {
  return processEventsSummary(entry.processEvents ?? [], entry.event);
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

const PROCESS_CATEGORY_LABELS: Record<string, string> = {
  command: "命令执行",
  file: "文件处理",
  mcp: "MCP 调用",
  plan: "计划更新",
  search: "搜索",
  subagent: "子代理任务",
  todo: "待办更新",
  tool: "工具调用",
};

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

function previewText(event: AgentTimelineEvent): string {
  return eventPreviewCache.value.get(event.id) ?? "";
}

function groupExpanded(entry: TimelineGroupEntry): boolean {
  return expandedGroupIds.value.has(entry.id);
}

function toggleGroup(entry: TimelineGroupEntry) {
  const next = new Set(expandedGroupIds.value);
  if (next.has(entry.id)) next.delete(entry.id);
  else next.add(entry.id);
  expandedGroupIds.value = next;
}

function isTimelineMessage(event: AgentTimelineEvent): boolean {
  return event.kind === "message";
}

function isTimelineUserMessage(event: AgentTimelineEvent): boolean {
  return isTimelineMessage(event) && !isTimelineFinalReply(event);
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
</script>

<template>
  <section
    v-if="orderedEntries.length || showThinkingIndicator"
    class="agent-timeline"
    aria-label="Agent 工作过程"
  >
    <ol class="agent-timeline__list">
      <template v-for="entry in orderedEntries" :key="entry.id">
        <li
          v-if="entry.type === 'event' && isTimelineUserMessage(entry.event)"
          class="agent-timeline__message-row"
          :class="[
            `agent-timeline__message-row--${messageFromEvent(entry.event).role}`,
            { 'is-queued': messageFromEvent(entry.event).queued },
          ]"
        >
          <ChatBubble :message="messageFromEvent(entry.event)" />
        </li>

        <TimelineEntryItem
          v-else
          :entry="entry"
          :expanded="expanded"
          :group-expanded="groupExpanded"
          :process-group-expanded="processGroupExpanded"
          :process-group-label="processGroupLabel"
          :process-group-running="processGroupRunning"
          :process-entries-for="processGroupEntries"
          :preview-text="previewText"
          @toggle-event="toggleEvent"
          @toggle-group="toggleGroup"
          @toggle-process-group="toggleProcessGroup"
        />
      </template>
      <li
        v-if="showThinkingIndicator"
        class="agent-timeline__item agent-timeline__item--thinking is-status-running"
        aria-live="polite"
      >
        <article class="agent-timeline__event">
          <div class="agent-timeline__rail">
            <TimelineNodeIcon :status="'running'" :icon="null" />
          </div>
          <div class="agent-timeline__body">
            <p class="agent-timeline__thinking-label">思考中…</p>
          </div>
        </article>
      </li>
    </ol>
  </section>
</template>
