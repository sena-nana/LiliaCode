<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  ChevronRight,
  ExternalLink,
  MessageSquarePlus,
  X,
} from "lucide-vue-next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getProject } from "../services/projectsStore";
import {
  getOrphanConversation,
  getTask,
  resolveConversationRouteState,
} from "../services/tasksStore";
import { focusMainWindow } from "../services/popupWindows";

const route = useRoute();
const router = useRouter();
const appWindow = getCurrentWindow();

function paramAsString(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) return value[0];
  return value;
}

const projectId = computed(() => paramAsString(route.params.projectId));
const taskId = computed(() => paramAsString(route.params.taskId));

const routeState = computed(() =>
  resolveConversationRouteState(projectId.value, taskId.value),
);

interface Crumb {
  text: string;
  muted?: boolean;
}

const crumbs = computed<Crumb[]>(() => {
  const pid = projectId.value;
  const tid = taskId.value;

  if (pid) {
    const project = getProject(pid);
    if (!tid || routeState.value.isLiveDraft || routeState.value.isLostDraft) {
      return [
        { text: project?.name ?? "未知项目", muted: true },
        { text: "新对话" },
      ];
    }
    const task = getTask(pid, tid);
    return [
      { text: project?.name ?? "未知项目", muted: true },
      { text: task?.title ?? "未知任务" },
    ];
  }

  if (tid) {
    const orphan = getOrphanConversation(tid);
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
  await appWindow.close();
}

async function onNewChat() {
  await router.replace(popupNewRoute());
}

async function onFocusMain() {
  await focusMainWindow(mainRouteForPopup());
  await appWindow.close();
}
</script>

<template>
  <header class="popup-titlebar" data-tauri-drag-region>
    <div class="popup-titlebar__controls popup-titlebar__controls--left">
      <button
        type="button"
        class="titlebar__btn titlebar__btn--danger"
        aria-label="关闭弹出窗口"
        title="关闭"
        @click="onClose"
      >
        <X :size="15" aria-hidden="true" />
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
        type="button"
        class="titlebar__btn"
        aria-label="新对话"
        title="新对话"
        @click="onNewChat"
      >
        <MessageSquarePlus :size="15" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="titlebar__btn"
        aria-label="回到主窗口"
        title="回到主窗口"
        @click="onFocusMain"
      >
        <ExternalLink :size="14" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>
