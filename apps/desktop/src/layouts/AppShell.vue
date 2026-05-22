<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { RouterView } from "vue-router";
import TitleBar from "../components/TitleBar.vue";
import SecondaryPanel from "./SecondaryPanel.vue";

/** 侧栏宽度的硬约束：太窄项目名糊成一团，太宽主区被挤掉。 */
const MIN_WIDTH = 180;
const MAX_WIDTH = 480;
const DEFAULT_WIDTH = 220;
const STORAGE_KEY = "lilia.sidebarWidth";

function clamp(n: number) {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, n));
}

function loadInitial(): number {
  const raw = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  const n = raw ? Number.parseFloat(raw) : NaN;
  return Number.isFinite(n) ? clamp(n) : DEFAULT_WIDTH;
}

const sidebarWidth = ref<number>(loadInitial());
const isResizing = ref(false);

let startX = 0;
let startWidth = 0;

function onPointerMove(e: PointerEvent) {
  const next = clamp(startWidth + (e.clientX - startX));
  sidebarWidth.value = next;
}

function onPointerUp(e: PointerEvent) {
  isResizing.value = false;
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  (e.target as Element | null)?.releasePointerCapture?.(e.pointerId);
  try {
    localStorage.setItem(STORAGE_KEY, String(sidebarWidth.value));
  } catch {
    /* 隐私模式 / 配额满：忽略。 */
  }
}

function startResize(e: PointerEvent) {
  if (e.button !== 0) return;
  e.preventDefault();
  isResizing.value = true;
  startX = e.clientX;
  startWidth = sidebarWidth.value;
  (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
}

function resetWidth() {
  sidebarWidth.value = DEFAULT_WIDTH;
  try {
    localStorage.setItem(STORAGE_KEY, String(DEFAULT_WIDTH));
  } catch {
    /* ignore */
  }
}

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
});
</script>

<template>
  <div
    class="shell"
    :class="{ 'is-resizing': isResizing }"
    :style="{ '--sidebar-width': sidebarWidth + 'px' }"
  >
    <TitleBar />
    <SecondaryPanel />
    <div
      class="shell__resizer"
      role="separator"
      aria-orientation="vertical"
      :aria-valuenow="sidebarWidth"
      :aria-valuemin="MIN_WIDTH"
      :aria-valuemax="MAX_WIDTH"
      title="拖动调整侧栏宽度（双击恢复默认）"
      @pointerdown="startResize"
      @dblclick="resetWidth"
    />
    <main class="shell__main">
      <RouterView />
    </main>
  </div>
</template>
