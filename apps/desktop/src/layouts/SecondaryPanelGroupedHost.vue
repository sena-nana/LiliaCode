<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { type Project } from "@lilia/contracts";
import { listProjects } from "../services/projectsStore";
import {
  areOrphansLoaded,
  createDraftOrphan,
  createDraftTask,
  listOrphanConversations,
  listProjectConversations,
} from "../services/tasksStore";
import {
  clearConversationActivityNotice,
  conversationActivityForTask,
  hydrateConversationActivities,
} from "../composables/useConversationActivity";
import { useProjectTreeExpansion } from "../composables/useProjectTreeExpansion";
import { useSidebarAddMenu } from "../composables/useSidebarAddMenu";
import { useSidebarTreeDrag } from "../composables/useSidebarTreeDrag";
import {
  beginPerfStage,
  cancelIdleRun,
  installPerfObservers,
  measurePerfAsync,
  runWhenIdle,
} from "../utils/perf";

const SidebarProjectsSection = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "sidebar.projects-section.load",
    async () => (await import("../components/sidebar/SidebarProjectsSection.vue")).default,
  ),
});

const SidebarInboxSection = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "sidebar.inbox-section.load",
    async () => (await import("../components/sidebar/SidebarInboxSection.vue")).default,
  ),
});

const SidebarProjectAddMenu = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "sidebar.project-add-menu.load",
    async () => (await import("../components/sidebar/SidebarProjectAddMenu.vue")).default,
  ),
});

const router = useRouter();
const route = useRoute();
const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const orphansLoaded = computed(() => areOrphansLoaded());
const projectError = ref<string | null>(null);
let activityHydrationTimer: number | null = null;
let mountIdleMeasureHandle: number | null = null;
let activityHydrationSeq = 0;
let disposed = false;

function reportProjectError(message: string) {
  if (disposed) return;
  projectError.value = message;
}

function dismissError() {
  projectError.value = null;
}

function cancelConversationActivityHydration() {
  if (activityHydrationTimer !== null) {
    cancelIdleRun(activityHydrationTimer);
    activityHydrationTimer = null;
  }
  activityHydrationSeq += 1;
}

function scheduleConversationActivityHydration(
  taskIds: string[],
  priorityTaskIds: string[],
) {
  if (disposed || taskIds.length === 0) return;
  const seq = activityHydrationSeq;
  activityHydrationTimer = runWhenIdle(() => {
    activityHydrationTimer = null;
    if (disposed || seq !== activityHydrationSeq) return;
    void measurePerfAsync(
      "sidebar.grouped.activity",
      () => hydrateConversationActivities(taskIds, { priorityTaskIds }),
      { detail: `${priorityTaskIds.length}/${taskIds.length}` },
    );
  });
}

const {
  allExpanded,
  forgetProject,
  isProjectExpanded,
  loadInitialSidebarData,
  orphansExpanded,
  rememberExpanded,
  toggle,
  toggleAll,
  toggleOrphans,
} = useProjectTreeExpansion(projects, reportProjectError);

const {
  orphanDropZoneClass,
  onTreeClickCapture,
  onTreePointerDown,
  treeDrag,
  treeDropTarget,
  treeRowStateClass,
} = useSidebarTreeDrag(projects, orphans, reportProjectError);

const {
  addMenuOpen,
  closeAddMenu,
  menuPos,
  openAddMenu,
} = useSidebarAddMenu();

const currentTaskId = computed(() =>
  typeof route.params.taskId === "string" ? route.params.taskId : null,
);

const sidebarTaskIds = computed(() => [
  ...projects.value.flatMap((project) => listProjectConversations(project.id).map((task) => task.id)),
  ...orphans.value.map((task) => task.id),
]);

const activityPriorityTaskIds = computed(() => {
  const taskIds = sidebarTaskIds.value;
  return currentTaskId.value ? [currentTaskId.value, ...taskIds] : taskIds;
});

watch(
  () => route.params.taskId,
  (taskId) => {
    if (typeof taskId === "string") clearConversationActivityNotice(taskId);
  },
  { immediate: true },
);

watch(
  () => sidebarTaskIds.value.join("\x1f"),
  () => {
    cancelConversationActivityHydration();
    scheduleConversationActivityHydration(
      sidebarTaskIds.value,
      activityPriorityTaskIds.value,
    );
  },
  { immediate: true },
);

onMounted(() => {
  disposed = false;
  installPerfObservers();
  const stage = beginPerfStage("sidebar.grouped.mount", { detail: route.fullPath });
  mountIdleMeasureHandle = runWhenIdle(() => {
    mountIdleMeasureHandle = null;
    if (disposed) return;
    stage.end("idle");
  });
  void loadInitialSidebarData().catch((err) => {
    reportProjectError(`加载首屏数据失败：${String(err)}`);
  });
});

onBeforeUnmount(() => {
  disposed = true;
  if (mountIdleMeasureHandle !== null) {
    cancelIdleRun(mountIdleMeasureHandle);
    mountIdleMeasureHandle = null;
  }
  cancelConversationActivityHydration();
});

function newChat() {
  const draft = createDraftOrphan();
  router.push(`/chats/${draft.id}`);
}

function newProjectChat(projectId: string) {
  openProjectChat(projectId);
}

function openProjectChat(projectId: string) {
  const draft = createDraftTask(projectId);
  if (!draft) return;
  rememberExpanded(projectId);
  router.push(`/projects/${projectId}/tasks/${draft.id}`);
}

function openProjectsOverview() {
  router.push("/projects");
}

function onProjectArchived(projectId: string) {
  openProjectChat(projectId);
}

function onProjectDeleted(projectId: string) {
  router.push("/");
  forgetProject(projectId);
}

function onProjectCreated(project: Project) {
  openProjectChat(project.id);
}
</script>

<template>
  <div
    class="secondary-panel__grouped"
    :class="{ 'is-tree-dragging': treeDrag?.active }"
    @pointerdown="onTreePointerDown($event)"
    @click.capture="onTreeClickCapture($event)"
  >
    <SidebarProjectsSection
      :add-menu-open="addMenuOpen"
      :all-expanded="allExpanded"
      :drag-source="treeDrag"
      :drop-target="treeDropTarget"
      :activity-for-task="conversationActivityForTask"
      :is-project-expanded="isProjectExpanded"
      :project-error="projectError"
      :projects="projects"
      @archived="onProjectArchived"
      @deleted="onProjectDeleted"
      @dismiss-error="dismissError"
      @error="reportProjectError"
      @new-chat="newProjectChat"
      @open-add-menu="openAddMenu"
      @open-overview="openProjectsOverview"
      @toggle="toggle"
      @toggle-all="toggleAll"
    />

    <SidebarInboxSection
      :orphans="orphans"
      :loaded="orphansLoaded"
      :orphans-expanded="orphansExpanded"
      :orphan-drop-zone-class="orphanDropZoneClass"
      :activity-for-task="conversationActivityForTask"
      :tree-row-state-class="treeRowStateClass"
      @error="reportProjectError"
      @new-chat="newChat"
      @toggle-orphans="toggleOrphans"
    />

    <SidebarProjectAddMenu
      :open="addMenuOpen"
      :position="menuPos"
      @close="closeAddMenu"
      @created="onProjectCreated"
      @error="reportProjectError"
    />
  </div>
</template>
