<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { ChevronRight } from "lucide-vue-next";
import { getProject, listTasks } from "../data/projectsStub";
import type { Task } from "@lilia/contracts";

const props = defineProps<{ projectId: string }>();
const router = useRouter();

const project = computed(() => getProject(props.projectId));
const tasks = computed(() => listTasks(props.projectId));

function open(t: Task) {
  router.push(`/projects/${props.projectId}/tasks/${t.id}`);
}

function statusClass(s: Task["status"]) {
  return `task-row__status task-row__status--${s}`;
}
</script>

<template>
  <section v-if="project">
    <div class="page-header">
      <div>
        <h1>{{ project.name }}</h1>
        <p>{{ project.cwd }}</p>
      </div>
      <button type="button" disabled title="即将上线">+ 新会话</button>
    </div>

    <div class="card" v-if="tasks.length === 0">
      <p class="muted">这个项目还没有会话。</p>
    </div>

    <ul class="task-list card" v-else>
      <li
        v-for="t in tasks"
        :key="t.id"
        class="task-row"
        style="cursor: pointer"
        @click="open(t)"
      >
        <div class="task-row__main">
          <div class="task-row__title">{{ t.title }}</div>
          <div class="task-row__meta">
            <span>{{ t.sessionId }}</span>
            <span v-if="t.dependsOn.length">· 依赖 {{ t.dependsOn.length }} 项</span>
          </div>
        </div>
        <span :class="statusClass(t.status)">{{ t.status }}</span>
        <ChevronRight :size="16" class="muted" aria-hidden="true" />
      </li>
    </ul>
  </section>

  <section v-else>
    <div class="empty-state">未找到项目 <code>{{ projectId }}</code></div>
  </section>
</template>
