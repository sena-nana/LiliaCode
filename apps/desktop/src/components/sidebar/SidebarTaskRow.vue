<script setup lang="ts">
import { ref } from "vue";
import { RouterLink } from "vue-router";
import { Archive, ExternalLink, Pin } from "lucide-vue-next";
import type { Task } from "@lilia/contracts";
import { vContextMenu } from "../../directives/contextMenu";
import type { ContextMenuItem } from "../../composables/useContextMenu";
import type { TreeDragKind } from "../../composables/useSidebarTreeDrag";
import { archiveTask, toggleTaskPin } from "../../services/tasksStore";
import { openPopupTask } from "../../services/popupWindows";

type SidebarTask = Pick<Task, "id" | "title" | "pinned">;

const props = defineProps<{
  active: boolean;
  archive?: (taskId: string) => Promise<boolean>;
  projectId: string | null;
  rowKind: "child" | "orphan";
  task: SidebarTask;
  to?: string;
  treeRowStateClass: (
    kind: TreeDragKind,
    projectId: string | null,
    taskId: string | null,
  ) => Record<string, boolean>;
}>();

const emit = defineEmits<{
  archived: [taskId: string];
  error: [message: string];
  open: [taskId: string];
}>();

const confirming = ref(false);

async function onArchiveClick(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (!confirming.value) {
    confirming.value = true;
    return;
  }

  try {
    const archived = await (props.archive ?? archiveTask)(props.task.id);
    confirming.value = false;
    if (archived) emit("archived", props.task.id);
  } catch (err) {
    confirming.value = false;
    emit("error", `归档对话失败：${String(err)}`);
  }
}

async function onPinClick(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  try {
    await toggleTaskPin(props.task.id);
  } catch (err) {
    emit("error", `切换对话置顶失败：${String(err)}`);
  }
}

function onRowLeave() {
  confirming.value = false;
}

async function openInPopup() {
  try {
    await openPopupTask(props.task.id, props.projectId);
  } catch (err) {
    emit("error", `打开弹出窗口对话失败：${String(err)}`);
  }
}

function onAuxClick(e: MouseEvent) {
  if (e.button !== 1) return;
  e.preventDefault();
  e.stopPropagation();
  void openInPopup();
}

function buildMenu(): ContextMenuItem[] {
  return [
    {
      id: "open-popup-task",
      label: "在弹出窗口中打开",
      icon: ExternalLink,
      onSelect: () => openInPopup(),
    },
  ];
}

function onClick() {
  if (props.to) return;
  emit("open", props.task.id);
}
</script>

<template>
  <component
    :is="to ? RouterLink : 'div'"
    :to="to"
    class="sb-tree__row"
    :class="[
      `sb-tree__row--${rowKind}`,
      { 'is-active': active },
      treeRowStateClass('task', projectId, task.id),
    ]"
    draggable="false"
    data-tree-kind="task"
    :data-task-id="task.id"
    :data-project-id="projectId ?? ''"
    :data-pinned="task.pinned ? 'true' : 'false'"
    v-context-menu="() => buildMenu()"
    @click="onClick"
    @dragstart.prevent
    @auxclick="onAuxClick"
    @mouseleave="onRowLeave"
  >
    <span class="sb-tree__name">{{ task.title }}</span>
    <div class="sb-tree__hover-tools" @click.stop>
      <button
        type="button"
        class="sb-icon-btn"
        :class="{ 'is-pinned': task.pinned }"
        :title="task.pinned ? '取消置顶' : '置顶'"
        :aria-label="task.pinned ? '取消置顶' : '置顶'"
        @click="onPinClick"
      >
        <Pin :size="13" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="sb-icon-btn"
        :class="{ 'is-confirming': confirming }"
        :title="confirming ? '确认归档，再点一次' : '归档'"
        :aria-label="confirming ? '确认归档，再点一次' : '归档'"
        @click="onArchiveClick"
      >
        <template v-if="confirming">确认</template>
        <Archive v-else :size="13" aria-hidden="true" />
      </button>
    </div>
  </component>
</template>
