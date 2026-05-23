<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import {
  createTimelineMarkdownView,
  readTimelinePayloadRecord,
  truncateTimelineText,
  type TimelinePayloadRecord,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
}>();

const summary = computed(() => firstText([
  props.event.summary,
  readTimelinePayloadRecord(props.event).summary,
  readTimelinePayloadRecord(props.event).text,
  readTimelinePayloadRecord(props.event).message,
  readTimelinePayloadRecord(props.event).result,
]));

const detail = computed(() => {
  const payload = readTimelinePayloadRecord(props.event);
  return firstText([
    payload.details,
    payload.detail,
    payload.content,
    payload.body,
    payload.output,
  ]);
});

const collapsed = computed(() => props.compact === true || !props.expanded);
const compactView = computed(() =>
  createTimelineMarkdownView(
    truncateTimelineText(summary.value || detail.value || fallbackLine(), 180),
    { forceSingleLine: true, singleLineTone: "muted" },
  ),
);
const summaryView = computed(() =>
  createTimelineMarkdownView(summary.value, {
    multilineTone: "muted",
    singleLineTone: "muted",
  }),
);
const detailView = computed(() =>
  createTimelineMarkdownView(detail.value, {
    multilineTone: "default",
    singleLineTone: "muted",
  }),
);

function fallbackLine(): string {
  if (props.event.kind === "turn") return props.event.title.trim() || "Agent 状态更新";
  return props.event.title.trim() || "暂无摘要。";
}

function firstText(values: unknown[]): string {
  for (const value of values) {
    const text = stringify(value);
    if (text) return text;
  }
  return "";
}

function stringify(value: unknown): string {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value) || isRecord(value)) {
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }
  return "";
}

function isRecord(value: unknown): value is TimelinePayloadRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
</script>

<template>
  <section
    class="timeline-card timeline-card--summary"
    :class="{ 'is-collapsed': collapsed }"
  >
    <MarkdownBlock
      v-if="collapsed && compactView"
      :content="compactView.content"
      :tone="compactView.tone"
      :single-line="compactView.singleLine"
      class="timeline-muted-line"
    />

    <template v-else>
      <MarkdownBlock
        v-if="summaryView"
        :content="summaryView.content"
        :tone="summaryView.tone"
        :single-line="summaryView.singleLine"
        class="timeline-muted-line"
      />
      <div v-if="summaryView && detailView" class="timeline-divider" />
      <MarkdownBlock
        v-if="detailView"
        :content="detailView.content"
        :tone="detailView.tone"
        :single-line="detailView.singleLine"
        class="timeline-markdown"
      />
      <p v-if="!summaryView && !detailView" class="timeline-muted-line">
        暂无摘要。
      </p>
    </template>
  </section>
</template>
