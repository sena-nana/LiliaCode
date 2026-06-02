<script setup lang="ts">
import { computed, type Component } from "vue";
import * as LucideIcons from "lucide-vue-next";

const props = defineProps<{
  icon?: string | null;
}>();

const icon = computed<Component | null>(() => resolveLucideIcon(props.icon));

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
    aria-hidden="true"
  >
    <component
      :is="icon"
      :size="13"
      :stroke-width="1.75"
    />
  </span>
</template>
