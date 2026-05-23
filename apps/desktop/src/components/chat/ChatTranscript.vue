<script setup lang="ts">
/**
 * Transcript：消息滚动容器。auto-scroll 阈值 24px，给 macOS 弹性滚动留余地——
 * 用户贴底时新消息进来自动跟到底，向上翻了就不打断阅读。
 */

import { computed, nextTick, ref, watch } from "vue";
import type { ChatMessage } from "@lilia/contracts";
import ChatBubble from "./ChatBubble.vue";

type StreamableMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

const props = defineProps<{
  messages: StreamableMessage[];
  /** 空状态居中显示的提示语。由调用方根据「绑了项目 / 收集箱对话」决定文案。 */
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
