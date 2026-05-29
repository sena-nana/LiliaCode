<script setup lang="ts">
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { Pin } from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import { listProjects } from "../../services/projectsStore";

const projects = computed<Project[]>(() => listProjects());
</script>

<template>
  <section class="projects-overview">
    <header class="projects-overview__head">
      <h1 class="projects-overview__title">项目</h1>
    </header>

    <div v-if="projects.length" class="projects-overview__list">
      <RouterLink
        v-for="project in projects"
        :key="project.id"
        :to="`/projects/${project.id}`"
        class="projects-overview__row"
      >
        <span class="projects-overview__main">
          <span class="projects-overview__name">
            {{ project.name }}
            <Pin
              v-if="project.pinned"
              :size="12"
              class="projects-overview__pin"
              aria-hidden="true"
            />
          </span>
          <span class="projects-overview__path">{{ project.cwd ?? "分类项目" }}</span>
        </span>
        <span class="projects-overview__count">
          {{ project.sessionCount }} 个对话
        </span>
      </RouterLink>
    </div>

    <div v-else class="projects-overview__empty">暂无项目</div>
  </section>
</template>

<style scoped>
.projects-overview {
  height: 100%;
  min-width: 0;
  padding: 20px;
  overflow: auto;
}

.projects-overview__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.projects-overview__title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.projects-overview__list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.projects-overview__row {
  min-height: 54px;
  padding: 9px 12px;
  border-radius: 6px;
  background: var(--bg-elev);
  color: var(--text);
  text-decoration: none;
  display: flex;
  align-items: center;
  gap: 14px;
}

.projects-overview__row:hover {
  background: var(--bg-hover);
}

.projects-overview__main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.projects-overview__name {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
}

.projects-overview__pin {
  flex-shrink: 0;
  color: var(--accent);
}

.projects-overview__path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-muted);
  font-size: 12px;
}

.projects-overview__count {
  flex: 0 0 auto;
  color: var(--text-muted);
  font-size: 12px;
}

.projects-overview__empty {
  padding: 28px 12px;
  text-align: center;
  color: var(--text-muted);
  font-size: 13px;
}
</style>
