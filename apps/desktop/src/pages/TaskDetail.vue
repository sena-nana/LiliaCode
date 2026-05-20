<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import { ArrowLeft } from "lucide-vue-next";
import { getProject, getTask, listTasks } from "../data/projectsStub";

const props = defineProps<{ projectId: string; taskId: string }>();
const router = useRouter();

const project = computed(() => getProject(props.projectId));
const task = computed(() => getTask(props.projectId, props.taskId));
const siblings = computed(() => listTasks(props.projectId));

const prerequisites = computed(() => {
  const t = task.value;
  if (!t) return [];
  return t.dependsOn
    .map((id) => siblings.value.find((s) => s.id === id))
    .filter((x): x is NonNullable<typeof x> => Boolean(x));
});

const subtasks = computed(() => {
  const t = task.value;
  if (!t) return [];
  return siblings.value.filter((s) => s.parentId === t.id);
});

function back() {
  router.push("/");
}
</script>

<template>
  <section v-if="task && project">
    <div class="page-header">
      <div class="row" style="flex-wrap: nowrap; gap: 10px; align-items: center">
        <button type="button" class="ghost" @click="back" aria-label="返回">
          <ArrowLeft :size="14" />
          返回
        </button>
        <div>
          <h1>{{ task.title }}</h1>
          <p>{{ project.name }} · 会话 {{ task.sessionId }}</p>
        </div>
      </div>
      <span :class="`task-row__status task-row__status--${task.status}`">
        {{ task.status }}
      </span>
    </div>

    <div class="detail-grid">
      <div class="card">
        <h2>对话占位</h2>
        <p class="muted">
          这里之后会渲染 Claude Code 会话的真实消息流。当前阶段仅占位。
        </p>
      </div>

      <aside class="detail-grid__side">
        <div class="card">
          <h2>任务信息</h2>
          <ul class="kv">
            <li><span>状态</span><span>{{ task.status }}</span></li>
            <li><span>会话 ID</span><span>{{ task.sessionId }}</span></li>
            <li>
              <span>创建于</span>
              <span>{{ new Date(task.createdAt).toLocaleString() }}</span>
            </li>
            <li v-if="task.parentId">
              <span>父任务</span><span>{{ task.parentId }}</span>
            </li>
          </ul>
        </div>

        <div class="card">
          <h2>前置任务</h2>
          <p class="muted" v-if="prerequisites.length === 0">无</p>
          <ul class="task-list" v-else>
            <li v-for="p in prerequisites" :key="p.id" class="task-row">
              <div class="task-row__main">
                <div class="task-row__title">{{ p.title }}</div>
              </div>
              <span :class="`task-row__status task-row__status--${p.status}`">
                {{ p.status }}
              </span>
            </li>
          </ul>
        </div>

        <div class="card">
          <h2>子任务</h2>
          <p class="muted" v-if="subtasks.length === 0">无</p>
          <ul class="task-list" v-else>
            <li v-for="s in subtasks" :key="s.id" class="task-row">
              <div class="task-row__main">
                <div class="task-row__title">{{ s.title }}</div>
              </div>
              <span :class="`task-row__status task-row__status--${s.status}`">
                {{ s.status }}
              </span>
            </li>
          </ul>
        </div>
      </aside>
    </div>
  </section>

  <section v-else>
    <div class="empty-state">未找到任务 <code>{{ taskId }}</code></div>
  </section>
</template>
