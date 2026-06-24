<script setup lang="ts">
import { Puzzle, Server, Sparkles, Workflow } from "lucide-vue-next";
import type { PluginsTab } from "./usePluginsOverview";

defineProps<{
  modelValue: PluginsTab;
  skillsCount: number;
  pluginsCount: number;
  claudeHooksCount: number;
  claudeMcpCount: number;
  codexHooksCount: number;
  codexMcpCount: number;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: PluginsTab];
}>();
</script>

<template>
  <div class="plugins-tabs ui-tabs ui-tabs--pill" role="tablist">
    <button
      type="button" role="tab"
      class="ui-tabs__tab"
      :aria-selected="modelValue === 'claude-skills'"
      :class="{ 'is-active': modelValue === 'claude-skills' }"
      data-agent-id="plugins.tab.claude-skills"
      @click="emit('update:modelValue', 'claude-skills')"
    >
      <Sparkles :size="14" aria-hidden="true" /> 技能
      <span class="ui-tabs__count">{{ skillsCount }}</span>
    </button>
    <button
      type="button" role="tab"
      class="ui-tabs__tab"
      :aria-selected="modelValue === 'claude-plugins'"
      :class="{ 'is-active': modelValue === 'claude-plugins' }"
      data-agent-id="plugins.tab.claude-plugins"
      @click="emit('update:modelValue', 'claude-plugins')"
    >
      <Puzzle :size="14" aria-hidden="true" /> 插件
      <span class="ui-tabs__count">{{ pluginsCount }}</span>
    </button>
    <button
      type="button" role="tab"
      class="ui-tabs__tab"
      :aria-selected="modelValue === 'claude-hooks'"
      :class="{ 'is-active': modelValue === 'claude-hooks' }"
      data-agent-id="plugins.tab.claude-hooks"
      @click="emit('update:modelValue', 'claude-hooks')"
    >
      <Workflow :size="14" aria-hidden="true" /> Claude Hooks
      <span class="ui-tabs__count">{{ claudeHooksCount }}</span>
    </button>
    <button
      type="button" role="tab"
      class="ui-tabs__tab"
      :aria-selected="modelValue === 'claude-mcp'"
      :class="{ 'is-active': modelValue === 'claude-mcp' }"
      data-agent-id="plugins.tab.claude-mcp"
      @click="emit('update:modelValue', 'claude-mcp')"
    >
      <Server :size="14" aria-hidden="true" /> Claude MCP
      <span class="ui-tabs__count">{{ claudeMcpCount }}</span>
    </button>
    <button
      type="button" role="tab"
      class="ui-tabs__tab"
      :aria-selected="modelValue === 'codex-hooks'"
      :class="{ 'is-active': modelValue === 'codex-hooks' }"
      data-agent-id="plugins.tab.codex-hooks"
      @click="emit('update:modelValue', 'codex-hooks')"
    >
      <Workflow :size="14" aria-hidden="true" /> Codex Hooks
      <span class="ui-tabs__count">{{ codexHooksCount }}</span>
    </button>
    <button
      type="button" role="tab"
      class="ui-tabs__tab"
      :aria-selected="modelValue === 'codex-mcp'"
      :class="{ 'is-active': modelValue === 'codex-mcp' }"
      data-agent-id="plugins.tab.codex-mcp"
      @click="emit('update:modelValue', 'codex-mcp')"
    >
      <Server :size="14" aria-hidden="true" /> Codex MCP
      <span class="ui-tabs__count">{{ codexMcpCount }}</span>
    </button>
  </div>
</template>
