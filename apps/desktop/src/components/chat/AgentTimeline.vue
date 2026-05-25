<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import type { AgentTimelineEvent, AgentTimelineEventStatus, ChatMessage } from "@lilia/contracts";
import ChatBubble from "./ChatBubble.vue";
import TimelineCommandEvent from "./TimelineCommandEvent.vue";
import TimelineErrorEvent from "./TimelineErrorEvent.vue";
import TimelineFileChangeEvent from "./TimelineFileChangeEvent.vue";
import TimelineFinalReply from "./TimelineFinalReply.vue";
import TimelineNodeIcon from "./TimelineNodeIcon.vue";
import TimelinePlanEvent from "./TimelinePlanEvent.vue";
import TimelineSubagentEvent from "./TimelineSubagentEvent.vue";
import TimelineSummaryEvent from "./TimelineSummaryEvent.vue";
import TimelineTodoEvent from "./TimelineTodoEvent.vue";
import TimelineToolEvent from "./TimelineToolEvent.vue";
import {
  aggregateTimelineStatus,
  isTimelineAssistantMessage,
  isTimelineExpanded,
  isTimelineFinalReply,
  isTimelineFinalReplyStreaming,
  timelineInlinePreview,
  pruneTimelineExpandedIds,
  timelineEventLabel,
  timelineGroupKey,
  timelineGroupLabel,
  toggleTimelineExpandedId,
} from "./timelineDisplay";

type StreamableMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

type TimelineEventEntry = {
  type: "event";
  id: string;
  createdAt: number;
  order: number;
  event: AgentTimelineEvent;
  processEvents?: AgentTimelineEvent[];
  isProcessChild?: boolean;
};

type TimelineGroupEntry = {
  type: "group";
  id: string;
  createdAt: number;
  order: number;
  groupKey: string;
  events: AgentTimelineEvent[];
  representative: AgentTimelineEvent;
  aggregatedStatus: AgentTimelineEventStatus;
  isProcessChild?: boolean;
};

type TimelineEntry = TimelineEventEntry | TimelineGroupEntry;

const props = defineProps<{
  events: AgentTimelineEvent[];
}>();

const toggledIds = ref<Set<string>>(new Set());
const expandedProcessGroupIds = ref<Set<string>>(new Set());
const expandedGroupIds = ref<Set<string>>(new Set());

const finalReplyCollapseKey = computed(() =>
  props.events
    .filter(isTimelineFinalReply)
    .map((event) => [event.id, event.status, event.order].join(":"))
    .join("|"),
);

const chronologicalEntries = computed<TimelineEventEntry[]>(() =>
  props.events
    .map((event): TimelineEventEntry => ({
      type: "event",
      id: `event:${event.id}`,
      createdAt: event.createdAt,
      order: event.order,
      event,
    }))
    .sort((a, b) =>
      a.createdAt - b.createdAt || a.order - b.order || a.id.localeCompare(b.id)
    ),
);

const orderedEntries = computed<TimelineEntry[]>(() => {
  const entries = chronologicalEntries.value;
  const finalByTurnId = new Map<string, TimelineEventEntry>();
  const processEventsByFinalId = new Map<string, AgentTimelineEvent[]>();
  const hiddenEventIds = new Set<string>();

  for (const entry of entries) {
    if (isTimelineAssistantMessage(entry.event) && entry.event.turnId) {
      finalByTurnId.set(entry.event.turnId, entry);
    }
  }

  // 同一 turnId 的非 message 事件统统折叠到该 turn 的最终回复下，
  // 不再做 createdAt 前后比较——流式 message 的 createdAt 会随 upsert 漂移到末尾，
  // 用 turnId 而不是时序更稳定。
  for (const entry of entries) {
    if (!entry.event.turnId) continue;
    if (isTimelineMessage(entry.event)) continue;
    const finalEntry = finalByTurnId.get(entry.event.turnId);
    if (!finalEntry) continue;
    const list = processEventsByFinalId.get(finalEntry.event.id) ?? [];
    if (!list.some((item) => item.id === entry.event.id)) list.push(entry.event);
    processEventsByFinalId.set(finalEntry.event.id, list);
    hiddenEventIds.add(entry.event.id);
  }

  const output: TimelineEventEntry[] = [];
  for (const entry of entries) {
    if (hiddenEventIds.has(entry.event.id)) continue;

    if (isTimelineFinalReply(entry.event)) {
      const processEvents = processEventsByFinalId.get(entry.event.id) ?? [];
      if (processGroupExpanded(entry.event)) {
        output.push(...processEvents.map((event): TimelineEventEntry => ({
          type: "event",
          id: `process:${entry.event.id}:${event.id}`,
          createdAt: event.createdAt,
          order: event.order + 1,
          event,
          isProcessChild: true,
        })));
      }
      output.push({
        ...entry,
        processEvents,
      });
    } else {
      output.push(entry);
    }
  }

  return mergeAdjacentGroups(output);
});

function mergeAdjacentGroups(entries: TimelineEventEntry[]): TimelineEntry[] {
  const result: TimelineEntry[] = [];
  let bucket: TimelineEventEntry[] = [];
  let bucketKey: string | null = null;

  const flush = () => {
    if (bucket.length === 0) return;
    if (bucket.length === 1 || !bucketKey) {
      result.push(...bucket);
    } else {
      const first = bucket[0];
      const events = bucket.map((b) => b.event);
      result.push({
        type: "group",
        id: `group:${bucketKey}:${first.event.id}`,
        createdAt: first.createdAt,
        order: first.order,
        groupKey: bucketKey,
        events,
        representative: first.event,
        aggregatedStatus: aggregateTimelineStatus(events),
        isProcessChild: first.isProcessChild,
      });
    }
    bucket = [];
    bucketKey = null;
  };

  for (const entry of entries) {
    const event = entry.event;
    const key = isTimelineFinalReply(event) || isTimelineMessage(event)
      ? null
      : timelineGroupKey(event);

    if (!key) {
      flush();
      result.push(entry);
      continue;
    }

    if (bucketKey === key) {
      bucket.push(entry);
    } else {
      flush();
      bucket = [entry];
      bucketKey = key;
    }
  }
  flush();

  return result;
}

const eventPreviewCache = computed(() => {
  const cache = new Map<string, string>();
  for (const event of props.events) {
    cache.set(event.id, timelineInlinePreview(event));
  }
  return cache;
});

watch(
  () => props.events.map((event) => event.id).join("|"),
  () => {
    toggledIds.value = pruneTimelineExpandedIds(toggledIds.value, props.events);
  },
);

watch(
  orderedEntries,
  (entries) => {
    const valid = new Set(
      entries.filter((entry): entry is TimelineGroupEntry => entry.type === "group").map((entry) => entry.id),
    );
    expandedGroupIds.value = new Set(
      [...expandedGroupIds.value].filter((id) => valid.has(id)),
    );
  },
);

watch(
  finalReplyCollapseKey,
  (key, previousKey) => {
    if (!key || key === previousKey) return;
    toggledIds.value = new Set();
    expandedProcessGroupIds.value = new Set();
    expandedGroupIds.value = new Set();
  },
);

function expanded(event: AgentTimelineEvent): boolean {
  return isTimelineExpanded(event, toggledIds.value);
}

function isCompact(event: AgentTimelineEvent): boolean {
  return !isTimelineFinalReply(event) && !isTimelineMessage(event) && !expanded(event);
}

function canToggle(event: AgentTimelineEvent): boolean {
  return !isTimelineFinalReply(event) && !isTimelineMessage(event);
}

function toggleEvent(event: AgentTimelineEvent) {
  if (!canToggle(event)) return;
  toggledIds.value = toggleTimelineExpandedId(toggledIds.value, event.id);
}

function processEventCount(entry: TimelineEntry): number {
  if (entry.type !== "event") return 0;
  return entry.processEvents?.length ?? 0;
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

function processGroupLabel(entry: TimelineEventEntry): string {
  const count = processEventCount(entry);
  const verb = processGroupExpanded(entry.event) ? "收起过程" : "展开过程";
  return `${verb} ${count} 项`;
}

function eventComponent(event: AgentTimelineEvent): Component {
  if (isTimelineFinalReply(event)) return TimelineFinalReply;
  switch (event.kind) {
    case "plan":
      return TimelinePlanEvent;
    case "todo_list":
      return TimelineTodoEvent;
    case "command":
      return TimelineCommandEvent;
    case "file_change":
      return TimelineFileChangeEvent;
    case "tool":
    case "mcp":
    case "web_search":
      return TimelineToolEvent;
    case "subagent":
      return TimelineSubagentEvent;
    case "error":
      return TimelineErrorEvent;
    case "message":
    case "reasoning":
    case "turn":
    default:
      return TimelineSummaryEvent;
  }
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

function groupLabelText(entry: TimelineGroupEntry): string {
  return timelineGroupLabel(entry.representative, entry.events.length, entry.aggregatedStatus);
}

function groupItemText(event: AgentTimelineEvent): string {
  return previewText(event) || event.summary?.trim() || event.title.trim() || event.kind;
}

function isTimelineMessage(event: AgentTimelineEvent): boolean {
  return event.kind === "message";
}

function isTimelineUserMessage(event: AgentTimelineEvent): boolean {
  return isTimelineMessage(event) && !isTimelineAssistantMessage(event);
}

function messageFromEvent(event: AgentTimelineEvent): StreamableMessage {
  const payload = event.payload && typeof event.payload === "object" && !Array.isArray(event.payload)
    ? event.payload as Record<string, unknown>
    : {};
  const role = payload.role === "system" || payload.role === "assistant"
    ? payload.role
    : "user";
  const content = typeof payload.content === "string"
    ? payload.content
    : event.summary ?? "";

  return {
    id: event.id,
    taskId: event.taskId,
    role,
    content,
    createdAt: event.createdAt,
    queued: payload.queued === true,
  };
}
</script>

<template>
  <section
    v-if="orderedEntries.length"
    class="agent-timeline"
    aria-label="Agent 工作过程"
  >
    <ol class="agent-timeline__list">
      <template v-for="entry in orderedEntries" :key="entry.id">
        <li
          v-if="entry.type === 'group'"
          class="agent-timeline__item agent-timeline__item--group"
          :class="[
            `agent-timeline__item--${entry.representative.kind}`,
            {
              'is-compact': !groupExpanded(entry),
              'is-process-child': entry.isProcessChild,
            },
          ]"
        >
          <article
            class="agent-timeline__event"
            :aria-labelledby="`agent-timeline-title-${entry.id}`"
          >
            <div class="agent-timeline__rail">
              <TimelineNodeIcon
                :kind="entry.representative.kind"
                :status="entry.aggregatedStatus"
              />
            </div>
            <div class="agent-timeline__body">
              <header class="agent-timeline__head">
                <button
                  type="button"
                  class="agent-timeline__title"
                  :aria-expanded="groupExpanded(entry)"
                  :aria-controls="`agent-timeline-details-${entry.id}`"
                  @click="toggleGroup(entry)"
                >
                  <span :id="`agent-timeline-title-${entry.id}`">
                    {{ groupLabelText(entry) }}
                  </span>
                  <component
                    :is="groupExpanded(entry) ? ChevronDown : ChevronRight"
                    class="agent-timeline__chevron"
                    :size="12"
                    aria-hidden="true"
                  />
                </button>
              </header>

              <ul
                v-if="groupExpanded(entry)"
                :id="`agent-timeline-details-${entry.id}`"
                class="agent-timeline__group-list"
              >
                <li
                  v-for="event in entry.events"
                  :key="event.id"
                  class="agent-timeline__group-item"
                >
                  {{ groupItemText(event) }}
                </li>
              </ul>
            </div>
          </article>
        </li>

        <li
          v-else-if="isTimelineUserMessage(entry.event)"
          class="agent-timeline__message-row"
          :class="[
            `agent-timeline__message-row--${messageFromEvent(entry.event).role}`,
            { 'is-queued': messageFromEvent(entry.event).queued },
          ]"
        >
          <ChatBubble :message="messageFromEvent(entry.event)" />
        </li>

        <li
          v-else
          class="agent-timeline__item"
          :class="[
            `agent-timeline__item--${entry.event.kind}`,
            {
              'is-final-reply': isTimelineFinalReply(entry.event),
              'is-compact': isCompact(entry.event),
              'is-process-child': entry.isProcessChild,
            },
          ]"
        >
          <article
            class="agent-timeline__event"
            :aria-labelledby="isTimelineFinalReply(entry.event) ? undefined : `agent-timeline-title-${entry.event.id}`"
            :aria-label="isTimelineFinalReply(entry.event) ? 'Agent 回复' : undefined"
          >
            <div class="agent-timeline__rail">
              <TimelineNodeIcon
                :kind="entry.event.kind"
                :status="entry.event.status"
              />
            </div>
            <div class="agent-timeline__body">
              <header
                v-if="!isTimelineFinalReply(entry.event) || processEventCount(entry) > 0"
                class="agent-timeline__head"
              >
                <button
                  v-if="!isTimelineFinalReply(entry.event)"
                  type="button"
                  class="agent-timeline__title"
                  :aria-expanded="expanded(entry.event)"
                  :aria-controls="`agent-timeline-details-${entry.event.id}`"
                  :disabled="!canToggle(entry.event)"
                  @click="toggleEvent(entry.event)"
                >
                  <span :id="`agent-timeline-title-${entry.event.id}`">
                    {{ timelineEventLabel(entry.event) }}
                  </span>
                  <component
                    v-if="canToggle(entry.event)"
                    :is="expanded(entry.event) ? ChevronDown : ChevronRight"
                    class="agent-timeline__chevron"
                    :size="12"
                    aria-hidden="true"
                  />
                </button>

                <p
                  v-if="!isTimelineFinalReply(entry.event) && previewText(entry.event)"
                  class="agent-timeline__preview"
                >
                  {{ previewText(entry.event) }}
                </p>

                <button
                  v-if="isTimelineFinalReply(entry.event) && processEventCount(entry) > 0"
                  type="button"
                  class="agent-timeline__process-toggle"
                  :aria-expanded="processGroupExpanded(entry.event)"
                  @click="toggleProcessGroup(entry.event)"
                >
                  {{ processGroupLabel(entry) }}
                </button>
              </header>

              <div
                v-if="expanded(entry.event) || isTimelineFinalReply(entry.event)"
                :id="`agent-timeline-details-${entry.event.id}`"
                class="agent-timeline__content"
              >
                <TimelineFinalReply
                  v-if="isTimelineFinalReply(entry.event)"
                  :event="entry.event"
                  :streaming="isTimelineFinalReplyStreaming(entry.event)"
                />
                <component
                  v-else
                  :is="eventComponent(entry.event)"
                  :event="entry.event"
                  :expanded="expanded(entry.event)"
                  :compact="isCompact(entry.event)"
                />
              </div>
            </div>
          </article>
        </li>
      </template>
    </ol>
  </section>
</template>
