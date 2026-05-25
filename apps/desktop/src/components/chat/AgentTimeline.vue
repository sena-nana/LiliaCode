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
  readPayloadRecord,
  readTimelinePayloadRecord,
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
    .map((event) => event.id)
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
  const summary = processEventsSummary(entry.processEvents ?? []);
  return summary ? `${verb} ${count} 项 · ${summary}` : `${verb} ${count} 项`;
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

function statusClass(status: AgentTimelineEventStatus): string {
  return `is-status-${status.replace(/_/g, "-")}`;
}

function processGroupTone(entry: TimelineEventEntry): "failed" | "running" | "done" {
  const events = entry.processEvents ?? [];
  if (events.some((event) => isFailedStatus(event.status))) return "failed";
  if (events.some((event) => isRunningStatus(event.status))) return "running";
  return "done";
}

function processGroupToneClass(entry: TimelineEventEntry): string {
  return `agent-timeline__process-toggle--${processGroupTone(entry)}`;
}

function processEventsSummary(events: AgentTimelineEvent[]): string {
  if (!events.length) return "";
  const counts = new Map<string, number>();
  for (const event of events) {
    const [key, count] = processEventSummaryUnit(event);
    counts.set(key, (counts.get(key) ?? 0) + count);
  }

  const orderedKeys = [
    "command",
    "file",
    "plan",
    "todo",
    "tool",
    "mcp",
    "web_search",
    "subagent",
    "error",
    "other",
  ];
  const parts = orderedKeys
    .map((key) => formatProcessSummaryCount(key, counts.get(key) ?? 0))
    .filter(Boolean);

  if (events.some((event) => isFailedStatus(event.status))) parts.push("有失败");
  else if (events.some((event) => isRunningStatus(event.status))) parts.push("运行中");

  return parts.join(" · ");
}

function processEventSummaryUnit(event: AgentTimelineEvent): [string, number] {
  switch (event.kind) {
    case "command":
      return ["command", 1];
    case "file_change":
      return ["file", fileChangeCount(event)];
    case "plan":
      return ["plan", 1];
    case "todo_list":
      return ["todo", 1];
    case "tool":
      return toolProcessEventSummaryUnit(event);
    case "mcp":
      return ["mcp", 1];
    case "web_search":
      return ["web_search", 1];
    case "subagent":
      return ["subagent", 1];
    case "error":
      return ["error", 1];
    default:
      return ["other", 1];
  }
}

function toolProcessEventSummaryUnit(event: AgentTimelineEvent): [string, number] {
  switch (event.title.trim()) {
    case "Bash":
      return ["command", 1];
    case "Read":
    case "Write":
    case "Edit":
    case "MultiEdit":
    case "NotebookRead":
    case "NotebookEdit":
      return ["file", toolFileCount(event)];
    case "TodoWrite":
      return ["todo", 1];
    case "WebSearch":
    case "WebFetch":
    case "Grep":
    case "Glob":
    case "LS":
      return ["web_search", 1];
    case "Task":
    case "Agent":
      return ["subagent", 1];
    default:
      return ["tool", 1];
  }
}

function fileChangeCount(event: AgentTimelineEvent): number {
  const changes = readTimelinePayloadRecord(event).changes;
  return Array.isArray(changes) && changes.length > 0 ? changes.length : 1;
}

function toolFileCount(event: AgentTimelineEvent): number {
  const payload = readTimelinePayloadRecord(event);
  const input = readPayloadRecord(payload.input);
  const args = readPayloadRecord(payload.args);
  const parameters = readPayloadRecord(payload.parameters);
  const changes = [
    payload.changes,
    input.changes,
    args.changes,
    parameters.changes,
  ].find((value) => Array.isArray(value));
  return Array.isArray(changes) && changes.length > 0 ? changes.length : 1;
}

function formatProcessSummaryCount(key: string, count: number): string {
  if (count <= 0) return "";
  switch (key) {
    case "command":
      return `${count} 条命令`;
    case "file":
      return `${count} 个文件`;
    case "plan":
      return `${count} 项计划`;
    case "todo":
      return `${count} 次待办`;
    case "tool":
      return `${count} 个工具`;
    case "mcp":
      return `${count} 次 MCP`;
    case "web_search":
      return `${count} 次搜索`;
    case "subagent":
      return `${count} 个子代理`;
    case "error":
      return `${count} 个错误`;
    default:
      return `${count} 项`;
  }
}

function isRunningStatus(status: AgentTimelineEventStatus): boolean {
  return status === "pending" ||
    status === "started" ||
    status === "running" ||
    status === "in_progress";
}

function isFailedStatus(status: AgentTimelineEventStatus): boolean {
  return status === "failed" ||
    status === "error" ||
    status === "cancelled";
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
            statusClass(entry.aggregatedStatus),
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
                  :class="[`agent-timeline__group-item--${event.kind}`, statusClass(event.status)]"
                >
                  <article class="agent-timeline__group-event">
                    <header class="agent-timeline__head agent-timeline__group-head">
                      <button
                        type="button"
                        class="agent-timeline__title agent-timeline__group-title"
                        :aria-expanded="expanded(event)"
                        :aria-controls="`agent-timeline-details-${event.id}`"
                        :disabled="!canToggle(event)"
                        @click="toggleEvent(event)"
                      >
                        <span :id="`agent-timeline-title-${event.id}`">
                          {{ timelineEventLabel(event) }}
                        </span>
                        <component
                          v-if="canToggle(event)"
                          :is="expanded(event) ? ChevronDown : ChevronRight"
                          class="agent-timeline__chevron"
                          :size="12"
                          aria-hidden="true"
                        />
                      </button>

                      <p
                        v-if="previewText(event)"
                        class="agent-timeline__preview"
                      >
                        {{ previewText(event) }}
                      </p>
                    </header>

                    <div
                      v-if="expanded(event)"
                      :id="`agent-timeline-details-${event.id}`"
                      class="agent-timeline__content"
                    >
                      <component
                        :is="eventComponent(event)"
                        :event="event"
                        :expanded="expanded(event)"
                        :compact="isCompact(event)"
                      />
                    </div>
                  </article>
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
            statusClass(entry.event.status),
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
                  :class="processGroupToneClass(entry)"
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
