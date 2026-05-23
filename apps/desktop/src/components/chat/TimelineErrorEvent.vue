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
const message = computed(() =>
  readString(payload.value, ["message", "error", "reason", "details", "stderr"]) ||
  props.event.summary?.trim() ||
  props.event.title.trim() ||
  "发生错误",
);
const code = computed(() => readString(payload.value, ["code", "exitCode", "statusCode"]));
const location = computed(() => {
  const file = readString(payload.value, ["file", "filePath", "path"]);
  const line = readString(payload.value, ["line", "lineNumber"]);
  if (file && line) return `${file}:${line}`;
  return file;
});
const command = computed(() => readString(payload.value, ["command", "cmd", "shellCommand"]));
const stack = computed(() => readString(payload.value, ["stack", "trace", "backtrace"]));
const extraPayload = computed(() => formatExtraPayload(payload.value));

const metaLine = computed(() => [
  code.value ? `code ${code.value}` : "",
  location.value,
  command.value,
].filter(Boolean).join(" · "));

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

function formatExtraPayload(payload: TimelinePayloadRecord): string {
  const hidden = new Set([
    "message",
    "error",
    "reason",
    "details",
    "stderr",
    "stack",
    "trace",
    "backtrace",
    "code",
    "exitCode",
    "statusCode",
    "file",
    "filePath",
    "path",
    "line",
    "lineNumber",
    "command",
    "cmd",
    "shellCommand",
  ]);
  const rest = Object.fromEntries(
    Object.entries(payload).filter(([key]) => !hidden.has(key)),
  );
  if (Object.keys(rest).length === 0) return "";
  try {
    return JSON.stringify(rest, null, 2);
  } catch {
    return String(rest);
  }
}
</script>

<template>
  <article
    class="timeline-card timeline-card--error"
    :class="{ 'timeline-card--compact': props.compact }"
    role="alert"
  >
    <div class="timeline-muted-line timeline-muted-line--single timeline-card__error-line">
      <span class="timeline-card__error-title">
        {{ props.event.title.trim() || "错误事件" }}
      </span>
      <MarkdownBlock
        :content="message"
        :single-line="true"
        tone="muted"
        class="timeline-card__error-message"
      />
    </div>
    <p v-if="metaLine" class="timeline-muted-line timeline-muted-line--single">
      {{ metaLine }}
    </p>

    <div
      v-if="props.expanded && !props.compact"
      class="timeline-card__details timeline-card__details--error"
    >
      <section v-if="stack" class="timeline-card__section">
        <p class="timeline-card__label">STACK</p>
        <pre class="timeline-code-block timeline-card__payload"><code>{{ stack }}</code></pre>
      </section>

      <section v-if="extraPayload" class="timeline-card__section">
        <p class="timeline-card__label">DETAILS</p>
        <pre class="timeline-code-block timeline-card__payload"><code>{{ extraPayload }}</code></pre>
      </section>
    </div>
  </article>
</template>
