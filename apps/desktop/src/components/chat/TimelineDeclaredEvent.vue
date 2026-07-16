<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/pendingAgentActions";
import TimelinePendingAction from "./TimelinePendingAction.vue";
import { measurePerfAsync } from "@lilia/ui/diagnostics";
import { createLazyLoadState } from "@lilia/ui/utils/lazyLoadState";
import {
  createTimelineMarkdownView,
  readTimelineDisplay,
  truncateTimelineText,
} from "./timelineDisplay";

const timelineCardDetailsLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "timeline.card-details.load",
    async () => (await import("./TimelineCardDetails.vue")).default,
  )
);

const TimelineCardDetails = defineAsyncComponent({
  suspensible: false,
  loader: () => timelineCardDetailsLoad.load(),
});

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
const compactSummaryText = computed(() =>
  truncateTimelineText(summaryLine.value.replace(/\s+/g, " ").trim(), 180),
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
    <p
      v-if="collapsed && compactSummaryText"
      class="timeline-muted-line"
    >
      {{ compactSummaryText }}
    </p>

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
