<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  readPayloadRecord,
  readTimelinePayloadRecord,
  timelineTodoItems,
  truncateTimelineText,
  type TimelinePayloadRecord,
  type TimelineTodoItem,
} from "./timelineDisplay";

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
}>();

const items = computed(() => {
  const direct = timelineTodoItems(props.event);
  if (direct.length) return direct;

  const payload = readTimelinePayloadRecord(props.event);
  const input = readPayloadRecord(payload.input ?? null);
  const rawItems = arrayValue(payload.todos) ?? arrayValue(input.todos) ?? arrayValue(input.items);
  return (rawItems ?? [])
    .map((item) => normalizeTodoItem(item))
    .filter((item): item is TimelineTodoItem => item !== null);
});
const doneCount = computed(() => items.value.filter((item) => item.completed).length);
const collapsed = computed(() => props.compact === true || !props.expanded);
const compactLine = computed(() => {
  if (!items.value.length) return props.event.summary?.trim() || "暂无 Todo。";
  const next = items.value.find((item) => !item.completed)?.text ?? items.value[0].text;
  return `${doneCount.value}/${items.value.length} 已完成 · ${truncateTimelineText(next, 120)}`;
});

function normalizeTodoItem(item: unknown): TimelineTodoItem | null {
  if (typeof item === "string") {
    const text = item.trim();
    return text ? { text, completed: false } : null;
  }

  const row = readPayloadRecord(item);
  const text = firstText([row.text, row.content, row.title, row.description]);
  if (!text) return null;
  const status = typeof row.status === "string" ? row.status.toLowerCase() : "";
  return {
    text,
    completed: row.completed === true || row.done === true || status === "completed",
  };
}

function arrayValue(value: unknown): unknown[] | null {
  return Array.isArray(value) ? value : null;
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
  if (isRecord(value)) return firstText([value.text, value.title, value.summary]);
  return "";
}

function isRecord(value: unknown): value is TimelinePayloadRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
</script>

<template>
  <section
    class="timeline-card timeline-card--todo"
    :class="{ 'is-collapsed': collapsed }"
  >
    <p v-if="collapsed" class="timeline-muted-line timeline-muted-line--single">
      {{ compactLine }}
    </p>

    <ul v-else-if="items.length" class="timeline-card__todo-list">
      <li
        v-for="item in items"
        :key="item.text"
        class="timeline-card__todo-item"
        :class="{ 'is-done': item.completed }"
      >
        <span class="timeline-card__todo-check" aria-hidden="true">
          {{ item.completed ? "✓" : "•" }}
        </span>
        <span>{{ item.text }}</span>
      </li>
    </ul>

    <p v-else class="timeline-muted-line">
      暂无 Todo。
    </p>
  </section>
</template>
