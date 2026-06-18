<script setup lang="ts">
import { computed, type Component } from "vue";
import {
  AlertTriangle,
  BookOpen,
  Bot,
  CircleHelp,
  FilePen,
  GitBranch,
  Globe,
  Goal,
  ListChecks,
  ListOrdered,
  MessageSquare,
  Plug,
  Search,
  Stethoscope,
  Terminal,
  TextCursorInput,
  Wrench,
} from "lucide-vue-next";

const props = defineProps<{
  icon?: string | null;
}>();

const TIMELINE_ICON_MAP: Record<string, Component> = {
  "alert-triangle": AlertTriangle,
  "book-open": BookOpen,
  bot: Bot,
  "circle-help": CircleHelp,
  "file-pen": FilePen,
  "git-branch": GitBranch,
  globe: Globe,
  goal: Goal,
  hook: Wrench,
  "list-checks": ListChecks,
  "list-ordered": ListOrdered,
  "message-square": MessageSquare,
  plug: Plug,
  search: Search,
  stethoscope: Stethoscope,
  terminal: Terminal,
  "text-cursor-input": TextCursorInput,
  wrench: Wrench,
};

const icon = computed<Component | null>(() => resolveLucideIcon(props.icon));

function resolveLucideIcon(name: string | null | undefined): Component | null {
  const normalized = name?.trim();
  if (!normalized) return null;
  return TIMELINE_ICON_MAP[normalized] ?? null;
}
</script>

<template>
  <span
    v-if="icon"
    class="agent-timeline__node"
    aria-hidden="true"
  >
    <component
      :is="icon"
      :size="13"
      :stroke-width="1.75"
    />
  </span>
</template>
