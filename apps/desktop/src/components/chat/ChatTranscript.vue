<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type CSSProperties } from "vue";
import { Copy, MessageSquarePlus, Quote } from "lucide-vue-next";
import type { AgentTimelineEvent } from "@lilia/contracts";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import { openPopupNewChat } from "../../services/popupWindows";
import AgentTimeline from "./AgentTimeline.vue";
import ChatScrollMap from "./ChatScrollMap.vue";
import type { ChatImageViewerSource } from "./imageViewer";

const props = defineProps<{
  timelineEvents: AgentTimelineEvent[];
  emptyHeadline: string;
  projectId?: string | null;
  isThinking?: boolean;
  projectCwd?: string | null;
  activePlanApprovalTurnId?: string | null;
  forceScrollBottomKey?: number;
  pendingAgentActions?: PendingAgentAction[];
  showExpiredPendingActions?: boolean;
  canRetryEvent?: (event: AgentTimelineEvent) => boolean;
}>();

const emit = defineEmits<{
  resolvePendingAgentAction: [resolution: PendingAgentActionResolution];
  "retry-event": [event: AgentTimelineEvent];
  "open-image": [image: ChatImageViewerSource];
  "insert-draft-text": [text: string];
}>();

const scroller = ref<HTMLElement | null>(null);
const frameEl = ref<HTMLElement | null>(null);
const selectionToolbarEl = ref<HTMLElement | null>(null);
const scrollMap = ref<{ show: () => void } | null>(null);
const isPinnedToBottom = ref(true);
const selectionToolbarVisible = ref(false);
const selectedAgentText = ref("");
const selectionToolbarStyle = ref<CSSProperties>({});
let pointerSelectionStarted = false;

const PLAN_REVEAL_PADDING = 8;
const SELECTION_TOOLBAR_GAP = 8;

function showScrollbar() {
  scrollMap.value?.show();
}

function checkPinned() {
  const el = scroller.value;
  if (!el) return;
  const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
  isPinnedToBottom.value = gap < 24;
}

function onScroll() {
  checkPinned();
  hideSelectionToolbar();
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

function nodeElement(node: Node | null): HTMLElement | null {
  if (!node) return null;
  if (node instanceof HTMLElement) return node;
  return node.parentNode instanceof HTMLElement ? node.parentNode : null;
}

function selectableForNode(node: Node | null): HTMLElement | null {
  return nodeElement(node)?.closest<HTMLElement>("[data-agent-selectable='true']") ?? null;
}

function selectionSelectable(selection: Selection): HTMLElement | null {
  if (selection.rangeCount === 0 || selection.isCollapsed) return null;
  const anchorSelectable = selectableForNode(selection.anchorNode);
  const focusSelectable = selectableForNode(selection.focusNode);
  if (!anchorSelectable || anchorSelectable !== focusSelectable) return null;
  const frame = frameEl.value;
  if (!frame?.contains(anchorSelectable)) return null;
  return anchorSelectable;
}

function hideSelectionToolbar() {
  selectionToolbarVisible.value = false;
  selectedAgentText.value = "";
}

async function updateSelectionToolbar() {
  const selection = window.getSelection();
  if (!selection) {
    hideSelectionToolbar();
    return;
  }
  const selectable = selectionSelectable(selection);
  const text = selection.toString();
  if (!selectable || !text.trim()) {
    hideSelectionToolbar();
    return;
  }

  const range = selection.getRangeAt(0);
  const rect = range.getBoundingClientRect();
  if (rect.width <= 0 && rect.height <= 0) {
    hideSelectionToolbar();
    return;
  }

  selectedAgentText.value = text;
  selectionToolbarVisible.value = true;
  await nextTick();
  placeSelectionToolbar(rect);
}

function placeSelectionToolbar(selectionRect: DOMRect) {
  const frameRect = frameEl.value?.getBoundingClientRect();
  const toolbarRect = selectionToolbarEl.value?.getBoundingClientRect();
  if (!frameRect || !toolbarRect) return;

  const toolbarWidth = toolbarRect.width || 112;
  const toolbarHeight = toolbarRect.height || 34;
  const minLeft = frameRect.left + 8;
  const maxLeft = frameRect.right - toolbarWidth - 8;
  const preferredLeft = selectionRect.left + selectionRect.width / 2 - toolbarWidth / 2;
  const left = Math.min(Math.max(preferredLeft, minLeft), Math.max(minLeft, maxLeft));
  const topAbove = selectionRect.top - toolbarHeight - SELECTION_TOOLBAR_GAP;
  const topBelow = selectionRect.bottom + SELECTION_TOOLBAR_GAP;
  const top = topAbove >= frameRect.top + 4 ? topAbove : topBelow;

  selectionToolbarStyle.value = {
    left: `${left}px`,
    top: `${top}px`,
  };
}

function quoteSelectedText(text: string): string {
  const normalized = text.replace(/\r\n?/g, "\n").trim();
  if (!normalized) return "";
  return `${normalized.split("\n").map((line) => `> ${line}`).join("\n")}\n\n`;
}

function popupAskText(text: string): string {
  return `请基于这段 Agent 回复继续提问：\n\n${quoteSelectedText(text)}`;
}

async function copySelection() {
  const text = selectedAgentText.value;
  if (!text.trim()) return;
  try {
    await navigator.clipboard.writeText(text);
    hideSelectionToolbar();
  } catch (err) {
    console.error("[agent-selection] copy failed", err);
  }
}

function insertSelectionQuote() {
  const text = quoteSelectedText(selectedAgentText.value);
  if (!text) return;
  emit("insert-draft-text", text);
  hideSelectionToolbar();
  window.getSelection()?.removeAllRanges();
}

async function askSelectionInPopup() {
  const text = selectedAgentText.value;
  if (!text.trim()) return;
  try {
    await openPopupNewChat(props.projectId ?? null, popupAskText(text));
    hideSelectionToolbar();
    window.getSelection()?.removeAllRanges();
  } catch (err) {
    console.error("[agent-selection] open popup failed", err);
  }
}

function onSelectionInteractionEnd() {
  void updateSelectionToolbar();
}

function onDocumentPointerDown(event: PointerEvent) {
  const target = event.target;
  if (!(target instanceof Node)) return;
  if (selectionToolbarEl.value?.contains(target)) return;
  pointerSelectionStarted = Boolean(selectableForNode(target));
  hideSelectionToolbar();
}

function onDocumentPointerUp() {
  if (!pointerSelectionStarted) return;
  pointerSelectionStarted = false;
  void updateSelectionToolbar();
}

function onDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") hideSelectionToolbar();
}

onMounted(() => {
  document.addEventListener("pointerdown", onDocumentPointerDown, true);
  document.addEventListener("pointerup", onDocumentPointerUp, true);
  document.addEventListener("keydown", onDocumentKeydown);
  window.addEventListener("resize", hideSelectionToolbar);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointerDown, true);
  document.removeEventListener("pointerup", onDocumentPointerUp, true);
  document.removeEventListener("keydown", onDocumentKeydown);
  window.removeEventListener("resize", hideSelectionToolbar);
});
</script>

<template>
  <div
    ref="frameEl"
    class="chat-transcript-frame"
  >
    <div
      ref="scroller"
      class="chat-transcript"
      :class="{
        'is-empty': isEmpty,
      }"
      @scroll="onScroll"
      @keyup="onSelectionInteractionEnd"
    >
      <div v-if="isEmpty" class="chat-empty">
        {{ emptyHeadline }}
      </div>
      <template v-else>
        <AgentTimeline
          :events="timelineEvents"
          :is-thinking="isThinking"
          :project-cwd="projectCwd"
          :active-plan-approval-turn-id="activePlanApprovalTurnId"
          :pending-actions="pendingAgentActions"
          :show-expired-pending-actions="showExpiredPendingActions"
          :can-retry-event="canRetryEvent"
          @event-toggled="onTimelineEventToggled"
          @resolve-pending-action="emit('resolvePendingAgentAction', $event)"
          @retry-event="emit('retry-event', $event)"
          @open-image="emit('open-image', $event)"
        />
      </template>
      <div class="chat-controls-wrap">
        <slot name="controls" />
      </div>
    </div>
    <Transition name="agent-selection-toolbar">
      <div
        v-if="selectionToolbarVisible"
        ref="selectionToolbarEl"
        class="agent-selection-toolbar"
        :style="selectionToolbarStyle"
        role="toolbar"
        aria-label="Agent 选中文本操作"
        @mousedown.prevent
      >
        <button
          type="button"
          class="agent-selection-toolbar__button"
          title="复制"
          aria-label="复制"
          @click="copySelection"
        >
          <Copy :size="15" aria-hidden="true" />
        </button>
        <button
          type="button"
          class="agent-selection-toolbar__button"
          title="引用"
          aria-label="引用"
          @click="insertSelectionQuote"
        >
          <Quote :size="15" aria-hidden="true" />
        </button>
        <button
          type="button"
          class="agent-selection-toolbar__button"
          title="在弹出窗口提问"
          aria-label="在弹出窗口提问"
          @click="askSelectionInPopup"
        >
          <MessageSquarePlus :size="15" aria-hidden="true" />
        </button>
      </div>
    </Transition>
    <ChatScrollMap
      ref="scrollMap"
      :events="timelineEvents"
      :hover-target="frameEl"
      :project-cwd="projectCwd"
      :scroller="scroller"
    />
  </div>
</template>
