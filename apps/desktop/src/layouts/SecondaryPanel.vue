<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { AlertTriangle, MessageSquarePlus, X } from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import { listProjects } from "../services/projectsStore";
import {
  createDraftOrphan,
  createDraftTask,
  ensureAllProjectTasksLoaded,
  ensureOrphansLoaded,
  listProjectConversations,
  listOrphanConversations,
} from "../services/tasksStore";
import { useSidebarDisplayMode } from "../composables/useSidebarDisplayMode";
import {
  clearConversationActivityNotice,
  conversationActivityForTask,
} from "../composables/useConversationActivity";
import { useProjectTreeExpansion } from "../composables/useProjectTreeExpansion";
import { useSidebarAddMenu } from "../composables/useSidebarAddMenu";
import { useSidebarTreeDrag } from "../composables/useSidebarTreeDrag";
import SidebarSearch from "../components/sidebar/SidebarSearch.vue";
import SidebarProjectsSection from "../components/sidebar/SidebarProjectsSection.vue";
import SidebarInboxSection from "../components/sidebar/SidebarInboxSection.vue";
import SidebarConnectionFooter from "../components/sidebar/SidebarConnectionFooter.vue";
import SidebarProjectAddMenu from "../components/sidebar/SidebarProjectAddMenu.vue";
import SidebarUnifiedSection from "../components/sidebar/SidebarUnifiedSection.vue";
import type { UnifiedSidebarConversation } from "../components/sidebar/sidebarTypes";

const router = useRouter();
const route = useRoute();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const projectError = ref<string | null>(null);
const searchActive = ref(false);
const unifiedLoaded = ref(false);
const { sidebarDisplayMode } = useSidebarDisplayMode();
const isUnifiedMode = computed(() => sidebarDisplayMode.value === "unified");

function reportProjectError(message: string) {
  projectError.value = message;
}

function dismissError() {
  projectError.value = null;
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

async function loadUnifiedSidebarData() {
  unifiedLoaded.value = false;
  try {
    await Promise.all([
      ensureAllProjectTasksLoaded(),
      ensureOrphansLoaded(),
    ]);
  } catch (err) {
    reportProjectError(`加载会话列表失败：${String(err)}`);
  } finally {
    unifiedLoaded.value = true;
  }
}

const unifiedConversations = computed<UnifiedSidebarConversation[]>(() => {
  const rows: Array<UnifiedSidebarConversation & { createdAt: number; pinned: boolean }> = [];
  for (const project of projects.value) {
    for (const task of listProjectConversations(project.id)) {
      rows.push({
        task,
        projectId: project.id,
        projectName: project.name,
        route: `/projects/${project.id}/tasks/${task.id}`,
        createdAt: task.createdAt,
        pinned: task.pinned,
      });
    }
  }
  for (const orphan of orphans.value) {
    rows.push({
      task: orphan,
      projectId: null,
      projectName: null,
      route: `/chats/${orphan.id}`,
      createdAt: orphan.createdAt,
      pinned: orphan.pinned,
    });
  }
  return rows
    .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.createdAt - a.createdAt)
    .map(({ createdAt: _createdAt, pinned: _pinned, ...row }) => row);
});

watch(
  isUnifiedMode,
  (enabled) => {
    if (enabled) {
      void loadUnifiedSidebarData();
      return;
    }
    void loadInitialSidebarData().catch((err) => {
      reportProjectError(`加载首屏数据失败：${String(err)}`);
    });
  },
  { immediate: true },
);

watch(
  () => route.params.taskId,
  (taskId) => {
    if (typeof taskId === "string") clearConversationActivityNotice(taskId);
  },
  { immediate: true },
);

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

function onSearchSelect(result: { route: string }) {
  router.push(result.route);
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
  <aside
    class="secondary-panel"
    :class="{ 'is-tree-dragging': !isUnifiedMode && treeDrag?.active }"
    @pointerdown="!isUnifiedMode && onTreePointerDown($event)"
    @click.capture="!isUnifiedMode && onTreeClickCapture($event)"
  >
    <div class="sb-section sb-section--actions">
      <button
        v-if="!searchActive"
        type="button"
        class="sb-primary-btn"
        title="新对话"
        aria-label="新对话"
        @click="newChat"
      >
        <MessageSquarePlus :size="15" aria-hidden="true" />
        <span class="sb-primary-btn__label">新对话</span>
      </button>
      <SidebarSearch v-model="searchActive" @select="onSearchSelect" />
    </div>

    <div v-if="isUnifiedMode && projectError" class="sb-banner sb-banner--err">
      <AlertTriangle :size="12" aria-hidden="true" />
      <span class="sb-banner__msg">{{ projectError }}</span>
      <button type="button" class="sb-icon-btn" @click="dismissError" aria-label="忽略错误">
        <X :size="12" aria-hidden="true" />
      </button>
    </div>

    <SidebarUnifiedSection
      v-if="isUnifiedMode"
      :conversations="unifiedConversations"
      :loaded="unifiedLoaded"
      :activity-for-task="conversationActivityForTask"
      :tree-row-state-class="treeRowStateClass"
      @error="reportProjectError"
    />

    <template v-else>
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
        :orphans-expanded="orphansExpanded"
        :orphan-drop-zone-class="orphanDropZoneClass"
        :activity-for-task="conversationActivityForTask"
        :tree-row-state-class="treeRowStateClass"
        @error="reportProjectError"
        @new-chat="newChat"
        @toggle-orphans="toggleOrphans"
      />
    </template>

    <SidebarConnectionFooter />

    <SidebarProjectAddMenu
      :open="addMenuOpen"
      :position="menuPos"
      @close="closeAddMenu"
      @created="onProjectCreated"
      @error="reportProjectError"
    />
  </aside>
</template>
