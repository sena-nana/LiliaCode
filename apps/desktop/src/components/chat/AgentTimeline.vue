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
import TimelineDeclaredEvent from "./TimelineDeclaredEvent.vue";
import TimelineFinalReply from "./TimelineFinalReply.vue";
import TimelineNodeIcon from "./TimelineNodeIcon.vue";
import {
  aggregateTimelineStatus,
  isHiddenTimelineEvent,
  isTimelineExpanded,
  isTimelineFinalReply,
  isTimelineFinalReplyStreaming,
  readTimelineDisplay,
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
  /** Final reply 卡上挂着的、被折叠到它下面的过程事件（按原始顺序）。 */
  processEvents?: AgentTimelineEvent[];
  /** expanded 时从 processEvents 还原出来的子项。 */
  isProcessChild?: boolean;
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
  isProcessChild?: boolean;
};

type TimelineEntry = TimelineEventEntry | TimelineGroupEntry;

const props = defineProps<{
  events: AgentTimelineEvent[];
  /** Turn 正在跑、但还没有最终回复时，在时间线末尾显示「思考中…」指示器。 */
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

/**
 * Turn 结束的权威信号：runner 在 `case "result"` emit 的 `kind:"turn"` + 终态
 * status。流式中 turn 未进集合 → 全部 inline；完成那一帧进集合 → 该 turn 的
 * 过程事件折叠到最后一条 assistant message 下，避免「最后一条」随新 text
 * block 漂移造成折叠抖动。
 */
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

// 主排序键 (turnSeq, intraTurnOrder) 由 Rust 落库时分配；createdAt / id 只兜
// 同序碰撞（如 turnSeq=MAX 的乐观事件）。
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

// turn 完成后把同 turn 内所有事件折叠到「最后一条 assistant message」下面，
// **只保留用户消息（锚点）和最终回复本身在外**。reasoning / 中间 text block /
// 工具 / 计划全部进 processEvents——保证 user message 和最终回复之间视觉上只
// 剩一个「展开过程 N 项」按钮，需要时点开还原完整时间线。
const orderedEntries = computed<TimelineEntry[]>(() => {
  const entries = chronologicalEntries.value;
  const completed = completedTurnIds.value;
  const lastFinalByTurnId = new Map<string, TimelineEventEntry>();

  for (const entry of entries) {
    const turnId = entry.event.turnId;
    if (!turnId || !completed.has(turnId)) continue;
    // 同 turn 多条 assistant message 时取**最后一条**：SDK 在 stop_reason=
    // end_turn 之前都可能再开新 text block。
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
    // intraTurnOrder >= finalEntry 自然排除 final 自身（同序）和 final 之后的事件。
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
    if (processEvents && expandedProcessGroupIds.value.has(entry.event.id)) {
      for (const event of processEvents) {
        output.push({
          type: "event",
          id: `process:${entry.event.id}:${event.id}`,
          createdAt: event.createdAt,
          turnSeq: event.turnSeq,
          intraTurnOrder: event.intraTurnOrder,
          event,
          isProcessChild: true,
        });
      }
    }
    output.push(processEvents ? { ...entry, processEvents } : entry);
  }

  return mergeAdjacentGroups(output);
});

function mergeAdjacentGroups(entries: TimelineEventEntry[]): TimelineEntry[] {
  const result: TimelineEntry[] = [];
  let bucket: TimelineEventEntry[] = [];
  let bucketKey: string | null = null;

  const flush = () => {
    if (bucket.length === 0) return;
    if (bucket.length === 1) {
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
        groupKey: bucketKey!,
        events,
        representative: first.event,
        aggregatedStatus: aggregateTimelineStatus(events),
        groupCount: events.reduce((total, event) => total + eventGroupCount(event), 0),
        isProcessChild: first.isProcessChild,
      });
    }
    bucket = [];
    bucketKey = null;
  };

  for (const entry of entries) {
    const event = entry.event;
    const key = isTimelineMessage(event) ? null : timelineGroupKey(event);

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
    const valid = new Set(
      entries.filter((entry): entry is TimelineGroupEntry => entry.type === "group").map((entry) => entry.id),
    );
    expandedGroupIds.value = new Set(
      [...expandedGroupIds.value].filter((id) => valid.has(id)),
    );
  },
);

// 用户切到另一个 task 或事件被裁掉时清理 expandedProcessGroupIds。
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

function isCompact(event: AgentTimelineEvent): boolean {
  return !isTimelineMessage(event) && !expanded(event);
}

function canToggle(event: AgentTimelineEvent): boolean {
  return !isTimelineMessage(event);
}

function toggleEvent(event: AgentTimelineEvent) {
  if (!canToggle(event)) return;
  toggledIds.value = toggleTimelineExpandedId(toggledIds.value, event.id);
}

function eventComponent(event: AgentTimelineEvent): Component {
  if (isTimelineFinalReply(event)) return TimelineFinalReply;
  return TimelineDeclaredEvent;
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

function processGroupTone(entry: TimelineEventEntry): "failed" | "running" | "done" {
  const events = entry.processEvents ?? [];
  if (events.some((event) => isFailedStatus(event.status))) return "failed";
  if (events.some((event) => isRunningStatus(event.status))) return "running";
  return "done";
}

function processGroupToneClass(entry: TimelineEventEntry): string {
  return `agent-timeline__process-toggle--${processGroupTone(entry)}`;
}

function processGroupLabel(entry: TimelineEventEntry): string {
  const count = processEventCount(entry);
  const verb = processGroupExpanded(entry.event) ? "收起过程" : "展开过程";
  const summary = processEventsSummary(entry.processEvents ?? []);
  return summary ? `${verb} ${count} 项 · ${summary}` : `${verb} ${count} 项`;
}

function processEventsSummary(events: AgentTimelineEvent[]): string {
  const counts = new Map<string, { count: number; unit: string }>();
  for (const event of events) {
    const declared = timelineDeclaredGroupUnit(event);
    if (!declared || !declared.unit) continue;
    const existing = counts.get(declared.key);
    if (existing) existing.count += declared.count;
    else counts.set(declared.key, { count: declared.count, unit: declared.unit });
  }
  const parts = [...counts.values()].map(({ count, unit }) => `${count} ${unit}`);
  if (events.some((e) => isFailedStatus(e.status))) parts.push("有失败");
  else if (events.some((e) => isRunningStatus(e.status))) parts.push("运行中");
  return parts.join(" · ");
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

function isTimelineUserMessage(event: AgentTimelineEvent): boolean {
  return isTimelineMessage(event) && !isTimelineFinalReply(event);
}

function messageFromEvent(event: AgentTimelineEvent): StreamableMessage {
  const payload = event.payload && typeof event.payload === "object" && !Array.isArray(event.payload)
    ? event.payload as Record<string, unknown>
    : {};
  // 仅在 isTimelineUserMessage 命中后调用——assistant 走 final reply 卡，
  // 不会落到 ChatBubble。这里只区分 user / system。
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
          v-if="entry.type === 'group'"
          class="agent-timeline__item agent-timeline__item--group"
          :class="[
            kindClass('agent-timeline__item--', entry.representative.kind),
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
          v-else
          class="agent-timeline__item"
          :class="[
            kindClass('agent-timeline__item--', entry.event.kind),
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
                :status="entry.event.status"
                :icon="nodeIcon(entry.event)"
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
                v-if="expanded(entry.event)"
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
