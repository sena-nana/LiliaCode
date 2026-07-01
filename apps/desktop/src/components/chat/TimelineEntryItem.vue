<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import ChevronDown from "@lucide/vue/dist/esm/icons/chevron-down.mjs";
import ChevronRight from "@lucide/vue/dist/esm/icons/chevron-right.mjs";
import RotateCcw from "@lucide/vue/dist/esm/icons/rotate-ccw.mjs";
import type { AgentTimelineEvent, ChatBranchAnchor } from "@lilia/contracts";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/pendingAgentActions";
import TimelineNodeIcon from "./TimelineNodeIcon.vue";
import type { LiliaBatchApplyInput } from "@lilia/contracts";
import type { ChatImageViewerSource } from "./imageViewer";
import type { TimelineEntry, TimelineEventEntry, TimelineGroupEntry } from "./timelineEntries";
import {
  hasTimelinePendingActionState,
  type TimelinePendingActionStateReader,
} from "./timelinePendingActions";
import {
  isTimelineErrorReply,
  isTimelineFinalReply,
  isTimelineFinalReplyStreaming,
  isTimelineMessageEvent,
  readTimelineDisplay,
  timelineCanExpand,
  timelineDisplayIcon,
  timelineEventLabel,
  timelineGroupLabel,
  timelineKindClass,
  timelineStatusClass,
  type TimelineDisplayContext,
} from "./timelineDisplay";
import { measurePerfAsync } from "@lilia/ui";
import { createLazyLoadState } from "@lilia/ui";

const timelineDeclaredEventLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "timeline.declared.load",
    async () => (await import("./TimelineDeclaredEvent.vue")).default,
  )
);
const timelineFinalReplyLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "timeline.final-reply.load",
    async () => (await import("./TimelineFinalReply.vue")).default,
  )
);
const timelinePlanCardLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "timeline.plan-card.load",
    async () => (await import("./TimelinePlanCard.vue")).default,
  )
);

const TimelineDeclaredEvent = defineAsyncComponent({
  suspensible: false,
  loader: () => timelineDeclaredEventLoad.load(),
});

const TimelineFinalReply = defineAsyncComponent({
  suspensible: false,
  loader: () => timelineFinalReplyLoad.load(),
});

const TimelinePlanCard = defineAsyncComponent({
  suspensible: false,
  loader: () => timelinePlanCardLoad.load(),
});

const props = defineProps<{
  entry: TimelineEntry;
  expanded: (event: AgentTimelineEvent) => boolean;
  groupExpanded: (entry: TimelineGroupEntry) => boolean;
  processGroupExpanded: (event: AgentTimelineEvent) => boolean;
  processGroupLabel: (entry: TimelineEventEntry) => string;
  processGroupRunning: (entry: TimelineEventEntry) => boolean;
  processEntriesFor: (entry: TimelineEventEntry) => TimelineEntry[];
  previewText: (event: AgentTimelineEvent) => string;
  projectCwd?: string | null;
  pendingState: TimelinePendingActionStateReader;
  canRetryEvent?: (event: AgentTimelineEvent) => boolean;
  canStartLiliaBatchApply?: boolean;
  canStartSessionFork?: boolean;
}>();

const emit = defineEmits<{
  toggleEvent: [event: AgentTimelineEvent];
  toggleGroup: [entry: TimelineGroupEntry];
  toggleProcessGroup: [event: AgentTimelineEvent];
  resolvePendingAction: [resolution: PendingAgentActionResolution];
  "retry-event": [event: AgentTimelineEvent];
  "open-image": [image: ChatImageViewerSource];
  "start-lilia-batch-apply": [input: LiliaBatchApplyInput];
  "start-session-fork": [anchor: ChatBranchAnchor];
}>();

const displayContext = computed<TimelineDisplayContext>(() => ({ projectCwd: props.projectCwd }));

function canToggle(event: AgentTimelineEvent): boolean {
  return timelineCanExpand(event, displayContext.value) ||
    hasTimelinePendingActionState(props.pendingState(event));
}

function canRetry(event: AgentTimelineEvent): boolean {
  return props.canRetryEvent?.(event) === true;
}

function isCompact(event: AgentTimelineEvent): boolean {
  return !isTimelineMessageEvent(event) && !props.expanded(event);
}

function hasProcessEvents(entry: TimelineEventEntry): boolean {
  return Boolean(entry.processEvents?.length);
}

function shouldShowHeader(entry: TimelineEventEntry): boolean {
  return !isTimelineFinalReply(entry.event) ||
    isTimelineErrorReply(entry.event) ||
    hasProcessEvents(entry) ||
    canRetry(entry.event);
}

function shouldShowNodeIcon(entry: TimelineEventEntry): boolean {
  return !entry.isProcessChild || !isTimelineFinalReply(entry.event);
}

function titleAriaLabel(event: AgentTimelineEvent): string {
  const context = displayContext.value;
  const label = timelineEventLabel(event, context);
  const object = readTimelineDisplay(event, context).object?.trim() ?? "";
  return object ? `${label} ${object}` : label;
}

function groupLabelText(entry: TimelineGroupEntry): string {
  return timelineGroupLabel(
    entry.representative,
    entry.groupCount,
    entry.aggregatedStatus,
    displayContext.value,
  );
}

function nodeIcon(event: AgentTimelineEvent) {
  return timelineDisplayIcon(event, displayContext.value);
}

function labelText(event: AgentTimelineEvent): string {
  return timelineEventLabel(event, displayContext.value);
}

function pendingAction(event: AgentTimelineEvent): PendingAgentAction | null {
  return props.pendingState(event).action;
}

function expiredPendingAction(event: AgentTimelineEvent): boolean {
  return props.pendingState(event).expired;
}

function groupScrollAnchorIds(entry: TimelineGroupEntry): string {
  return entry.events.map((event) => event.id).join(" ");
}
</script>

<template>
  <li
    v-if="entry.type === 'group'"
    class="agent-timeline__item agent-timeline__item--group"
    :data-agent-id="`timeline.group.${entry.id}`"
    :data-scroll-anchor-ids="groupScrollAnchorIds(entry)"
    :class="[
      timelineKindClass('agent-timeline__item--', entry.representative.kind),
      timelineStatusClass(entry.aggregatedStatus),
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
          :icon="nodeIcon(entry.representative)"
        />
      </div>
      <div class="agent-timeline__body">
        <header class="agent-timeline__head">
          <button
            type="button"
            class="agent-timeline__title"
            :data-agent-id="`timeline.group.${entry.id}.toggle`"
            :aria-expanded="groupExpanded(entry)"
            :aria-controls="`agent-timeline-details-${entry.id}`"
            @click="emit('toggleGroup', entry)"
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
            :data-agent-id="`timeline.event.${event.id}`"
            :data-scroll-anchor-id="event.id"
            :class="[timelineKindClass('agent-timeline__group-item--', event.kind), timelineStatusClass(event.status)]"
          >
            <TimelinePlanCard
              v-if="event.kind === 'plan'"
              :event="event"
              :expanded="expanded(event)"
              :can-toggle="canToggle(event)"
              :project-cwd="projectCwd"
              :pending-action="pendingAction(event)"
              :action-expired="expiredPendingAction(event)"
              @toggle="emit('toggleEvent', $event)"
              @resolve-pending-action="emit('resolvePendingAction', $event)"
            />
            <article v-else class="agent-timeline__group-event">
              <header class="agent-timeline__head agent-timeline__group-head">
                <button
                  type="button"
                  class="agent-timeline__title agent-timeline__group-title"
                  :data-agent-id="`timeline.event.${event.id}.toggle`"
                  :aria-expanded="expanded(event)"
                  :aria-controls="`agent-timeline-details-${event.id}`"
                  :aria-label="titleAriaLabel(event)"
                  :disabled="!canToggle(event)"
                  @click="emit('toggleEvent', event)"
                >
                  <span :id="`agent-timeline-title-${event.id}`">
                    {{ labelText(event) }}
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

                <button
                  v-if="canRetry(event)"
                  type="button"
                  class="agent-timeline__retry"
                  :data-agent-id="`timeline.event.${event.id}.retry`"
                  title="重新发送上下文"
                  aria-label="重试"
                  @click.stop="emit('retry-event', event)"
                >
                  <RotateCcw :size="12" aria-hidden="true" />
                </button>
              </header>

              <div
                v-if="canToggle(event) && expanded(event)"
                :id="`agent-timeline-details-${event.id}`"
                class="agent-timeline__content"
              >
                <TimelineDeclaredEvent
                  :event="event"
                  :expanded="expanded(event)"
                  :compact="isCompact(event)"
                  :project-cwd="projectCwd"
                  :pending-action="pendingAction(event)"
                  :action-expired="expiredPendingAction(event)"
                  @resolve-pending-action="emit('resolvePendingAction', $event)"
                />
              </div>
            </article>
          </li>
        </ul>
      </div>
    </article>
  </li>

  <li
    v-else
    class="agent-timeline__item"
    :data-agent-id="`timeline.event.${entry.event.id}`"
    :data-scroll-anchor-id="entry.event.id"
    :class="[
      timelineKindClass('agent-timeline__item--', entry.event.kind),
      timelineStatusClass(entry.event.status),
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
          v-if="shouldShowNodeIcon(entry)"
          :icon="nodeIcon(entry.event)"
        />
      </div>
      <div class="agent-timeline__body">
        <TimelinePlanCard
          v-if="entry.event.kind === 'plan'"
          :event="entry.event"
          :expanded="expanded(entry.event)"
          :can-toggle="canToggle(entry.event)"
          :project-cwd="projectCwd"
          :pending-action="pendingAction(entry.event)"
          :action-expired="expiredPendingAction(entry.event)"
          @toggle="emit('toggleEvent', $event)"
          @resolve-pending-action="emit('resolvePendingAction', $event)"
        />

        <template v-else>
          <header
            v-if="shouldShowHeader(entry)"
            class="agent-timeline__head"
          >
            <span
              v-if="isTimelineErrorReply(entry.event)"
              class="agent-timeline__title"
              :data-agent-id="`timeline.event.${entry.event.id}.toggle`"
            >
              <span>{{ labelText(entry.event) }}</span>
            </span>
            <button
              v-if="!isTimelineFinalReply(entry.event)"
      type="button"
      class="agent-timeline__title"
      :data-agent-id="`timeline.entry.toggle.${entry.event.id}`"
      :aria-expanded="expanded(entry.event)"
              :aria-controls="`agent-timeline-details-${entry.event.id}`"
              :aria-label="titleAriaLabel(entry.event)"
              :disabled="!canToggle(entry.event)"
              @click="emit('toggleEvent', entry.event)"
            >
              <span :id="`agent-timeline-title-${entry.event.id}`">
                {{ labelText(entry.event) }}
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
              v-if="canRetry(entry.event)"
              type="button"
              class="agent-timeline__retry"
              :data-agent-id="`timeline.event.${entry.event.id}.retry`"
              title="重新发送上下文"
              aria-label="重试"
              @click.stop="emit('retry-event', entry.event)"
            >
              <RotateCcw :size="12" aria-hidden="true" />
            </button>

            <button
              v-if="hasProcessEvents(entry)"
              type="button"
              class="agent-timeline__process-toggle"
              :data-agent-id="`timeline.event.${entry.event.id}.process-toggle`"
              :class="{ 'agent-timeline__process-toggle--running': processGroupRunning(entry) }"
              :aria-expanded="processGroupExpanded(entry.event)"
              @click="emit('toggleProcessGroup', entry.event)"
            >
              <span class="agent-timeline__process-summary">
                {{ processGroupLabel(entry) }}
              </span>
            </button>
          </header>

          <div
            v-if="hasProcessEvents(entry)"
            class="agent-timeline__process-collapse"
            :class="{ 'is-open': processGroupExpanded(entry.event) }"
            :aria-hidden="!processGroupExpanded(entry.event)"
            :inert="!processGroupExpanded(entry.event) ? true : undefined"
          >
            <ol class="agent-timeline__process-collapse-inner">
              <TimelineEntryItem
                v-for="processEntry in processEntriesFor(entry)"
                :key="processEntry.id"
                :entry="processEntry"
                :expanded="expanded"
                :group-expanded="groupExpanded"
                :process-group-expanded="processGroupExpanded"
                :process-group-label="processGroupLabel"
                :process-group-running="processGroupRunning"
                :process-entries-for="processEntriesFor"
                :preview-text="previewText"
                :project-cwd="projectCwd"
                :pending-state="pendingState"
                :can-retry-event="canRetryEvent"
                :can-start-lilia-batch-apply="canStartLiliaBatchApply"
                :can-start-session-fork="canStartSessionFork"
                @toggle-event="emit('toggleEvent', $event)"
                @toggle-group="emit('toggleGroup', $event)"
                @toggle-process-group="emit('toggleProcessGroup', $event)"
                @resolve-pending-action="emit('resolvePendingAction', $event)"
                @retry-event="emit('retry-event', $event)"
                @open-image="emit('open-image', $event)"
                @start-lilia-batch-apply="emit('start-lilia-batch-apply', $event)"
                @start-session-fork="emit('start-session-fork', $event)"
              />
            </ol>
          </div>

          <div
            v-if="canToggle(entry.event) && expanded(entry.event)"
            :id="`agent-timeline-details-${entry.event.id}`"
            class="agent-timeline__content"
          >
            <TimelineFinalReply
              v-if="isTimelineFinalReply(entry.event)"
              :event="entry.event"
              :streaming="isTimelineFinalReplyStreaming(entry.event)"
              :can-start-lilia-batch-apply="canStartLiliaBatchApply"
              :can-start-session-fork="canStartSessionFork"
              @open-image="emit('open-image', $event)"
              @start-lilia-batch-apply="emit('start-lilia-batch-apply', $event)"
              @start-session-fork="emit('start-session-fork', $event)"
            />
            <TimelineDeclaredEvent
              v-else
              :event="entry.event"
              :expanded="expanded(entry.event)"
              :compact="isCompact(entry.event)"
              :project-cwd="projectCwd"
              :pending-action="pendingAction(entry.event)"
              :action-expired="expiredPendingAction(entry.event)"
              @resolve-pending-action="emit('resolvePendingAction', $event)"
            />
          </div>
        </template>
      </div>
    </article>
  </li>
</template>

