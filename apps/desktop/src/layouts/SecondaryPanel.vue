<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import AlertTriangle from "@lucide/vue/dist/esm/icons/triangle-alert.mjs";
import MessageSquarePlus from "@lucide/vue/dist/esm/icons/message-square-plus.mjs";
import X from "@lucide/vue/dist/esm/icons/x.mjs";
import type { TreeDragKind } from "../composables/useSidebarTreeDrag";
import {
  createDraftOrphan,
} from "../services/tasksStore";
import { useSidebarDisplayMode } from "../composables/useSidebarDisplayMode";
import {
  clearConversationActivityNotice,
  conversationActivityForTask,
  hydrateConversationActivities,
} from "../composables/useConversationActivity";
import { createConversationActivityStagePlan } from "../composables/conversationActivityStages";
import { useSidebarRunningProcesses } from "../composables/useSidebarRunningProcesses";
import SidebarConnectionFooter from "../components/sidebar/SidebarConnectionFooter.vue";
import type { SidebarRunningProcessItem } from "../components/sidebar/sidebarTypes";
import {
  ensureSidebarConversationsLoaded,
  listSidebarConversations,
} from "../services/sidebarConversations";
import { scheduleTaskDetailPreload } from "../router";
import { beginPerfStage, cancelIdleRun, measurePerfAsync, runWhenIdle, scheduleAfterPaint } from "@lilia/ui/diagnostics";
import { createLazyLoadState } from "@lilia/ui/utils/lazyLoadState";

const sidebarSearchLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "sidebar.search.load",
    async () => (await import("../components/sidebar/SidebarSearch.vue")).default,
  )
);
const sidebarUnifiedSectionLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "sidebar.unified-section.load",
    async () => (await import("../components/sidebar/SidebarUnifiedSection.vue")).default,
  )
);
const sidebarRunningProcessesSectionLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "sidebar.running-processes-section.load",
    async () => (await import("../components/sidebar/SidebarRunningProcessesSection.vue")).default,
  )
);
const secondaryPanelGroupedHostLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "sidebar.grouped-host.load",
    async () => (await import("./SecondaryPanelGroupedHost.vue")).default,
  )
);

const SidebarSearch = defineAsyncComponent({
  suspensible: false,
  loader: () => sidebarSearchLoad.load(),
});

const SidebarUnifiedSection = defineAsyncComponent({
  suspensible: false,
  loader: () => sidebarUnifiedSectionLoad.load(),
});

const SidebarRunningProcessesSection = defineAsyncComponent({
  suspensible: false,
  loader: () => sidebarRunningProcessesSectionLoad.load(),
});

const SecondaryPanelGroupedHost = defineAsyncComponent({
  suspensible: false,
  loader: () => secondaryPanelGroupedHostLoad.load(),
});

const router = useRouter();
const route = useRoute();

const projectError = ref<string | null>(null);
const searchActive = ref(false);
const unifiedLoaded = ref(false);
const { sidebarDisplayMode } = useSidebarDisplayMode();
const isUnifiedMode = computed(() => sidebarDisplayMode.value === "unified");
const {
  openRunningProcess,
  stoppingTaskIds,
  stopRunningProcess,
} = useSidebarRunningProcesses({ router, reportError: reportProjectError });
let activityHydrationTimer: number | null = null;
let deferredActivityHydrationTimer: number | null = null;
let mountIdleMeasureHandle: number | null = null;
let cancelDeferredActivityHydrationPaint: (() => void) | null = null;
let activityHydrationSeq = 0;
let disposed = false;

function reportProjectError(message: string) {
  projectError.value = message;
}

function dismissError() {
  projectError.value = null;
}

function cancelConversationActivityHydration() {
  cancelDeferredActivityHydrationPaint?.();
  cancelDeferredActivityHydrationPaint = null;
  if (activityHydrationTimer !== null) {
    cancelIdleRun(activityHydrationTimer);
    activityHydrationTimer = null;
  }
  if (deferredActivityHydrationTimer !== null) {
    cancelIdleRun(deferredActivityHydrationTimer);
    deferredActivityHydrationTimer = null;
  }
  activityHydrationSeq += 1;
}

function scheduleConversationActivityHydration(
  taskIds: string[],
  priorityTaskIds: string[],
  perfName: string,
) {
  if (taskIds.length === 0) return;
  activityHydrationTimer = runWhenIdle(() => {
    activityHydrationTimer = null;
    void measurePerfAsync(
      perfName,
      () => hydrateConversationActivities(taskIds, { priorityTaskIds }),
      { detail: `${priorityTaskIds.length}/${taskIds.length}` },
    );
  });
}

function treeRowStateClass(
  _kind: TreeDragKind,
  _projectId: string | null,
  _taskId: string | null,
) {
  return {};
}

async function loadUnifiedSidebarData() {
  if (disposed) return;
  unifiedLoaded.value = false;
  try {
    await measurePerfAsync(
      "sidebar.unified.load",
      () => ensureSidebarConversationsLoaded(true),
      { detail: route.fullPath },
    );
  } catch (err) {
    if (disposed) return;
    reportProjectError(`加载会话列表失败：${String(err)}`);
  } finally {
    if (disposed) return;
    unifiedLoaded.value = true;
  }
}

const unifiedConversations = computed(() => listSidebarConversations());
const runningProcesses = computed<SidebarRunningProcessItem[]>(() =>
  unifiedConversations.value
    .filter((item) => conversationActivityForTask(item.taskId) === "running")
    .map((item) => ({
      taskId: item.taskId,
      title: item.title,
      projectName: item.projectName,
      route: item.route,
    }))
);

const currentTaskId = computed(() =>
  typeof route.params.taskId === "string" ? route.params.taskId : null,
);

const visibleUnifiedTaskIds = computed(() => (
  unifiedConversations.value.slice(0, 24).map((item) => item.taskId)
));

const unifiedTaskIds = computed(() => unifiedConversations.value.map((item) => item.taskId));

const activityPriorityTaskIds = computed(() => {
  return currentTaskId.value
    ? [currentTaskId.value, ...visibleUnifiedTaskIds.value]
    : visibleUnifiedTaskIds.value;
});

const unifiedActivityHydrationPlan = computed(() => createConversationActivityStagePlan({
  taskIds: unifiedTaskIds.value,
  initialTaskIds: activityPriorityTaskIds.value,
  priorityTaskIds: activityPriorityTaskIds.value,
}));

watch(
  isUnifiedMode,
  (enabled) => {
    if (enabled) {
      void loadUnifiedSidebarData();
    }
  },
  { immediate: true },
);

watch(
  () => route.params.taskId,
  (taskId) => {
    if (typeof taskId === "string") clearConversationActivityNotice(taskId);
  },
  { immediate: true },
);

watch(
  () => [isUnifiedMode.value, unifiedTaskIds.value.join("\x1f")] as const,
  ([enabled]) => {
    cancelConversationActivityHydration();
    if (!enabled) return;
    const plan = unifiedActivityHydrationPlan.value;
    scheduleConversationActivityHydration(
      plan.initialTaskIds,
      plan.initialPriorityTaskIds,
      "sidebar.unified.activity.primary",
    );
    if (plan.deferredTaskIds.length === 0) return;
    const seq = activityHydrationSeq;
    cancelDeferredActivityHydrationPaint = scheduleAfterPaint(() => {
      cancelDeferredActivityHydrationPaint = null;
      if (seq !== activityHydrationSeq) return;
      deferredActivityHydrationTimer = runWhenIdle(() => {
        deferredActivityHydrationTimer = null;
        if (seq !== activityHydrationSeq) return;
        void measurePerfAsync(
          "sidebar.unified.activity.deferred",
          () => hydrateConversationActivities(
            plan.deferredTaskIds,
            { priorityTaskIds: plan.deferredPriorityTaskIds },
          ),
          { detail: `${plan.deferredPriorityTaskIds.length}/${plan.deferredTaskIds.length}` },
        );
      });
    });
  },
  { immediate: true },
);

onMounted(() => {
  disposed = false;
  const stage = beginPerfStage("sidebar.mount", { detail: route.fullPath });
  mountIdleMeasureHandle = runWhenIdle(() => {
    mountIdleMeasureHandle = null;
    stage.end("idle");
  });
});

onBeforeUnmount(() => {
  disposed = true;
  if (mountIdleMeasureHandle !== null) {
    cancelIdleRun(mountIdleMeasureHandle);
    mountIdleMeasureHandle = null;
  }
  cancelConversationActivityHydration();
});

function newChat() {
  const draft = createDraftOrphan();
  router.push(`/chats/${draft.id}`);
}

function onSearchSelect(result: { route: string }) {
  router.push(result.route);
}

function prefetchTaskDetailIntent(detail: string) {
  scheduleTaskDetailPreload(detail);
}
</script>

<template>
  <aside
    class="secondary-panel"
    data-agent-id="sidebar"
  >
    <div class="sb-section sb-section--actions">
      <button
        v-if="!searchActive"
        type="button"
        class="sb-primary-btn"
        data-agent-id="sidebar.new-chat"
        title="新对话"
        aria-label="新对话"
        @mouseenter="prefetchTaskDetailIntent('sidebar:new-chat')"
        @focusin="prefetchTaskDetailIntent('sidebar:new-chat')"
        @click="newChat"
      >
        <MessageSquarePlus :size="15" aria-hidden="true" />
        <span class="sb-primary-btn__label">新对话</span>
      </button>
    <SidebarSearch v-model="searchActive" @select="onSearchSelect" />
    </div>

    <SidebarRunningProcessesSection
      v-if="isUnifiedMode && runningProcesses.length > 0"
      :items="runningProcesses"
      :stopping-task-ids="stoppingTaskIds"
      @open="openRunningProcess"
      @stop="stopRunningProcess"
    />

    <div v-if="isUnifiedMode && projectError" class="sb-banner sb-banner--err">
      <AlertTriangle :size="12" aria-hidden="true" />
      <span class="sb-banner__msg">{{ projectError }}</span>
      <button type="button" class="sb-icon-btn" data-agent-id="sidebar.error.dismiss" @click="dismissError" aria-label="忽略错误">
        <span class="sr-only">忽略错误</span>
        <X :size="12" aria-hidden="true" />
      </button>
    </div>

    <SidebarUnifiedSection
      v-if="isUnifiedMode"
      :conversations="unifiedConversations"
      :loaded="unifiedLoaded"
      :activity-for-task="conversationActivityForTask"
      :tree-row-state-class="treeRowStateClass"
      @error="reportProjectError"
    />

    <SecondaryPanelGroupedHost v-else />

    <SidebarConnectionFooter />
  </aside>
</template>
