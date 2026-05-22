<script setup lang="ts">
/**
 * 项目主区壳：顶部挂 ViewTabs（sessions / roadmap / memory），下方 router-view 渲染当前 tab。
 * TaskDetail 不嵌在这里——/projects/:projectId/tasks/:taskId 是平级路由，
 * 进入聊天页时 ViewTabs 不出现，避免主对话被"上面有一栏 tab"持续打扰。
 */
import { computed } from "vue";
import { useRoute } from "vue-router";
import { getProject } from "../../services/projectsStore";
import ViewTabs from "../../components/ViewTabs.vue";

const props = defineProps<{ projectId: string }>();
const route = useRoute();

const project = computed(() => getProject(props.projectId));

const activeTab = computed<"sessions" | "roadmap" | "memory">(() => {
  const tail = route.path.split("/").pop() ?? "";
  if (tail === "roadmap") return "roadmap";
  if (tail === "memory") return "memory";
  return "sessions";
});
</script>

<template>
  <section v-if="project" class="project-shell">
    <header class="project-shell__head">
      <h1 class="project-shell__title">{{ project.name }}</h1>
      <ViewTabs :project-id="projectId" :active="activeTab" />
    </header>
    <div class="project-shell__body">
      <router-view />
    </div>
  </section>

  <section v-else>
    <div class="empty-state">未找到项目 <code>{{ projectId }}</code></div>
  </section>
</template>

<style scoped>
.project-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
}
.project-shell__head {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 14px 20px 0;
  border-bottom: 1px solid var(--border, transparent);
}
.project-shell__title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}
.project-shell__body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
</style>
