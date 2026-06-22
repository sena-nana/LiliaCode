<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, ref, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import { ArrowLeft } from "lucide-vue-next";
import TitleBar from "../components/TitleBar.vue";
import { useResizablePane } from "../composables/useResizablePane";
import {
  beginPerfStage,
  installPerfObservers,
  measurePerfAsync,
  scheduleAfterPaint,
} from "../utils/perf";
import { createLazyLoadState } from "../utils/lazyLoadState";

const secondaryPanelLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "app-shell.secondary-panel.load",
    async () => (await import("./SecondaryPanel.vue")).default,
  )
);
const settingsSidebarLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "app-shell.settings-sidebar.load",
    async () => (await import("./SettingsSidebar.vue")).default,
  )
);

const SecondaryPanel = defineAsyncComponent({
  suspensible: false,
  loader: () => secondaryPanelLoad.load(),
});

const SettingsSidebar = defineAsyncComponent({
  suspensible: false,
  loader: () => settingsSidebarLoad.load(),
});

/** 侧栏宽度的硬约束：太窄项目名糊成一团，太宽主区被挤掉。 */
const MIN_WIDTH = 180;
const MAX_WIDTH = 480;
const DEFAULT_WIDTH = 220;
const WIDTH_STORAGE_KEY = "lilia.sidebarWidth";
const COLLAPSED_STORAGE_KEY = "lilia.sidebarCollapsed";

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

function loadSidebarCollapsed(): boolean {
  return readStorage(COLLAPSED_STORAGE_KEY) === "1";
}

const route = useRoute();
const router = useRouter();
const sidebarCollapsed = ref(loadSidebarCollapsed());
const isSettingsRoute = computed(() => route.path === "/settings");
const isAutomationsRoute = computed(() => route.path === "/automations");
const isSidebarReplacementRoute = computed(() =>
  isSettingsRoute.value || isAutomationsRoute.value
);
const effectiveSidebarCollapsed = computed(
  () => !isSidebarReplacementRoute.value && sidebarCollapsed.value,
);
const previousSidebarReplacementRoute = ref<string | null>(null);
const sidebarWidth = useResizablePane({
  storageKey: WIDTH_STORAGE_KEY,
  minWidth: MIN_WIDTH,
  maxWidth: MAX_WIDTH,
  defaultWidth: DEFAULT_WIDTH,
  edge: "right",
  disabled: effectiveSidebarCollapsed,
});

function toggleSidebarCollapsed() {
  if (isSidebarReplacementRoute.value) return;
  sidebarCollapsed.value = !sidebarCollapsed.value;
  writeStorage(COLLAPSED_STORAGE_KEY, sidebarCollapsed.value ? "1" : "0");
}

function isSidebarReturnCandidate(path: string): boolean {
  return path.startsWith("/") &&
    !path.startsWith("/popup/") &&
    !path.startsWith("/settings") &&
    !path.startsWith("/automations");
}

const sidebarReturnTo = computed(() =>
  previousSidebarReplacementRoute.value &&
    isSidebarReturnCandidate(previousSidebarReplacementRoute.value)
    ? previousSidebarReplacementRoute.value
    : "/",
);
let cancelRoutePaintMeasure: (() => void) | null = null;

const removeBeforeEach = router.beforeEach((to, from) => {
  if (
    (to.path === "/settings" || to.path === "/automations") &&
    isSidebarReturnCandidate(from.fullPath)
  ) {
    previousSidebarReplacementRoute.value = from.fullPath;
  }
});

function goBackFromAutomation() {
  router.push(sidebarReturnTo.value);
}

installPerfObservers();

watch(
  () => route.fullPath,
  (path) => {
    cancelRoutePaintMeasure?.();
    const stage = beginPerfStage("route.paint", { detail: path });
    const cancelPaint = scheduleAfterPaint(() => {
      if (cancelRoutePaintMeasure === cancelPaint) {
        cancelRoutePaintMeasure = null;
      }
      stage.end("paint");
    });
    cancelRoutePaintMeasure = cancelPaint;
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  cancelRoutePaintMeasure?.();
  cancelRoutePaintMeasure = null;
  removeBeforeEach();
});
</script>

<template>
  <div
    class="shell"
    :class="{
      'is-resizing': sidebarWidth.isResizing.value,
      'is-sidebar-collapsed': effectiveSidebarCollapsed,
      'is-settings-mode': isSettingsRoute,
      'is-automations-mode': isAutomationsRoute,
    }"
    :style="{ '--sidebar-width': effectiveSidebarCollapsed ? '0px' : sidebarWidth.width.value + 'px' }"
  >
    <TitleBar
      :left-sidebar-collapsed="effectiveSidebarCollapsed"
      :sidebar-toggles-disabled="isSidebarReplacementRoute"
      @toggle-left-sidebar="toggleSidebarCollapsed"
    />
    <SettingsSidebar
      v-if="isSettingsRoute"
      :return-to="sidebarReturnTo"
    />
    <aside
      v-else-if="isAutomationsRoute"
      class="secondary-panel automations-sidebar"
      aria-label="自动化列表"
    >
      <div class="settings-sidebar__head">
        <button
          type="button"
          class="settings-sidebar__back"
          aria-label="返回"
          title="返回"
          @click="goBackFromAutomation"
        >
          <ArrowLeft :size="15" aria-hidden="true" />
          <span>返回</span>
        </button>
        <div id="automation-sidebar-actions" />
      </div>
      <div id="automation-sidebar-host" class="automations-sidebar__host" />
    </aside>
    <SecondaryPanel v-else />
    <div
      class="shell__resizer"
      role="separator"
      aria-orientation="vertical"
      :aria-disabled="effectiveSidebarCollapsed ? 'true' : undefined"
      :aria-valuenow="sidebarWidth.width.value"
      :aria-valuemin="MIN_WIDTH"
      :aria-valuemax="MAX_WIDTH"
      title="拖动调整侧栏宽度（双击恢复默认）"
      @pointerdown="sidebarWidth.startResize"
      @dblclick="sidebarWidth.resetWidth"
    />
    <main class="shell__main">
      <RouterView />
    </main>
  </div>
</template>
