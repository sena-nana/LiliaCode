<script setup lang="ts">
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import type { AgentTimelineEvent, AgentTimelineEventStatus } from "@lilia/contracts";
import TimelineDeclaredEvent from "./TimelineDeclaredEvent.vue";
import TimelineFinalReply from "./TimelineFinalReply.vue";
import TimelineNodeIcon from "./TimelineNodeIcon.vue";
import type { TimelineEntry, TimelineEventEntry, TimelineGroupEntry } from "./timelineEntries";
import {
  isTimelineFinalReply,
  isTimelineFinalReplyStreaming,
  readTimelineDisplay,
  timelineDisplayIcon,
  timelineEventLabel,
  timelineGroupLabel,
} from "./timelineDisplay";

const props = defineProps<{
  entry: TimelineEntry;
  expanded: (event: AgentTimelineEvent) => boolean;
  groupExpanded: (entry: TimelineGroupEntry) => boolean;
  processGroupExpanded: (event: AgentTimelineEvent) => boolean;
  processGroupLabel: (entry: TimelineEventEntry) => string;
  processGroupRunning: (entry: TimelineEventEntry) => boolean;
  processEntriesFor: (entry: TimelineEventEntry) => TimelineEntry[];
  previewText: (event: AgentTimelineEvent) => string;
}>();

const emit = defineEmits<{
  toggleEvent: [event: AgentTimelineEvent];
  toggleGroup: [entry: TimelineGroupEntry];
  toggleProcessGroup: [event: AgentTimelineEvent];
}>();

function isTimelineMessage(event: AgentTimelineEvent): boolean {
  return event.kind === "message";
}

function canToggle(event: AgentTimelineEvent): boolean {
  return !isTimelineMessage(event);
}

function isCompact(event: AgentTimelineEvent): boolean {
  return !isTimelineMessage(event) && !props.expanded(event);
}

function hasProcessEvents(entry: TimelineEventEntry): boolean {
  return Boolean(entry.processEvents?.length);
}

function shouldShowNodeIcon(entry: TimelineEventEntry): boolean {
  return !entry.isProcessChild || !isTimelineFinalReply(entry.event);
}

function titleAriaLabel(event: AgentTimelineEvent): string {
  const label = timelineEventLabel(event);
  const object = readTimelineDisplay(event).object?.trim() ?? "";
  return object ? `${label} ${object}` : label;
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
</script>

<template>
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
                  @click="emit('toggleEvent', event)"
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
                <TimelineDeclaredEvent
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
          v-if="shouldShowNodeIcon(entry)"
          :status="entry.event.status"
          :icon="nodeIcon(entry.event)"
        />
      </div>
      <div class="agent-timeline__body">
        <header
          v-if="!isTimelineFinalReply(entry.event) || hasProcessEvents(entry)"
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
            @click="emit('toggleEvent', entry.event)"
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
            v-if="isTimelineFinalReply(entry.event) && hasProcessEvents(entry)"
            type="button"
            class="agent-timeline__process-toggle"
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
          v-if="isTimelineFinalReply(entry.event) && hasProcessEvents(entry)"
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
              @toggle-event="emit('toggleEvent', $event)"
              @toggle-group="emit('toggleGroup', $event)"
              @toggle-process-group="emit('toggleProcessGroup', $event)"
            />
          </ol>
        </div>

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
          <TimelineDeclaredEvent
            v-else
            :event="entry.event"
            :expanded="expanded(entry.event)"
            :compact="isCompact(entry.event)"
          />
        </div>
      </div>
    </article>
  </li>
</template>
