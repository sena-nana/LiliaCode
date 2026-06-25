<script setup lang="ts">
import { computed, ref } from "vue";
import type { CSSProperties } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import BaseScrollMap from "./BaseScrollMap.vue";
import {
  clamp,
  markerTopForElement,
  type ScrollMapGeometry,
  type ScrollMapMetrics,
} from "./useScrollMap";
import {
  readTimelinePayloadRecord,
  timelineDeclaredGroupUnit,
  timelineEventLabel,
  timelineInlinePreview,
  type TimelineDisplayContext,
} from "./timelineDisplay";

type ScrollMarkerKind = "user" | "plan" | "error";

interface ScrollMarkerTooltipItem {
  id: string;
  summary: string;
  title: string;
}

interface ScrollMarker {
  id: string;
  kind: ScrollMarkerKind;
  anchorId: string;
  top: number;
  tooltipItems: ScrollMarkerTooltipItem[];
}

const props = defineProps<{
  events: AgentTimelineEvent[];
  hoverTarget?: HTMLElement | null;
  projectCwd?: string | null;
  scroller: HTMLElement | null;
}>();

const baseScrollMap = ref<{ show: () => void } | null>(null);

const MARKER_CLUSTER_DISTANCE = 4;
const SCROLL_TARGET_OFFSET = 16;
const TOOLTIP_ITEM_LIMIT = 4;
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

const displayContext = computed<TimelineDisplayContext>(() => ({
  projectCwd: props.projectCwd,
}));
const measureKey = computed(() =>
  [
    props.projectCwd ?? "",
    props.events.map(markerEventSignature).join("|"),
  ].join("\n"),
);

function show() {
  baseScrollMap.value?.show();
}

function mapStyle(metrics: ScrollMapMetrics): CSSProperties {
  return {
    "--chat-scroll-map-bottom-offset": `${metrics.bottomOffset}px`,
  };
}

function markersForMetrics(metrics: ScrollMapMetrics): ScrollMarker[] {
  const scroller = props.scroller;
  if (!scroller || !metrics.scrollable) return [];
  return buildMarkers(scroller, metrics);
}

function readObservedTargets(scroller: HTMLElement): HTMLElement[] {
  return [".chat-controls-wrap", ".agent-timeline"].flatMap((selector) => {
    const element = scroller.querySelector(selector);
    return element instanceof HTMLElement ? [element] : [];
  });
}

function readChatScrollGeometry(scroller: HTMLElement): ScrollMapGeometry {
  const scrollerRect = scroller.getBoundingClientRect();
  const controls = scroller.querySelector(".chat-controls-wrap");
  const bottomOverlap = controls instanceof HTMLElement
    ? visibleBottomOverlap(scrollerRect, controls.getBoundingClientRect())
    : 0;
  const visibleHeight = Math.max(0, scroller.clientHeight - bottomOverlap);

  return {
    bottomOffset: bottomOverlap,
    domainHeight: Math.max(visibleHeight, scroller.scrollHeight - bottomOverlap),
    visibleHeight,
  };
}

function visibleBottomOverlap(scrollerRect: DOMRect, controlsRect: DOMRect): number {
  if (controlsRect.height <= 0) return 0;
  if (controlsRect.top >= scrollerRect.bottom || controlsRect.bottom <= scrollerRect.top) return 0;
  return clamp(scrollerRect.bottom - Math.max(scrollerRect.top, controlsRect.top), 0, scrollerRect.height);
}

function buildMarkers(scroller: HTMLElement, metrics: ScrollMapMetrics): ScrollMarker[] {
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
      top: markerTopForElement(scroller, anchor, metrics),
      tooltipItems: [markerTooltipItem(event)],
    });
  }

  return mergeNearbyMarkers(raw);
}

function markerTooltipItem(event: AgentTimelineEvent): ScrollMarkerTooltipItem {
  const title = timelineEventLabel(event, displayContext.value);
  const summary = timelineInlinePreview(event, displayContext.value);
  return {
    id: event.id,
    title,
    summary: summary && summary !== title ? summary : "",
  };
}

function markerEventSignature(event: AgentTimelineEvent): string {
  const kind = markerKind(event);
  if (!kind) return `${event.id}:${event.kind}:${event.status}`;
  const item = markerTooltipItem(event);
  return `${event.id}:${kind}:${event.status}:${item.title}:${item.summary}`;
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

    previous.tooltipItems.push(...marker.tooltipItems);
    if (markerPriority(marker.kind) > markerPriority(previous.kind)) {
      previous.kind = marker.kind;
      previous.anchorId = marker.anchorId;
      previous.id = marker.id;
      previous.top = marker.top;
    }
  }

  return merged;
}

function markerPriority(kind: ScrollMarkerKind): number {
  return MARKER_PRIORITIES[kind];
}

function visibleTooltipItems(marker: ScrollMarker): ScrollMarkerTooltipItem[] {
  return marker.tooltipItems.slice(0, TOOLTIP_ITEM_LIMIT);
}

function hiddenTooltipItemCount(marker: ScrollMarker): number {
  return Math.max(0, marker.tooltipItems.length - TOOLTIP_ITEM_LIMIT);
}

function markerButtonStyle(marker: ScrollMarker): CSSProperties {
  return {
    top: `${marker.top}px`,
  };
}

function markerTooltipId(marker: ScrollMarker): string {
  return `chat-scroll-map-tooltip-${marker.id.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

function markerAriaLabel(marker: ScrollMarker): string {
  const count = marker.tooltipItems.length;
  const label = MARKER_LABELS[marker.kind];
  if (count <= 1) return `跳到${label}`;
  return `跳到${count} 个关键位置中的${label}`;
}

function jumpTo(marker: ScrollMarker, scrollTo: (top: number, behavior: ScrollBehavior) => void) {
  const scroller = props.scroller;
  if (!scroller) return;
  const anchor = readAnchorMap(scroller).get(marker.anchorId);
  if (!anchor) return;

  const scrollerRect = scroller.getBoundingClientRect();
  const anchorRect = anchor.getBoundingClientRect();
  const targetTop = anchorRect.top - scrollerRect.top + scroller.scrollTop - SCROLL_TARGET_OFFSET;
  scrollTo(targetTop, "smooth");
}

defineExpose({ show });
</script>

<template>
  <BaseScrollMap
    ref="baseScrollMap"
    :scroller="scroller"
    :hover-target="hoverTarget"
    :observe-targets="readObservedTargets"
    :read-geometry="readChatScrollGeometry"
    :measure-key="measureKey"
    :map-style="mapStyle"
    schedule-mode="frame"
    class="chat-scroll-map"
    track-class="chat-scroll-map__track"
    thumb-class="chat-scroll-map__thumb"
  >
    <template #default="{ metrics, scrollTo }">
      <button
        v-for="marker in markersForMetrics(metrics)"
        :key="`${marker.kind}:${marker.id}:${marker.tooltipItems.length}`"
      type="button"
      class="chat-scroll-map__marker"
      :class="`chat-scroll-map__marker--${marker.kind}`"
      :data-agent-id="`chat.scroll-map.marker.${marker.kind}.${marker.id}`"
      :style="markerButtonStyle(marker)"
        :aria-label="markerAriaLabel(marker)"
        :aria-describedby="markerTooltipId(marker)"
        @pointerdown.stop
        @click.stop="jumpTo(marker, scrollTo)"
      >
        <span
          :id="markerTooltipId(marker)"
          role="tooltip"
          class="chat-scroll-map__tooltip"
        >
          <span
            v-for="item in visibleTooltipItems(marker)"
            :key="item.id"
            class="chat-scroll-map__tooltip-item"
          >
            <span class="chat-scroll-map__tooltip-title">{{ item.title }}</span>
            <span v-if="item.summary" class="chat-scroll-map__tooltip-summary">
              {{ item.summary }}
            </span>
          </span>
          <span
            v-if="hiddenTooltipItemCount(marker) > 0"
            class="chat-scroll-map__tooltip-more"
          >
            另有 {{ hiddenTooltipItemCount(marker) }} 个位置
          </span>
        </span>
      </button>
    </template>
  </BaseScrollMap>
</template>
