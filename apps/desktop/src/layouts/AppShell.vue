<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import ArrowLeft from "@lucide/vue/dist/esm/icons/arrow-left.mjs";
import SettingsSidebar from "@lilia/ui/layouts/SettingsSidebar";
import {
  SETTINGS_TABS,
  beginPerfStage,
  cancelIdleRun,
  createLazyLoadState,
  installPerfObservers,
  measurePerfAsync,
  normalizeSettingsTab,
  runWhenIdle,
  scheduleAfterPaint,
  useShellSidebar,
} from "@lilia/ui";
import TitleBar from "../components/TitleBar.vue";
import { preloadTaskDetailCore } from "../pages/taskDetail/taskDetailLazyLoaders";

const secondaryPanelLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "app-shell.secondary-panel.load",
    async () => (await import("./SecondaryPanel.vue")).default,
  )
);
const SecondaryPanel = defineAsyncComponent({
  suspensible: false,
  loader: () => secondaryPanelLoad.load(),
});

const route = useRoute();
const router = useRouter();
const isSettingsRoute = computed(() => route.path === "/settings");
const activeSettingsTab = computed(() => normalizeSettingsTab(route.query.tab));
const isAutomationsRoute = computed(() => route.path === "/automations");
const isSidebarReplacementRoute = computed(() =>
  isSettingsRoute.value || isAutomationsRoute.value
);
const sidebar = useShellSidebar(isSidebarReplacementRoute);
const effectiveSidebarCollapsed = sidebar.effectiveCollapsed;
const previousSidebarReplacementRoute = ref<string | null>(null);

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
type PendingRoutePaintMeasure = {
  cancelPaint: () => void;
  stage: ReturnType<typeof beginPerfStage>;
};
let pendingRoutePaintMeasure: PendingRoutePaintMeasure | null = null;
let routePaintSeq = 0;
let cancelTaskDetailPreloadPaint: (() => void) | null = null;
let taskDetailPreloadIdleHandle: number | null = null;

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

function cancelPendingRoutePaintMeasure(stage: "cancelled" | "replaced") {
  pendingRoutePaintMeasure?.cancelPaint();
  pendingRoutePaintMeasure?.stage.end(stage);
  pendingRoutePaintMeasure = null;
}

watch(
  () => route.fullPath,
  (path) => {
    cancelPendingRoutePaintMeasure("replaced");
    const seq = ++routePaintSeq;
    const stage = beginPerfStage("route.paint", {
      detail: path,
      feature: "route.paint",
      id: path,
      route: path,
      seq,
    });
    const cancelPaint = scheduleAfterPaint(() => {
      if (pendingRoutePaintMeasure?.cancelPaint === cancelPaint) {
        pendingRoutePaintMeasure = null;
      }
      stage.end("paint");
    });
    pendingRoutePaintMeasure = { cancelPaint, stage };
  },
  { immediate: true },
);

onMounted(() => {
  cancelTaskDetailPreloadPaint = scheduleAfterPaint(() => {
    cancelTaskDetailPreloadPaint = null;
    taskDetailPreloadIdleHandle = runWhenIdle(() => {
      taskDetailPreloadIdleHandle = null;
      void preloadTaskDetailCore().catch(() => undefined);
    });
  });
});

onBeforeUnmount(() => {
  cancelPendingRoutePaintMeasure("cancelled");
  cancelTaskDetailPreloadPaint?.();
  cancelTaskDetailPreloadPaint = null;
  if (taskDetailPreloadIdleHandle !== null) {
    cancelIdleRun(taskDetailPreloadIdleHandle);
    taskDetailPreloadIdleHandle = null;
  }
  removeBeforeEach();
});
</script>

<template>
  <div
    class="shell"
    data-agent-id="app.shell"
    :class="{
      'is-resizing': sidebar.isResizing.value,
      'is-sidebar-collapsed': effectiveSidebarCollapsed,
      'is-settings-mode': isSettingsRoute,
      'is-automations-mode': isAutomationsRoute,
    }"
    :style="{ '--sidebar-width': sidebar.widthStyle.value }"
  >
    <TitleBar
      :left-sidebar-collapsed="effectiveSidebarCollapsed"
      :sidebar-toggles-disabled="isSidebarReplacementRoute"
      @toggle-left-sidebar="sidebar.toggleCollapsed"
    />
    <SettingsSidebar
      v-if="isSettingsRoute"
      :tabs="SETTINGS_TABS"
      :active-key="activeSettingsTab"
      :return-to="sidebarReturnTo"
    />
    <aside
      v-else-if="isAutomationsRoute"
      class="secondary-panel automations-sidebar"
      aria-label="自动化列表"
      data-agent-id="automations.sidebar"
    >
      <div class="settings-sidebar__head">
        <button
          type="button"
          class="settings-sidebar__back"
          data-agent-id="automations.sidebar.back"
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
      data-agent-id="app.sidebar.resizer"
      role="separator"
      aria-orientation="vertical"
      :aria-disabled="effectiveSidebarCollapsed ? 'true' : undefined"
      :aria-valuenow="sidebar.width.value"
      :aria-valuemin="sidebar.minWidth"
      :aria-valuemax="sidebar.maxWidth"
      title="拖动调整侧栏宽度（双击恢复默认）"
      @pointerdown="sidebar.startResize"
      @dblclick="sidebar.resetWidth"
    />
    <main class="shell__main" data-agent-id="app.main">
      <RouterView />
    </main>
  </div>
</template>

