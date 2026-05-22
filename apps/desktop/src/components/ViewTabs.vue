<script setup lang="ts">
/**
 * 项目级 ViewTabs：sessions / roadmap / memory。
 * 每个 tab 是一个 router-link，路径对应 ProjectShell 的子路由。
 * 任务详情 /projects/:projectId/tasks/:taskId 是 ProjectShell 的兄弟路由，
 * 进入聊天时 ViewTabs 不渲染——这是有意的（守"打开就能聊"的轻量感）。
 */
import { RouterLink } from "vue-router";
import { MessagesSquare, Milestone, Brain } from "lucide-vue-next";

type ViewKey = "sessions" | "roadmap" | "memory";

interface Props {
  projectId: string;
  active: ViewKey;
}

const props = defineProps<Props>();

const tabs: Array<{ key: ViewKey; label: string; icon: any; path: (pid: string) => string }> = [
  { key: "sessions", label: "Sessions", icon: MessagesSquare, path: (pid) => `/projects/${pid}` },
  { key: "roadmap", label: "路线图", icon: Milestone, path: (pid) => `/projects/${pid}/roadmap` },
  { key: "memory", label: "记忆", icon: Brain, path: (pid) => `/projects/${pid}/memory` },
];
</script>

<template>
  <nav class="view-tabs" role="tablist" aria-label="项目视图">
    <RouterLink
      v-for="t in tabs"
      :key="t.key"
      :to="t.path(props.projectId)"
      class="view-tabs__tab"
      :class="{ 'is-active': active === t.key }"
      :aria-selected="active === t.key"
      role="tab"
    >
      <component :is="t.icon" :size="14" aria-hidden="true" />
      <span>{{ t.label }}</span>
    </RouterLink>
  </nav>
</template>

<style scoped>
.view-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
}
.view-tabs__tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 12px;
  color: var(--text-muted, #888);
  text-decoration: none;
  background: transparent;
  transition: background-color 0.12s, color 0.12s;
}
.view-tabs__tab:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.view-tabs__tab.is-active {
  background: var(--bg-active);
  color: var(--text);
}
</style>
