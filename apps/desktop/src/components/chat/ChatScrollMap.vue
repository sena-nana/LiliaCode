<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  readTimelinePayloadRecord,
  timelineDeclaredGroupUnit,
} from "./timelineDisplay";

type ScrollMarkerKind = "user" | "plan" | "error";

interface ScrollMarker {
  id: string;
  kind: ScrollMarkerKind;
  anchorId: string;
  top: number;
  count: number;
  label: string;
}

interface ScrollMetrics {
  bottomOffset: number;
  domainHeight: number;
  scrollable: boolean;
  thumbHeight: number;
  thumbTop: number;
  trackHeight: number;
  visibleHeight: number;
}

interface ScrollDragState {
  pointerId: number;
  startScrollTop: number;
  startY: number;
  metrics: ScrollMetrics;
}

const props = defineProps<{
  events: AgentTimelineEvent[];
  scroller: HTMLElement | null;
  visible: boolean;
}>();

const track = ref<HTMLElement | null>(null);
const markers = ref<ScrollMarker[]>([]);
const bottomOffset = ref(0);
const isScrollable = ref(false);
const isDragging = ref(false);
const thumbTop = ref(0);
const thumbHeight = ref(0);

const TRACK_EDGE_PADDING = 8;
const MARKER_CLUSTER_DISTANCE = 4;
const SCROLL_TARGET_OFFSET = 16;
const MARKER_LABELS: Record<ScrollMarkerKind, string> = {
  error: "错误位置",
  plan: "计划位置",
  user: "用户消息",
};
const MARKER_PRIORITIES: Record<ScrollMarkerKind, number> = {
  error: 3,
  plan: 2,
  user: 1,
};

let frameId: number | null = null;
let resizeObserver: ResizeObserver | null = null;
let observedControls: HTMLElement | null = null;
let observedTimeline: HTMLElement | null = null;
let dragState: ScrollDragState | null = null;

const thumbStyle = computed<CSSProperties>(() => ({
  transform: `translateY(${thumbTop.value}px)`,
  height: `${thumbHeight.value}px`,
}));

const mapStyle = computed<CSSProperties>(() => ({
  "--chat-scroll-map-bottom-offset": `${bottomOffset.value}px`,
}));

const shouldRender = computed(() => isScrollable.value && thumbHeight.value > 0);

watch(
  () => props.scroller,
  (next, previous) => {
    if (previous) previous.removeEventListener("scroll", scheduleMeasure);
    resizeObserver?.disconnect();
    resizeObserver = null;
    observedControls = null;
    observedTimeline = null;

    if (next) {
      next.addEventListener("scroll", scheduleMeasure, { passive: true });
      if (typeof ResizeObserver !== "undefined") {
        resizeObserver = new ResizeObserver(scheduleMeasure);
        resizeObserver.observe(next);
        refreshResizeTargets(next);
      }
    }
    scheduleMeasure();
  },
  { immediate: true },
);

watch(
  () => [
    props.visible,
    props.events.map((event) => `${event.id}:${event.kind}:${event.status}`).join("|"),
  ],
  () => scheduleMeasure(),
  { flush: "post" },
);

onMounted(() => {
  scheduleMeasure();
});

onBeforeUnmount(() => {
  if (frameId !== null) window.cancelAnimationFrame(frameId);
  props.scroller?.removeEventListener("scroll", scheduleMeasure);
  clearDragListeners();
  resizeObserver?.disconnect();
  observedControls = null;
  observedTimeline = null;
});

function scheduleMeasure() {
  if (frameId !== null) return;
  frameId = window.requestAnimationFrame(() => {
    frameId = null;
    void nextTick(() => measure());
  });
}

function measure() {
  const scroller = props.scroller;
  if (!scroller) {
    resetMeasurements();
    return;
  }

  const metrics = readScrollMetrics(scroller);
  refreshResizeTargets(scroller);
  bottomOffset.value = metrics.bottomOffset;
  isScrollable.value = metrics.scrollable;
  thumbTop.value = metrics.thumbTop;
  thumbHeight.value = metrics.thumbHeight;

  if (!metrics.scrollable) {
    markers.value = [];
    return;
  }

  markers.value = buildMarkers(scroller, metrics);
}

function refreshResizeTargets(scroller: HTMLElement) {
  if (!resizeObserver) return;
  const controls = scroller.querySelector(".chat-controls-wrap");
  const nextControls = controls instanceof HTMLElement ? controls : null;
  if (nextControls !== observedControls) {
    if (observedControls) resizeObserver.unobserve(observedControls);
    if (nextControls) resizeObserver.observe(nextControls);
    observedControls = nextControls;
  }

  const timeline = scroller.querySelector(".agent-timeline");
  const nextTimeline = timeline instanceof HTMLElement ? timeline : null;
  if (nextTimeline !== observedTimeline) {
    if (observedTimeline) resizeObserver.unobserve(observedTimeline);
    if (nextTimeline) resizeObserver.observe(nextTimeline);
    observedTimeline = nextTimeline;
  }
}

function resetMeasurements() {
  bottomOffset.value = 0;
  isScrollable.value = false;
  isDragging.value = false;
  thumbTop.value = 0;
  thumbHeight.value = 0;
  markers.value = [];
}

function readScrollMetrics(scroller: HTMLElement): ScrollMetrics {
  const scrollerRect = scroller.getBoundingClientRect();
  const controls = scroller.querySelector(".chat-controls-wrap");
  const bottomOverlap = controls instanceof HTMLElement
    ? visibleBottomOverlap(scrollerRect, controls.getBoundingClientRect())
    : 0;
  const visibleHeight = Math.max(0, scroller.clientHeight - bottomOverlap);
  const domainHeight = Math.max(visibleHeight, scroller.scrollHeight - bottomOverlap);
  const scrollable = domainHeight - visibleHeight > 1;
  const trackHeight = readTrackHeight(visibleHeight);
  const visibleTop = clamp(scroller.scrollTop, 0, Math.max(0, domainHeight - visibleHeight));

  return {
    bottomOffset: bottomOverlap,
    domainHeight,
    scrollable,
    thumbHeight: domainHeight > 0 ? trackHeight * visibleHeight / domainHeight : 0,
    thumbTop: domainHeight > 0 ? trackHeight * visibleTop / domainHeight : 0,
    trackHeight,
    visibleHeight,
  };
}

function visibleBottomOverlap(scrollerRect: DOMRect, controlsRect: DOMRect): number {
  if (controlsRect.height <= 0) return 0;
  if (controlsRect.top >= scrollerRect.bottom || controlsRect.bottom <= scrollerRect.top) return 0;
  return clamp(scrollerRect.bottom - Math.max(scrollerRect.top, controlsRect.top), 0, scrollerRect.height);
}

function readTrackHeight(visibleHeight: number): number {
  const rectHeight = track.value?.getBoundingClientRect().height ?? 0;
  if (rectHeight > 0) return rectHeight;
  return Math.max(0, visibleHeight - TRACK_EDGE_PADDING * 2);
}

function buildMarkers(scroller: HTMLElement, metrics: ScrollMetrics): ScrollMarker[] {
  if (metrics.domainHeight <= 0 || metrics.trackHeight <= 0) return [];

  const anchors = readAnchorMap(scroller);
  const raw: ScrollMarker[] = [];

  for (const event of props.events) {
    const kind = markerKind(event);
    if (!kind) continue;

    const anchor = anchors.get(event.id);
    if (!anchor) continue;

    raw.push({
      id: event.id,
      kind,
      anchorId: event.id,
      top: markerTop(scroller, anchor, metrics),
      count: 1,
      label: MARKER_LABELS[kind],
    });
  }

  return mergeNearbyMarkers(raw);
}

function markerTop(scroller: HTMLElement, anchor: HTMLElement, metrics: ScrollMetrics): number {
  const scrollerRect = scroller.getBoundingClientRect();
  const anchorRect = anchor.getBoundingClientRect();
  const contentTop = anchorRect.top - scrollerRect.top + scroller.scrollTop;
  return clamp(metrics.trackHeight * contentTop / metrics.domainHeight, 0, metrics.trackHeight);
}

function markerKind(event: AgentTimelineEvent): ScrollMarkerKind | null {
  if (event.kind === "error" || event.status === "error" || event.status === "failed") return "error";
  if (event.kind === "plan" || timelineDeclaredGroupUnit(event)?.key === "plan") return "plan";
  if (event.kind === "message" && readTimelinePayloadRecord(event).role === "user") return "user";
  return null;
}

function readAnchorMap(scroller: HTMLElement): Map<string, HTMLElement> {
  const anchors = new Map<string, HTMLElement>();
  for (const element of scroller.querySelectorAll("[data-scroll-anchor-ids]")) {
    if (!(element instanceof HTMLElement)) continue;
    const ids = (element.dataset.scrollAnchorIds ?? "").split(/\s+/).filter(Boolean);
    for (const id of ids) {
      if (!anchors.has(id)) anchors.set(id, element);
    }
  }
  for (const element of scroller.querySelectorAll("[data-scroll-anchor-id]")) {
    if (!(element instanceof HTMLElement)) continue;
    const id = element.dataset.scrollAnchorId;
    if (id) anchors.set(id, element);
  }
  return anchors;
}

function mergeNearbyMarkers(rawMarkers: ScrollMarker[]): ScrollMarker[] {
  const sorted = rawMarkers
    .slice()
    .sort((a, b) => a.top - b.top || markerPriority(b.kind) - markerPriority(a.kind));
  const merged: ScrollMarker[] = [];

  for (const marker of sorted) {
    const previous = merged[merged.length - 1];
    if (!previous || Math.abs(previous.top - marker.top) > MARKER_CLUSTER_DISTANCE) {
      merged.push({ ...marker });
      continue;
    }

    previous.count += marker.count;
    if (markerPriority(marker.kind) > markerPriority(previous.kind)) {
      previous.kind = marker.kind;
      previous.anchorId = marker.anchorId;
      previous.label = marker.label;
      previous.id = marker.id;
      previous.top = marker.top;
    }
  }

  return merged;
}

function markerPriority(kind: ScrollMarkerKind): number {
  return MARKER_PRIORITIES[kind];
}

function markerButtonStyle(marker: ScrollMarker): CSSProperties {
  return {
    top: `${marker.top}px`,
  };
}

function onTrackPointerDown(event: PointerEvent) {
  const scroller = props.scroller;
  if (!scroller || !track.value) return;

  const metrics = readScrollMetrics(scroller);
  if (!metrics.scrollable) return;

  const trackRect = track.value.getBoundingClientRect();
  const y = event.clientY - trackRect.top;
  const thumbBottom = metrics.thumbTop + metrics.thumbHeight;
  if (y >= metrics.thumbTop && y <= thumbBottom) return;

  event.preventDefault();
  const pageDelta = y < metrics.thumbTop ? -metrics.visibleHeight : metrics.visibleHeight;
  scroller.scrollTo({
    top: clamp(scroller.scrollTop + pageDelta, 0, maxScrollTop(scroller)),
    behavior: "auto",
  });
  scheduleMeasure();
}

function onThumbPointerDown(event: PointerEvent) {
  const scroller = props.scroller;
  if (!scroller) return;

  const metrics = readScrollMetrics(scroller);
  if (!metrics.scrollable || metrics.trackHeight <= 0) return;

  event.preventDefault();
  event.stopPropagation();
  dragState = {
    pointerId: event.pointerId,
    startScrollTop: scroller.scrollTop,
    startY: event.clientY,
    metrics,
  };
  isDragging.value = true;
  window.addEventListener("pointermove", onWindowPointerMove);
  window.addEventListener("pointerup", onWindowPointerEnd);
  window.addEventListener("pointercancel", onWindowPointerEnd);
}

function onWindowPointerMove(event: PointerEvent) {
  if (!dragState || event.pointerId !== dragState.pointerId) return;
  const scroller = props.scroller;
  if (!scroller) return;

  event.preventDefault();
  const deltaY = event.clientY - dragState.startY;
  const nextTop = dragState.startScrollTop +
    deltaY * dragState.metrics.domainHeight / dragState.metrics.trackHeight;
  scroller.scrollTop = clamp(nextTop, 0, maxScrollTop(scroller));
  scheduleMeasure();
}

function onWindowPointerEnd(event: PointerEvent) {
  if (dragState && event.pointerId !== dragState.pointerId) return;
  clearDragListeners();
}

function clearDragListeners() {
  dragState = null;
  isDragging.value = false;
  window.removeEventListener("pointermove", onWindowPointerMove);
  window.removeEventListener("pointerup", onWindowPointerEnd);
  window.removeEventListener("pointercancel", onWindowPointerEnd);
}

function markerAriaLabel(marker: ScrollMarker): string {
  if (marker.count <= 1) return `跳到${marker.label}`;
  return `跳到${marker.count} 个关键位置中的${marker.label}`;
}

function jumpTo(marker: ScrollMarker) {
  const scroller = props.scroller;
  if (!scroller) return;
  const anchor = readAnchorMap(scroller).get(marker.anchorId);
  if (!anchor) return;

  const scrollerRect = scroller.getBoundingClientRect();
  const anchorRect = anchor.getBoundingClientRect();
  const targetTop = anchorRect.top - scrollerRect.top + scroller.scrollTop - SCROLL_TARGET_OFFSET;
  scroller.scrollTo({
    top: clamp(targetTop, 0, maxScrollTop(scroller)),
    behavior: prefersReducedMotion() ? "auto" : "smooth",
  });
  scheduleMeasure();
}

function prefersReducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(Math.max(value, min), max);
}

function maxScrollTop(scroller: HTMLElement): number {
  return Math.max(0, scroller.scrollHeight - scroller.clientHeight);
}
</script>

<template>
  <div
    v-if="shouldRender"
    class="chat-scroll-map"
    :class="{
      'is-visible': visible,
      'is-dragging': isDragging,
    }"
    :style="mapStyle"
  >
    <div
      ref="track"
      class="chat-scroll-map__track"
      @pointerdown="onTrackPointerDown"
    >
      <div
        class="chat-scroll-map__thumb"
        :style="thumbStyle"
        @pointerdown="onThumbPointerDown"
      />
      <button
        v-for="marker in markers"
        :key="`${marker.kind}:${marker.id}:${marker.count}`"
        type="button"
        class="chat-scroll-map__marker"
        :class="`chat-scroll-map__marker--${marker.kind}`"
        :style="markerButtonStyle(marker)"
        :aria-label="markerAriaLabel(marker)"
        @pointerdown.stop
        @click.stop="jumpTo(marker)"
      />
    </div>
  </div>
</template>
