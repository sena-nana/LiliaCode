<script setup lang="ts">
import "../../styles/pages/project.css";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import { Pin } from "@lucide/vue";
import {
  PROJECT_DASHBOARD_STATUS_ORDER,
  taskStatusLabel,
  type ProjectDashboardSummary,
  type TaskStatus,
} from "@lilia/contracts";
import {
  ensureProjectDashboardLoaded,
  listProjectDashboardSummaries,
} from "../../services/projectsStore";

const statusOrder = PROJECT_DASHBOARD_STATUS_ORDER.map((key) => ({
  key,
  label: taskStatusLabel(key),
}));

const loading = ref(false);
const errorMessage = ref("");
const summaries = computed<ProjectDashboardSummary[]>(() => listProjectDashboardSummaries());
let disposed = false;

async function loadDashboard(force = false) {
  if (disposed) return;
  loading.value = true;
  errorMessage.value = "";
  try {
    await ensureProjectDashboardLoaded(force);
  } catch (err) {
    if (!disposed) errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (!disposed) loading.value = false;
  }
}

function statusCount(summary: ProjectDashboardSummary, key: TaskStatus): number {
  return summary.statusCounts[key] ?? 0;
}

function statusSegments(summary: ProjectDashboardSummary) {
  const total = Math.max(summary.taskCount, 0);
  return statusOrder
    .map((status) => {
      const count = statusCount(summary, status.key);
      return {
        ...status,
        count,
        width: total > 0 ? `${(count / total) * 100}%` : "0%",
      };
    })
    .filter((status) => status.count > 0);
}

function formatActivity(timestamp: number | null): string {
  if (timestamp === null) return "暂无活跃";
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "暂无活跃";
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  const hour = `${date.getHours()}`.padStart(2, "0");
  const minute = `${date.getMinutes()}`.padStart(2, "0");
  return `${year}-${month}-${day} ${hour}:${minute}`;
}

function formatTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M tokens`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K tokens`;
  return `${value} tokens`;
}

function formatCost(summary: ProjectDashboardSummary): string {
  if (summary.knownCostUsd === null) return "暂无成本";
  const digits = summary.knownCostUsd < 1 ? 4 : 2;
  return `$${summary.knownCostUsd.toFixed(digits)}`;
}

function costCoverage(summary: ProjectDashboardSummary): string {
  if (summary.usageRecordCount === 0) return "无用量记录";
  if (summary.costRecordCount === 0) return `${summary.usageRecordCount} 条用量`;
  return `${summary.costRecordCount}/${summary.usageRecordCount} 条成本`;
}

onMounted(() => {
  disposed = false;
  void loadDashboard(true);
});

onBeforeUnmount(() => {
  disposed = true;
});
</script>

<template>
  <section class="projects-overview">
    <header class="projects-overview__head">
      <div>
        <h1 class="projects-overview__title">项目</h1>
        <p class="projects-overview__subtitle">
          {{ summaries.length }} 个项目 · 任务、状态和用量总览
        </p>
      </div>
    </header>

    <div v-if="errorMessage" class="projects-overview__error">
      <span>{{ errorMessage }}</span>
    </div>

    <div v-if="loading && summaries.length === 0" class="projects-overview__empty">
      正在加载项目总览…
    </div>

    <div v-else-if="summaries.length" class="projects-overview__dashboard">
      <RouterLink
        v-for="project in summaries"
      :key="project.id"
      :to="`/projects/${project.id}`"
      class="projects-overview__card"
      :data-agent-id="`projects.overview.open.${project.id}`"
    >
        <span class="projects-overview__card-head">
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
          <span class="projects-overview__activity">
            最近活跃 {{ formatActivity(project.recentActivityAt) }}
          </span>
        </span>

        <span class="projects-overview__status-bar" aria-hidden="true">
          <span
            v-for="segment in statusSegments(project)"
            :key="segment.key"
            class="projects-overview__status-segment"
            :class="`projects-overview__status-segment--${segment.key}`"
            :style="{ width: segment.width }"
          />
          <span
            v-if="project.taskCount === 0"
            class="projects-overview__status-segment projects-overview__status-segment--empty"
          />
        </span>

        <span class="projects-overview__metrics">
          <span class="projects-overview__metric">
            <strong>{{ project.activeCount }}</strong>
            <span>进行中</span>
          </span>
          <span class="projects-overview__metric projects-overview__metric--blocked">
            <strong>{{ project.blockedCount }}</strong>
            <span>阻塞</span>
          </span>
          <span class="projects-overview__metric">
            <strong>{{ project.statusCounts.done }}</strong>
            <span>完成</span>
          </span>
          <span class="projects-overview__metric">
            <strong>{{ project.sessionCount }}</strong>
            <span>会话</span>
          </span>
          <span class="projects-overview__metric">
            <strong>{{ project.taskCount }}</strong>
            <span>任务</span>
          </span>
        </span>

        <span class="projects-overview__usage">
          <span>{{ formatTokens(project.totalTokens) }}</span>
          <span>{{ formatCost(project) }}</span>
          <span>{{ costCoverage(project) }}</span>
        </span>

        <span class="projects-overview__status-list">
          <span v-for="status in statusOrder" :key="status.key">
            {{ status.label }} {{ statusCount(project, status.key) }}
          </span>
        </span>
      </RouterLink>
    </div>

    <div v-else class="projects-overview__empty">暂无项目</div>
  </section>
</template>

