<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import {
  readTimelinePayloadRecord,
  type TimelinePayloadRecord,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
}>();

const payload = computed(() => readTimelinePayloadRecord(props.event));
const agentName = computed(() => readString(payload.value, [
  "agentType",
  "subagentType",
  "agentName",
  "name",
  "type",
]));
const taskDescription = computed(() => readString(payload.value, [
  "taskDescription",
  "description",
  "prompt",
  "task",
]));
const result = computed(() => readString(payload.value, [
  "result",
  "response",
  "output",
  "summary",
]));

const summaryLine = computed(() => {
  const title = agentName.value ? `子代理 ${agentName.value}` : "子代理";
  if (taskDescription.value) return `${title}: ${taskDescription.value}`;
  return props.event.summary?.trim() || props.event.title.trim() || `${title}事件`;
});

function readString(payload: TimelinePayloadRecord, keys: string[]): string {
  for (const key of keys) {
    const text = stringify(payload[key]);
    if (text) return text;
  }
  return "";
}

function stringify(value: unknown): string {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value) || (value && typeof value === "object")) {
    try {
      return JSON.stringify(value, null, 2);
    } catch {
      return String(value);
    }
  }
  return "";
}
</script>

<template>
  <article
    class="timeline-card timeline-card--subagent"
    :class="{ 'timeline-card--compact': props.compact }"
  >
    <p class="timeline-muted-line timeline-muted-line--single">
      {{ summaryLine }}
    </p>

    <div
      v-if="props.expanded && !props.compact"
      class="timeline-card__details"
    >
      <section v-if="taskDescription" class="timeline-card__section">
        <p class="timeline-card__label">TASK</p>
        <MarkdownBlock :content="taskDescription" class="timeline-card__text" />
      </section>

      <section v-if="result" class="timeline-card__section">
        <p class="timeline-card__label">RESULT</p>
        <MarkdownBlock :content="result" class="timeline-card__text" />
      </section>
    </div>
  </article>
</template>
