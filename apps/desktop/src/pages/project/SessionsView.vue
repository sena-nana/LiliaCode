<script setup lang="ts">
/** 项目下默认 tab：列出该项目的所有非草稿 Task，点击进入 TaskDetail。 */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import {
  ensureProjectTasksLoaded,
  isProjectTasksLoaded,
  listProjectConversations,
} from "../../services/tasksStore";
import type { Task } from "@lilia/contracts";
import {
  beginPerfStage,
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "@lilia/ui";

const props = defineProps<{ projectId: string }>();

const SESSION_PAGE_SIZE = 80;

const tasks = computed<Task[]>(() => listProjectConversations(props.projectId));
const loaded = computed(() => isProjectTasksLoaded(props.projectId));
const visibleTaskCount = ref(SESSION_PAGE_SIZE);
let tasksHydrationHandle: number | null = null;
let cancelTasksHydrationPaint: (() => void) | null = null;
let tasksHydrationSeq = 0;

function cancelProjectTasksHydration() {
  cancelTasksHydrationPaint?.();
  cancelTasksHydrationPaint = null;
  if (tasksHydrationHandle !== null) {
    cancelIdleRun(tasksHydrationHandle);
    tasksHydrationHandle = null;
  }
  tasksHydrationSeq += 1;
}

function scheduleProjectTasksHydration(projectId: string) {
  const seq = ++tasksHydrationSeq;
  const stage = beginPerfStage("project.sessions.switch", { detail: projectId });
  cancelTasksHydrationPaint = scheduleAfterPaint(() => {
    cancelTasksHydrationPaint = null;
    if (seq !== tasksHydrationSeq) {
      stage.end("cancelled");
      return;
    }
    stage.end("paint");
    if (loaded.value) return;
    tasksHydrationHandle = runWhenIdle(() => {
      tasksHydrationHandle = null;
      if (seq !== tasksHydrationSeq || loaded.value) return;
      void measurePerfAsync(
        "project.sessions.load",
        () => ensureProjectTasksLoaded(projectId),
        { detail: projectId },
      ).catch((err) => {
        console.error("[sessions-view] load project tasks failed", err);
      });
    });
  });
}

const sessionRows = computed(() =>
  tasks.value.slice(0, visibleTaskCount.value).map((task) => ({
    task,
    route: `/projects/${props.projectId}/tasks/${task.id}`,
  })),
);

const hiddenTaskCount = computed(() =>
  Math.max(0, tasks.value.length - sessionRows.value.length),
);

function revealMoreSessions() {
  visibleTaskCount.value = Math.min(
    tasks.value.length,
    visibleTaskCount.value + SESSION_PAGE_SIZE,
  );
}

watch(
  () => props.projectId,
  (projectId) => {
    visibleTaskCount.value = SESSION_PAGE_SIZE;
    cancelProjectTasksHydration();
    scheduleProjectTasksHydration(projectId);
  },
  { immediate: true },
);

watch(
  () => tasks.value.length,
  (taskCount, previousTaskCount) => {
    if (taskCount <= SESSION_PAGE_SIZE) {
      visibleTaskCount.value = SESSION_PAGE_SIZE;
      return;
    }
    if (previousTaskCount !== undefined && visibleTaskCount.value >= previousTaskCount) {
      visibleTaskCount.value = taskCount;
    }
  },
);

onBeforeUnmount(() => {
  cancelProjectTasksHydration();
});
</script>

<template>
  <div class="sessions-view">
    <div v-if="!loaded" class="sessions-view__empty">
      正在加载对话…
    </div>
    <ul v-else-if="tasks.length" class="sessions-view__list ui-list">
      <li v-for="row in sessionRows" :key="row.task.id" class="sessions-view__row">
        <RouterLink
          :to="row.route"
          class="sessions-view__link ui-list-item"
          :data-agent-id="`project.sessions.open.${row.task.id}`"
        >
          <span class="sessions-view__title">{{ row.task.title }}</span>
          <span class="sessions-view__status">{{ row.task.status }}</span>
        </RouterLink>
      </li>
      <li v-if="hiddenTaskCount > 0" class="sessions-view__row">
        <button
          type="button"
          class="sessions-view__more ui-list-item"
          data-agent-id="project.sessions.load-more"
          @click="revealMoreSessions"
        >
          加载更多 {{ hiddenTaskCount }}
        </button>
      </li>
    </ul>
    <div v-else class="sessions-view__empty">
      还没有任务。点左侧项目行的 + 开一段新对话。
    </div>
  </div>
</template>

