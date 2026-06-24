<script setup lang="ts">
import type { SearchResult } from "../../services/sessionSearch";

defineProps<{
  results: SearchResult[];
  activeIndex: number;
  loading: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  activate: [index: number];
  select: [result: SearchResult];
}>();

function resultScope(result: SearchResult): string {
  return result.projectName ?? "收集箱";
}
</script>

<template>
  <div
    class="chat-composer__context-panel chat-composer__context-panel--conversation"
    role="listbox"
    aria-label="对话引用搜索"
  >
    <p v-if="loading && !results.length" class="chat-composer__context-note">
      正在搜索对话…
    </p>
    <p v-else-if="error && !results.length" class="chat-composer__context-note is-error">
      {{ error }}
    </p>
    <div v-else-if="results.length" class="chat-composer__context-list">
      <button
        v-for="(result, index) in results"
        :key="result.route"
      type="button"
      class="chat-composer__context-item chat-composer__context-item--conversation"
      :class="{ 'is-active': index === activeIndex }"
      :data-agent-id="`chat.composer.conversation-reference.${result.route}`"
      role="option"
        :aria-selected="index === activeIndex"
        @mousedown.prevent
        @mouseenter="emit('activate', index)"
        @click="emit('select', result)"
      >
        <span class="chat-composer__context-main">
          <span class="chat-composer__context-name">{{ result.title }}</span>
          <span class="chat-composer__context-path">{{ result.route }}</span>
        </span>
        <span class="chat-composer__context-meta">
          {{ resultScope(result) }}
        </span>
      </button>
    </div>
    <p v-else class="chat-composer__context-note">
      输入关键词搜索其他对话
    </p>
  </div>
</template>
