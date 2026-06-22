<script setup lang="ts">
import { computed } from "vue";
import { RouterLink } from "vue-router";
import { ArrowDown, ArrowUp, Trash2 } from "lucide-vue-next";
import {
  PROJECT_ROADMAP_STATUS_ORDER,
  countProjectTaskStatuses,
  taskStatusLabel,
  type Milestone,
  type MilestoneStatus,
  type Task,
  type TaskMilestoneLink,
  type TaskStatus,
} from "@lilia/contracts";

const props = defineProps<{
  projectId: string;
  tasks: Task[];
  milestones: Milestone[];
  links: TaskMilestoneLink[];
  tasksReady: boolean;
  savingMilestoneId: string | null;
  milestoneStatusOptions: Array<{ value: MilestoneStatus; label: string }>;
  formatDateInput: (timestamp: number | null) => string;
}>();

const emit = defineEmits<{
  changeStatus: [milestoneId: string, event: Event];
  changeDescription: [milestone: Milestone, event: Event];
  changeDueDate: [milestone: Milestone, event: Event];
  remove: [milestone: Milestone];
  move: [milestoneId: string, direction: -1 | 1];
  toggleTask: [milestoneId: string, currentTaskIds: Set<string>, taskId: string, event: Event];
}>();

const taskById = computed(() => new Map(props.tasks.map((task) => [task.id, task])));
const childCountByTaskId = computed(() => {
  const out = new Map<string, number>();
  const taskIds = new Set(props.tasks.map((task) => task.id));
  for (const task of props.tasks) {
    if (!task.parentId || !taskIds.has(task.parentId)) continue;
    out.set(task.parentId, (out.get(task.parentId) ?? 0) + 1);
  }
  return out;
});

const linkedTasks = computed(() => {
  const seen = new Set<string>();
  const out: Task[] = [];
  for (const link of props.links) {
    if (seen.has(link.taskId)) continue;
    const task = taskById.value.get(link.taskId);
    if (!task) continue;
    seen.add(link.taskId);
    out.push(task);
  }
  return out;
});

const statusCounts = computed(() => {
  return countProjectTaskStatuses(linkedTasks.value);
});

const totalLinkedCount = computed(() => linkedTasks.value.length);
const doneLinkedCount = computed(() => statusCounts.value.done);
const totalProgress = computed(() =>
  totalLinkedCount.value === 0 ? 0 : Math.round((doneLinkedCount.value / totalLinkedCount.value) * 100),
);

const statusItems = computed(() =>
  PROJECT_ROADMAP_STATUS_ORDER.map((key) => ({
    key,
    label: taskStatusLabel(key, "state"),
    count: statusCounts.value[key],
  })) satisfies Array<{ key: TaskStatus; label: string; count: number }>,
);

const milestoneViews = computed(() =>
  props.milestones.map((milestone, index) => {
    const milestoneLinks = props.links.filter((link) => link.milestoneId === milestone.id);
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

function isSaving(milestoneId: string): boolean {
  return props.savingMilestoneId === milestoneId;
}

function taskRelationMeta(task: Task): string {
  const parts: string[] = [];
  const childCount = childCountByTaskId.value.get(task.id) ?? 0;
  if (task.parentId) parts.push("子任务");
  if (childCount > 0) parts.push(`${childCount} 子任务`);
  if (task.dependsOn.length > 0) parts.push(`${task.dependsOn.length} 依赖`);
  return parts.join(" · ");
}
</script>

<template>
  <template v-if="tasksReady">
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
  </template>

  <div v-else class="roadmap-view__summary">
    <div class="roadmap-view__summary-main">
      <span class="roadmap-view__eyebrow">路线图</span>
      <h2>Milestone</h2>
      <p>{{ milestones.length }} 个 milestone，正在补齐关联任务统计…</p>
    </div>
    <div class="roadmap-view__score" aria-label="路线图完成度">
      <strong>--</strong>
      <span>关联任务统计加载中</span>
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
        <div class="roadmap-milestone__actions">
          <button
            type="button"
            class="ui-button ui-icon-button"
            :disabled="isSaving(view.milestone.id) || view.index === 0"
            :aria-label="`${view.milestone.title} 上移`"
            title="上移"
            @click="emit('move', view.milestone.id, -1)"
          >
            <ArrowUp :size="14" aria-hidden="true" />
          </button>
          <button
            type="button"
            class="ui-button ui-icon-button"
            :disabled="isSaving(view.milestone.id) || view.index === milestoneViews.length - 1"
            :aria-label="`${view.milestone.title} 下移`"
            title="下移"
            @click="emit('move', view.milestone.id, 1)"
          >
            <ArrowDown :size="14" aria-hidden="true" />
          </button>
          <select
            class="roadmap-milestone__status-select"
            :value="view.milestone.status"
            :disabled="isSaving(view.milestone.id)"
            :aria-label="`${view.milestone.title} 状态`"
            @change="emit('changeStatus', view.milestone.id, $event)"
          >
            <option
              v-for="option in milestoneStatusOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
          <button
            type="button"
            class="ui-button ui-icon-button ui-button--danger"
            :disabled="isSaving(view.milestone.id)"
            :aria-label="`删除 ${view.milestone.title}`"
            title="删除"
            @click="emit('remove', view.milestone)"
          >
            <Trash2 :size="14" aria-hidden="true" />
          </button>
        </div>
      </header>

      <div class="roadmap-milestone__details">
        <label class="roadmap-milestone__field roadmap-milestone__field--description">
          <span>描述</span>
          <textarea
            class="ui-input roadmap-milestone__description"
            :value="view.milestone.description"
            :disabled="isSaving(view.milestone.id)"
            :aria-label="`${view.milestone.title} 描述`"
            placeholder="补充 milestone 目标、范围或验收口径"
            rows="3"
            @change="emit('changeDescription', view.milestone, $event)"
          />
        </label>
        <label class="roadmap-milestone__field">
          <span>截止日期</span>
          <input
            class="ui-input roadmap-milestone__date"
            type="date"
            :value="formatDateInput(view.milestone.dueDate)"
            :disabled="isSaving(view.milestone.id)"
            :aria-label="`${view.milestone.title} 截止日期`"
            @change="emit('changeDueDate', view.milestone, $event)"
          />
        </label>
      </div>

      <div class="roadmap-milestone__progress">
        <div class="roadmap-milestone__progress-bar">
          <span :style="{ width: `${tasksReady ? view.progress : 0}%` }" />
        </div>
        <span>{{ tasksReady ? `${view.progress}%` : "加载中" }}</span>
      </div>

      <div class="roadmap-milestone__columns">
        <template v-if="tasksReady">
          <section class="roadmap-milestone__section">
            <h4>关联任务</h4>
            <ul v-if="view.tasks.length" class="roadmap-task-list">
              <li v-for="task in view.tasks" :key="task.id" class="roadmap-task">
                <RouterLink :to="`/projects/${projectId}/tasks/${task.id}`" class="roadmap-task__link">
                  <span class="roadmap-task__title">{{ task.title }}</span>
                  <span class="roadmap-task__meta">
                    <span :class="`roadmap-task__status roadmap-task__status--${task.status}`">
                      {{ taskStatusLabel(task.status, "state") }}
                    </span>
                    <span v-if="taskRelationMeta(task)" class="roadmap-task__relation">
                      {{ taskRelationMeta(task) }}
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
                    @change="emit('toggleTask', view.milestone.id, view.taskIds, task.id, $event)"
                  />
                  <span class="roadmap-task-choice__title">{{ task.title }}</span>
                  <span :class="`roadmap-task__status roadmap-task__status--${task.status}`">
                    {{ taskStatusLabel(task.status, "state") }}
                  </span>
                  <span v-if="taskRelationMeta(task)" class="roadmap-task__relation">
                    {{ taskRelationMeta(task) }}
                  </span>
                </label>
              </li>
            </ul>
            <p v-else class="roadmap-milestone__muted">
              还没有任务。
            </p>
          </section>
        </template>

        <section v-else class="roadmap-milestone__section" aria-busy="true">
          <h4>关联任务</h4>
          <p class="roadmap-milestone__muted">首屏渲染后补齐任务关联与统计…</p>
        </section>
      </div>

      <footer class="roadmap-milestone__foot">
        <template v-if="tasksReady">
          <span>{{ view.doneCount }}/{{ view.totalCount }} done</span>
          <span>{{ view.totalCount }} 个关联任务</span>
        </template>
        <span v-else>关联任务统计加载中</span>
      </footer>
    </div>
  </section>
</template>
