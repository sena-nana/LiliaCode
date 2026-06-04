<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { MessageSquarePlus } from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import { listProjects } from "../services/projectsStore";
import {
  createDraftOrphan,
  createDraftTask,
  listOrphanConversations,
} from "../services/tasksStore";
import { useProjectTreeExpansion } from "../composables/useProjectTreeExpansion";
import { useSidebarAddMenu } from "../composables/useSidebarAddMenu";
import { useSidebarTreeDrag } from "../composables/useSidebarTreeDrag";
import SidebarSearch from "../components/sidebar/SidebarSearch.vue";
import SidebarProjectsSection from "../components/sidebar/SidebarProjectsSection.vue";
import SidebarInboxSection from "../components/sidebar/SidebarInboxSection.vue";
import SidebarConnectionFooter from "../components/sidebar/SidebarConnectionFooter.vue";
import SidebarProjectAddMenu from "../components/sidebar/SidebarProjectAddMenu.vue";

const router = useRouter();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const projectError = ref<string | null>(null);
const searchActive = ref(false);

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

onMounted(() => {
  void loadInitialSidebarData().catch((err) => {
    reportProjectError(`加载首屏数据失败：${String(err)}`);
  });
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
    :class="{ 'is-tree-dragging': treeDrag?.active }"
    @pointerdown="onTreePointerDown"
    @click.capture="onTreeClickCapture"
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

    <SidebarProjectsSection
      :add-menu-open="addMenuOpen"
      :all-expanded="allExpanded"
      :drag-source="treeDrag"
      :drop-target="treeDropTarget"
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
      :tree-row-state-class="treeRowStateClass"
      @error="reportProjectError"
      @new-chat="newChat"
      @toggle-orphans="toggleOrphans"
    />

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
