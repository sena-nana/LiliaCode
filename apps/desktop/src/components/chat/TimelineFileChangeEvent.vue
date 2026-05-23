<script setup lang="ts">
import { computed } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  readTimelinePayloadRecord,
  timelineFileChanges,
  type TimelinePayloadRecord,
} from "./timelineDisplay";

interface FileChangeRow {
  kind: string;
  label: string;
  path: string;
  detail: string;
}

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  compact?: boolean;
}>();

const payload = computed(() => readTimelinePayloadRecord(props.event));
const changes = computed<FileChangeRow[]>(() => {
  const localChanges = timelineFileChanges(props.event).map((change) => ({
    ...change,
    label: kindLabel(change.kind),
    detail: "",
  }));
  if (localChanges.length) return localChanges;

  return [payload.value]
    .map((item) => asChange(item))
    .filter((item): item is FileChangeRow => !!item);
});

const summaryLine = computed(() => {
  if (changes.value.length === 0) {
    return props.event.summary?.trim() || props.event.title.trim() || "文件修改";
  }
  const first = changes.value[0];
  const suffix = changes.value.length > 1 ? ` 等 ${changes.value.length} 个文件` : "";
  return `${first.label} ${first.path}${suffix}`;
});

function asRecord(value: unknown): TimelinePayloadRecord {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as TimelinePayloadRecord
    : {};
}

function asChange(value: unknown): FileChangeRow | null {
  const row = asRecord(value);
  const path = readString(row, ["path", "filePath", "relativePath", "targetPath", "name"]);
  if (!path) return null;

  const rawKind = readString(row, ["kind", "operation", "type", "status"]) || "update";
  const detail = [
    formatCount(row.additions, "+"),
    formatCount(row.deletions, "-"),
    readString(row, ["reason", "summary", "description"]),
  ].filter(Boolean).join(" · ");

  return {
    kind: rawKind,
    label: kindLabel(rawKind),
    path,
    detail,
  };
}

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
  return "";
}

function formatCount(value: unknown, prefix: string): string {
  return typeof value === "number" && value > 0 ? `${prefix}${value}` : "";
}

function kindLabel(kind: string): string {
  const normalized = kind.toLowerCase();
  if (["add", "added", "create", "created", "new"].includes(normalized)) return "新增";
  if (["delete", "deleted", "remove", "removed"].includes(normalized)) return "删除";
  if (["rename", "renamed", "move", "moved"].includes(normalized)) return "移动";
  if (["read", "open"].includes(normalized)) return "读取";
  return "修改";
}
</script>

<template>
  <article
    class="timeline-card timeline-card--file-change"
    :class="{ 'timeline-card--compact': props.compact }"
  >
    <p class="timeline-muted-line timeline-muted-line--single">
      {{ summaryLine }}
    </p>

    <ul
      v-if="props.expanded && !props.compact && changes.length"
      class="timeline-card__file-list"
    >
      <li
        v-for="change in changes"
        :key="`${change.kind}:${change.path}`"
        class="timeline-card__file-row"
      >
        <span class="timeline-card__badge">
          {{ change.label }}
        </span>
        <span class="timeline-card__file-main">
          <code class="timeline-card__path">
            {{ change.path }}
          </code>
          <span
            v-if="change.detail"
            class="timeline-muted-line timeline-muted-line--single"
          >
            {{ change.detail }}
          </span>
        </span>
      </li>
    </ul>
  </article>
</template>
