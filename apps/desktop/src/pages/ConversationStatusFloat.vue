<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  AlertTriangle,
  MessageSquarePlus,
  Pin,
  PinOff,
  X,
} from "lucide-vue-next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ensureProjectsLoaded, listProjects } from "../services/projectsStore";
import {
  ensureAllProjectTasksLoaded,
  ensureOrphansLoaded,
  listOrphanConversations,
  listProjectConversations,
} from "../services/tasksStore";
import {
  openPopupNewChat,
  openPopupTask,
} from "../services/popupWindows";
import {
  conversationActivityForTask,
  hydrateConversationActivities,
  type ConversationActivity,
} from "../composables/useConversationActivity";
import type { TreeDragKind } from "../composables/useSidebarTreeDrag";
import SidebarUnifiedSection from "../components/sidebar/SidebarUnifiedSection.vue";
import type { UnifiedSidebarConversation } from "../components/sidebar/sidebarTypes";

const ALWAYS_ON_TOP_STORAGE_KEY = "lilia.conversationStatus.alwaysOnTop";
const OPACITY_STORAGE_KEY = "lilia.conversationStatus.opacity";

type StatusOpacity = "1" | "0.96" | "0.92" | "0.84";

const appWindow = getCurrentWindow();
const loaded = ref(false);
const error = ref<string | null>(null);
const alwaysOnTop = ref(readStoredBoolean(ALWAYS_ON_TOP_STORAGE_KEY, false));
const opacity = ref<StatusOpacity>(readStoredOpacity());

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());

const conversations = computed<UnifiedSidebarConversation[]>(() => {
  const rows: UnifiedSidebarConversation[] = [];
  for (const project of projects.value) {
    for (const task of listProjectConversations(project.id)) {
      rows.push({
        task,
        projectId: project.id,
        projectName: project.name,
        route: `/projects/${project.id}/tasks/${task.id}`,
      });
    }
  }
  for (const orphan of orphans.value) {
    rows.push({
      task: orphan,
      projectId: null,
      projectName: "收集箱",
      route: `/chats/${orphan.id}`,
    });
  }
  return rows.sort((a, b) =>
    Number(b.task.pinned) - Number(a.task.pinned) ||
    b.task.createdAt - a.task.createdAt ||
    a.task.id.localeCompare(b.task.id)
  );
});

const taskIds = computed(() => conversations.value.map((item) => item.task.id));

const opacityOptions: Array<{ value: StatusOpacity; label: string }> = [
  { value: "1", label: "100" },
  { value: "0.96", label: "96" },
  { value: "0.92", label: "92" },
  { value: "0.84", label: "84" },
];

function readStoredBoolean(key: string, fallback: boolean): boolean {
  try {
    const value = localStorage.getItem(key);
    if (value === "1") return true;
    if (value === "0") return false;
  } catch {
    return fallback;
  }
  return fallback;
}

function readStoredOpacity(): StatusOpacity {
  try {
    const stored = localStorage.getItem(OPACITY_STORAGE_KEY);
    if (stored === "1" || stored === "0.96" || stored === "0.92" || stored === "0.84") {
      return stored;
    }
  } catch {
    return "0.96";
  }
  return "0.96";
}

function persistBoolean(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    /* ignore */
  }
}

function persistOpacity(value: StatusOpacity) {
  try {
    localStorage.setItem(OPACITY_STORAGE_KEY, value);
  } catch {
    /* ignore */
  }
}

async function loadConversations() {
  loaded.value = false;
  error.value = null;
  try {
    await ensureProjectsLoaded(true);
    await Promise.all([
      ensureAllProjectTasksLoaded(),
      ensureOrphansLoaded(),
    ]);
  } catch (err) {
    error.value = `加载会话状态失败：${String(err)}`;
  } finally {
    loaded.value = true;
  }
}

async function toggleAlwaysOnTop() {
  alwaysOnTop.value = !alwaysOnTop.value;
  await applyAlwaysOnTop(alwaysOnTop.value);
}

async function applyAlwaysOnTop(value: boolean) {
  persistBoolean(ALWAYS_ON_TOP_STORAGE_KEY, value);
  try {
    await appWindow.setAlwaysOnTop(value);
  } catch (err) {
    error.value = `设置窗口置顶失败：${String(err)}`;
  }
}

function setOpacity(value: StatusOpacity) {
  opacity.value = value;
  persistOpacity(value);
}

async function newChat() {
  try {
    await openPopupNewChat();
  } catch (err) {
    error.value = `创建弹出窗口对话失败：${String(err)}`;
  }
}

async function closeWindow() {
  await appWindow.close();
}

async function openConversation(item: UnifiedSidebarConversation) {
  try {
    await openPopupTask(item.task.id, item.projectId);
  } catch (err) {
    error.value = `打开弹出窗口对话失败：${String(err)}`;
  }
}

function taskActivity(taskId: string): ConversationActivity | null {
  return conversationActivityForTask(taskId);
}

function dismissError() {
  error.value = null;
}

function treeRowStateClass(
  _kind: TreeDragKind,
  _projectId: string | null,
  _taskId: string | null,
): Record<string, boolean> {
  return {};
}

watch(
  () => taskIds.value.join("\x1f"),
  () => {
    void hydrateConversationActivities(taskIds.value);
  },
  { immediate: true },
);

onMounted(() => {
  void applyAlwaysOnTop(alwaysOnTop.value);
  void loadConversations();
});
</script>

<template>
  <section
    class="conversation-status-float"
    :class="{ 'conversation-status-float--has-banner': error }"
    :style="{ '--conversation-status-opacity': opacity }"
  >
    <header class="conversation-status-float__titlebar" data-tauri-drag-region>
      <div class="conversation-status-float__heading" data-tauri-drag-region>
        <span class="conversation-status-float__title" data-tauri-drag-region>对话状态</span>
        <span class="conversation-status-float__count" data-tauri-drag-region>{{ conversations.length }}</span>
      </div>

      <div class="conversation-status-float__tools">
        <button
          type="button"
          class="titlebar__btn"
          :class="{ 'is-active': alwaysOnTop }"
          :aria-pressed="alwaysOnTop"
          :aria-label="alwaysOnTop ? '取消置顶' : '置顶'"
          :title="alwaysOnTop ? '取消置顶' : '置顶'"
          @click="toggleAlwaysOnTop"
        >
          <PinOff v-if="alwaysOnTop" :size="14" aria-hidden="true" />
          <Pin v-else :size="14" aria-hidden="true" />
        </button>

        <div class="conversation-status-float__opacity" aria-label="窗口透明度">
          <button
            v-for="option in opacityOptions"
            :key="option.value"
            type="button"
            class="conversation-status-float__opacity-btn"
            :class="{ 'is-active': opacity === option.value }"
            :aria-pressed="opacity === option.value"
            :title="`透明度 ${option.label}%`"
            @click="setOpacity(option.value)"
          >
            {{ option.label }}
          </button>
        </div>

        <button
          type="button"
          class="titlebar__btn"
          aria-label="新对话"
          title="新对话"
          @click="newChat"
        >
          <MessageSquarePlus :size="15" aria-hidden="true" />
        </button>
        <button
          type="button"
          class="titlebar__btn titlebar__btn--danger"
          aria-label="关闭对话悬浮窗"
          title="关闭"
          @click="closeWindow"
        >
          <X :size="15" aria-hidden="true" />
        </button>
      </div>
    </header>

    <div v-if="error" class="conversation-status-float__banner">
      <AlertTriangle :size="13" aria-hidden="true" />
      <span>{{ error }}</span>
      <button type="button" class="sb-icon-btn" aria-label="忽略错误" @click="dismissError">
        <X :size="12" aria-hidden="true" />
      </button>
    </div>

    <SidebarUnifiedSection
      class="conversation-status-float__list"
      :activity-for-task="taskActivity"
      :conversations="conversations"
      hide-header
      :loaded="loaded"
      readonly
      :tree-row-state-class="treeRowStateClass"
      @error="error = $event"
      @open="openConversation"
    />
  </section>
</template>
