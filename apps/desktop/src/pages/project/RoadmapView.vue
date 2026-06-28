<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, ref, type Component, watch } from "vue";
import {
  ensureProjectTasksLoaded,
  isProjectTasksLoaded,
  listProjectConversations,
} from "../../services/tasksStore";
import {
  createMilestone,
  deleteMilestone,
  ensureProjectRoadmapLoaded,
  isProjectRoadmapLoaded,
  listProjectMilestoneLinks,
  listProjectMilestones,
  reorderMilestones,
  setMilestoneTasks,
  updateMilestone,
} from "../../services/milestonesStore";
import {
  MILESTONE_STATUSES,
  milestoneStatusLabel,
  normalizeMilestoneStatus,
  type Milestone,
  type Task,
} from "@lilia/contracts";
import {
  beginPerfStage,
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../utils/perf";
import { createLazyLoadState } from "../../utils/lazyLoadState";

const props = defineProps<{ projectId: string }>();
const roadmapMilestoneInsightsLoad = createLazyLoadState<Component>(() =>
  measurePerfAsync(
    "project.roadmap.insights.load",
    async () => (await import("./RoadmapMilestoneInsights.vue")).default as Component,
    { detail: "RoadmapMilestoneInsights" },
  )
);

const RoadmapMilestoneInsights = defineAsyncComponent({
  suspensible: false,
  loader: () => roadmapMilestoneInsightsLoad.load(),
});

const milestoneStatusOptions = MILESTONE_STATUSES.map((value) => ({
  value,
  label: milestoneStatusLabel(value),
}));

const newMilestoneTitle = ref("");
const creatingMilestone = ref(false);
const savingMilestoneId = ref<string | null>(null);
const errorMessage = ref("");
const tasks = computed<Task[]>(() => listProjectConversations(props.projectId));
const milestones = computed<Milestone[]>(() => listProjectMilestones(props.projectId));
const links = computed(() => listProjectMilestoneLinks(props.projectId));
const roadmapLoaded = computed(() => isProjectRoadmapLoaded(props.projectId));
const tasksReady = computed(() => isProjectTasksLoaded(props.projectId));
const loaded = roadmapLoaded;
const insightsReady = ref(false);
let tasksLoadHandle: number | null = null;
let cancelTasksLoadPaint: (() => void) | null = null;
let cancelInsightsPaint: (() => void) | null = null;
let roadmapLoadSeq = 0;
let disposed = false;

function cancelDeferredLoads() {
  cancelTasksLoadPaint?.();
  cancelTasksLoadPaint = null;
  cancelInsightsPaint?.();
  cancelInsightsPaint = null;
  if (tasksLoadHandle !== null) {
    cancelIdleRun(tasksLoadHandle);
    tasksLoadHandle = null;
  }
}

function scheduleTaskHydration(projectId: string, seq: number) {
  if (tasksReady.value) return;
  cancelTasksLoadPaint?.();
  cancelTasksLoadPaint = null;
  cancelTasksLoadPaint = scheduleAfterPaint(() => {
    cancelTasksLoadPaint = null;
    if (seq !== roadmapLoadSeq || tasksReady.value) return;
    tasksLoadHandle = runWhenIdle(() => {
      tasksLoadHandle = null;
      if (seq !== roadmapLoadSeq || tasksReady.value) return;
      void measurePerfAsync(
        "project.roadmap.tasks.load",
        () => ensureProjectTasksLoaded(projectId),
        { detail: projectId },
      ).catch((err) => {
        if (disposed || seq !== roadmapLoadSeq) return;
        errorMessage.value = err instanceof Error ? err.message : String(err);
      });
    });
  });
}

function formatDateInput(timestamp: number | null): string {
  if (timestamp === null) return "";
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "";
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function parseDateInput(value: string): number | null {
  if (!value) return null;
  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day) return null;
  return new Date(year, month - 1, day).getTime();
}

async function addMilestone() {
  const title = newMilestoneTitle.value.trim();
  if (disposed || !title || creatingMilestone.value) return;
  creatingMilestone.value = true;
  errorMessage.value = "";
  try {
    await createMilestone(props.projectId, title);
    if (disposed) return;
    newMilestoneTitle.value = "";
  } catch (err) {
    if (!disposed) errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (!disposed) creatingMilestone.value = false;
  }
}

async function saveMilestoneChange(milestoneId: string, run: () => Promise<void>) {
  if (disposed) return;
  savingMilestoneId.value = milestoneId;
  errorMessage.value = "";
  try {
    await run();
  } catch (err) {
    if (!disposed) errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (!disposed) savingMilestoneId.value = null;
  }
}

async function changeMilestoneStatus(milestoneId: string, event: Event) {
  const currentStatus = milestones.value.find((milestone) => milestone.id === milestoneId)?.status;
  const status = normalizeMilestoneStatus(
    (event.target as HTMLSelectElement).value,
    currentStatus ?? "upcoming",
  );
  await saveMilestoneChange(milestoneId, () =>
    updateMilestone(props.projectId, milestoneId, { status }),
  );
}

async function changeMilestoneDescription(milestone: Milestone, event: Event) {
  const description = (event.target as HTMLTextAreaElement).value.trim();
  if (description === milestone.description) return;
  await saveMilestoneChange(milestone.id, () =>
    updateMilestone(props.projectId, milestone.id, { description }),
  );
}

async function changeMilestoneDueDate(milestone: Milestone, event: Event) {
  const dueDate = parseDateInput((event.target as HTMLInputElement).value);
  if (dueDate === milestone.dueDate) return;
  await saveMilestoneChange(milestone.id, () =>
    updateMilestone(props.projectId, milestone.id, { dueDate }),
  );
}

async function removeMilestone(milestone: Milestone) {
  await saveMilestoneChange(milestone.id, () => deleteMilestone(props.projectId, milestone.id));
}

async function moveMilestone(milestoneId: string, direction: -1 | 1) {
  const currentIds = milestones.value.map((milestone) => milestone.id);
  const index = currentIds.indexOf(milestoneId);
  const nextIndex = index + direction;
  if (index < 0 || nextIndex < 0 || nextIndex >= currentIds.length) return;
  const nextIds = [...currentIds];
  [nextIds[index], nextIds[nextIndex]] = [nextIds[nextIndex], nextIds[index]];
  await saveMilestoneChange(milestoneId, () => reorderMilestones(props.projectId, nextIds));
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
    const seq = ++roadmapLoadSeq;
    insightsReady.value = false;
    errorMessage.value = "";
    cancelDeferredLoads();
    const stage = beginPerfStage("project.roadmap.switch", { detail: projectId });
    void measurePerfAsync(
      "project.roadmap.load",
      () => ensureProjectRoadmapLoaded(projectId),
      { detail: projectId },
    ).then(() => {
      if (seq !== roadmapLoadSeq) {
        stage.end("cancelled");
        return;
      }
      cancelInsightsPaint = scheduleAfterPaint(() => {
        cancelInsightsPaint = null;
        if (seq !== roadmapLoadSeq) {
          stage.end("cancelled");
          return;
        }
        insightsReady.value = true;
        stage.end("paint");
        scheduleTaskHydration(projectId, seq);
      });
    }).catch((err) => {
      if (disposed || seq !== roadmapLoadSeq) return;
      errorMessage.value = err instanceof Error ? err.message : String(err);
      stage.end("error");
    });
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  disposed = true;
  roadmapLoadSeq += 1;
  cancelDeferredLoads();
});
</script>

<template>
  <div class="roadmap-view">
    <form class="roadmap-create" @submit.prevent="addMilestone">
      <input
        v-model="newMilestoneTitle"
        class="ui-input roadmap-create__input"
        data-agent-id="roadmap.create.title"
        aria-label="新 Milestone 标题"
        placeholder="新建 Milestone"
      />
      <button
        type="submit"
        class="ui-button ui-button--primary"
        data-agent-id="roadmap.create.submit"
        :disabled="creatingMilestone || !newMilestoneTitle.trim()"
      >
        新增
      </button>
    </form>
    <p v-if="errorMessage" class="roadmap-view__error" role="alert">{{ errorMessage }}</p>

    <div v-if="!loaded" class="roadmap-view__empty">
      正在加载路线图…
    </div>

    <RoadmapMilestoneInsights
      v-else-if="insightsReady"
      :project-id="projectId"
      :tasks="tasks"
      :milestones="milestones"
      :links="links"
      :tasks-ready="tasksReady"
      :saving-milestone-id="savingMilestoneId"
      :milestone-status-options="milestoneStatusOptions"
      :format-date-input="formatDateInput"
      @change-status="changeMilestoneStatus"
      @change-description="changeMilestoneDescription"
      @change-due-date="changeMilestoneDueDate"
      @remove="removeMilestone"
      @move="moveMilestone"
      @toggle-task="onTaskToggle"
    />

    <div v-else class="roadmap-view__empty">
      正在准备路线图详情…
    </div>
  </div>
</template>

