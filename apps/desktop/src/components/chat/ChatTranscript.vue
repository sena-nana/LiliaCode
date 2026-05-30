<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import AgentTimeline from "./AgentTimeline.vue";
import ChatScrollMap from "./ChatScrollMap.vue";

const props = defineProps<{
  timelineEvents: AgentTimelineEvent[];
  emptyHeadline: string;
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
const PLAN_REVEAL_PADDING = 8;

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

async function onTimelineEventToggled(payload: { event: AgentTimelineEvent; expanded: boolean }) {
  if (!payload.expanded || payload.event.kind !== "plan") return;
  await nextTick();
  revealPlanEvent(payload.event.id);
}

function revealPlanEvent(eventId: string) {
  const el = scroller.value;
  if (!el) return;
  const card = findTimelinePlanCard(el, eventId);
  if (!card) return;

  const visibleArea = readPlanVisibleArea(el);
  if (!visibleArea) return;
  const cardRect = card.getBoundingClientRect();
  const visibleHeight = visibleArea.bottom - visibleArea.top;
  let delta = 0;
  if (cardRect.height > visibleHeight || cardRect.top < visibleArea.top) {
    delta = cardRect.top - visibleArea.top;
  } else if (cardRect.bottom > visibleArea.bottom) {
    delta = cardRect.bottom - visibleArea.bottom;
  }
  if (Math.abs(delta) < 1) return;

  const top = Math.min(
    Math.max(0, el.scrollTop + delta),
    Math.max(0, el.scrollHeight - el.clientHeight),
  );
  el.scrollTo({ top, behavior: "smooth" });
  showScrollbar();
}

function findTimelinePlanCard(scrollerEl: HTMLElement, eventId: string): HTMLElement | null {
  for (const item of scrollerEl.querySelectorAll<HTMLElement>("[data-scroll-anchor-id]")) {
    if (item.dataset.scrollAnchorId !== eventId) continue;
    return item.querySelector<HTMLElement>(".timeline-card--plan") ?? item;
  }
  return null;
}

function readPlanVisibleArea(el: HTMLElement): { top: number; bottom: number } | null {
  const scrollerRect = el.getBoundingClientRect();
  const controlsRect = el.querySelector<HTMLElement>(".chat-controls-wrap")?.getBoundingClientRect();
  let controlsOverlap = 0;
  if (
    controlsRect &&
    controlsRect.top < scrollerRect.bottom &&
    controlsRect.bottom > scrollerRect.top
  ) {
    controlsOverlap = Math.min(
      scrollerRect.height,
      Math.max(0, scrollerRect.bottom - Math.max(scrollerRect.top, controlsRect.top)),
    );
  }
  const top = scrollerRect.top + PLAN_REVEAL_PADDING;
  const bottom = scrollerRect.bottom - controlsOverlap - PLAN_REVEAL_PADDING;
  return bottom > top ? { top, bottom } : null;
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
          @event-toggled="onTimelineEventToggled"
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
