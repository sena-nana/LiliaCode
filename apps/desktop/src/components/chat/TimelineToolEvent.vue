<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
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
const label = computed(() => {
  if (props.event.kind === "mcp") return "MCP";
  if (props.event.kind === "web_search") return "搜索";
  return "工具";
});
const toolName = computed(() => readString(payload.value, [
  "toolName",
  "name",
  "tool",
  "function",
]));
const server = computed(() => readString(payload.value, [
  "server",
  "serverName",
  "mcpServer",
]));
const query = computed(() => readString(payload.value, [
  "query",
  "searchQuery",
  "q",
  "url",
]));
const input = computed(() => readPayload(payload.value, [
  "arguments",
  "args",
  "input",
  "parameters",
  "params",
  "request",
]));
const output = computed(() => readPayload(payload.value, [
  "result",
  "response",
  "output",
  "text",
  "content",
]));

const summaryLine = computed(() => {
  if (props.event.kind === "web_search") {
    return query.value ? `${label.value} ${query.value}` : "搜索请求";
  }

  const target = [server.value, toolName.value].filter(Boolean).join("/");
  if (target) return `${label.value} ${target}`;
  return props.event.summary?.trim() || props.event.title.trim() || `${label.value}事件`;
});

function readString(payload: TimelinePayloadRecord, keys: string[]): string {
  for (const key of keys) {
    const text = stringify(payload[key], false);
    if (text) return text;
  }
  return "";
}

function readPayload(payload: TimelinePayloadRecord, keys: string[]): string {
  for (const key of keys) {
    const text = stringify(payload[key], true);
    if (text) return text;
  }
  return "";
}

function stringify(value: unknown, pretty: boolean): string {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value) || (value && typeof value === "object")) {
    try {
      return JSON.stringify(value, null, pretty ? 2 : 0);
    } catch {
      return String(value);
    }
  }
  return "";
}
</script>

<template>
  <article
    class="timeline-card timeline-card--tool"
    :class="[
      `timeline-card--${props.event.kind}`,
      { 'timeline-card--compact': props.compact },
    ]"
  >
    <p class="timeline-muted-line timeline-muted-line--single">
      {{ summaryLine }}
    </p>

    <div
      v-if="props.expanded && !props.compact"
      class="timeline-card__details"
    >
      <p v-if="server" class="timeline-card__field">
        <span>服务</span>
        <code>{{ server }}</code>
      </p>
      <p v-if="toolName" class="timeline-card__field">
        <span>工具</span>
        <code>{{ toolName }}</code>
      </p>
      <p v-if="query" class="timeline-card__field">
        <span>查询</span>
        <code>{{ query }}</code>
      </p>

      <section v-if="input" class="timeline-card__section">
        <p class="timeline-card__label">INPUT</p>
        <pre class="timeline-code-block timeline-card__payload"><code>{{ input }}</code></pre>
      </section>

      <section v-if="output" class="timeline-card__section">
        <p class="timeline-card__label">OUTPUT</p>
        <pre class="timeline-code-block timeline-card__payload"><code>{{ output }}</code></pre>
      </section>
    </div>
  </article>
</template>
