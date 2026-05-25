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
  AgentTimelineEventKind,
  AgentTimelineEventStatus,
} from "@lilia/contracts";

type StatusTone = "pending" | "running" | "done" | "failed" | "warn";

const props = defineProps<{
  kind: AgentTimelineEventKind;
  status: AgentTimelineEventStatus;
}>();

const tone = computed<StatusTone>(() => statusToTone(props.status));
const icon = computed<Component | null>(() => iconForKind(props.kind, props.status));

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

function iconForKind(
  kind: AgentTimelineEventKind,
  status: AgentTimelineEventStatus,
): Component | null {
  // 最终回复（assistant message）始终保留 icon
  if (kind === "message") return MessageSquare;
  // reasoning 只给低权重点位，不和「Agent 思考」文案抢视觉焦点。
  if (kind === "reasoning") return null;

  switch (kind) {
    case "plan":
      return ListOrdered;
    case "todo_list":
      return ListChecks;
    case "command":
      return Terminal;
    case "file_change":
      return FilePen;
    case "tool":
      return Wrench;
    case "mcp":
      return Plug;
    case "web_search":
      return Search;
    case "subagent":
      return Bot;
    case "error":
      return AlertTriangle;
    case "turn":
    default:
      return statusToTone(status) === "running" ? Circle : null;
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
