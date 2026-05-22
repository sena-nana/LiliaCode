<script setup lang="ts">
/** 项目下默认 tab：列出该项目的所有非草稿 Task，点击进入 TaskDetail。 */
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { listProjectConversations } from "../../services/tasksStore";
import type { Task } from "@lilia/contracts";

const props = defineProps<{ projectId: string }>();

const tasks = computed<Task[]>(() => listProjectConversations(props.projectId));
</script>

<template>
  <div class="sessions-view">
    <ul v-if="tasks.length" class="sessions-view__list">
      <li v-for="t in tasks" :key="t.id" class="sessions-view__row">
        <RouterLink
          :to="`/projects/${projectId}/tasks/${t.id}`"
          class="sessions-view__link"
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

<style scoped>
.sessions-view {
  padding: 16px 20px;
}
.sessions-view__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.sessions-view__link {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-radius: 6px;
  color: var(--text);
  text-decoration: none;
  background: var(--bg-elev);
}
.sessions-view__link:hover {
  background: var(--bg-hover);
}
.sessions-view__title {
  font-size: 13px;
}
.sessions-view__status {
  font-size: 11px;
  color: var(--text-muted, #888);
  text-transform: lowercase;
}
.sessions-view__empty {
  color: var(--text-muted, #888);
  font-size: 13px;
  padding: 24px 12px;
  text-align: center;
}
</style>
