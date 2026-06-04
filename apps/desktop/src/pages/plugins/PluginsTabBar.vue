<script setup lang="ts">
import { Puzzle, Server, Sparkles } from "lucide-vue-next";
import type { PluginsTab } from "./usePluginsOverview";

defineProps<{
  modelValue: PluginsTab;
  skillsCount: number;
  pluginsCount: number;
  claudeMcpCount: number;
  codexMcpCount: number;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: PluginsTab];
}>();
</script>

<template>
  <div class="plugins-tabs" role="tablist">
    <button
      type="button" role="tab"
      :aria-selected="modelValue === 'claude-skills'"
      :class="{ 'is-active': modelValue === 'claude-skills' }"
      @click="emit('update:modelValue', 'claude-skills')"
    >
      <Sparkles :size="14" aria-hidden="true" /> Claude Skills
      <span class="plugins-tabs__count">{{ skillsCount }}</span>
    </button>
    <button
      type="button" role="tab"
      :aria-selected="modelValue === 'claude-plugins'"
      :class="{ 'is-active': modelValue === 'claude-plugins' }"
      @click="emit('update:modelValue', 'claude-plugins')"
    >
      <Puzzle :size="14" aria-hidden="true" /> Claude Plugins
      <span class="plugins-tabs__count">{{ pluginsCount }}</span>
    </button>
    <button
      type="button" role="tab"
      :aria-selected="modelValue === 'claude-mcp'"
      :class="{ 'is-active': modelValue === 'claude-mcp' }"
      @click="emit('update:modelValue', 'claude-mcp')"
    >
      <Server :size="14" aria-hidden="true" /> Claude MCP
      <span class="plugins-tabs__count">{{ claudeMcpCount }}</span>
    </button>
    <button
      type="button" role="tab"
      :aria-selected="modelValue === 'codex-mcp'"
      :class="{ 'is-active': modelValue === 'codex-mcp' }"
      @click="emit('update:modelValue', 'codex-mcp')"
    >
      <Server :size="14" aria-hidden="true" /> Codex MCP
      <span class="plugins-tabs__count">{{ codexMcpCount }}</span>
    </button>
  </div>
</template>
