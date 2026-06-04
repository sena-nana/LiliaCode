<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import ChatBubble from "./ChatBubble.vue";
import TimelineEntryItem from "./TimelineEntryItem.vue";
import type { ChatImageViewerSource } from "./imageViewer";
import { processGroupEntries, type TimelineGroupEntry } from "./timelineEntries";
import {
  isTimelineInterruptEvent,
  isTimelineExpanded,
  isTimelineFinalReply,
  pruneTimelineExpandedIds,
  toggleTimelineExpandedId,
} from "./timelineDisplay";
import {
  hasTimelinePendingActionState,
  timelinePendingActionState,
} from "./timelinePendingActions";
import { useAgentTimelineEntries } from "./useAgentTimelineEntries";
import { useTimelineRailMask } from "./useTimelineRailMask";

const props = defineProps<{
  events: AgentTimelineEvent[];
  isThinking?: boolean;
  projectCwd?: string | null;
  activePlanApprovalTurnId?: string | null;
  pendingActions?: PendingAgentAction[];
  showExpiredPendingActions?: boolean;
  canRetryEvent?: (event: AgentTimelineEvent) => boolean;
}>();

const emit = defineEmits<{
  eventToggled: [payload: { event: AgentTimelineEvent; expanded: boolean }];
  resolvePendingAction: [resolution: PendingAgentActionResolution];
  "retry-event": [event: AgentTimelineEvent];
  "open-image": [image: ChatImageViewerSource];
}>();

const toggledIds = ref<Set<string>>(new Set());
const expandedGroupIds = ref<Set<string>>(new Set());
const expandedProcessGroupIds = ref<Set<string>>(new Set());

const {
  displayContext,
  isTimelineUserMessage,
  orderedEntries,
  previewText,
  processGroupLabel,
  processGroupRunning,
  showThinkingIndicator,
  thinkingIndicatorLabel,
  timelineGroupEntryIds,
  turnState,
  userMessage,
  visibleEvents,
} = useAgentTimelineEntries({
  activePlanApprovalTurnId: computed(() => props.activePlanApprovalTurnId),
  events: computed(() => props.events),
  isThinking: computed(() => props.isThinking),
  projectCwd: computed(() => props.projectCwd),
});

const { railLineStyle, timelineRef } = useTimelineRailMask([
  orderedEntries,
  showThinkingIndicator,
  toggledIds,
  expandedGroupIds,
  expandedProcessGroupIds,
]);
void timelineRef;

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
  () => visibleEvents.value.filter(isProcessAnchor).map((e) => e.id).join("|"),
  () => {
    const valid = new Set(
      visibleEvents.value.filter(isProcessAnchor).map((e) => e.id),
    );
    expandedProcessGroupIds.value = new Set(
      [...expandedProcessGroupIds.value].filter((id) => valid.has(id)),
    );
  },
);

watch(
  () => [...turnState.value.interrupted].join("|"),
  () => {
    const turnIds = turnState.value.interrupted;
    if (turnIds.size === 0) return;
    const interruptedEventIds = new Set(
      visibleEvents.value
        .filter((event) => event.turnId && turnIds.has(event.turnId))
        .map((event) => event.id),
    );
    toggledIds.value = new Set(
      [...toggledIds.value].filter((id) => !interruptedEventIds.has(id)),
    );
    expandedProcessGroupIds.value = new Set(
      [...expandedProcessGroupIds.value].filter((id) => !interruptedEventIds.has(id)),
    );
  },
);

function expanded(event: AgentTimelineEvent): boolean {
  if (hasTimelinePendingActionState(pendingState(event))) return true;
  return isTimelineExpanded(event, toggledIds.value, displayContext.value);
}

function toggleEvent(event: AgentTimelineEvent) {
  if (isTimelineMessage(event)) return;
  const nextExpanded = !expanded(event);
  toggledIds.value = toggleTimelineExpandedId(toggledIds.value, event.id);
  emit("eventToggled", { event, expanded: nextExpanded });
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

function groupExpanded(entry: TimelineGroupEntry): boolean {
  if (entry.events.some((event) =>
    hasTimelinePendingActionState(pendingState(event))
  )) {
    return true;
  }
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

function isProcessAnchor(event: AgentTimelineEvent): boolean {
  return isTimelineFinalReply(event) || isTimelineInterruptEvent(event);
}

const pendingActions = computed(() => props.pendingActions ?? []);

function pendingState(event: AgentTimelineEvent) {
  return timelinePendingActionState(
    event,
    pendingActions.value,
    props.showExpiredPendingActions,
  );
}

</script>

<template>
  <section
    v-if="orderedEntries.length || showThinkingIndicator"
    ref="timelineRef"
    class="agent-timeline"
    aria-label="Agent 工作过程"
  >
    <div class="agent-timeline__rail-layer" aria-hidden="true">
      <span class="agent-timeline__rail-line" :style="railLineStyle" />
    </div>
    <ol class="agent-timeline__list">
      <template v-for="entry in orderedEntries" :key="entry.id">
        <li
          v-if="entry.type === 'event' && isTimelineUserMessage(entry.event)"
          class="agent-timeline__message-row"
          :data-scroll-anchor-id="entry.event.id"
          :class="[
            `agent-timeline__message-row--${userMessage(entry.event).role}`,
            { 'is-queued': userMessage(entry.event).queued },
          ]"
        >
          <ChatBubble
            :message="userMessage(entry.event)"
            @open-image="emit('open-image', $event)"
          />
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
          :project-cwd="projectCwd"
          :pending-actions="pendingActions"
          :show-expired-pending-actions="showExpiredPendingActions"
          :can-retry-event="canRetryEvent"
          @toggle-event="toggleEvent"
          @toggle-group="toggleGroup"
          @toggle-process-group="toggleProcessGroup"
          @resolve-pending-action="emit('resolvePendingAction', $event)"
          @retry-event="emit('retry-event', $event)"
          @open-image="emit('open-image', $event)"
        />
      </template>
      <li
        v-if="showThinkingIndicator"
        class="agent-timeline__item agent-timeline__item--thinking is-status-running"
        aria-live="polite"
      >
        <article class="agent-timeline__event">
          <div class="agent-timeline__rail" />
          <div class="agent-timeline__body">
            <p class="agent-timeline__thinking-label">{{ thinkingIndicatorLabel }}</p>
          </div>
        </article>
      </li>
    </ol>
  </section>
</template>
