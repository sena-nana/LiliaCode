<script setup lang="ts">
/**
 * Transcript：消息滚动容器。贴底时跟随新事件，用户发送后由父级信号强制贴底。
 */

import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import AgentTimeline from "./AgentTimeline.vue";
import ChatScrollMap from "./ChatScrollMap.vue";

const props = defineProps<{
  timelineEvents: AgentTimelineEvent[];
  /** 空状态居中显示的提示语。由调用方根据「绑了项目 / 收集箱对话」决定文案。 */
  emptyHeadline: string;
  /** Turn 正在跑——传给 AgentTimeline 后用来在末尾叠一个「思考中…」指示器。 */
  isThinking?: boolean;
  projectCwd?: string | null;
  forceScrollBottomKey?: number;
}>();

const scroller = ref<HTMLElement | null>(null);
const isPinnedToBottom = ref(true);
const isScrollbarVisible = ref(false);
const isPointerInScrollbarZone = ref(false);
let scrollbarHideTimer: ReturnType<typeof window.setTimeout> | null = null;

const SCROLLBAR_HOT_ZONE = 18;
const SCROLLBAR_HIDE_DELAY = 180;

function clearScrollbarHideTimer() {
  if (scrollbarHideTimer === null) return;
  window.clearTimeout(scrollbarHideTimer);
  scrollbarHideTimer = null;
}

function showScrollbar() {
  clearScrollbarHideTimer();
  isScrollbarVisible.value = true;
}

function hideScrollbarSoon() {
  if (scrollbarHideTimer !== null) return;
  scrollbarHideTimer = window.setTimeout(() => {
    if (!isPointerInScrollbarZone.value) {
      isScrollbarVisible.value = false;
    }
    scrollbarHideTimer = null;
  }, SCROLLBAR_HIDE_DELAY);
}

function checkPinned() {
  const el = scroller.value;
  if (!el) return;
  const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
  isPinnedToBottom.value = gap < 24;
}

function onScroll() {
  checkPinned();
  showScrollbar();
}

function onScrollEnd() {
  if (!isPointerInScrollbarZone.value) hideScrollbarSoon();
}

function isInScrollbarZone(event: MouseEvent): boolean {
  const el = scroller.value;
  if (!el) return false;
  const rect = el.getBoundingClientRect();
  return event.clientX >= rect.right - SCROLLBAR_HOT_ZONE && event.clientX <= rect.right;
}

function onMouseMove(event: MouseEvent) {
  const inZone = isInScrollbarZone(event);
  isPointerInScrollbarZone.value = inZone;
  if (inZone) {
    showScrollbar();
    return;
  }
  if (isScrollbarVisible.value) hideScrollbarSoon();
}

function onMouseLeave() {
  isPointerInScrollbarZone.value = false;
  if (isScrollbarVisible.value) hideScrollbarSoon();
}

async function scrollToBottom() {
  await nextTick();
  const el = scroller.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
  isPinnedToBottom.value = true;
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

watch(
  () => props.forceScrollBottomKey,
  () => scrollToBottom(),
);

const isEmpty = computed(() =>
  props.timelineEvents.length === 0 && !props.isThinking,
);

onBeforeUnmount(() => {
  clearScrollbarHideTimer();
});
</script>

<template>
  <div
    class="chat-transcript-frame"
    @mousemove="onMouseMove"
    @mouseleave="onMouseLeave"
  >
    <div
      ref="scroller"
      class="chat-transcript"
      :class="{
        'is-empty': isEmpty,
        'is-scrollbar-visible': isScrollbarVisible,
      }"
      @scroll="onScroll"
      @scrollend="onScrollEnd"
    >
      <div v-if="isEmpty" class="chat-empty">
        {{ emptyHeadline }}
      </div>
      <template v-else>
        <AgentTimeline
          :events="timelineEvents"
          :is-thinking="isThinking"
          :project-cwd="projectCwd"
        />
      </template>
      <div class="chat-controls-wrap">
        <slot name="controls" />
      </div>
    </div>
    <ChatScrollMap
      :events="timelineEvents"
      :project-cwd="projectCwd"
      :scroller="scroller"
      :visible="isScrollbarVisible"
    />
  </div>
</template>
