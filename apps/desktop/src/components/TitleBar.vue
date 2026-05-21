<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useRoute } from "vue-router";
import { Minus, Square, Copy, X, ChevronRight } from "lucide-vue-next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  getOrphanConversation,
  getProject,
  getTask,
} from "../data/projectsStub";

const route = useRoute();

function paramAsString(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) return value[0];
  return value;
}

const projectId = computed(() => paramAsString(route.params.projectId));
const taskId = computed(() => paramAsString(route.params.taskId));

interface Crumb {
  text: string;
  muted?: boolean;
}

/**
 * 面包屑：根据当前路由生成 1-2 段文字，标题栏永远有"我在哪"的语义。
 * - 项目内任务：[项目名(muted)] / [任务标题]
 * - 收集箱对话：[收集箱(muted)] / [对话标题]——形态跟项目内一致，不再单段。
 * - 设置 / 插件 / 首页：单段顶级标题
 *
 * 收集箱被当成"虚拟项目"，让项目对话与收集箱对话在标题栏走同一套两段结构，
 * prefixKey 在两者之间切换时自然触发过渡，模板里不必再为 orphan 留差异分支。
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
 * 两段独立 Transition 的 key：
 * - prefixKey：非叶子段（项目名等）拼接。同项目内切 session 时 key 不变，
 *   前缀 Transition 不触发；跨项目切换 key 变，触发淡入淡出。
 * - leafKey：包含前缀 + 叶子，避免不同项目同名 task 误判为同一段。同项目内
 *   切 session 时前缀部分相同、叶子文本变，仍能触发叶子的过渡。
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
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <!-- 左侧占位列：和右侧窗控列同为 1fr，对称把中间面包屑列推到窗口中心。 -->
    <div class="titlebar__spacer" data-tauri-drag-region></div>

    <div class="titlebar__crumbs" data-tauri-drag-region>
      <!-- 非叶子段（项目名等）：同项目内 prefixKey 不变 → 不动；跨项目变 → 走过渡。
           wrapper span 是单根 DOM，让 <Transition> 能正确捕获进出元素。 -->
      <Transition name="tb-crumbs" mode="out-in">
        <span
          v-if="nonLeafCrumbs.length > 0"
          :key="prefixKey"
          class="titlebar__crumbs-prefix"
        >
          <template v-for="(c, i) in nonLeafCrumbs" :key="i">
            <span
              class="titlebar__crumb"
              :class="{ 'titlebar__crumb--muted': c.muted }"
              :title="c.text"
            >{{ c.text }}</span>
            <ChevronRight
              class="titlebar__crumb-sep"
              :size="12"
              aria-hidden="true"
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
        >{{ leafCrumb.text }}</span>
      </Transition>
    </div>

    <div class="titlebar__controls">
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
