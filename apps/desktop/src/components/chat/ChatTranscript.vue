<script setup lang="ts">
/**
 * Transcript：消息滚动容器。auto-scroll 阈值 24px，给 macOS 弹性滚动留余地——
 * 用户贴底时新消息进来自动跟到底，向上翻了就不打断阅读。
 */

import { computed, nextTick, ref, watch } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import AgentTimeline from "./AgentTimeline.vue";

const props = defineProps<{
  timelineEvents: AgentTimelineEvent[];
  /** 空状态居中显示的提示语。由调用方根据「绑了项目 / 收集箱对话」决定文案。 */
  emptyHeadline: string;
  /** Turn 正在跑——传给 AgentTimeline 后用来在末尾叠一个「思考中…」指示器。 */
  isThinking?: boolean;
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
  () => props.timelineEvents.length,
  async () => {
    if (isPinnedToBottom.value) await scrollToBottom();
  },
);

watch(
  () => props.isThinking,
  async () => {
    if (isPinnedToBottom.value) await scrollToBottom();
  },
);

const isEmpty = computed(() =>
  props.timelineEvents.length === 0 && !props.isThinking,
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
      <AgentTimeline :events="timelineEvents" :is-thinking="isThinking" />
    </template>
  </div>
</template>
