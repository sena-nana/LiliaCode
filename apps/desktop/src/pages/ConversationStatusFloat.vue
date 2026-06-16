<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  PhysicalPosition,
  PhysicalSize,
} from "@tauri-apps/api/dpi";
import {
  AlertTriangle,
  Blend,
  MessageSquarePlus,
  Pin,
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
import ProviderConnectionBadge from "../components/ProviderConnectionBadge.vue";
import type { UnifiedSidebarConversation } from "../components/sidebar/sidebarTypes";

const ALWAYS_ON_TOP_STORAGE_KEY = "lilia.conversationStatus.alwaysOnTop";
const OPACITY_STORAGE_KEY = "lilia.conversationStatus.opacity";
const GEOMETRY_STORAGE_KEY = "lilia.conversationStatus.geometry";
const TRANSPARENT_BODY_CLASS = "body--transparent-popup";
const MIN_OPACITY = 0.4;
const MAX_OPACITY = 1;
const MIN_WINDOW_WIDTH = 260;
const MIN_WINDOW_HEIGHT = 320;

type StoredWindowGeometry = {
  x: number;
  y: number;
  width: number;
  height: number;
};

const appWindow = getCurrentWindow();
const loaded = ref(false);
const error = ref<string | null>(null);
const alwaysOnTop = ref(readStoredBoolean(ALWAYS_ON_TOP_STORAGE_KEY, false));
const opacity = ref(readStoredOpacity());
const opacityPanelOpen = ref(false);
let geometryUnlisteners: Array<() => void> = [];

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

const opacityCssValue = computed(() => opacity.value.toFixed(2));
const opacityPercentLabel = computed(() => `${Math.round(opacity.value * 100)}%`);

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

function readStoredOpacity(): number {
  try {
    const stored = Number(localStorage.getItem(OPACITY_STORAGE_KEY));
    if (Number.isFinite(stored)) return clampOpacity(stored);
  } catch {
    return 0.96;
  }
  return 0.96;
}

function persistBoolean(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    /* ignore */
  }
}

function persistOpacity(value: number) {
  try {
    localStorage.setItem(OPACITY_STORAGE_KEY, value.toFixed(2));
  } catch {
    /* ignore */
  }
}

function clampOpacity(value: number): number {
  return Math.min(MAX_OPACITY, Math.max(MIN_OPACITY, value));
}

function readStoredGeometry(): StoredWindowGeometry | null {
  try {
    const raw = localStorage.getItem(GEOMETRY_STORAGE_KEY);
    if (!raw) return null;
    const { x, y, width, height } = JSON.parse(raw) as Partial<StoredWindowGeometry>;
    if (
      typeof x !== "number" || !Number.isFinite(x) ||
      typeof y !== "number" || !Number.isFinite(y) ||
      typeof width !== "number" || !Number.isFinite(width) ||
      typeof height !== "number" || !Number.isFinite(height)
    ) return null;
    return {
      x,
      y,
      width: Math.max(MIN_WINDOW_WIDTH, width),
      height: Math.max(MIN_WINDOW_HEIGHT, height),
    };
  } catch {
    return null;
  }
}

async function persistCurrentGeometry() {
  try {
    const [position, size] = await Promise.all([
      appWindow.outerPosition(),
      appWindow.innerSize(),
    ]);
    localStorage.setItem(GEOMETRY_STORAGE_KEY, JSON.stringify({
      x: Math.round(position.x),
      y: Math.round(position.y),
      width: Math.round(Math.max(MIN_WINDOW_WIDTH, size.width)),
      height: Math.round(Math.max(MIN_WINDOW_HEIGHT, size.height)),
    }));
  } catch {
    /* ignore */
  }
}

async function restoreStoredGeometry() {
  const geometry = readStoredGeometry();
  if (!geometry) return;
  try {
    await appWindow.setSize(new PhysicalSize(geometry.width, geometry.height));
    await appWindow.setPosition(new PhysicalPosition(geometry.x, geometry.y));
  } catch {
    /* ignore */
  }
}

async function bindGeometryPersistence() {
  try {
    const [unlistenResize, unlistenMove] = await Promise.all([
      appWindow.onResized(() => {
        void persistCurrentGeometry();
      }),
      appWindow.onMoved(() => {
        void persistCurrentGeometry();
      }),
    ]);
    geometryUnlisteners = [unlistenResize, unlistenMove];
  } catch {
    geometryUnlisteners = [];
  }
}

async function initializeGeometryPersistence() {
  await restoreStoredGeometry();
  await bindGeometryPersistence();
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

function setOpacity(value: number | string) {
  opacity.value = clampOpacity(Number(value));
  persistOpacity(opacity.value);
}

function toggleOpacityPanel() {
  opacityPanelOpen.value = !opacityPanelOpen.value;
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
  document.body.classList.add(TRANSPARENT_BODY_CLASS);
  void initializeGeometryPersistence();
  void applyAlwaysOnTop(alwaysOnTop.value);
  void loadConversations();
});

onUnmounted(() => {
  for (const unlisten of geometryUnlisteners) unlisten();
  geometryUnlisteners = [];
  document.body.classList.remove(TRANSPARENT_BODY_CLASS);
});
</script>

<template>
  <section
    class="conversation-status-float"
    :class="{ 'conversation-status-float--has-banner': error }"
    :style="{ '--conversation-status-opacity': opacityCssValue }"
  >
    <header class="conversation-status-float__titlebar" data-tauri-drag-region>
      <div class="conversation-status-float__pin">
        <button
          type="button"
          class="titlebar__btn"
          :class="{ 'is-active': alwaysOnTop }"
          :aria-pressed="alwaysOnTop"
          :aria-label="alwaysOnTop ? '取消置顶' : '置顶'"
          :title="alwaysOnTop ? '取消置顶' : '置顶'"
          @click="toggleAlwaysOnTop"
        >
          <Pin :size="14" aria-hidden="true" />
        </button>
      </div>

      <div class="conversation-status-float__drag" data-tauri-drag-region>
      </div>

      <div class="conversation-status-float__tools">
        <ProviderConnectionBadge popover-id="conversation-status-provider-popover" />

        <div class="conversation-status-float__opacity">
          <button
            type="button"
            class="titlebar__btn"
            :class="{ 'is-active': opacityPanelOpen }"
            :aria-expanded="opacityPanelOpen"
            aria-controls="conversation-status-opacity-panel"
            :aria-label="`窗口透明度 ${opacityPercentLabel}`"
            :title="`窗口透明度 ${opacityPercentLabel}`"
            @click="toggleOpacityPanel"
          >
            <Blend :size="14" aria-hidden="true" />
          </button>
          <div
            v-if="opacityPanelOpen"
            id="conversation-status-opacity-panel"
            class="conversation-status-float__opacity-panel"
          >
            <input
              type="range"
              class="conversation-status-float__opacity-range"
              :min="MIN_OPACITY"
              max="1"
              step="0.01"
              :value="opacity"
              aria-label="窗口透明度"
              @input="setOpacity(($event.target as HTMLInputElement).value)"
            />
            <span class="conversation-status-float__opacity-value">{{ opacityPercentLabel }}</span>
          </div>
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
