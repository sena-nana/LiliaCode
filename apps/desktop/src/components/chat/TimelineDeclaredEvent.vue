<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import MarkdownBlock from "./MarkdownBlock.vue";
import TimelineCardDetails from "./TimelineCardDetails.vue";
import TimelinePendingAction from "./TimelinePendingAction.vue";
import {
  createTimelineMarkdownView,
  readTimelineDisplay,
  truncateTimelineText,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
  projectCwd?: string | null;
  pendingAction?: PendingAgentAction | null;
  actionExpired?: boolean;
}>();

const emit = defineEmits<{
  resolvePendingAction: [resolution: PendingAgentActionResolution];
}>();

const display = computed(() =>
  readTimelineDisplay(props.event, { projectCwd: props.projectCwd }),
);
const details = computed(() => display.value.details ?? []);
const collapsed = computed(() => props.compact === true || !props.expanded);
const summaryLine = computed(() =>
  display.value.preview?.trim() ||
  display.value.label?.trim() ||
  "暂无摘要。",
);
const compactView = computed(() =>
  createTimelineMarkdownView(
    truncateTimelineText(summaryLine.value, 180),
    { forceSingleLine: true, singleLineTone: "muted" },
  ),
);
const expandedFallbackView = computed(() =>
  details.value.length
    ? null
    : createTimelineMarkdownView(summaryLine.value, {
        multilineTone: "muted",
        singleLineTone: "muted",
      }),
);
</script>

<template>
  <article
    class="timeline-card timeline-card--declared"
    :class="{ 'timeline-card--compact': props.compact }"
  >
    <MarkdownBlock
      v-if="collapsed && compactView"
      :content="compactView.content"
      :tone="compactView.tone"
      :single-line="compactView.singleLine"
      class="timeline-muted-line"
    />

    <template v-else>
      <TimelineCardDetails
        :details="details"
        :fallback-view="expandedFallbackView"
      />
    </template>

    <TimelinePendingAction
      v-if="props.pendingAction"
      :action="props.pendingAction"
      @resolve="emit('resolvePendingAction', $event)"
    />
    <section
      v-else-if="props.actionExpired"
      class="timeline-pending-action timeline-pending-action--expired"
      role="note"
    >
      已失效
    </section>
  </article>
</template>
