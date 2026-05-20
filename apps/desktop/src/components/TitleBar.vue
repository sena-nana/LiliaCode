<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { Minus, Square, Copy, X } from "lucide-vue-next";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface Props {
  title?: string;
}
withDefaults(defineProps<Props>(), { title: "Lilia" });

const isMaximized = ref(false);
const appWindow = getCurrentWindow();
let unlisten: (() => void) | null = null;

async function syncMaximized() {
  try {
    isMaximized.value = await appWindow.isMaximized();
  } catch {
    isMaximized.value = false;
  }
}

onMounted(async () => {
  await syncMaximized();
  unlisten = await appWindow.onResized(() => {
    void syncMaximized();
  });
});

onUnmounted(() => {
  unlisten?.();
});

async function onMinimize() {
  await appWindow.minimize();
}

async function onToggleMaximize() {
  await appWindow.toggleMaximize();
  await syncMaximized();
}

async function onClose() {
  await appWindow.close();
}
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <div class="titlebar__brand" data-tauri-drag-region>{{ title }}</div>
    <div class="titlebar__spacer" data-tauri-drag-region></div>
    <div class="titlebar__controls">
      <button
        type="button"
        class="titlebar__btn"
        aria-label="最小化"
        @click="onMinimize"
      >
        <Minus :size="14" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="titlebar__btn"
        :aria-label="isMaximized ? '还原' : '最大化'"
        @click="onToggleMaximize"
      >
        <Copy v-if="isMaximized" :size="13" aria-hidden="true" />
        <Square v-else :size="13" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="titlebar__btn titlebar__btn--danger"
        aria-label="关闭"
        @click="onClose"
      >
        <X :size="15" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>
