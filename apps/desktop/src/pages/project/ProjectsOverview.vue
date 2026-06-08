<script setup lang="ts">
import "../../styles/pages/project.css";
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

    <div v-if="projects.length" class="projects-overview__list ui-list">
      <RouterLink
        v-for="project in projects"
        :key="project.id"
        :to="`/projects/${project.id}`"
        class="projects-overview__row ui-list-item"
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
