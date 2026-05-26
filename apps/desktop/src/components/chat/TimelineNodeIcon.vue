<script setup lang="ts">
import { computed, type Component } from "vue";
import * as LucideIcons from "lucide-vue-next";
import type { AgentTimelineEventStatus } from "@lilia/contracts";

type StatusTone = "pending" | "running" | "done" | "failed" | "warn";

const props = defineProps<{
  status: AgentTimelineEventStatus;
  icon?: string | null;
}>();

const tone = computed<StatusTone>(() => statusToTone(props.status));
const icon = computed<Component | null>(() => resolveLucideIcon(props.icon));

function statusToTone(status: AgentTimelineEventStatus): StatusTone {
  switch (status) {
    case "pending":
      return "pending";
    case "started":
    case "running":
    case "in_progress":
      return "running";
    case "failed":
    case "error":
    case "cancelled":
      return "failed";
    case "info":
    case "requires_action":
      return "warn";
    case "completed":
    case "done":
    case "success":
    case "skipped":
    default:
      return "done";
  }
}

function resolveLucideIcon(name: string | null | undefined): Component | null {
  const normalized = name?.trim();
  if (!normalized) return null;
  const pascal = kebabToPascal(normalized);
  const found = (LucideIcons as Record<string, unknown>)[pascal];
  return typeof found === "function" || (found && typeof found === "object")
    ? (found as Component)
    : null;
}

function kebabToPascal(name: string): string {
  return name
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join("");
}
</script>

<template>
  <span
    v-if="icon"
    class="agent-timeline__node"
    :class="`agent-timeline__node--${tone}`"
    aria-hidden="true"
  >
    <component
      :is="icon"
      :size="13"
      :stroke-width="1.75"
    />
  </span>
</template>
