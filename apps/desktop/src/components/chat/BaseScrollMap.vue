<script setup lang="ts">
import { computed, watch } from "vue";
import type { CSSProperties } from "vue";
import {
  useScrollMap,
  useScrollMapVisibility,
  type ScrollMapGeometry,
  type ScrollMapMetrics,
} from "./useScrollMap";

type MapStyleInput = CSSProperties | ((metrics: ScrollMapMetrics) => CSSProperties);

const props = withDefaults(defineProps<{
  ariaLabel?: string;
  enabled?: boolean;
  hoverTarget?: HTMLElement | null;
  mapStyle?: MapStyleInput;
  measureKey?: string | number | boolean | null;
  observeTargets?: (scroller: HTMLElement) => HTMLElement[];
  readGeometry?: (scroller: HTMLElement) => ScrollMapGeometry;
  scheduleMode?: "frame" | "tick";
  scroller: HTMLElement | null;
  thumbClass?: string;
  trackClass?: string;
}>(), {
  ariaLabel: undefined,
  enabled: true,
  hoverTarget: null,
  mapStyle: undefined,
  measureKey: null,
  observeTargets: undefined,
  readGeometry: undefined,
  scheduleMode: "tick",
  thumbClass: "",
  trackClass: "",
});

const scrollerRef = computed(() => props.scroller);
const hoverTargetRef = computed(() => props.hoverTarget ?? null);
const enabledRef = computed(() => props.enabled);
const scrollMap = useScrollMap({
  enabled: enabledRef,
  observeTargets: props.observeTargets,
  readGeometry: props.readGeometry,
  scheduleMode: props.scheduleMode,
  scroller: scrollerRef,
});
const visibility = useScrollMapVisibility({
  enabled: enabledRef,
  hoverTarget: hoverTargetRef,
  isDragging: scrollMap.isDragging,
  scroller: scrollerRef,
});

const { isDragging, metrics, thumbStyle } = scrollMap;
const { isVisible } = visibility;
const resolvedMapStyle = computed<CSSProperties | undefined>(() => {
  if (!props.mapStyle) return undefined;
  return typeof props.mapStyle === "function"
    ? props.mapStyle(metrics.value)
    : props.mapStyle;
});
const shouldRender = computed(() => props.enabled && scrollMap.shouldRender.value);

watch(
  () => props.measureKey,
  () => scrollMap.scheduleMeasure(),
  { flush: "post" },
);

function show() {
  scrollMap.scheduleMeasure();
  visibility.show();
}

defineExpose({
  hide: visibility.hide,
  scheduleMeasure: scrollMap.scheduleMeasure,
  scrollTo: scrollMap.scrollTo,
  show,
});
</script>

<template>
  <div
    v-if="shouldRender"
    class="scroll-map"
    :class="{
      'is-visible': isVisible,
      'is-dragging': isDragging,
    }"
    :style="resolvedMapStyle"
    :aria-label="ariaLabel"
  >
    <div
      :ref="scrollMap.setTrackElement"
      class="scroll-map__track"
      :class="trackClass"
      @pointerdown="scrollMap.onTrackPointerDown"
    >
      <div
        class="scroll-map__thumb"
        :class="thumbClass"
        :style="thumbStyle"
        @pointerdown="scrollMap.onThumbPointerDown"
      />
      <slot
        :metrics="metrics"
        :scroll-to="scrollMap.scrollTo"
      />
    </div>
  </div>
</template>
