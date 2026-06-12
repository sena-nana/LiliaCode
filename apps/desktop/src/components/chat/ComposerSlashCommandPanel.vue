<script setup lang="ts">
import { TerminalSquare } from "lucide-vue-next";
import type { ChatSlashCommandSearchResult } from "@lilia/contracts";

defineProps<{
  results: ChatSlashCommandSearchResult[];
  activeIndex: number;
  loading: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  activate: [index: number];
  select: [result: ChatSlashCommandSearchResult];
}>();

function sourceLabel(source: ChatSlashCommandSearchResult["command"]["source"]): string {
  return source === "project" ? "项目" : "内置";
}
</script>

<template>
  <div
    class="chat-composer__context-panel chat-composer__slash-panel"
    role="listbox"
    aria-label="斜杠命令"
  >
    <p v-if="loading && !results.length" class="chat-composer__context-note">
      正在搜索命令…
    </p>
    <p v-else-if="error && !results.length" class="chat-composer__context-note is-error">
      {{ error }}
    </p>
    <div v-else-if="results.length" class="chat-composer__context-list">
      <button
        v-for="(result, index) in results"
        :key="result.command.id"
        type="button"
        class="chat-composer__context-item chat-composer__slash-item"
        :class="{ 'is-active': index === activeIndex }"
        role="option"
        :aria-selected="index === activeIndex"
        @mousedown.prevent
        @mouseenter="emit('activate', index)"
        @click="emit('select', result)"
      >
        <span class="chat-composer__context-icon" aria-hidden="true">
          <TerminalSquare :size="14" />
        </span>
        <span class="chat-composer__context-main">
          <span class="chat-composer__context-name">/{{ result.command.name }}</span>
          <span class="chat-composer__context-path">{{ result.command.title }}</span>
        </span>
        <span class="chat-composer__context-meta">
          {{ sourceLabel(result.command.source) }}
        </span>
      </button>
    </div>
    <p v-else class="chat-composer__context-note">
      没有匹配的命令
    </p>
  </div>
</template>
