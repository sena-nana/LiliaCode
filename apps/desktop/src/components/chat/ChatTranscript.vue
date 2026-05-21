<script setup lang="ts">
/**
 * Transcript：消息滚动容器。
 *
 * 关键行为：
 * - 空状态时居中显示「要在 {project} 中构建什么？」，参考截图。
 * - auto-scroll：用户已经贴底时新消息进来自动跟到底；如果用户向上翻了，不打断阅读。
 *   判定阈值取 24px，给 macOS 弹性滚动留点余地。
 */

import { computed, nextTick, ref, watch } from "vue";
import type { ChatMessage } from "@lilia/contracts";
import ChatBubble from "./ChatBubble.vue";

type StreamableMessage = ChatMessage & { streaming?: boolean };

const props = defineProps<{
  messages: StreamableMessage[];
  /** 空状态居中显示的提示语。由调用方根据「绑了项目 / 零散对话」自行决定文案。 */
  emptyHeadline: string;
}>();

const scroller = ref<HTMLElement | null>(null);
const isPinnedToBottom = ref(true);

function checkPinned() {
  const el = scroller.value;
  if (!el) return;
  const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
  isPinnedToBottom.value = gap < 24;
}

async function scrollToBottom() {
  await nextTick();
  const el = scroller.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

watch(
  () => props.messages.length,
  async () => {
    if (isPinnedToBottom.value) await scrollToBottom();
  },
);

const isEmpty = computed(() => props.messages.length === 0);
</script>

<template>
  <div
    ref="scroller"
    class="chat-transcript"
    :class="{ 'is-empty': isEmpty }"
    @scroll="checkPinned"
  >
    <div v-if="isEmpty" class="chat-empty">
      {{ emptyHeadline }}
    </div>
    <template v-else>
      <ChatBubble v-for="m in messages" :key="m.id" :message="m" />
    </template>
  </div>
</template>
