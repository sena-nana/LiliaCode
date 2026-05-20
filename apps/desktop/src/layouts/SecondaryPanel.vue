<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";
import { Folder, Cog, Plug, Info } from "lucide-vue-next";
import { listProjects } from "../data/projectsStub";

interface Props {
  section: "projects" | "settings";
}
const props = defineProps<Props>();
const route = useRoute();

const projects = computed(() => listProjects());

const settingsNav = [
  { to: "/settings", label: "通用", icon: Cog },
  { to: "/settings", label: "Claude CLI", icon: Plug },
  { to: "/settings", label: "关于", icon: Info },
];

function isActiveProject(projectId: string) {
  return route.path.startsWith(`/projects/${projectId}`);
}
</script>

<template>
  <aside class="secondary-panel">
    <div v-if="props.section === 'projects'">
      <div class="secondary-panel__title">项目</div>
      <nav class="secondary-panel__nav">
        <RouterLink
          v-for="p in projects"
          :key="p.id"
          :to="`/projects/${p.id}`"
          class="secondary-panel__item"
          :class="{ 'is-active': isActiveProject(p.id) }"
        >
          <Folder :size="16" aria-hidden="true" />
          <span>{{ p.name }}</span>
          <span class="secondary-panel__item-meta">{{ p.sessionCount }}</span>
        </RouterLink>
        <p v-if="projects.length === 0" class="muted" style="padding: 8px 10px">
          暂无项目
        </p>
      </nav>
    </div>

    <div v-else>
      <div class="secondary-panel__title">设置</div>
      <nav class="secondary-panel__nav">
        <RouterLink
          v-for="item in settingsNav"
          :key="item.label"
          :to="item.to"
          class="secondary-panel__item"
          active-class="is-active"
        >
          <component :is="item.icon" :size="16" aria-hidden="true" />
          {{ item.label }}
        </RouterLink>
      </nav>
    </div>
  </aside>
</template>
