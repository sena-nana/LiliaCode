<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import TitleBar from "../components/TitleBar.vue";
import SecondaryPanel from "./SecondaryPanel.vue";
import SettingsSidebar from "./SettingsSidebar.vue";
import { useResizablePane } from "../composables/useResizablePane";

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
const effectiveSidebarCollapsed = computed(
  () => !isSettingsRoute.value && sidebarCollapsed.value,
);
const previousSettingsRoute = ref<string | null>(null);
const sidebarWidth = useResizablePane({
  storageKey: WIDTH_STORAGE_KEY,
  minWidth: MIN_WIDTH,
  maxWidth: MAX_WIDTH,
  defaultWidth: DEFAULT_WIDTH,
  edge: "right",
  disabled: effectiveSidebarCollapsed,
});

function toggleSidebarCollapsed() {
  if (isSettingsRoute.value) return;
  sidebarCollapsed.value = !sidebarCollapsed.value;
  writeStorage(COLLAPSED_STORAGE_KEY, sidebarCollapsed.value ? "1" : "0");
}

function isSettingsReturnCandidate(path: string): boolean {
  return path.startsWith("/") &&
    !path.startsWith("/popup/") &&
    !path.startsWith("/settings");
}

const settingsReturnTo = computed(() =>
  previousSettingsRoute.value && isSettingsReturnCandidate(previousSettingsRoute.value)
    ? previousSettingsRoute.value
    : "/",
);

const removeBeforeEach = router.beforeEach((to, from) => {
  if (to.path === "/settings" && isSettingsReturnCandidate(from.fullPath)) {
    previousSettingsRoute.value = from.fullPath;
  }
});

onBeforeUnmount(() => {
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
    }"
    :style="{ '--sidebar-width': effectiveSidebarCollapsed ? '0px' : sidebarWidth.width.value + 'px' }"
  >
    <TitleBar
      :left-sidebar-collapsed="effectiveSidebarCollapsed"
      :sidebar-toggles-disabled="isSettingsRoute"
      @toggle-left-sidebar="toggleSidebarCollapsed"
    />
    <SettingsSidebar
      v-if="isSettingsRoute"
      :return-to="settingsReturnTo"
    />
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
