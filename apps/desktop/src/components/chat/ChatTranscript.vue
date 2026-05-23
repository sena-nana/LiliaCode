<script setup lang="ts">
/**
 * Transcript：消息滚动容器。auto-scroll 阈值 24px，给 macOS 弹性滚动留余地——
 * 用户贴底时新消息进来自动跟到底，向上翻了就不打断阅读。
 */

import { computed, nextTick, ref, watch } from "vue";
import type { AgentTimelineEvent, ChatMessage } from "@lilia/contracts";
import AgentTimeline from "./AgentTimeline.vue";

type StreamableMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

const props = defineProps<{
  messages: StreamableMessage[];
  timelineEvents: AgentTimelineEvent[];
  /** 空状态居中显示的提示语。由调用方根据「绑了项目 / 收集箱对话」决定文案。 */
  emptyHeadline: string;
}>();

const scroller = ref<HTMLElement | null>(null);
const isPinnedToBottom = ref(true);

const visibleMessages = computed(() =>
  props.messages.filter((message) => message.role !== "assistant"),
);

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
  () => [props.messages.length, props.timelineEvents.length] as const,
  async () => {
    if (isPinnedToBottom.value) await scrollToBottom();
  },
);

const isEmpty = computed(() =>
  visibleMessages.value.length === 0 && props.timelineEvents.length === 0
);
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
      <AgentTimeline :events="timelineEvents" :messages="visibleMessages" />
    </template>
  </div>
</template>
