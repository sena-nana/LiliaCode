<script setup lang="ts">
import { computed, type Component } from "vue";
import {
  AlertTriangle,
  Bot,
  Circle,
  FilePen,
  ListChecks,
  ListOrdered,
  MessageSquare,
  Plug,
  Search,
  Terminal,
  Wrench,
} from "lucide-vue-next";
import type {
  AgentTimelineDisplayIcon,
  AgentTimelineEventStatus,
} from "@lilia/contracts";

type StatusTone = "pending" | "running" | "done" | "failed" | "warn";

const props = defineProps<{
  status: AgentTimelineEventStatus;
  icon?: AgentTimelineDisplayIcon | null;
}>();

const tone = computed<StatusTone>(() => statusToTone(props.status));
const icon = computed<Component | null>(() => {
  const declared = iconForDisplay(props.icon);
  return declared === undefined
    ? statusToTone(props.status) === "running" ? Circle : null
    : declared;
});

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

function iconForDisplay(iconName: AgentTimelineDisplayIcon | null | undefined): Component | null | undefined {
  switch (iconName) {
    case "message":
      return MessageSquare;
    case "reasoning":
      return null;
    case "plan":
      return ListOrdered;
    case "todo":
      return ListChecks;
    case "terminal":
      return Terminal;
    case "file":
      return FilePen;
    case "tool":
      return Wrench;
    case "plug":
      return Plug;
    case "search":
      return Search;
    case "subagent":
      return Bot;
    case "error":
      return AlertTriangle;
    case "turn":
      return Circle;
    case "none":
      return null;
    case null:
    case undefined:
      return undefined;
    default:
      return Wrench;
  }
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
