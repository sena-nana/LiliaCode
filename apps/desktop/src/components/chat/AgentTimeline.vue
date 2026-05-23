<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import {
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleDot,
  Code2,
  FilePenLine,
  GitBranch,
  Globe2,
  ListChecks,
  Terminal,
  TriangleAlert,
  UserRound,
  Wrench,
  type LucideIcon,
} from "lucide-vue-next";
import type {
  AgentTimelineEvent,
  AgentTimelineEventKind,
  ChatMessage,
} from "@lilia/contracts";
import ChatBubble from "./ChatBubble.vue";
import TimelineCommandEvent from "./TimelineCommandEvent.vue";
import TimelineErrorEvent from "./TimelineErrorEvent.vue";
import TimelineFileChangeEvent from "./TimelineFileChangeEvent.vue";
import TimelineFinalReply from "./TimelineFinalReply.vue";
import TimelinePlanEvent from "./TimelinePlanEvent.vue";
import TimelineSubagentEvent from "./TimelineSubagentEvent.vue";
import TimelineSummaryEvent from "./TimelineSummaryEvent.vue";
import TimelineTodoEvent from "./TimelineTodoEvent.vue";
import TimelineToolEvent from "./TimelineToolEvent.vue";
import {
  isTimelineExpanded,
  isTimelineFinalReply,
  timelineInlinePreview,
  pruneTimelineExpandedIds,
  timelineEventLabel,
  timelineKindLabel,
  timelineStatusClass,
  timelineStatusLabel,
  toggleTimelineExpandedId,
} from "./timelineDisplay";

type StreamableMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

type TimelineEntry =
  | {
      type: "message";
      id: string;
      createdAt: number;
      order: number;
      message: StreamableMessage;
    }
  | {
      type: "event";
      id: string;
      createdAt: number;
      order: number;
      event: AgentTimelineEvent;
    };

const props = defineProps<{
  events: AgentTimelineEvent[];
  messages?: StreamableMessage[];
}>();

const toggledIds = ref<Set<string>>(new Set());

const visibleMessages = computed(() =>
  (props.messages ?? []).filter((message) => message.role !== "assistant"),
);

const finalReplySeen = computed(() => props.events.some(isTimelineFinalReply));

const orderedEntries = computed<TimelineEntry[]>(() =>
  [
    ...visibleMessages.value.map((message): TimelineEntry => ({
      type: "message",
      id: `message:${message.id}`,
      createdAt: message.createdAt,
      order: 0,
      message,
    })),
    ...props.events.map((event): TimelineEntry => ({
      type: "event",
      id: `event:${event.id}`,
      createdAt: event.createdAt,
      order: event.order + 1,
      event,
    })),
  ].sort((a, b) =>
    a.createdAt - b.createdAt || a.order - b.order || a.id.localeCompare(b.id)
  ),
);

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
  finalReplySeen,
  (seen) => {
    if (!seen) return;
    toggledIds.value = new Set();
  },
);

function expanded(event: AgentTimelineEvent): boolean {
  if (finalReplySeen.value && !isTimelineFinalReply(event)) {
    return toggledIds.value.has(event.id);
  }
  return isTimelineExpanded(event, toggledIds.value);
}

function isCompact(event: AgentTimelineEvent): boolean {
  return !isTimelineFinalReply(event) && !expanded(event);
}

function canToggle(event: AgentTimelineEvent): boolean {
  return !isTimelineFinalReply(event);
}

function toggleEvent(event: AgentTimelineEvent) {
  if (!canToggle(event)) return;
  toggledIds.value = toggleTimelineExpandedId(toggledIds.value, event.id);
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
    case "reasoning":
    case "turn":
    default:
      return TimelineSummaryEvent;
  }
}

function kindIcon(kind: AgentTimelineEventKind): LucideIcon {
  const icons: Record<AgentTimelineEventKind, LucideIcon> = {
    reasoning: Brain,
    plan: ListChecks,
    todo_list: CheckCircle2,
    tool: Wrench,
    command: Terminal,
    subagent: GitBranch,
    file_change: FilePenLine,
    mcp: Code2,
    web_search: Globe2,
    error: TriangleAlert,
    turn: CircleDot,
  };
  return icons[kind] ?? CircleDot;
}

function previewText(event: AgentTimelineEvent): string {
  return eventPreviewCache.value.get(event.id) ?? "";
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
          v-if="entry.type === 'message'"
          class="agent-timeline__message-row"
          :class="[
            `agent-timeline__message-row--${entry.message.role}`,
            { 'is-queued': entry.message.queued },
          ]"
        >
          <span class="agent-timeline__message-icon" aria-hidden="true">
            <UserRound :size="14" />
          </span>
          <ChatBubble :message="entry.message" />
        </li>

        <li
          v-else
          class="agent-timeline__item"
          :class="[
            `agent-timeline--${entry.event.kind}`,
            `agent-timeline__item--${entry.event.status}`,
            timelineStatusClass(entry.event.status),
            {
              'is-expanded': expanded(entry.event),
              'is-final-reply': isTimelineFinalReply(entry.event),
              'is-compact': isCompact(entry.event),
            },
          ]"
        >
          <span class="agent-timeline__icon" aria-hidden="true">
            <component :is="kindIcon(entry.event.kind)" :size="14" />
          </span>

          <article
            class="agent-timeline__event"
            :aria-labelledby="`agent-timeline-title-${entry.event.id}`"
          >
            <header class="agent-timeline__head">
              <div
                class="agent-timeline__title-row"
                :class="{ 'has-preview': previewText(entry.event) }"
              >
                <button
                  type="button"
                  class="agent-timeline__title"
                  :aria-expanded="expanded(entry.event)"
                  :aria-controls="`agent-timeline-details-${entry.event.id}`"
                  :disabled="!canToggle(entry.event)"
                  @click="toggleEvent(entry.event)"
                >
                  <component
                    :is="expanded(entry.event) ? ChevronDown : ChevronRight"
                    class="agent-timeline__chevron"
                    :size="14"
                    aria-hidden="true"
                  />
                  <span :id="`agent-timeline-title-${entry.event.id}`">
                    {{ timelineEventLabel(entry.event) }}
                  </span>
                </button>

                <p v-if="previewText(entry.event)" class="agent-timeline__preview">
                  {{ previewText(entry.event) }}
                </p>
              </div>

              <div
                v-if="expanded(entry.event) || isTimelineFinalReply(entry.event)"
                class="agent-timeline__meta"
                aria-label="事件分类和状态"
              >
                <span class="agent-timeline__badge">{{ timelineKindLabel(entry.event.kind) }}</span>
                <span class="agent-timeline__badge">{{ timelineStatusLabel(entry.event.status) }}</span>
              </div>
            </header>

            <div
              v-if="expanded(entry.event) || isTimelineFinalReply(entry.event)"
              :id="`agent-timeline-details-${entry.event.id}`"
              class="agent-timeline__content"
            >
              <component
                :is="eventComponent(entry.event)"
                :event="entry.event"
                :expanded="expanded(entry.event)"
                :compact="isCompact(entry.event)"
              />
            </div>
          </article>
        </li>
      </template>
    </ol>
  </section>
</template>
