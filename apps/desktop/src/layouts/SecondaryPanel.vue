<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { AlertTriangle, MessageSquarePlus, X } from "lucide-vue-next";
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
import SidebarConnectionFooter from "../components/sidebar/SidebarConnectionFooter.vue";
import {
  ensureSidebarConversationsLoaded,
  listSidebarConversations,
} from "../services/sidebarConversations";
import { scheduleTaskDetailPreload } from "../router";
import {
  beginPerfStage,
  cancelIdleRun,
  installPerfObservers,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../utils/perf";

const SidebarSearch = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "sidebar.search.load",
    async () => (await import("../components/sidebar/SidebarSearch.vue")).default,
  ),
});

const SidebarUnifiedSection = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "sidebar.unified-section.load",
    async () => (await import("../components/sidebar/SidebarUnifiedSection.vue")).default,
  ),
});

const SecondaryPanelGroupedHost = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "sidebar.grouped-host.load",
    async () => (await import("./SecondaryPanelGroupedHost.vue")).default,
  ),
});

const router = useRouter();
const route = useRoute();

const projectError = ref<string | null>(null);
const searchActive = ref(false);
const unifiedLoaded = ref(false);
const { sidebarDisplayMode } = useSidebarDisplayMode();
const isUnifiedMode = computed(() => sidebarDisplayMode.value === "unified");
let activityHydrationTimer: number | null = null;
let deferredActivityHydrationTimer: number | null = null;
let activityHydrationSeq = 0;

function reportProjectError(message: string) {
  projectError.value = message;
}

function dismissError() {
  projectError.value = null;
}

function cancelConversationActivityHydration() {
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
  unifiedLoaded.value = false;
  try {
    await measurePerfAsync(
      "sidebar.unified.load",
      () => ensureSidebarConversationsLoaded(true),
      { detail: route.fullPath },
    );
  } catch (err) {
    reportProjectError(`加载会话列表失败：${String(err)}`);
  } finally {
    unifiedLoaded.value = true;
  }
}

const unifiedConversations = computed(() => listSidebarConversations());

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
    scheduleAfterPaint(() => {
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
  installPerfObservers();
  const stage = beginPerfStage("sidebar.mount", { detail: route.fullPath });
  runWhenIdle(() => stage.end("idle"));
});

onBeforeUnmount(() => {
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
  >
    <div class="sb-section sb-section--actions">
      <button
        v-if="!searchActive"
        type="button"
        class="sb-primary-btn"
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

    <div v-if="isUnifiedMode && projectError" class="sb-banner sb-banner--err">
      <AlertTriangle :size="12" aria-hidden="true" />
      <span class="sb-banner__msg">{{ projectError }}</span>
      <button type="button" class="sb-icon-btn" @click="dismissError" aria-label="忽略错误">
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
