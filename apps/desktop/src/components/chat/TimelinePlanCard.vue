<script setup lang="ts">
import { computed } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
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
  readTimelinePayloadRecord,
  truncateTimelineText,
} from "./timelineDisplay";

type PlanStatusKind =
  | "pending"
  | "revision"
  | "approved"
  | "rejected"
  | "cancelled"
  | "expired"
  | "neutral";

const STATUS_LABELS: Record<PlanStatusKind, string> = {
  pending: "待确认",
  revision: "修改要求",
  approved: "已同意",
  rejected: "已拒绝",
  cancelled: "已取消",
  expired: "已失效",
  neutral: "计划",
};

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
const planText = computed(() => readPayloadText(payload.value.plan));
const revisionRequest = computed(() => readPayloadText(payload.value.revisionRequest));
const allowedPromptRows = computed(() => {
  const prompts = payload.value.allowedPrompts;
  if (!Array.isArray(prompts)) return [];
  return prompts.flatMap((item, index) => {
    if (!isRecord(item)) return [];
    const tool = compactPayloadLine(item.tool, 80);
    const prompt = compactPayloadLine(item.prompt, 400);
    const text = [tool, prompt].filter(Boolean).join("：");
    return text ? [{ key: `${index}:${text}`, text }] : [];
  });
});
const hasStructuredBody = computed(() =>
  Boolean(planText.value || revisionRequest.value || allowedPromptRows.value.length),
);
const eventTitle = computed(() => props.event.title?.trim() ?? "");
const statusKind = computed<PlanStatusKind>(() => {
  if (props.actionExpired) return "expired";
  if (revisionRequest.value) return "revision";
  if (payload.value.approved === null) return "pending";
  if (payload.value.approved === true) return "approved";
  if (payload.value.approved === false) return "rejected";
  if (props.event.status === "requires_action") return "pending";
  if (props.event.status === "cancelled") return "cancelled";
  return "neutral";
});
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
const statusBadge = computed(() => STATUS_LABELS[statusKind.value]);
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function readPayloadText(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function compactPayloadLine(value: unknown, max: number): string {
  const text = readPayloadText(value).replace(/\s+/g, " ").trim();
  return text.length > max ? `${text.slice(0, max)}...` : text;
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
