<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { ChevronRight } from "lucide-vue-next";
import { listProjects } from "../data/projectsStub";

const router = useRouter();
const projects = computed(() => listProjects());

function open(id: string) {
  router.push(`/projects/${id}`);
}
</script>

<template>
  <section>
    <div class="page-header">
      <div>
        <h1>项目</h1>
        <p>每一个项目对应一个 Claude Code 工作目录。</p>
      </div>
      <button type="button" disabled title="即将上线">
        + 添加项目
      </button>
    </div>

    <div class="card" v-if="projects.length === 0">
      <p class="muted">尚未发现任何项目，先在某个工作目录下启动一次 Claude Code 即可。</p>
    </div>

    <ul class="task-list card" v-else>
      <li
        v-for="p in projects"
        :key="p.id"
        class="task-row"
        style="cursor: pointer"
        @click="open(p.id)"
      >
        <div class="task-row__main">
          <div class="task-row__title">{{ p.name }}</div>
          <div class="task-row__meta">
            <span>{{ p.cwd }}</span>
            <span>·</span>
            <span>{{ p.sessionCount }} 个会话</span>
          </div>
        </div>
        <ChevronRight :size="16" class="muted" aria-hidden="true" />
      </li>
    </ul>
  </section>
</template>
