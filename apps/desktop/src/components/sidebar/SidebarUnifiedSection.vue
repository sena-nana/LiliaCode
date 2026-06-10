<script setup lang="ts">
import { useRoute } from "vue-router";
import type { TreeDragKind } from "../../composables/useSidebarTreeDrag";
import type { UnifiedSidebarConversation } from "./sidebarTypes";
import SidebarTaskRow from "./SidebarTaskRow.vue";

defineProps<{
  conversations: UnifiedSidebarConversation[];
  loaded: boolean;
  treeRowStateClass: (
    kind: TreeDragKind,
    projectId: string | null,
    taskId: string | null,
  ) => Record<string, boolean>;
}>();

const emit = defineEmits<{
  error: [message: string];
}>();

const route = useRoute();

function isActiveConversation(item: UnifiedSidebarConversation): boolean {
  return route.path === item.route;
}
</script>

<template>
  <div class="sb-section">
    <div class="sb-section__header">
      <span class="sb-section__title">会话</span>
    </div>

    <div class="sb-tree sb-tree--unified">
      <SidebarTaskRow
        v-for="item in conversations"
        :key="item.task.id"
        :task="item.task"
        :project-id="item.projectId"
        :project-label="item.projectName"
        :to="item.route"
        row-kind="unified"
        :active="isActiveConversation(item)"
        :tree-row-state-class="treeRowStateClass"
        @error="emit('error', $event)"
      />
      <p v-if="!loaded" class="sb-tree__empty">加载中...</p>
      <p v-else-if="conversations.length === 0" class="sb-tree__empty">还没有会话</p>
    </div>
  </div>
</template>
