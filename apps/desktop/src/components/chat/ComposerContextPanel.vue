<script setup lang="ts">
import type { Component } from "vue";
import type { ChatAttachment, ChatContextSearchResult } from "@lilia/contracts";
import { attachmentImageSrc } from "./imageViewer";

defineProps<{
  results: ChatContextSearchResult[];
  activeIndex: number;
  loading: boolean;
  error: string | null;
  missingPath: string | null;
  showMissingPath: boolean;
  isLargeDirectory: (attachment: ChatAttachment) => boolean;
  attachmentMetaLabel: (attachment: ChatAttachment) => string;
  contextInlinePath: (result: ChatContextSearchResult) => string | null;
  contextAttachmentIcon: (attachment: ChatAttachment) => Component;
}>();

const emit = defineEmits<{
  activate: [index: number];
  select: [result: ChatContextSearchResult];
}>();
</script>

<template>
  <div
    class="chat-composer__context-panel"
    role="listbox"
    aria-label="文件上下文搜索"
  >
    <p v-if="loading && !results.length" class="chat-composer__context-note">
      正在搜索文件…
    </p>
    <p v-else-if="error && !results.length" class="chat-composer__context-note is-error">
      {{ error }}
    </p>
    <div v-else-if="results.length" class="chat-composer__context-list">
      <button
        v-for="(result, index) in results"
        :key="result.attachment.path"
        type="button"
        class="chat-composer__context-item"
        :class="{
          'is-active': index === activeIndex,
          'is-large-directory': isLargeDirectory(result.attachment),
        }"
        role="option"
        :aria-selected="index === activeIndex"
        @mousedown.prevent
        @mouseenter="emit('activate', index)"
        @click="emit('select', result)"
      >
        <span class="chat-composer__context-icon" aria-hidden="true">
          <img
            v-if="attachmentImageSrc(result.attachment)"
            class="chat-composer__context-thumb"
            :src="attachmentImageSrc(result.attachment) ?? undefined"
            alt=""
          />
          <component
            :is="contextAttachmentIcon(result.attachment)"
            v-else
            :size="14"
          />
        </span>
        <span class="chat-composer__context-main">
          <span class="chat-composer__context-name">{{ result.attachment.name }}</span>
          <span
            v-if="contextInlinePath(result)"
            class="chat-composer__context-path"
          >{{ contextInlinePath(result) }}</span>
        </span>
        <span class="chat-composer__context-meta">
          {{ attachmentMetaLabel(result.attachment) }}
        </span>
      </button>
    </div>
    <p v-else-if="showMissingPath" class="chat-composer__context-note is-error">
      路径不存在：{{ missingPath }}
    </p>
    <p v-else class="chat-composer__context-note">
      没有匹配的文件或目录
    </p>
  </div>
</template>
