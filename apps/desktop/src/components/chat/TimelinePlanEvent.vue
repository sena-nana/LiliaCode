<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import {
  createTimelineMarkdownView,
  readPayloadRecord,
  readTimelinePayloadRecord,
  truncateTimelineText,
  type TimelinePayloadRecord,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
}>();

const planDetailText = computed(() => {
  const payload = readTimelinePayloadRecord(props.event);
  const input = readPayloadRecord(payload.input ?? null);
  return firstText([
    payload.plan,
    payload.steps,
    input.plan,
    input.steps,
    payload.content,
    payload.text,
    payload.summary,
    input.summary,
  ]);
});

const compactText = computed(() =>
  props.event.summary?.trim() || planDetailText.value,
);

const planText = computed(() =>
  planDetailText.value || props.event.summary?.trim() || "",
);

const collapsed = computed(() => props.compact === true || !props.expanded);
const planView = computed(() =>
  createTimelineMarkdownView(planText.value, {
    multilineTone: "default",
    singleLineTone: "muted",
  }),
);
const compactView = computed(() =>
  createTimelineMarkdownView(
    truncateTimelineText(compactText.value || "暂无计划内容。", 180),
    { forceSingleLine: true, singleLineTone: "muted" },
  ),
);

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
  if (Array.isArray(value)) {
    return value
      .map((item, index) => `${index + 1}. ${stringify(item)}`)
      .filter((line) => !line.endsWith(". "))
      .join("\n");
  }
  if (isRecord(value)) {
    return firstText([value.text, value.title, value.summary, value.content]);
  }
  return "";
}

function isRecord(value: unknown): value is TimelinePayloadRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
</script>

<template>
  <section
    class="timeline-card timeline-card--plan"
    :class="{ 'is-collapsed': collapsed }"
  >
    <MarkdownBlock
      v-if="collapsed && compactView"
      :content="compactView.content"
      :tone="compactView.tone"
      :single-line="compactView.singleLine"
      class="timeline-muted-line"
    />
    <MarkdownBlock
      v-else-if="planView"
      :content="planView.content"
      :tone="planView.tone"
      :single-line="planView.singleLine"
      class="timeline-markdown"
    />
    <p v-else class="timeline-muted-line">
      暂无计划内容。
    </p>
  </section>
</template>
