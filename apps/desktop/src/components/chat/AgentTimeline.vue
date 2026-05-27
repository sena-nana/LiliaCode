<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import type {
  AgentTimelineEvent,
  AgentTimelineEventStatus,
  ChatAttachment,
  ChatMessage,
} from "@lilia/contracts";
import ChatBubble from "./ChatBubble.vue";
import MarkdownBlock from "./MarkdownBlock.vue";
import TimelineDeclaredEvent from "./TimelineDeclaredEvent.vue";
import TimelineFinalReply from "./TimelineFinalReply.vue";
import TimelineNodeIcon from "./TimelineNodeIcon.vue";
import {
  aggregateTimelineStatus,
  isHiddenTimelineEvent,
  isTimelineAssistantMessage,
  isTimelineExpanded,
  isTimelineFinalReply,
  isTimelineFinalReplyStreaming,
  readTimelineDisplay,
  readTimelinePayloadRecord,
  timelineInlinePreview,
  pruneTimelineExpandedIds,
  timelineDeclaredGroupUnit,
  timelineDisplayIcon,
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
  turnSeq: number;
  intraTurnOrder: number;
  event: AgentTimelineEvent;
};

type TimelineGroupEntry = {
  type: "group";
  id: string;
  createdAt: number;
  turnSeq: number;
  intraTurnOrder: number;
  groupKey: string;
  events: AgentTimelineEvent[];
  representative: AgentTimelineEvent;
  aggregatedStatus: AgentTimelineEventStatus;
  groupCount: number;
};

type TimelineEntry = TimelineEventEntry | TimelineGroupEntry;

const props = defineProps<{
  events: AgentTimelineEvent[];
  /** Turn 正在跑、但还没有最终回复时，在时间线末尾显示「思考中…」指示器。 */
  isThinking?: boolean;
}>();

const toggledIds = ref<Set<string>>(new Set());
const expandedGroupIds = ref<Set<string>>(new Set());

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
    // 主排序键 (turnSeq, intraTurnOrder) 由 Rust 端在落库时分配——turnSeq 按
    // turn_id 首次出现单调递增，intraTurnOrder 按 turn 内事件落库顺序单调递增。
    // 把"按 turn 隔离"做到 DB 层，前端只忠实地按 schema 提供的复合键展开即可；
    // createdAt / id 仅作为同序兜底。
    .sort((a, b) =>
      a.turnSeq - b.turnSeq ||
      a.intraTurnOrder - b.intraTurnOrder ||
      a.createdAt - b.createdAt ||
      a.id.localeCompare(b.id)
    ),
);

// runner 已经把 assistant 回复拆成 per-block 的 inline message 事件，UI 这层
// 不再聚合「最终回复 + 折叠过程」——所有事件按 order 内联渲染，相邻同类工具
// 走 mergeAdjacentGroups 折叠成一组以避免噪点。
const orderedEntries = computed<TimelineEntry[]>(() =>
  mergeAdjacentGroups(chronologicalEntries.value),
);

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
        turnSeq: first.turnSeq,
        intraTurnOrder: first.intraTurnOrder,
        groupKey: bucketKey,
        events,
        representative: first.event,
        aggregatedStatus: aggregateTimelineStatus(events),
        groupCount: events.reduce((total, event) => total + eventGroupCount(event), 0),
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
  for (const event of visibleEvents.value) {
    cache.set(event.id, timelineInlinePreview(event));
  }
  return cache;
});

/**
 * 「思考中」指示器只在 turn 在跑、且当前 turn 还没开始流式回复时出现。
 * 一旦有 running 状态的 assistant message（流式开头）落地，回复卡片本身
 * 的光标就够用了；这里只想覆盖 turn 启动 → 第一个 token 到达 之间的空窗。
 */
const showThinkingIndicator = computed(() => {
  if (!props.isThinking) return false;
  return !visibleEvents.value.some((event) => {
    if (!isTimelineAssistantMessage(event)) return false;
    return (
      event.status === "pending" ||
      event.status === "started" ||
      event.status === "running" ||
      event.status === "in_progress"
    );
  });
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
    const valid = new Set(
      entries.filter((entry): entry is TimelineGroupEntry => entry.type === "group").map((entry) => entry.id),
    );
    expandedGroupIds.value = new Set(
      [...expandedGroupIds.value].filter((id) => valid.has(id)),
    );
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

function eventComponent(event: AgentTimelineEvent): Component {
  if (isTimelineFinalReply(event)) return TimelineFinalReply;
  return TimelineDeclaredEvent;
}

function previewText(event: AgentTimelineEvent): string {
  return eventPreviewCache.value.get(event.id) ?? "";
}

function titleAriaLabel(event: AgentTimelineEvent): string {
  const label = timelineEventLabel(event);
  const object = readTimelineDisplay(event).object?.trim() ?? "";
  return object ? `${label} ${object}` : label;
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
  return timelineGroupLabel(entry.representative, entry.groupCount, entry.aggregatedStatus);
}

function nodeIcon(event: AgentTimelineEvent) {
  return timelineDisplayIcon(event);
}

function statusClass(status: AgentTimelineEventStatus): string {
  return `is-status-${status.replace(/_/g, "-")}`;
}

function kindClass(prefix: string, kind: string): string {
  return `${prefix}${kind.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

function eventGroupCount(event: AgentTimelineEvent): number {
  return timelineDeclaredGroupUnit(event)?.count ?? 1;
}

function isTimelineMessage(event: AgentTimelineEvent): boolean {
  return event.kind === "message";
}

function isTimelineReasoning(event: AgentTimelineEvent): boolean {
  return event.kind === "reasoning";
}

function reasoningContent(event: AgentTimelineEvent): string {
  // payload.text 是完整累加文本；event.summary 在中间层被截到 1200 字符。
  const payload = readTimelinePayloadRecord(event);
  const fromPayload = typeof payload.text === "string" ? payload.text.trim() : "";
  if (fromPayload) return fromPayload;
  const fromSummary = (event.summary ?? "").trim();
  if (fromSummary) return fromSummary;
  return (readTimelineDisplay(event).preview ?? "").trim();
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
          v-if="entry.type === 'group'"
          class="agent-timeline__item agent-timeline__item--group"
          :class="[
            kindClass('agent-timeline__item--', entry.representative.kind),
            statusClass(entry.aggregatedStatus),
            {
              'is-compact': !groupExpanded(entry),
            },
          ]"
        >
          <article
            class="agent-timeline__event"
            :aria-labelledby="`agent-timeline-title-${entry.id}`"
          >
            <div class="agent-timeline__rail">
              <TimelineNodeIcon
                :status="entry.aggregatedStatus"
                :icon="nodeIcon(entry.representative)"
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
                  :class="[kindClass('agent-timeline__group-item--', event.kind), statusClass(event.status)]"
                >
                  <article class="agent-timeline__group-event">
                    <header class="agent-timeline__head agent-timeline__group-head">
                      <button
                        type="button"
                        class="agent-timeline__title agent-timeline__group-title"
                        :aria-expanded="expanded(event)"
                        :aria-controls="`agent-timeline-details-${event.id}`"
                        :aria-label="titleAriaLabel(event)"
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
          v-else-if="isTimelineReasoning(entry.event) && reasoningContent(entry.event)"
          class="agent-timeline__item agent-timeline__item--reasoning-inline"
          :class="[statusClass(entry.event.status)]"
        >
          <MarkdownBlock
            :content="reasoningContent(entry.event)"
            tone="default"
            class="agent-timeline__reasoning-markdown"
          />
        </li>

        <li
          v-else
          class="agent-timeline__item"
          :class="[
            kindClass('agent-timeline__item--', entry.event.kind),
            statusClass(entry.event.status),
            {
              'is-final-reply': isTimelineFinalReply(entry.event),
              'is-compact': isCompact(entry.event),
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
                :status="entry.event.status"
                :icon="nodeIcon(entry.event)"
              />
            </div>
            <div class="agent-timeline__body">
              <header
                v-if="!isTimelineFinalReply(entry.event)"
                class="agent-timeline__head"
              >
                <button
                  type="button"
                  class="agent-timeline__title"
                  :aria-expanded="expanded(entry.event)"
                  :aria-controls="`agent-timeline-details-${entry.event.id}`"
                  :aria-label="titleAriaLabel(entry.event)"
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
                  v-if="previewText(entry.event)"
                  class="agent-timeline__preview"
                >
                  {{ previewText(entry.event) }}
                </p>
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
