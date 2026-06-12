<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import {
  ensureProjectTasksLoaded,
  isProjectTasksLoaded,
  listProjectConversations,
} from "../../services/tasksStore";
import {
  createMilestone,
  ensureProjectRoadmapLoaded,
  isProjectRoadmapLoaded,
  listProjectMilestoneLinks,
  listProjectMilestones,
  setMilestoneTasks,
  updateMilestone,
} from "../../services/milestonesStore";
import type { Milestone, MilestoneStatus, Task, TaskStatus } from "@lilia/contracts";

const props = defineProps<{ projectId: string }>();

const milestoneStatusOptions: Array<{ value: MilestoneStatus; label: string }> = [
  { value: "upcoming", label: "待开始" },
  { value: "in-progress", label: "进行中" },
  { value: "done", label: "完成" },
  { value: "abandoned", label: "已放弃" },
];

const taskStatusMeta: Record<TaskStatus, { label: string }> = {
  blocked: { label: "阻塞" },
  running: { label: "运行中" },
  waiting: { label: "等待" },
  draft: { label: "草稿" },
  done: { label: "完成" },
  cancelled: { label: "取消" },
};

const tasks = computed<Task[]>(() => listProjectConversations(props.projectId));
const milestones = computed<Milestone[]>(() => listProjectMilestones(props.projectId));
const links = computed(() => listProjectMilestoneLinks(props.projectId));
const loaded = computed(() =>
  isProjectTasksLoaded(props.projectId) && isProjectRoadmapLoaded(props.projectId),
);

const newMilestoneTitle = ref("");
const creatingMilestone = ref(false);
const savingMilestoneId = ref<string | null>(null);
const errorMessage = ref("");

const taskById = computed(() => new Map(tasks.value.map((task) => [task.id, task])));

const linkedTasks = computed(() => {
  const seen = new Set<string>();
  const out: Task[] = [];
  for (const link of links.value) {
    if (seen.has(link.taskId)) continue;
    const task = taskById.value.get(link.taskId);
    if (!task) continue;
    seen.add(link.taskId);
    out.push(task);
  }
  return out;
});

const statusCounts = computed(() => {
  const next: Record<TaskStatus, number> = {
    draft: 0,
    waiting: 0,
    running: 0,
    blocked: 0,
    done: 0,
    cancelled: 0,
  };
  for (const task of linkedTasks.value) next[task.status] += 1;
  return next;
});

const totalLinkedCount = computed(() => linkedTasks.value.length);
const doneLinkedCount = computed(() => statusCounts.value.done);
const totalProgress = computed(() =>
  totalLinkedCount.value === 0 ? 0 : Math.round((doneLinkedCount.value / totalLinkedCount.value) * 100),
);

const statusItems = computed(() => [
  { key: "running", label: "运行中", count: statusCounts.value.running },
  { key: "waiting", label: "等待", count: statusCounts.value.waiting },
  { key: "blocked", label: "阻塞", count: statusCounts.value.blocked },
  { key: "done", label: "已完成", count: statusCounts.value.done },
  { key: "cancelled", label: "已取消", count: statusCounts.value.cancelled },
] satisfies Array<{ key: TaskStatus; label: string; count: number }>);

const milestoneViews = computed(() =>
  milestones.value.map((milestone, index) => {
    const milestoneLinks = links.value.filter((link) => link.milestoneId === milestone.id);
    const milestoneTasks = milestoneLinks
      .map((link) => taskById.value.get(link.taskId))
      .filter((task): task is Task => Boolean(task));
    const doneCount = milestoneTasks.filter((task) => task.status === "done").length;
    const totalCount = milestoneTasks.length;
    return {
      milestone,
      index,
      tasks: milestoneTasks,
      taskIds: new Set(milestoneTasks.map((task) => task.id)),
      doneCount,
      totalCount,
      progress: totalCount === 0 ? 0 : Math.round((doneCount / totalCount) * 100),
    };
  }),
);

function statusLabel(status: TaskStatus): string {
  return taskStatusMeta[status].label;
}

function isSaving(milestoneId: string): boolean {
  return savingMilestoneId.value === milestoneId;
}

async function addMilestone() {
  const title = newMilestoneTitle.value.trim();
  if (!title || creatingMilestone.value) return;
  creatingMilestone.value = true;
  errorMessage.value = "";
  try {
    await createMilestone(props.projectId, title);
    newMilestoneTitle.value = "";
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    creatingMilestone.value = false;
  }
}

async function saveMilestoneChange(milestoneId: string, run: () => Promise<void>) {
  savingMilestoneId.value = milestoneId;
  errorMessage.value = "";
  try {
    await run();
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    savingMilestoneId.value = null;
  }
}

async function changeMilestoneStatus(milestoneId: string, event: Event) {
  const status = (event.target as HTMLSelectElement).value as MilestoneStatus;
  await saveMilestoneChange(milestoneId, () =>
    updateMilestone(props.projectId, milestoneId, { status }),
  );
}

async function toggleMilestoneTask(
  milestoneId: string,
  currentTaskIds: Set<string>,
  taskId: string,
  checked: boolean,
) {
  const next = new Set(currentTaskIds);
  if (checked) next.add(taskId);
  else next.delete(taskId);
  await saveMilestoneChange(milestoneId, () =>
    setMilestoneTasks(props.projectId, milestoneId, [...next]),
  );
}

function onTaskToggle(milestoneId: string, currentTaskIds: Set<string>, taskId: string, event: Event) {
  void toggleMilestoneTask(
    milestoneId,
    currentTaskIds,
    taskId,
    (event.target as HTMLInputElement).checked,
  );
}

watch(
  () => props.projectId,
  (projectId) => {
    void Promise.all([
      ensureProjectTasksLoaded(projectId),
      ensureProjectRoadmapLoaded(projectId),
    ]);
  },
  { immediate: true },
);
</script>

<template>
  <div class="roadmap-view">
    <div class="roadmap-view__summary">
      <div class="roadmap-view__summary-main">
        <span class="roadmap-view__eyebrow">路线图</span>
        <h2>Milestone</h2>
        <p>{{ milestones.length }} 个 milestone，{{ totalLinkedCount }} 个关联任务。</p>
      </div>
      <div class="roadmap-view__score" aria-label="路线图完成度">
        <strong>{{ totalProgress }}%</strong>
        <span>{{ doneLinkedCount }}/{{ totalLinkedCount }} done</span>
      </div>
    </div>

    <form class="roadmap-create" @submit.prevent="addMilestone">
      <input
        v-model="newMilestoneTitle"
        class="ui-input roadmap-create__input"
        aria-label="新 Milestone 标题"
        placeholder="新建 Milestone"
      />
      <button
        type="submit"
        class="ui-button ui-button--primary"
        :disabled="creatingMilestone || !newMilestoneTitle.trim()"
      >
        新增
      </button>
    </form>
    <p v-if="errorMessage" class="roadmap-view__error" role="alert">{{ errorMessage }}</p>

    <div v-if="!loaded" class="roadmap-view__empty">
      正在加载路线图…
    </div>

    <template v-else>
      <div class="roadmap-view__metrics" aria-label="关联任务状态">
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

      <div v-if="milestones.length === 0" class="roadmap-view__empty">
        暂无路线图
      </div>

      <section
        v-for="view in milestoneViews"
        :key="view.milestone.id"
        class="roadmap-milestone"
        :class="`roadmap-milestone--${view.milestone.status}`"
      >
        <div class="roadmap-milestone__rail" aria-hidden="true" />
        <div class="roadmap-milestone__body">
          <header class="roadmap-milestone__head">
            <div class="roadmap-milestone__title-wrap">
              <span class="roadmap-milestone__index">M{{ view.index + 1 }}</span>
              <h3>{{ view.milestone.title }}</h3>
            </div>
            <select
              class="roadmap-milestone__status-select"
              :value="view.milestone.status"
              :disabled="isSaving(view.milestone.id)"
              :aria-label="`${view.milestone.title} 状态`"
              @change="changeMilestoneStatus(view.milestone.id, $event)"
            >
              <option
                v-for="option in milestoneStatusOptions"
                :key="option.value"
                :value="option.value"
              >
                {{ option.label }}
              </option>
            </select>
          </header>

          <div class="roadmap-milestone__progress">
            <div class="roadmap-milestone__progress-bar">
              <span :style="{ width: `${view.progress}%` }" />
            </div>
            <span>{{ view.progress }}%</span>
          </div>

          <div class="roadmap-milestone__columns">
            <section class="roadmap-milestone__section">
              <h4>关联任务</h4>
              <ul v-if="view.tasks.length" class="roadmap-task-list">
                <li v-for="task in view.tasks" :key="task.id" class="roadmap-task">
                  <RouterLink :to="`/projects/${projectId}/tasks/${task.id}`" class="roadmap-task__link">
                    <span class="roadmap-task__title">{{ task.title }}</span>
                    <span class="roadmap-task__meta">
                      <span :class="`roadmap-task__status roadmap-task__status--${task.status}`">
                        {{ statusLabel(task.status) }}
                      </span>
                    </span>
                  </RouterLink>
                </li>
              </ul>
              <p v-else class="roadmap-milestone__muted">
                暂无关联任务。
              </p>
            </section>

            <section class="roadmap-milestone__section">
              <h4>任务关联</h4>
              <ul v-if="tasks.length" class="roadmap-task-picker">
                <li v-for="task in tasks" :key="task.id">
                  <label class="roadmap-task-choice">
                    <input
                      type="checkbox"
                      :checked="view.taskIds.has(task.id)"
                      :disabled="isSaving(view.milestone.id)"
                      @change="onTaskToggle(view.milestone.id, view.taskIds, task.id, $event)"
                    />
                    <span class="roadmap-task-choice__title">{{ task.title }}</span>
                    <span :class="`roadmap-task__status roadmap-task__status--${task.status}`">
                      {{ statusLabel(task.status) }}
                    </span>
                  </label>
                </li>
              </ul>
              <p v-else class="roadmap-milestone__muted">
                还没有任务。
              </p>
            </section>
          </div>

          <footer class="roadmap-milestone__foot">
            <span>{{ view.doneCount }}/{{ view.totalCount }} done</span>
            <span>{{ view.totalCount }} 个关联任务</span>
          </footer>
        </div>
      </section>
    </template>
  </div>
</template>
