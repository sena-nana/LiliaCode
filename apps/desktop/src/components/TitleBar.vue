<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useRoute } from "vue-router";
import {
  Minus,
  Square,
  Copy,
  X,
  ChevronRight,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
} from "lucide-vue-next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  getOrphanConversation,
  getTask,
} from "../services/tasksStore";
import { getProject } from "../services/projectsStore";
import { toggleChatSidebar, useChatSidebar } from "../composables/useChatSidebar";

defineProps<{
  leftSidebarCollapsed?: boolean;
  sidebarTogglesDisabled?: boolean;
}>();

defineEmits<{
  toggleLeftSidebar: [];
}>();

const route = useRoute();

function paramAsString(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) return value[0];
  return value;
}

const projectId = computed(() => paramAsString(route.params.projectId));
const taskId = computed(() => paramAsString(route.params.taskId));
const chatSidebar = useChatSidebar();

const canToggleChatSidebar = computed(() =>
  !!taskId.value &&
  (route.path.startsWith("/chats/") || !!projectId.value) &&
  chatSidebar.panels.value.length > 0,
);

interface Crumb {
  text: string;
  muted?: boolean;
}

/**
 * 面包屑：根据路由生成 1-2 段。收集箱被当成「虚拟项目」，让项目对话和收集箱
 * 对话走同一套两段结构，prefixKey 在两者之间切换时自然触发过渡。
 */
const crumbs = computed<Crumb[]>(() => {
  const pid = projectId.value;
  const tid = taskId.value;

  if (pid && tid) {
    const proj = getProject(pid);
    const task = getTask(pid, tid);
    return [
      { text: proj?.name ?? "未知项目", muted: true },
      { text: task?.title ?? "未知任务" },
    ];
  }

  if (route.path.startsWith("/chats/") && tid) {
    const orphan = getOrphanConversation(tid);
    return [
      { text: "收集箱", muted: true },
      { text: orphan?.title ?? "新对话" },
    ];
  }

  if (route.path === "/settings") return [{ text: "设置" }];
  if (route.path === "/plugins") return [{ text: "插件 / 技能" }];

  return [{ text: "Lilia" }];
});

const leafCrumb = computed<Crumb | null>(() => {
  const arr = crumbs.value;
  return arr.length > 0 ? arr[arr.length - 1] : null;
});

const nonLeafCrumbs = computed<Crumb[]>(() => crumbs.value.slice(0, -1));

/**
 * 前缀 key 同项目内切 session 时不变；叶子 key 含前缀，避免不同项目同名 task
 * 误判为同一段，仍能触发叶子段的过渡。
 */
const prefixKey = computed(() =>
  nonLeafCrumbs.value.map((c) => c.text).join("|"),
);

const leafKey = computed(() => {
  return `${prefixKey.value}|${leafCrumb.value?.text ?? ""}`;
});

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

function onToggleChatSidebar() {
  toggleChatSidebar();
}
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <div class="titlebar__left-controls">
      <button
        type="button"
        class="titlebar__btn titlebar__left-sidebar-btn"
        :aria-label="leftSidebarCollapsed ? '展开左侧栏' : '折叠左侧栏'"
        :title="leftSidebarCollapsed ? '展开左侧栏' : '折叠左侧栏'"
        :aria-pressed="leftSidebarCollapsed"
        :disabled="sidebarTogglesDisabled"
        @click="$emit('toggleLeftSidebar')"
      >
        <PanelLeftOpen
          v-if="leftSidebarCollapsed"
          :size="15"
          aria-hidden="true"
        />
        <PanelLeftClose
          v-else
          :size="15"
          aria-hidden="true"
        />
      </button>
    </div>

    <div class="titlebar__crumbs" data-tauri-drag-region>
      <!-- 非叶子段（项目名等）：同项目内 prefixKey 不变 → 不动；跨项目变 → 走过渡。 -->
      <!-- 每个子节点都标 data-tauri-drag-region：Tauri v2 只看 event.target 自身，
           不上溯祖先，否则 span/SVG 会拦截 mousedown 让面包屑拖不动。 -->
      <Transition name="tb-crumbs" mode="out-in">
        <span
          v-if="nonLeafCrumbs.length > 0"
          :key="prefixKey"
          class="titlebar__crumbs-prefix"
          data-tauri-drag-region
        >
          <template v-for="(c, i) in nonLeafCrumbs" :key="i">
            <span
              class="titlebar__crumb"
              :class="{ 'titlebar__crumb--muted': c.muted }"
              :title="c.text"
              data-tauri-drag-region
            >{{ c.text }}</span>
            <ChevronRight
              class="titlebar__crumb-sep"
              :size="12"
              aria-hidden="true"
              data-tauri-drag-region
            />
          </template>
        </span>
      </Transition>

      <!-- 叶子段：key=拼接文本，文本变就触发淡入淡出。 -->
      <Transition name="tb-crumbs" mode="out-in">
        <span
          v-if="leafCrumb"
          :key="leafKey"
          class="titlebar__crumb"
          :class="{
            'titlebar__crumb--muted': leafCrumb.muted,
            'titlebar__crumb--leaf': !leafCrumb.muted,
          }"
          :title="leafCrumb.text"
          data-tauri-drag-region
        >{{ leafCrumb.text }}</span>
      </Transition>
    </div>

    <div class="titlebar__controls">
      <button
        v-if="canToggleChatSidebar"
        type="button"
        class="titlebar__btn titlebar__chat-sidebar-btn"
        :aria-label="chatSidebar.state.open ? '关闭对话侧栏' : '打开对话侧栏'"
        :title="chatSidebar.state.open ? '关闭对话侧栏' : '打开对话侧栏'"
        :aria-pressed="chatSidebar.state.open"
        :disabled="sidebarTogglesDisabled"
        @click="onToggleChatSidebar"
      >
        <PanelRightClose
          v-if="chatSidebar.state.open"
          :size="15"
          aria-hidden="true"
        />
        <PanelRightOpen
          v-else
          :size="15"
          aria-hidden="true"
        />
      </button>
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
