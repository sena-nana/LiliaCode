<script setup lang="ts">
import {
  AlertTriangle,
  ChevronsDownUp,
  ChevronsUpDown,
  LayoutGrid,
  Plus,
  X,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import type {
  TreeDragSource,
  TreeDropTarget,
} from "../../composables/useSidebarTreeDrag";
import ProjectTreeItem from "./ProjectTreeItem.vue";

defineProps<{
  addMenuOpen: boolean;
  allExpanded: boolean;
  dragSource: TreeDragSource | null;
  dropTarget: TreeDropTarget | null;
  isProjectExpanded: (projectId: string) => boolean;
  projectError: string | null;
  projects: Project[];
}>();

const emit = defineEmits<{
  archived: [projectId: string];
  deleted: [projectId: string];
  dismissError: [];
  error: [message: string];
  newChat: [projectId: string];
  openAddMenu: [event: MouseEvent];
  openOverview: [];
  toggle: [projectId: string];
  toggleAll: [];
}>();
</script>

<template>
  <div class="sb-section">
    <div class="sb-section__header">
      <span class="sb-section__title">项目</span>
      <div class="sb-section__tools">
        <button
          type="button"
          class="sb-icon-btn"
          title="项目总览"
          aria-label="项目总览"
          @click="emit('openOverview')"
        >
          <LayoutGrid :size="14" aria-hidden="true" />
        </button>
        <button type="button" class="sb-icon-btn"
          :title="allExpanded ? '全部折叠' : '全部展开'"
          :aria-label="allExpanded ? '全部折叠' : '全部展开'"
          @click="emit('toggleAll')">
          <ChevronsDownUp v-if="allExpanded" :size="14" aria-hidden="true" />
          <ChevronsUpDown v-else :size="14" aria-hidden="true" />
        </button>
        <button type="button" class="sb-icon-btn" title="添加项目" aria-label="添加项目"
          :aria-expanded="addMenuOpen" :aria-haspopup="true"
          @click="emit('openAddMenu', $event)">
          <Plus :size="14" aria-hidden="true" />
        </button>
      </div>
    </div>

    <div v-if="projectError" class="sb-banner sb-banner--err">
      <AlertTriangle :size="12" aria-hidden="true" />
      <span class="sb-banner__msg">{{ projectError }}</span>
      <button type="button" class="sb-icon-btn" @click="emit('dismissError')" aria-label="忽略错误">
        <X :size="12" aria-hidden="true" />
      </button>
    </div>

    <div class="sb-tree">
      <ProjectTreeItem
        v-for="p in projects"
        :key="p.id"
        :project="p"
        :is-expanded="isProjectExpanded(p.id)"
        :drag-source="dragSource"
        :drop-target="dropTarget"
        @toggle="emit('toggle', $event)"
        @new-chat="emit('newChat', $event)"
        @error="emit('error', $event)"
        @archived="emit('archived', p.id)"
        @deleted="emit('deleted', p.id)"
      />

      <p v-if="projects.length === 0" class="sb-tree__empty">暂无项目</p>
    </div>
  </div>
</template>
