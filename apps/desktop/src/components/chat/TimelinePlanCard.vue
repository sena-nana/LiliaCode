<script setup lang="ts">
import { computed } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import type { AgentTimelineEvent } from "@lilia/contracts";
import TimelineCardDetails from "./TimelineCardDetails.vue";
import {
  createTimelineMarkdownView,
  readTimelineDisplay,
  readTimelinePayloadRecord,
  truncateTimelineText,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  canToggle: boolean;
  projectCwd?: string | null;
}>();

const emit = defineEmits<{
  toggle: [event: AgentTimelineEvent];
}>();

const display = computed(() =>
  readTimelineDisplay(props.event, { projectCwd: props.projectCwd }),
);
const payload = computed(() => readTimelinePayloadRecord(props.event));
const details = computed(() => display.value.details ?? []);
const label = computed(() =>
  display.value.label?.trim() ||
  display.value.action?.trim() ||
  "计划更新",
);
const summaryLine = computed(() =>
  display.value.preview?.trim() ||
  display.value.label?.trim() ||
  "暂无摘要。",
);
const compactSummaryText = computed(() =>
  truncateTimelineText(summaryLine.value.replace(/\s+/g, " ").trim(), 180),
);
const statusBadge = computed(() => {
  if (typeof payload.value.revisionRequest === "string" && payload.value.revisionRequest.trim()) {
    return "修改要求";
  }
  if (payload.value.approved === null) return "待确认";
  if (payload.value.approved === true) return "已同意";
  if (payload.value.approved === false) return "已拒绝";
  if (props.event.status === "requires_action") return "待确认";
  if (props.event.status === "cancelled") return "已拒绝";
  return "计划";
});
const detailsId = computed(() => `agent-timeline-details-${props.event.id}`);
const titleId = computed(() => `agent-timeline-title-${props.event.id}`);
const titleAriaLabel = computed(() => `${label.value} ${summaryLine.value}`.trim());
const expandedFallbackView = computed(() =>
  details.value.length
    ? null
    : createTimelineMarkdownView(summaryLine.value, {
        multilineTone: "muted",
        singleLineTone: "muted",
      }),
);

function onToggle() {
  if (!props.canToggle) return;
  emit("toggle", props.event);
}
</script>

<template>
  <article
    class="timeline-card timeline-card--plan"
    :class="{
      'is-expanded': expanded,
      'is-collapsed': !expanded,
    }"
    :aria-labelledby="titleId"
  >
    <button
      type="button"
      class="timeline-plan-card__head"
      :aria-expanded="expanded"
      :aria-controls="detailsId"
      :aria-label="titleAriaLabel"
      :disabled="!canToggle"
      @click="onToggle"
    >
      <span class="timeline-plan-card__title-row">
        <span :id="titleId" class="timeline-plan-card__title">
          {{ label }}
        </span>
        <span class="timeline-plan-card__badge">
          {{ statusBadge }}
        </span>
        <component
          v-if="canToggle"
          :is="expanded ? ChevronDown : ChevronRight"
          class="timeline-plan-card__chevron"
          :size="13"
          aria-hidden="true"
        />
      </span>
      <span
        v-if="!expanded && compactSummaryText"
        class="timeline-plan-card__summary"
      >
        {{ compactSummaryText }}
      </span>
    </button>

    <div
      v-if="expanded"
      :id="detailsId"
      class="timeline-plan-card__body"
    >
      <TimelineCardDetails
        :details="details"
        :fallback-view="expandedFallbackView"
      />
    </div>
  </article>
</template>
