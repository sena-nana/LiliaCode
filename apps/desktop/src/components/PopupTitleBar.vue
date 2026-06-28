<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  ChevronRight,
  ExternalLink,
  MessageSquarePlus,
  RefreshCw,
  X,
} from "@lucide/vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invalidateConversationContextSnapshot } from "../services/conversationContextInvalidation";
import { focusMainWindow } from "../services/popupWindows";
import { createLazyLoadState } from "../utils/lazyLoadState";
import { measurePerfAsync } from "../utils/perf";

const route = useRoute();
const router = useRouter();
const appWindow = getCurrentWindow();

interface PopupRouteState {
  isDraftRoute: boolean;
  isLiveDraft: boolean;
  isLostDraft: boolean;
}

interface PopupStoreBindings {
  getProject: (projectId: string) => { name: string } | undefined;
  getOrphanConversation: (taskId: string) => { title: string } | undefined;
  getTask: (projectId: string, taskId: string) => { title: string } | undefined;
  resolveConversationRouteState: (
    projectId: string | null | undefined,
    taskId: string | null | undefined,
  ) => PopupRouteState;
}

const popupStores = shallowRef<PopupStoreBindings | null>(null);
let disposed = false;
let popupStoresSeq = 0;

function paramAsString(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) return value[0];
  return value;
}

const projectId = computed(() => paramAsString(route.params.projectId));
const taskId = computed(() => paramAsString(route.params.taskId));
const popupStoresLoad = createLazyLoadState<PopupStoreBindings>(() =>
  measurePerfAsync(
    "popup-titlebar.stores.load",
    async () => {
      const [projectsStore, tasksStore] = await Promise.all([
        import("../services/projectsStore"),
        import("../services/tasksStore"),
      ]);
      return {
        getProject: projectsStore.getProject,
        getOrphanConversation: tasksStore.getOrphanConversation,
        getTask: tasksStore.getTask,
        resolveConversationRouteState: tasksStore.resolveConversationRouteState,
      };
    },
    { detail: route.fullPath },
  )
);
const popupStoresStatus = popupStoresLoad.status;

function inferDraftRouteState(
  projectIdValue: string | undefined,
  taskIdValue: string | undefined,
): PopupRouteState {
  if (!taskIdValue) {
    return {
      isDraftRoute: false,
      isLiveDraft: false,
      isLostDraft: false,
    };
  }
  const isDraftRoute = projectIdValue
    ? taskIdValue.startsWith("t-draft-")
    : taskIdValue.startsWith("o-draft-");
  return {
    isDraftRoute,
    isLiveDraft: false,
    isLostDraft: false,
  };
}

function ensurePopupStoresLoaded(): Promise<PopupStoreBindings> {
  if (popupStores.value) return Promise.resolve(popupStores.value);
  const seq = ++popupStoresSeq;
  return popupStoresLoad.load().then((bindings) => {
    if (!disposed && seq === popupStoresSeq) {
      popupStores.value = bindings;
    }
    return bindings;
  });
}

function retryPopupStoresLoad() {
  if (disposed) return;
  void ensurePopupStoresLoaded().catch((err) => {
    console.error("[popup-titlebar] retry load stores failed", err);
  });
}

watch(
  () => [projectId.value, taskId.value] as const,
  ([projectIdValue, taskIdValue]) => {
    if (!projectIdValue && !taskIdValue) return;
    void ensurePopupStoresLoaded().catch((err) => {
      console.error("[popup-titlebar] load stores failed", err);
    });
  },
  { immediate: true },
);

const routeState = computed(() =>
  popupStores.value?.resolveConversationRouteState(projectId.value, taskId.value) ??
    inferDraftRouteState(projectId.value, taskId.value),
);

interface Crumb {
  text: string;
  muted?: boolean;
}

const crumbs = computed<Crumb[]>(() => {
  const pid = projectId.value;
  const tid = taskId.value;
  const stores = popupStores.value;

  if (pid) {
    const project = stores?.getProject(pid);
    if (!tid || routeState.value.isLiveDraft || routeState.value.isLostDraft) {
      return [
        { text: project?.name ?? "未知项目", muted: true },
        { text: "新对话" },
      ];
    }
    const task = stores?.getTask(pid, tid);
    return [
      { text: project?.name ?? "未知项目", muted: true },
      { text: task?.title ?? "未知任务" },
    ];
  }

  if (tid) {
    const orphan = stores?.getOrphanConversation(tid);
    return [
      { text: "收集箱", muted: true },
      {
        text: routeState.value.isLiveDraft || routeState.value.isLostDraft
          ? "新对话"
          : orphan?.title ?? "新对话",
      },
    ];
  }

  return [{ text: "新对话" }];
});

function popupNewRoute(): string {
  return projectId.value ? `/popup/projects/${projectId.value}/new` : "/popup/chats/new";
}

function mainRouteForPopup(): string {
  const pid = projectId.value;
  const tid = taskId.value;
  if (pid && tid && !routeState.value.isLiveDraft && !routeState.value.isLostDraft) {
    return `/projects/${pid}/tasks/${tid}`;
  }
  if (pid) return `/projects/${pid}`;
  if (tid && !routeState.value.isLiveDraft && !routeState.value.isLostDraft) {
    return `/chats/${tid}`;
  }
  return "/";
}

async function onClose() {
  if (disposed) return;
  invalidateConversationContextSnapshot("popup-close");
  await appWindow.close();
}

async function onNewChat() {
  if (disposed) return;
  await router.replace(popupNewRoute());
}

async function onFocusMain() {
  if (disposed) return;
  if (taskId.value) {
    try {
      await ensurePopupStoresLoaded();
    } catch (err) {
      console.error("[popup-titlebar] ensure stores before focus main failed", err);
    }
  }
  if (disposed) return;
  invalidateConversationContextSnapshot("popup-close");
  await focusMainWindow(mainRouteForPopup());
  if (disposed) return;
  await appWindow.close();
}

onBeforeUnmount(() => {
  disposed = true;
  popupStoresSeq += 1;
});
</script>

<template>
  <header class="popup-titlebar" data-agent-id="popup.titlebar" data-tauri-drag-region>
    <div class="popup-titlebar__controls popup-titlebar__controls--left">
      <button
        type="button"
        class="titlebar__btn"
        aria-label="回到主窗口"
        title="回到主窗口"
        data-agent-id="popup.titlebar.focus-main"
        @click="onFocusMain"
      >
        <ExternalLink :size="14" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="titlebar__btn"
        aria-label="新对话"
        title="新对话"
        data-agent-id="popup.titlebar.new-chat"
        @click="onNewChat"
      >
        <MessageSquarePlus :size="15" aria-hidden="true" />
      </button>
    </div>

    <div class="popup-titlebar__crumbs" data-tauri-drag-region>
      <template v-for="(crumb, index) in crumbs" :key="`${index}:${crumb.text}`">
        <span
          class="titlebar__crumb"
          :class="{
            'titlebar__crumb--muted': crumb.muted,
            'titlebar__crumb--leaf': !crumb.muted,
          }"
          :title="crumb.text"
          data-tauri-drag-region
        >{{ crumb.text }}</span>
        <ChevronRight
          v-if="index < crumbs.length - 1"
          class="titlebar__crumb-sep"
          :size="12"
          aria-hidden="true"
          data-tauri-drag-region
        />
      </template>
    </div>

    <div class="popup-titlebar__controls popup-titlebar__controls--right">
      <button
        v-if="popupStoresStatus === 'error'"
        type="button"
        class="titlebar__btn"
        aria-label="重试加载弹窗数据"
        title="重试加载弹窗数据"
        data-agent-id="popup.titlebar.retry"
        @click="retryPopupStoresLoad"
      >
        <RefreshCw :size="14" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="titlebar__btn titlebar__btn--danger"
        aria-label="关闭弹出窗口"
        title="关闭"
        data-agent-id="popup.titlebar.close"
        @click="onClose"
      >
        <X :size="15" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>

