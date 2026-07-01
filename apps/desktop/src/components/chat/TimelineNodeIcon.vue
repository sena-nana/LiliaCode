<script setup lang="ts">
import { computed, type Component } from "vue";
import AlertTriangle from "@lucide/vue/dist/esm/icons/triangle-alert.mjs";
import BookOpen from "@lucide/vue/dist/esm/icons/book-open.mjs";
import Bot from "@lucide/vue/dist/esm/icons/bot.mjs";
import CircleHelp from "@lucide/vue/dist/esm/icons/circle-question-mark.mjs";
import FilePen from "@lucide/vue/dist/esm/icons/file-pen.mjs";
import GitBranch from "@lucide/vue/dist/esm/icons/git-branch.mjs";
import Globe from "@lucide/vue/dist/esm/icons/globe.mjs";
import Goal from "@lucide/vue/dist/esm/icons/goal.mjs";
import ListChecks from "@lucide/vue/dist/esm/icons/list-checks.mjs";
import ListOrdered from "@lucide/vue/dist/esm/icons/list-ordered.mjs";
import MessageSquare from "@lucide/vue/dist/esm/icons/message-square.mjs";
import Plug from "@lucide/vue/dist/esm/icons/plug.mjs";
import Search from "@lucide/vue/dist/esm/icons/search.mjs";
import Stethoscope from "@lucide/vue/dist/esm/icons/stethoscope.mjs";
import Terminal from "@lucide/vue/dist/esm/icons/terminal.mjs";
import TextCursorInput from "@lucide/vue/dist/esm/icons/text-cursor-input.mjs";
import Wrench from "@lucide/vue/dist/esm/icons/wrench.mjs";

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

