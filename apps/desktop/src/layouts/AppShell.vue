<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { RouterView } from "vue-router";
import TitleBar from "../components/TitleBar.vue";
import SecondaryPanel from "./SecondaryPanel.vue";

/** 侧栏宽度的硬约束：太窄项目名糊成一团，太宽主区被挤掉。 */
const MIN_WIDTH = 180;
const MAX_WIDTH = 480;
const DEFAULT_WIDTH = 220;
const WIDTH_STORAGE_KEY = "lilia.sidebarWidth";
const COLLAPSED_STORAGE_KEY = "lilia.sidebarCollapsed";

function clamp(n: number) {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, n));
}

function readStorage(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* ignore */
  }
}

function loadSidebarWidth(): number {
  const raw = readStorage(WIDTH_STORAGE_KEY);
  const n = raw ? Number.parseFloat(raw) : NaN;
  return Number.isFinite(n) ? clamp(n) : DEFAULT_WIDTH;
}

function loadSidebarCollapsed(): boolean {
  return readStorage(COLLAPSED_STORAGE_KEY) === "1";
}

const sidebarWidth = ref<number>(loadSidebarWidth());
const sidebarCollapsed = ref(loadSidebarCollapsed());
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
  writeStorage(WIDTH_STORAGE_KEY, String(sidebarWidth.value));
}

function startResize(e: PointerEvent) {
  if (sidebarCollapsed.value) return;
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
  writeStorage(WIDTH_STORAGE_KEY, String(DEFAULT_WIDTH));
}

function toggleSidebarCollapsed() {
  sidebarCollapsed.value = !sidebarCollapsed.value;
  writeStorage(COLLAPSED_STORAGE_KEY, sidebarCollapsed.value ? "1" : "0");
}

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
});
</script>

<template>
  <div
    class="shell"
    :class="{ 'is-resizing': isResizing, 'is-sidebar-collapsed': sidebarCollapsed }"
    :style="{ '--sidebar-width': sidebarCollapsed ? '0px' : sidebarWidth + 'px' }"
  >
    <TitleBar
      :left-sidebar-collapsed="sidebarCollapsed"
      @toggle-left-sidebar="toggleSidebarCollapsed"
    />
    <SecondaryPanel />
    <div
      class="shell__resizer"
      role="separator"
      aria-orientation="vertical"
      :aria-disabled="sidebarCollapsed ? 'true' : undefined"
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
