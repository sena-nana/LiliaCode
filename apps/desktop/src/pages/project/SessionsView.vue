<script setup lang="ts">
/** 项目下默认 tab：列出该项目的所有非草稿 Task，点击进入 TaskDetail。 */
import { computed, watch } from "vue";
import { RouterLink } from "vue-router";
import {
  ensureProjectTasksLoaded,
  isProjectTasksLoaded,
  listProjectConversations,
} from "../../services/tasksStore";
import type { Task } from "@lilia/contracts";

const props = defineProps<{ projectId: string }>();

const tasks = computed<Task[]>(() => listProjectConversations(props.projectId));
const loaded = computed(() => isProjectTasksLoaded(props.projectId));

watch(
  () => props.projectId,
  (projectId) => {
    void ensureProjectTasksLoaded(projectId);
  },
  { immediate: true },
);
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
