<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch, type CSSProperties } from "vue";
import { ChevronDown, ChevronRight } from "lucide-vue-next";
import type { AgentTimelineEvent } from "@lilia/contracts";
import MarkdownBlock from "./MarkdownBlock.vue";
import TimelineCardDetails from "./TimelineCardDetails.vue";
import {
  createTimelineMarkdownView,
  readTimelineDisplay,
  readTimelinePayloadRecord,
  truncateTimelineText,
} from "./timelineDisplay";

type PlanStatusKind = "pending" | "revision" | "approved" | "rejected" | "cancelled" | "neutral";

interface HeadingMarker {
  index: number;
  key: string;
  label: string;
  level: 4 | 5 | 6;
  top: number;
}

interface PlanScrollMetrics {
  domainHeight: number;
  scrollable: boolean;
  thumbHeight: number;
  thumbTop: number;
  trackHeight: number;
  visibleHeight: number;
}

interface PlanScrollDragState {
  pointerId: number;
  startScrollTop: number;
  startY: number;
  metrics: PlanScrollMetrics;
}

const STATUS_LABELS: Record<PlanStatusKind, string> = {
  pending: "待确认",
  revision: "修改要求",
  approved: "已同意",
  rejected: "已拒绝",
  cancelled: "已取消",
  neutral: "计划",
};

const props = defineProps<{
  event: AgentTimelineEvent;
  expanded: boolean;
  canToggle: boolean;
  projectCwd?: string | null;
}>();

const emit = defineEmits<{
  toggle: [event: AgentTimelineEvent];
}>();

const bodyEl = ref<HTMLElement | null>(null);
const scrollTrackEl = ref<HTMLElement | null>(null);
const headingMarkers = ref<HeadingMarker[]>([]);
const isPlanScrollbarVisible = ref(false);
const isPointerInPlanScrollbarZone = ref(false);
const isPlanScrollbarDragging = ref(false);
const thumbTop = ref(0);
const thumbHeight = ref(0);
const display = computed(() =>
  readTimelineDisplay(props.event, { projectCwd: props.projectCwd }),
);
const payload = computed(() => readTimelinePayloadRecord(props.event));
const details = computed(() => display.value.details ?? []);
const planText = computed(() => readPayloadText(payload.value.plan));
const revisionRequest = computed(() => readPayloadText(payload.value.revisionRequest));
const allowedPromptRows = computed(() => {
  const prompts = payload.value.allowedPrompts;
  if (!Array.isArray(prompts)) return [];
  return prompts.flatMap((item, index) => {
    if (!isRecord(item)) return [];
    const tool = compactPayloadLine(item.tool, 80);
    const prompt = compactPayloadLine(item.prompt, 400);
    const text = [tool, prompt].filter(Boolean).join("：");
    return text ? [{ key: `${index}:${text}`, text }] : [];
  });
});
const hasStructuredBody = computed(() =>
  Boolean(planText.value || revisionRequest.value || allowedPromptRows.value.length),
);
const label = computed(() =>
  display.value.label?.trim() ||
  display.value.action?.trim() ||
  "计划更新",
);
const summaryLine = computed(() =>
  display.value.preview?.trim() ||
  display.value.label?.trim() ||
  "暂无摘要。",
);
const compactSummaryText = computed(() =>
  truncateTimelineText(summaryLine.value.replace(/\s+/g, " ").trim(), 180),
);
const statusKind = computed<PlanStatusKind>(() => {
  if (revisionRequest.value) return "revision";
  if (payload.value.approved === null) return "pending";
  if (payload.value.approved === true) return "approved";
  if (payload.value.approved === false) return "rejected";
  if (props.event.status === "requires_action") return "pending";
  if (props.event.status === "cancelled") return "cancelled";
  return "neutral";
});
const statusBadge = computed(() => STATUS_LABELS[statusKind.value]);
const detailsId = computed(() => `agent-timeline-details-${props.event.id}`);
const titleId = computed(() => `agent-timeline-title-${props.event.id}`);
const titleAriaLabel = computed(() =>
  [label.value, statusBadge.value, compactSummaryText.value].filter(Boolean).join(" "),
);
const expandedFallbackView = computed(() =>
  details.value.length
    ? null
    : createTimelineMarkdownView(summaryLine.value, {
        multilineTone: "muted",
        singleLineTone: "muted",
      }),
);
const planThumbStyle = computed<CSSProperties>(() => ({
  transform: `translateY(${thumbTop.value}px)`,
  height: `${thumbHeight.value}px`,
}));
const shouldRenderPlanScrollMap = computed(() =>
  thumbHeight.value > 0,
);

const PLAN_SCROLLBAR_HOT_ZONE = 18;
const PLAN_SCROLLBAR_HIDE_DELAY = 180;
const PLAN_SCROLL_TRACK_EDGE_PADDING = 8;
const HEADING_SCROLL_OFFSET = 8;
let planMeasurePending = false;
let resizeObserver: ResizeObserver | null = null;
let planScrollbarHideTimer: ReturnType<typeof window.setTimeout> | null = null;
let planScrollDragState: PlanScrollDragState | null = null;

watch(
  () => props.expanded,
  (expanded) => {
    if (expanded) schedulePlanScrollMeasure();
    else resetPlanScrollMap();
  },
  { flush: "post", immediate: true },
);

watch(planText, () => schedulePlanScrollMeasure(), { flush: "post" });

watch(
  bodyEl,
  (next) => {
    resizeObserver?.disconnect();
    resizeObserver = null;
    if (!next) return;

    if (typeof ResizeObserver !== "undefined") {
      resizeObserver = new ResizeObserver(schedulePlanScrollMeasure);
      resizeObserver.observe(next);
      const content = next.querySelector(".timeline-plan-card__content");
      if (content instanceof HTMLElement) resizeObserver.observe(content);
    }

    schedulePlanScrollMeasure();
  },
  { flush: "post" },
);

onBeforeUnmount(() => {
  clearPlanScrollbarHideTimer();
  clearPlanScrollDragListeners();
  resizeObserver?.disconnect();
  resizeObserver = null;
});

function onToggle() {
  if (!props.canToggle) return;
  emit("toggle", props.event);
}

function clearPlanScrollbarHideTimer() {
  if (planScrollbarHideTimer === null) return;
  window.clearTimeout(planScrollbarHideTimer);
  planScrollbarHideTimer = null;
}

function showPlanScrollbar() {
  clearPlanScrollbarHideTimer();
  isPlanScrollbarVisible.value = true;
}

function hidePlanScrollbarSoon() {
  if (planScrollbarHideTimer !== null) return;
  planScrollbarHideTimer = window.setTimeout(() => {
    if (!isPointerInPlanScrollbarZone.value && !isPlanScrollbarDragging.value) {
      isPlanScrollbarVisible.value = false;
    }
    planScrollbarHideTimer = null;
  }, PLAN_SCROLLBAR_HIDE_DELAY);
}

function schedulePlanScrollMeasure() {
  if (!props.expanded) return;
  if (planMeasurePending) return;
  planMeasurePending = true;
  void nextTick(() => {
    planMeasurePending = false;
    measurePlanScrollMap();
  });
}

function measurePlanScrollMap() {
  const body = bodyEl.value;
  if (!props.expanded || !body) {
    resetPlanScrollMap();
    return;
  }

  const metrics = readPlanScrollMetrics(body);
  thumbTop.value = metrics.scrollable ? metrics.thumbTop : 0;
  thumbHeight.value = metrics.scrollable ? metrics.thumbHeight : 0;

  if (!metrics.scrollable || !planText.value) {
    headingMarkers.value = [];
    return;
  }

  measureHeadingMarkers(body, metrics);
}

function measureHeadingMarkers(body: HTMLElement, metrics: PlanScrollMetrics) {
  const headings = planHeadingElements(body);
  if (!headings.length || metrics.trackHeight <= 0 || metrics.domainHeight <= 0) {
    headingMarkers.value = [];
    return;
  }

  const bodyRect = body.getBoundingClientRect();
  headingMarkers.value = headings.flatMap((heading, index) => {
    const label = compactHeadingLabel(heading.textContent);
    if (!label) return [];
    const headingRect = heading.getBoundingClientRect();
    const contentTop = headingRect.top - bodyRect.top + body.scrollTop;
    const top = metrics.trackHeight * contentTop / metrics.domainHeight;
    return [{
      index,
      key: `${index}:${label}`,
      label,
      level: readHeadingLevel(heading),
      top: clamp(top, 0, metrics.trackHeight),
    }];
  });
}

function resetPlanScrollMap() {
  clearPlanScrollbarHideTimer();
  clearPlanScrollDragListeners();
  headingMarkers.value = [];
  isPlanScrollbarVisible.value = false;
  isPointerInPlanScrollbarZone.value = false;
  thumbTop.value = 0;
  thumbHeight.value = 0;
}

function readPlanScrollMetrics(body: HTMLElement): PlanScrollMetrics {
  const visibleHeight = body.clientHeight;
  const domainHeight = Math.max(visibleHeight, body.scrollHeight);
  const scrollable = body.scrollHeight - visibleHeight > 1;
  const trackHeight = readPlanTrackHeight(visibleHeight);
  const visibleTop = clamp(body.scrollTop, 0, Math.max(0, domainHeight - visibleHeight));

  return {
    domainHeight,
    scrollable,
    thumbHeight: domainHeight > 0 ? trackHeight * visibleHeight / domainHeight : 0,
    thumbTop: domainHeight > 0 ? trackHeight * visibleTop / domainHeight : 0,
    trackHeight,
    visibleHeight,
  };
}

function readPlanTrackHeight(visibleHeight: number): number {
  const rectHeight = scrollTrackEl.value?.getBoundingClientRect().height ?? 0;
  if (rectHeight > 0) return rectHeight;
  return Math.max(0, visibleHeight - PLAN_SCROLL_TRACK_EDGE_PADDING * 2);
}

function planHeadingElements(body: HTMLElement): HTMLElement[] {
  return Array.from(
    body.querySelectorAll<HTMLElement>(
      ".timeline-plan-card__markdown--plan .markdown-block__heading",
    ),
  );
}

function headingMarkerStyle(marker: HeadingMarker): CSSProperties {
  return { top: `${marker.top}px` };
}

function onPlanBodyScroll() {
  showPlanScrollbar();
  schedulePlanScrollMeasure();
}

function onPlanBodyScrollEnd() {
  if (!isPointerInPlanScrollbarZone.value) hidePlanScrollbarSoon();
}

function onPlanMouseMove(event: MouseEvent) {
  const inZone = isInPlanScrollbarZone(event);
  isPointerInPlanScrollbarZone.value = inZone;
  if (inZone) {
    showPlanScrollbar();
    return;
  }
  if (isPlanScrollbarVisible.value) hidePlanScrollbarSoon();
}

function onPlanMouseLeave() {
  isPointerInPlanScrollbarZone.value = false;
  if (isPlanScrollbarVisible.value) hidePlanScrollbarSoon();
}

function isInPlanScrollbarZone(event: MouseEvent): boolean {
  const body = bodyEl.value;
  if (!body) return false;
  const rect = body.getBoundingClientRect();
  return event.clientX >= rect.right - PLAN_SCROLLBAR_HOT_ZONE && event.clientX <= rect.right;
}

function onPlanTrackPointerDown(event: PointerEvent) {
  const body = bodyEl.value;
  const track = scrollTrackEl.value;
  if (!body || !track) return;

  const metrics = readPlanScrollMetrics(body);
  if (!metrics.scrollable) return;

  const trackRect = track.getBoundingClientRect();
  const y = event.clientY - trackRect.top;
  const thumbBottom = metrics.thumbTop + metrics.thumbHeight;
  if (y >= metrics.thumbTop && y <= thumbBottom) return;

  event.preventDefault();
  showPlanScrollbar();
  scrollPlanBodyTo(
    body,
    body.scrollTop + (y < metrics.thumbTop ? -metrics.visibleHeight : metrics.visibleHeight),
    "auto",
  );
  schedulePlanScrollMeasure();
}

function onPlanThumbPointerDown(event: PointerEvent) {
  const body = bodyEl.value;
  if (!body) return;

  const metrics = readPlanScrollMetrics(body);
  if (!metrics.scrollable || metrics.trackHeight <= 0) return;

  event.preventDefault();
  event.stopPropagation();
  clearPlanScrollbarHideTimer();
  planScrollDragState = {
    pointerId: event.pointerId,
    startScrollTop: body.scrollTop,
    startY: event.clientY,
    metrics,
  };
  isPlanScrollbarDragging.value = true;
  isPlanScrollbarVisible.value = true;
  window.addEventListener("pointermove", onPlanWindowPointerMove);
  window.addEventListener("pointerup", onPlanWindowPointerEnd);
  window.addEventListener("pointercancel", onPlanWindowPointerEnd);
}

function onPlanWindowPointerMove(event: PointerEvent) {
  const body = bodyEl.value;
  if (!body || !planScrollDragState || event.pointerId !== planScrollDragState.pointerId) {
    return;
  }

  event.preventDefault();
  const deltaY = event.clientY - planScrollDragState.startY;
  body.scrollTop = clamp(
    planScrollDragState.startScrollTop +
      deltaY * planScrollDragState.metrics.domainHeight / planScrollDragState.metrics.trackHeight,
    0,
    maxPlanScrollTop(body),
  );
  schedulePlanScrollMeasure();
}

function onPlanWindowPointerEnd(event: PointerEvent) {
  if (planScrollDragState && event.pointerId !== planScrollDragState.pointerId) return;
  clearPlanScrollDragListeners();
  if (!isPointerInPlanScrollbarZone.value) hidePlanScrollbarSoon();
}

function clearPlanScrollDragListeners() {
  planScrollDragState = null;
  isPlanScrollbarDragging.value = false;
  window.removeEventListener("pointermove", onPlanWindowPointerMove);
  window.removeEventListener("pointerup", onPlanWindowPointerEnd);
  window.removeEventListener("pointercancel", onPlanWindowPointerEnd);
}

function readHeadingLevel(heading: HTMLElement): 4 | 5 | 6 {
  const level = Number(heading.tagName.replace(/^H/i, ""));
  return level === 4 || level === 5 || level === 6 ? level : 6;
}

function jumpToHeading(marker: HeadingMarker) {
  const body = bodyEl.value;
  const heading = body ? planHeadingElements(body)[marker.index] : null;
  if (!body || !heading) return;

  const bodyRect = body.getBoundingClientRect();
  const headingRect = heading.getBoundingClientRect();
  scrollPlanBodyTo(
    body,
    body.scrollTop + headingRect.top - bodyRect.top - HEADING_SCROLL_OFFSET,
    prefersReducedMotion() ? "auto" : "smooth",
  );
}

function scrollPlanBodyTo(body: HTMLElement, top: number, behavior: ScrollBehavior) {
  const nextTop = clamp(top, 0, maxPlanScrollTop(body));
  if (typeof body.scrollTo === "function") {
    body.scrollTo({ top: nextTop, behavior });
  } else {
    body.scrollTop = nextTop;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function readPayloadText(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function compactPayloadLine(value: unknown, max: number): string {
  const text = readPayloadText(value).replace(/\s+/g, " ").trim();
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function compactHeadingLabel(value: string | null): string {
  return (value ?? "").replace(/\s+/g, " ").trim();
}

function prefersReducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(Math.max(value, min), max);
}

function maxPlanScrollTop(body: HTMLElement): number {
  return Math.max(0, body.scrollHeight - body.clientHeight);
}
</script>

<template>
  <article
    class="timeline-card timeline-card--plan"
    :class="{
      'is-expanded': expanded,
      'is-collapsed': !expanded,
    }"
    :aria-labelledby="titleId"
  >
    <button
      type="button"
      class="timeline-plan-card__head"
      :aria-expanded="expanded"
      :aria-controls="detailsId"
      :aria-label="titleAriaLabel"
      :disabled="!canToggle"
      @click="onToggle"
    >
      <span class="timeline-plan-card__title-row">
        <span :id="titleId" class="timeline-plan-card__title">
          {{ label }}
        </span>
        <span
          class="timeline-plan-card__badge"
          :class="`timeline-plan-card__badge--${statusKind}`"
        >
          {{ statusBadge }}
        </span>
        <component
          v-if="canToggle"
          :is="expanded ? ChevronDown : ChevronRight"
          class="timeline-plan-card__chevron"
          :size="13"
          aria-hidden="true"
        />
      </span>
      <span
        v-if="!expanded && compactSummaryText"
        class="timeline-plan-card__summary"
      >
        {{ compactSummaryText }}
      </span>
    </button>

    <div
      v-if="expanded"
      :id="detailsId"
      class="timeline-plan-card__body-shell"
      @mousemove="onPlanMouseMove"
      @mouseleave="onPlanMouseLeave"
    >
      <div
        ref="bodyEl"
        class="timeline-plan-card__body"
        @scroll="onPlanBodyScroll"
        @scrollend="onPlanBodyScrollEnd"
      >
        <div v-if="hasStructuredBody" class="timeline-plan-card__content">
          <MarkdownBlock
            v-if="planText"
            :content="planText"
            class="timeline-plan-card__markdown timeline-plan-card__markdown--plan"
          />

          <section
            v-if="revisionRequest"
            class="timeline-plan-card__section timeline-plan-card__section--revision"
            aria-label="修改要求"
          >
            <p class="timeline-plan-card__section-label">修改要求</p>
            <MarkdownBlock
              :content="revisionRequest"
              tone="muted"
              class="timeline-plan-card__markdown"
            />
          </section>

          <section
            v-if="allowedPromptRows.length"
            class="timeline-plan-card__section"
            aria-label="可能调用"
          >
            <p class="timeline-plan-card__section-label">可能调用</p>
            <ul class="timeline-plan-card__prompt-list">
              <li
                v-for="row in allowedPromptRows"
                :key="row.key"
                class="timeline-plan-card__prompt-item"
              >
                {{ row.text }}
              </li>
            </ul>
          </section>
        </div>

        <TimelineCardDetails
          v-else
          :details="details"
          :fallback-view="expandedFallbackView"
        />
      </div>

      <div
        v-if="shouldRenderPlanScrollMap"
        class="timeline-plan-card__scroll-map"
        :class="{
          'is-visible': isPlanScrollbarVisible,
          'is-dragging': isPlanScrollbarDragging,
        }"
        aria-label="计划正文滚动"
      >
        <div
          ref="scrollTrackEl"
          class="timeline-plan-card__scroll-track"
          @pointerdown="onPlanTrackPointerDown"
        >
          <div
            class="timeline-plan-card__scroll-thumb"
            :style="planThumbStyle"
            @pointerdown="onPlanThumbPointerDown"
          />
          <button
            v-for="marker in headingMarkers"
            :key="marker.key"
            type="button"
            class="timeline-plan-card__heading-marker"
            :class="`timeline-plan-card__heading-marker--level-${marker.level}`"
            :style="headingMarkerStyle(marker)"
            :aria-label="`跳到计划标题：${marker.label}`"
            :title="marker.label"
            @pointerdown.stop
            @click.stop="jumpToHeading(marker)"
          >
            <span class="timeline-plan-card__heading-marker-dot" />
            <span class="timeline-plan-card__heading-marker-tooltip">
              {{ marker.label }}
            </span>
          </button>
        </div>
      </div>
    </div>
  </article>
</template>
