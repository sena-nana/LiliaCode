<script setup lang="ts">
/** 项目下默认 tab：列出该项目的所有非草稿 Task，点击进入 TaskDetail。 */
import { computed, onBeforeUnmount, watch } from "vue";
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
} from "../../utils/perf";

const props = defineProps<{ projectId: string }>();

const tasks = computed<Task[]>(() => listProjectConversations(props.projectId));
const loaded = computed(() => isProjectTasksLoaded(props.projectId));
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

watch(
  () => props.projectId,
  (projectId) => {
    cancelProjectTasksHydration();
    scheduleProjectTasksHydration(projectId);
  },
  { immediate: true },
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
      <li v-for="t in tasks" :key="t.id" class="sessions-view__row">
        <RouterLink
          :to="`/projects/${projectId}/tasks/${t.id}`"
          class="sessions-view__link ui-list-item"
        >
          <span class="sessions-view__title">{{ t.title }}</span>
          <span class="sessions-view__status">{{ t.status }}</span>
        </RouterLink>
      </li>
    </ul>
    <div v-else class="sessions-view__empty">
      还没有任务。点左侧项目行的 + 开一段新对话。
    </div>
  </div>
</template>
