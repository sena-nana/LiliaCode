<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import type { ChatImageViewerSource } from "./imageViewer";

const props = defineProps<{
  image: ChatImageViewerSource;
}>();

const emit = defineEmits<{
  close: [];
}>();

const MIN_SCALE = 1;
const MAX_SCALE = 6;
const WHEEL_SCALE_STEP = 0.0016;

const naturalWidth = ref<number | null>(null);
const naturalHeight = ref<number | null>(null);
const scale = ref(1);
const offset = ref({ x: 0, y: 0 });
const drag = ref<{
  pointerId: number;
  startX: number;
  startY: number;
  originX: number;
  originY: number;
} | null>(null);

const canDrag = computed(() => scale.value > MIN_SCALE);
const imageStyle = computed<CSSProperties>(() => ({
  transform: `translate3d(${offset.value.x}px, ${offset.value.y}px, 0) scale(${scale.value})`,
  cursor: drag.value ? "grabbing" : canDrag.value ? "grab" : "default",
}));
const metadataText = computed(() => {
  const dimensions = naturalWidth.value && naturalHeight.value
    ? `${naturalWidth.value} x ${naturalHeight.value}`
    : "";
  return [
    dimensions,
    imageFormatLabel(props.image.mime, props.image.path ?? props.image.src),
    formatFileSize(props.image.size),
  ].filter(Boolean).join(" · ");
});

watch(
  () => props.image.src,
  () => {
    naturalWidth.value = null;
    naturalHeight.value = null;
    scale.value = MIN_SCALE;
    offset.value = { x: 0, y: 0 };
    drag.value = null;
  },
  { immediate: true },
);

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function onImageLoad(event: Event) {
  const image = event.currentTarget;
  if (!(image instanceof HTMLImageElement)) return;
  naturalWidth.value = image.naturalWidth || null;
  naturalHeight.value = image.naturalHeight || null;
}

function onWheel(event: WheelEvent) {
  event.preventDefault();
  const nextScale = clamp(
    scale.value * (1 - event.deltaY * WHEEL_SCALE_STEP),
    MIN_SCALE,
    MAX_SCALE,
  );
  if (Math.abs(nextScale - scale.value) < 0.001) return;
  scale.value = nextScale;
  if (nextScale === MIN_SCALE) offset.value = { x: 0, y: 0 };
}

function onImagePointerDown(event: PointerEvent) {
  if (!canDrag.value || event.button !== 0) return;
  event.preventDefault();
  const target = event.currentTarget;
  if (target instanceof HTMLElement) target.setPointerCapture(event.pointerId);
  drag.value = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    originX: offset.value.x,
    originY: offset.value.y,
  };
}

function onImagePointerMove(event: PointerEvent) {
  const state = drag.value;
  if (!state || event.pointerId !== state.pointerId) return;
  offset.value = {
    x: state.originX + event.clientX - state.startX,
    y: state.originY + event.clientY - state.startY,
  };
}

function clearDrag(event?: PointerEvent) {
  if (!drag.value) return;
  if (event?.currentTarget instanceof HTMLElement) {
    try {
      event.currentTarget.releasePointerCapture(drag.value.pointerId);
    } catch {
      // Pointer capture may already have been released by the browser.
    }
  }
  drag.value = null;
}

function imageFormatLabel(mime: string | null | undefined, source: string): string {
  const normalizedMime = mime?.trim().toLowerCase();
  if (normalizedMime?.startsWith("image/")) {
    return normalizedMime.slice("image/".length).toUpperCase();
  }
  const cleanSource = source.split(/[?#]/)[0];
  const match = cleanSource.match(/\.([a-z0-9]+)$/i);
  return match ? match[1].toUpperCase() : "";
}

function formatFileSize(size: number | null | undefined): string {
  if (typeof size !== "number" || !Number.isFinite(size) || size < 0) return "";
  if (size < 1024) return `${size} B`;
  const units = ["KB", "MB", "GB"];
  let value = size / 1024;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  const digits = value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[index]}`;
}
</script>

<template>
  <div
    class="image-viewer chat-file-drop-overlay"
    role="dialog"
    aria-modal="true"
    aria-label="图片查看器"
    @click="emit('close')"
    @wheel="onWheel"
  >
    <figure class="image-viewer__figure">
      <div class="image-viewer__stage">
        <img
          class="image-viewer__image"
          :src="image.src"
          :alt="image.name || '图片'"
          :style="imageStyle"
          draggable="false"
          @load="onImageLoad"
          @click.stop
          @pointerdown="onImagePointerDown"
          @pointermove="onImagePointerMove"
          @pointerup="clearDrag"
          @pointercancel="clearDrag"
          @lostpointercapture="clearDrag"
        >
      </div>
      <figcaption class="image-viewer__meta" @click.stop>
        <span v-if="image.name" class="image-viewer__name">{{ image.name }}</span>
        <span v-if="metadataText">{{ metadataText }}</span>
      </figcaption>
    </figure>
  </div>
</template>
