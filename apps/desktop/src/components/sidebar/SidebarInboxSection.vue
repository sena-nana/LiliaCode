<script setup lang="ts">
import { ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import {
  Archive,
  ChevronsDownUp,
  ChevronsUpDown,
  ExternalLink,
  Pin,
  Plus,
} from "lucide-vue-next";
import { vContextMenu } from "../../directives/contextMenu";
import type { ContextMenuItem } from "../../composables/useContextMenu";
import type { TreeDragKind } from "../../composables/useSidebarTreeDrag";
import {
  archiveTask,
  toggleTaskPin,
  type OrphanConversation,
} from "../../services/tasksStore";
import { openPopupTask } from "../../services/popupWindows";

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
const confirmingOrphanId = ref<string | null>(null);

function onOrphanArchiveClick(e: MouseEvent, orphanId: string) {
  e.preventDefault();
  e.stopPropagation();
  if (confirmingOrphanId.value === orphanId) {
    archiveTask(orphanId);
    confirmingOrphanId.value = null;
  } else {
    confirmingOrphanId.value = orphanId;
  }
}

async function onOrphanPinClick(e: MouseEvent, orphanId: string) {
  e.preventDefault();
  e.stopPropagation();
  try {
    await toggleTaskPin(orphanId);
  } catch (err) {
    emit("error", `切换对话置顶失败：${String(err)}`);
  }
}

function onOrphanRowLeave() {
  confirmingOrphanId.value = null;
}

async function openOrphanInPopup(taskId: string) {
  try {
    await openPopupTask(taskId, null);
  } catch (err) {
    emit("error", `打开弹出窗口对话失败：${String(err)}`);
  }
}

function onOrphanAuxClick(e: MouseEvent, taskId: string) {
  if (e.button !== 1) return;
  e.preventDefault();
  e.stopPropagation();
  void openOrphanInPopup(taskId);
}

function buildOrphanMenu(taskId: string): ContextMenuItem[] {
  return [
    {
      id: "open-popup-task",
      label: "在弹出窗口中打开",
      icon: ExternalLink,
      onSelect: () => openOrphanInPopup(taskId),
    },
  ];
}

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
          <RouterLink v-for="o in orphans" :key="o.id" :to="`/chats/${o.id}`" class="sb-tree__row sb-tree__row--orphan"
            :class="[
              { 'is-active': isActiveOrphan(o.id) },
              treeRowStateClass('task', null, o.id),
            ]"
            draggable="false"
            data-tree-kind="task"
            :data-task-id="o.id"
            data-project-id=""
            :data-pinned="o.pinned ? 'true' : 'false'"
            v-context-menu="() => buildOrphanMenu(o.id)"
            @dragstart.prevent
            @auxclick="onOrphanAuxClick($event, o.id)"
            @mouseleave="onOrphanRowLeave">
            <span class="sb-tree__name">{{ o.title }}</span>
            <div class="sb-tree__hover-tools" @click.stop>
              <button type="button" class="sb-icon-btn" :class="{ 'is-pinned': o.pinned }"
                :title="o.pinned ? '取消置顶' : '置顶'"
                :aria-label="o.pinned ? '取消置顶' : '置顶'"
                @click="onOrphanPinClick($event, o.id)">
                <Pin :size="13" aria-hidden="true" />
              </button>
              <button type="button" class="sb-icon-btn" :class="{ 'is-confirming': confirmingOrphanId === o.id }"
                :title="confirmingOrphanId === o.id ? '确认归档，再点一次' : '归档'"
                :aria-label="confirmingOrphanId === o.id ? '确认归档，再点一次' : '归档'"
                @click="onOrphanArchiveClick($event, o.id)">
                <template v-if="confirmingOrphanId === o.id">确认</template>
                <Archive v-else :size="13" aria-hidden="true" />
              </button>
            </div>
          </RouterLink>
          <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
        </div>
      </div>
    </div>
  </div>
</template>
