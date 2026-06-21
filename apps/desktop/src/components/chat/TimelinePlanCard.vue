<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import {
  agentTimelinePlanAllowedPromptRows,
  agentTimelinePlanArchitectureImpactRows,
  timelinePlanStatusKind,
  timelinePlanStatusLabel,
  type AgentTimelineEvent,
  type TimelinePlanStatusKind,
} from "@lilia/contracts";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/pendingAgentActions";
import TimelinePendingAction from "./TimelinePendingAction.vue";
import { measurePerfAsync } from "../../utils/perf";
import {
  createTimelineMarkdownView,
  readTimelineDisplay,
  readTimelinePayloadRecord,
  truncateTimelineText,
} from "./timelineDisplay";

const MarkdownBlock = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "timeline.markdown.load",
    async () => (await import("./MarkdownBlock.vue")).default,
  ),
});

const TimelineCardDetails = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "timeline.card-details.load",
    async () => (await import("./TimelineCardDetails.vue")).default,
  ),
});

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  canToggle: boolean;
  projectCwd?: string | null;
  pendingAction?: PendingAgentAction | null;
  actionExpired?: boolean;
}>();

const emit = defineEmits<{
  toggle: [event: AgentTimelineEvent];
  resolvePendingAction: [resolution: PendingAgentActionResolution];
}>();

const display = computed(() =>
  readTimelineDisplay(props.event, { projectCwd: props.projectCwd }),
);
const payload = computed(() => readTimelinePayloadRecord(props.event));
const details = computed(() => display.value.details ?? []);
const planText = computed(() => stringPayload(payload.value.plan));
const revisionRequest = computed(() => stringPayload(payload.value.revisionRequest));
const allowedPromptRows = computed(() =>
  agentTimelinePlanAllowedPromptRows(payload.value).map((row) => ({
    key: `${row.index}:${row.text}`,
    text: row.text,
  })),
);
const architectureImpactRows = computed(() =>
  agentTimelinePlanArchitectureImpactRows(payload.value).map((row) => ({
    key: `${row.impactIndex}:${row.rowIndex}:${row.text}`,
    text: row.text,
  })),
);
const hasStructuredBody = computed(() =>
  Boolean(
    planText.value ||
    revisionRequest.value ||
    allowedPromptRows.value.length ||
    architectureImpactRows.value.length,
  ),
);
const eventTitle = computed(() => props.event.title?.trim() ?? "");
const statusKind = computed<TimelinePlanStatusKind>(() =>
  timelinePlanStatusKind(props.event, props.actionExpired),
);
const neutralEventTitle = computed(() =>
  statusKind.value === "neutral" && eventTitle.value && eventTitle.value !== props.event.kind
    ? eventTitle.value
    : "",
);
const label = computed(() =>
  neutralEventTitle.value ||
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
const statusBadge = computed(() => timelinePlanStatusLabel(statusKind.value));
const detailsId = computed(() => `agent-timeline-details-${props.event.id}`);
const titleId = computed(() => `agent-timeline-title-${props.event.id}`);
const titleAriaLabel = computed(() =>
  [label.value, statusBadge.value, compactSummaryText.value].filter(Boolean).join(" "),
);
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

function stringPayload(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
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
        <span
          class="timeline-plan-card__badge"
          :class="`timeline-plan-card__badge--${statusKind}`"
        >
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
      class="timeline-plan-card__body-shell"
    >
      <div
        class="timeline-plan-card__body"
      >
        <div v-if="hasStructuredBody" class="timeline-plan-card__content">
          <MarkdownBlock
            v-if="planText"
            :content="planText"
            class="timeline-plan-card__markdown timeline-plan-card__markdown--plan"
          />

          <section
            v-if="revisionRequest"
            class="timeline-plan-card__section timeline-plan-card__section--revision"
            aria-label="修改要求"
          >
            <p class="timeline-plan-card__section-label">修改要求</p>
            <MarkdownBlock
              :content="revisionRequest"
              tone="muted"
              class="timeline-plan-card__markdown"
            />
          </section>

          <section
            v-if="architectureImpactRows.length"
            class="timeline-plan-card__section"
            aria-label="架构影响"
          >
            <p class="timeline-plan-card__section-label">架构影响</p>
            <ul class="timeline-plan-card__prompt-list">
              <li
                v-for="row in architectureImpactRows"
                :key="row.key"
                class="timeline-plan-card__prompt-item"
              >
                {{ row.text }}
              </li>
            </ul>
          </section>

          <section
            v-if="allowedPromptRows.length"
            class="timeline-plan-card__section"
            aria-label="可能调用"
          >
            <p class="timeline-plan-card__section-label">可能调用</p>
            <ul class="timeline-plan-card__prompt-list">
              <li
                v-for="row in allowedPromptRows"
                :key="row.key"
                class="timeline-plan-card__prompt-item"
              >
                {{ row.text }}
              </li>
            </ul>
          </section>
        </div>

        <TimelineCardDetails
          v-else
          :details="details"
          :fallback-view="expandedFallbackView"
        />
      </div>
    </div>

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
