<script setup lang="ts">
import { useRoute } from "vue-router";
import {
  ChevronsDownUp,
  ChevronsUpDown,
  Plus,
} from "lucide-vue-next";
import type { TreeDragKind } from "../../composables/useSidebarTreeDrag";
import type { OrphanConversation } from "../../services/tasksStore";
import SidebarTaskRow from "./SidebarTaskRow.vue";

defineProps<{
  orphans: OrphanConversation[];
  orphansExpanded: boolean;
  orphanDropZoneClass: Record<string, boolean>;
  treeRowStateClass: (
    kind: TreeDragKind,
    projectId: string | null,
    taskId: string | null,
  ) => Record<string, boolean>;
}>();

const emit = defineEmits<{
  error: [message: string];
  newChat: [];
  toggleOrphans: [];
}>();

const route = useRoute();

function isActiveOrphan(taskId: string) {
  return route.path === `/chats/${taskId}`;
}
</script>

<template>
  <div class="sb-section">
    <div class="sb-section__header">
      <span class="sb-section__title">收集箱</span>
      <div class="sb-section__tools">
        <button type="button" class="sb-icon-btn"
          :title="orphansExpanded ? '折叠收集箱' : '展开收集箱'"
          :aria-label="orphansExpanded ? '折叠收集箱' : '展开收集箱'"
          @click="emit('toggleOrphans')">
          <ChevronsDownUp v-if="orphansExpanded" :size="14" aria-hidden="true" />
          <ChevronsUpDown v-else :size="14" aria-hidden="true" />
        </button>
        <button type="button" class="sb-icon-btn" title="新对话" aria-label="新对话" @click="emit('newChat')">
          <Plus :size="14" aria-hidden="true" />
        </button>
      </div>
    </div>

    <div class="sb-collapse" :class="{ 'is-open': orphansExpanded }" :aria-hidden="!orphansExpanded">
      <div class="sb-collapse__inner">
        <div class="sb-tree" :class="orphanDropZoneClass" data-tree-drop-zone="orphans">
          <SidebarTaskRow
            v-for="o in orphans"
            :key="o.id"
            :task="o"
            :project-id="null"
            :to="`/chats/${o.id}`"
            row-kind="orphan"
            :active="isActiveOrphan(o.id)"
            :tree-row-state-class="treeRowStateClass"
            @error="emit('error', $event)"
          />
          <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
        </div>
      </div>
    </div>
  </div>
</template>
