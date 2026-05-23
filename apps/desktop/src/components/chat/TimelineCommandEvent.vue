<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  joinTimelineLines,
  readTimelinePayloadRecord,
  type TimelinePayloadRecord,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
}>();

const payload = computed(() => readTimelinePayloadRecord(props.event));
const command = computed(() => readString(payload.value, [
  "command",
  "cmd",
  "shellCommand",
  "script",
  "argv",
]));
const cwd = computed(() => readString(payload.value, [
  "cwd",
  "workdir",
  "workingDirectory",
  "currentDirectory",
]));
const stdout = computed(() => readString(payload.value, ["stdout", "output"]));
const stderr = computed(() => readString(payload.value, ["stderr", "errorOutput"]));
const aggregatedOutput = computed(() =>
  readString(payload.value, ["aggregatedOutput", "combinedOutput", "outputText"]),
);
const message = computed(() => readString(payload.value, ["message", "error"]));
const exitCode = computed(() => readString(payload.value, [
  "exitCode",
  "code",
  "statusCode",
]));
const duration = computed(() => formatDuration(payload.value));
const isError = computed(() =>
  ["failed", "error"].includes(props.event.status) || !!stderr.value || !!message.value,
);

const summaryLine = computed(() => props.event.summary?.trim() || "");
const primaryLine = computed(() =>
  command.value || summaryLine.value || props.event.title.trim() || "命令事件",
);

const metaLine = computed(() => {
  const parts = [
    cwd.value ? `cwd ${cwd.value}` : "",
    exitCode.value ? `exit ${exitCode.value}` : "",
    duration.value,
  ].filter(Boolean);
  return parts.join(" · ");
});

const outputText = computed(() =>
  aggregatedOutput.value || joinTimelineLines([stdout.value, stderr.value, message.value]) || "",
);

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
  if (Array.isArray(value)) {
    const parts = value.map((item) => stringify(item)).filter(Boolean);
    return parts.join(" ").trim();
  }
  if (value && typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return "";
}

function formatDuration(payload: TimelinePayloadRecord): string {
  const raw = payload.durationMs ?? payload.elapsedMs ?? payload.duration;
  if (typeof raw === "number") return raw >= 1000 ? `${(raw / 1000).toFixed(1)}s` : `${raw}ms`;
  if (typeof raw === "string" && raw.trim()) return raw.trim();
  return "";
}

</script>

<template>
  <article
    class="timeline-card timeline-card--command"
    :class="{
      'timeline-card--compact': props.compact,
      'timeline-card--error': isError,
    }"
  >
    <p class="timeline-muted-line timeline-muted-line--single">
      <span>命令</span>
      <code>{{ primaryLine }}</code>
    </p>

    <p v-if="summaryLine" class="timeline-muted-line timeline-muted-line--single">
      {{ summaryLine }}
    </p>

    <p
      v-if="metaLine && (!props.expanded || props.compact)"
      class="timeline-muted-line timeline-muted-line--single"
    >
      {{ metaLine }}
    </p>

    <div
      v-if="props.expanded && !props.compact"
      class="timeline-card__details"
    >
      <section v-if="command" class="timeline-card__section">
        <p class="timeline-card__label">COMMAND</p>
        <pre class="timeline-code-block"><code>{{ command }}</code></pre>
      </section>

      <p
        v-if="metaLine"
        class="timeline-muted-line timeline-muted-line--single"
      >
        {{ metaLine }}
      </p>

      <section v-if="outputText" class="timeline-card__section">
        <p class="timeline-card__label">
          {{ isError ? "ERROR / OUTPUT" : "OUTPUT" }}
        </p>
        <pre class="timeline-code-block timeline-card__output"><code>{{ outputText }}</code></pre>
      </section>
    </div>
  </article>
</template>
