<script setup lang="ts">
import { computed, watch } from "vue";
import { RouterLink } from "vue-router";
import {
  ensureProjectTasksLoaded,
  isProjectTasksLoaded,
  listProjectConversations,
} from "../../services/tasksStore";
import type { MilestoneStatus, Task, TaskStatus } from "@lilia/contracts";

const props = defineProps<{ projectId: string }>();

const tasks = computed<Task[]>(() => listProjectConversations(props.projectId));
const loaded = computed(() => isProjectTasksLoaded(props.projectId));

const activeStatuses: TaskStatus[] = ["running", "waiting", "blocked"];
const taskStatusMeta: Record<TaskStatus, { label: string; rank: number }> = {
  blocked: { label: "阻塞", rank: 0 },
  running: { label: "运行中", rank: 1 },
  waiting: { label: "等待", rank: 2 },
  draft: { label: "草稿", rank: 3 },
  done: { label: "完成", rank: 4 },
  cancelled: { label: "取消", rank: 5 },
};
const milestoneStatusLabel: Record<MilestoneStatus, string> = {
  upcoming: "upcoming",
  "in-progress": "in-progress",
  done: "done",
  abandoned: "abandoned",
};

const counts = computed(() => {
  const next: Record<TaskStatus, number> = {
    draft: 0,
    waiting: 0,
    running: 0,
    blocked: 0,
    done: 0,
    cancelled: 0,
  };
  for (const task of tasks.value) next[task.status] += 1;
  return next;
});

const activeTasks = computed(() =>
  tasks.value
    .filter((task) => activeStatuses.includes(task.status))
    .sort((a, b) => taskStatusMeta[a.status].rank - taskStatusMeta[b.status].rank || b.createdAt - a.createdAt),
);

const recentlyDoneTasks = computed(() =>
  tasks.value
    .filter((task) => task.status === "done")
    .sort((a, b) => b.createdAt - a.createdAt)
    .slice(0, 4),
);

const visibleActiveTasks = computed(() => activeTasks.value.slice(0, 5));
const backlogCount = computed(() => Math.max(tasks.value.length - activeTasks.value.length - counts.value.done, 0));
const totalCount = computed(() => tasks.value.length);
const doneCount = computed(() => counts.value.done);
const progress = computed(() =>
  totalCount.value === 0 ? 0 : Math.round((doneCount.value / totalCount.value) * 100),
);

const statusItems = computed(() => [
  { key: "running", label: "运行中", count: counts.value.running },
  { key: "waiting", label: "等待", count: counts.value.waiting },
  { key: "blocked", label: "阻塞", count: counts.value.blocked },
  { key: "done", label: "已完成", count: counts.value.done },
  { key: "cancelled", label: "已取消", count: counts.value.cancelled },
] satisfies Array<{ key: TaskStatus; label: string; count: number }>);

const milestoneStatus = computed<MilestoneStatus>(() => {
  if (counts.value.running > 0 || counts.value.waiting > 0 || counts.value.blocked > 0) return "in-progress";
  if (totalCount.value > 0 && doneCount.value === totalCount.value) return "done";
  return "upcoming";
});

function statusLabel(status: TaskStatus): string {
  return taskStatusMeta[status].label;
}

function formatDate(value: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

watch(
  () => props.projectId,
  (projectId) => {
    void ensureProjectTasksLoaded(projectId);
  },
  { immediate: true },
);
</script>

<template>
  <div class="roadmap-view">
    <div class="roadmap-view__summary">
      <div class="roadmap-view__summary-main">
        <span class="roadmap-view__eyebrow">首发进度</span>
        <h2>项目会话任务化</h2>
        <p>基于当前项目 Task 状态聚合 milestone 进度；持久化 Milestone / TaskMilestoneLink 数据源尚未接入。</p>
      </div>
      <div class="roadmap-view__score" aria-label="当前 milestone 完成度">
        <strong>{{ progress }}%</strong>
        <span>{{ doneCount }}/{{ totalCount }} done</span>
      </div>
    </div>

    <div v-if="!loaded" class="roadmap-view__empty">
      正在加载路线图…
    </div>

    <template v-else>
      <div class="roadmap-view__metrics" aria-label="项目任务状态">
        <div
          v-for="item in statusItems"
          :key="item.key"
          class="roadmap-view__metric"
          :class="`roadmap-view__metric--${item.key}`"
        >
          <span>{{ item.label }}</span>
          <strong>{{ item.count }}</strong>
        </div>
      </div>

      <section class="roadmap-milestone" :class="`roadmap-milestone--${milestoneStatus}`">
        <div class="roadmap-milestone__rail" aria-hidden="true" />
        <div class="roadmap-milestone__body">
          <header class="roadmap-milestone__head">
            <div class="roadmap-milestone__title-wrap">
              <span class="roadmap-milestone__index">M1</span>
              <h3>首发可用路线图</h3>
            </div>
            <span class="roadmap-milestone__status">
              {{ milestoneStatusLabel[milestoneStatus] }}
            </span>
          </header>

          <div class="roadmap-milestone__progress">
            <div class="roadmap-milestone__progress-bar">
              <span :style="{ width: `${progress}%` }" />
            </div>
            <span>{{ progress }}%</span>
          </div>

          <div class="roadmap-milestone__columns">
            <section class="roadmap-milestone__section">
              <h4>当前重点</h4>
              <ul v-if="visibleActiveTasks.length" class="roadmap-task-list">
                <li v-for="task in visibleActiveTasks" :key="task.id" class="roadmap-task">
                  <RouterLink :to="`/projects/${projectId}/tasks/${task.id}`" class="roadmap-task__link">
                    <span class="roadmap-task__title">{{ task.title }}</span>
                    <span class="roadmap-task__meta">
                      <span :class="`roadmap-task__status roadmap-task__status--${task.status}`">
                        {{ statusLabel(task.status) }}
                      </span>
                      <span>{{ formatDate(task.createdAt) }}</span>
                    </span>
                  </RouterLink>
                </li>
              </ul>
              <p v-else class="roadmap-milestone__muted">
                暂无运行中、等待或阻塞任务。
              </p>
            </section>

            <section class="roadmap-milestone__section">
              <h4>完成前检查</h4>
              <ul class="roadmap-checks">
                <li :class="{ 'is-done': doneCount > 0 }">
                  <span aria-hidden="true" />
                  已有任务进入完成状态
                </li>
                <li :class="{ 'is-done': counts.blocked === 0 }">
                  <span aria-hidden="true" />
                  无阻塞任务
                </li>
                <li :class="{ 'is-done': totalCount > 0 && doneCount === totalCount }">
                  <span aria-hidden="true" />
                  主要任务已完成或移出本阶段
                </li>
              </ul>
            </section>
          </div>

          <footer class="roadmap-milestone__foot">
            <span>{{ backlogCount }} 个未归类/草稿/取消项仍在项目任务池</span>
            <span>模板来源：docs/github/milestone-progress-template.md</span>
          </footer>
        </div>
      </section>

      <section class="roadmap-recent">
        <h3>最近完成</h3>
        <ul v-if="recentlyDoneTasks.length" class="roadmap-recent__list">
          <li v-for="task in recentlyDoneTasks" :key="task.id">
            <RouterLink :to="`/projects/${projectId}/tasks/${task.id}`">
              <span>{{ task.title }}</span>
              <time>{{ formatDate(task.createdAt) }}</time>
            </RouterLink>
          </li>
        </ul>
        <p v-else class="roadmap-milestone__muted">还没有完成项。</p>
      </section>
    </template>
  </div>
</template>
