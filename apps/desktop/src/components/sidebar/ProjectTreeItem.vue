<script setup lang="ts">
import { nextTick, ref } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import {
  Folder,
  FolderOpen,
  LayoutGrid,
  MoreHorizontal,
  Plus,
  Pencil,
  Archive,
  Trash2,
  Code2,
  Pin,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import { vContextMenu } from "../../directives/contextMenu";
import {
  openContextMenuAt,
  type ContextMenuItem,
} from "../../composables/useContextMenu";
import {
  archiveProjectConversations,
  removeProject,
  renameProject,
  toggleProjectPin,
} from "../../services/projectsStore";
import {
  archiveTask,
  listProjectConversations,
  toggleTaskPin,
} from "../../services/tasksStore";
import { openInFileManager, openInVSCode } from "../../services/projects";

type TreeDragKind = "project" | "task";
type TreeDropPosition = "before" | "after" | "inside";

interface TreeDragMarker {
  kind: TreeDragKind;
  active: boolean;
  projectId: string | null;
  taskId: string | null;
}

interface TreeDropMarker {
  kind: "project" | "task" | "orphans";
  projectId: string | null;
  taskId: string | null;
  position: TreeDropPosition;
  valid: boolean;
}

const props = defineProps<{
  project: Project;
  isExpanded: boolean;
  dragSource?: TreeDragMarker | null;
  dropTarget?: TreeDropMarker | null;
}>();

const emit = defineEmits<{
  toggle: [projectId: string];
  newChat: [projectId: string];
  error: [msg: string];
  deleted: [];
  archived: [];
}>();

const route = useRoute();
const router = useRouter();

const editingId = ref<string | null>(null);
const editingValue = ref("");
const editingInput = ref<HTMLInputElement | null>(null);

const confirmingId = ref<string | null>(null);

async function onArchiveClick(e: MouseEvent, taskId: string) {
  e.preventDefault();
  e.stopPropagation();
  if (confirmingId.value === taskId) {
    try {
      const archived = await archiveTask(taskId);
      confirmingId.value = null;
      if (archived && isActiveTask(taskId)) {
        emit("archived");
      }
    } catch (err) {
      confirmingId.value = null;
      emit("error", `归档对话失败：${String(err)}`);
    }
  } else {
    confirmingId.value = taskId;
  }
}

async function onTaskPinClick(e: MouseEvent, taskId: string) {
  e.preventDefault();
  e.stopPropagation();
  try {
    await toggleTaskPin(taskId);
  } catch (err) {
    emit("error", `切换对话置顶失败：${String(err)}`);
  }
}

function onRowLeave() {
  confirmingId.value = null;
}

async function startRename() {
  editingId.value = props.project.id;
  editingValue.value = props.project.name;
  await nextTick();
  editingInput.value?.focus();
  editingInput.value?.select();
}

function commitRename() {
  const id = editingId.value;
  if (!id) return;
  const next = editingValue.value.trim();
  if (next) renameProject(id, next);
  editingId.value = null;
  editingValue.value = "";
}

function cancelRename() {
  editingId.value = null;
  editingValue.value = "";
}

function onEditingKeydown(e: KeyboardEvent) {
  e.stopPropagation();
  if (e.key === "Enter") {
    e.preventDefault();
    commitRename();
  } else if (e.key === "Escape") {
    e.preventDefault();
    cancelRename();
  }
}

function bindEditingInput(el: unknown) {
  editingInput.value = (el as HTMLInputElement | null) ?? null;
}

async function openInExplorer() {
  if (!props.project.cwd) return;
  try {
    await openInFileManager(props.project.cwd);
  } catch (err) {
    emit("error", `在资源管理器中打开失败：${String(err)}`);
  }
}

async function openWithVSCode() {
  if (!props.project.cwd) return;
  try {
    await openInVSCode(props.project.cwd);
  } catch (err) {
    emit("error", `用 VSCode 打开失败：${String(err)}`);
  }
}

async function archiveAllConversations() {
  try {
    await archiveProjectConversations(props.project.id);
    if (
      route.params.projectId &&
      route.params.taskId &&
      String(route.params.projectId) === props.project.id
    ) {
      emit("archived");
    }
  } catch (err) {
    emit("error", `归档所有对话失败：${String(err)}`);
  }
}

async function togglePin() {
  await toggleProjectPin(props.project.id);
}

async function deleteProject() {
  await removeProject(props.project.id);
  if (
    route.params.projectId &&
    String(route.params.projectId) === props.project.id
  ) {
    emit("deleted");
  }
}

function isActiveTask(taskId: string) {
  return route.path === `/projects/${props.project.id}/tasks/${taskId}`;
}

function isActiveProject() {
  return String(route.params.projectId ?? "") === props.project.id &&
    !route.params.taskId;
}

function isSameTreeRow(
  marker: TreeDragMarker | TreeDropMarker | null | undefined,
  kind: TreeDragKind,
  projectId: string | null,
  taskId: string | null,
): boolean {
  return !!marker &&
    marker.kind === kind &&
    marker.projectId === projectId &&
    marker.taskId === taskId;
}

function treeRowStateClass(
  kind: TreeDragKind,
  projectId: string | null,
  taskId: string | null,
) {
  const isSource = props.dragSource?.active === true &&
    isSameTreeRow(props.dragSource, kind, projectId, taskId);
  const isTarget = isSameTreeRow(props.dropTarget, kind, projectId, taskId);
  const position = isTarget ? props.dropTarget?.position : null;
  const valid = isTarget && props.dropTarget?.valid === true;
  return {
    "is-tree-drag-source": isSource,
    "is-tree-drop-target": isTarget,
    "is-tree-drop-invalid": isTarget && !valid,
    "is-tree-drop-before": valid && position === "before",
    "is-tree-drop-after": valid && position === "after",
    "is-tree-drop-inside": valid && position === "inside",
  };
}

function buildMenu(): ContextMenuItem[] {
  const hasCwd = !!props.project.cwd;
  return [
    {
      id: "open-project",
      label: "进入项目",
      icon: LayoutGrid,
      onSelect: () => router.push(`/projects/${props.project.id}`),
    },
    {
      id: "pin",
      label: props.project.pinned ? "取消置顶" : "置顶项目",
      icon: Pin,
      onSelect: () => togglePin(),
    },
    {
      id: "open-explorer",
      label: "在资源管理器中打开",
      icon: FolderOpen,
      disabled: !hasCwd,
      onSelect: () => openInExplorer(),
    },
    {
      id: "open-vscode",
      label: "在 VSCode 中打开",
      icon: Code2,
      disabled: !hasCwd,
      onSelect: () => openWithVSCode(),
    },
    {
      id: "rename",
      label: "重命名项目",
      icon: Pencil,
      onSelect: () => startRename(),
    },
    {
      id: "archive",
      label: "归档所有对话",
      icon: Archive,
      confirmLabel: "确认归档？再点一次",
      onSelect: () => archiveAllConversations(),
    },
    {
      id: "remove",
      label: "移除项目",
      icon: Trash2,
      danger: true,
      confirmLabel: "确认移除？再点一次",
      onSelect: () => deleteProject(),
    },
  ];
}

function onMoreClick(e: MouseEvent) {
  e.stopPropagation();
  const btn = e.currentTarget as HTMLElement | null;
  if (btn) {
    const rect = btn.getBoundingClientRect();
    openContextMenuAt(rect.right, rect.bottom + 2, buildMenu());
  } else {
    openContextMenuAt(e.clientX, e.clientY, buildMenu());
  }
}
</script>

<template>
  <div class="sb-tree__group">
    <div class="sb-tree__row sb-tree__row--project"
      :class="[
        {
          'is-editing': editingId === project.id,
          'is-active': isActiveProject(),
        },
        treeRowStateClass('project', project.id, null),
      ]"
      data-tree-kind="project"
      :data-project-id="project.id"
      :data-pinned="project.pinned ? 'true' : 'false'"
      :aria-expanded="isExpanded"
      v-context-menu="() => buildMenu()"
      @click="emit('toggle', project.id)"
    >
      <span
        class="sb-tree__project-icon"
        aria-hidden="true"
      >
        <FolderOpen v-if="isExpanded" :size="14" aria-hidden="true" />
        <Folder v-else :size="14" aria-hidden="true" />
      </span>
      <input
        v-if="editingId === project.id"
        :ref="bindEditingInput"
        v-model="editingValue"
        type="text"
        class="sb-tree__rename-input"
        spellcheck="false"
        @click.stop
        @pointerdown.stop
        @keydown="onEditingKeydown"
        @blur="commitRename"
      />
      <span
        v-else
        class="sb-tree__link"
      >
        <span class="sb-tree__name">{{ project.name }}</span>
        <Pin v-if="project.pinned" :size="12" class="sb-tree__pin-icon" aria-hidden="true" />
      </span>
      <div v-if="editingId !== project.id" class="sb-tree__hover-tools" @click.stop>
        <button type="button" class="sb-icon-btn" title="更多" aria-label="更多" @click="onMoreClick">
          <MoreHorizontal :size="13" aria-hidden="true" />
        </button>
        <button type="button" class="sb-icon-btn" title="新对话" aria-label="新对话"
          @click="emit('newChat', project.id)">
          <Plus :size="13" aria-hidden="true" />
        </button>
      </div>
    </div>

    <div class="sb-collapse" :class="{ 'is-open': isExpanded }" :aria-hidden="!isExpanded">
      <div class="sb-collapse__inner">
        <RouterLink v-for="c in listProjectConversations(project.id)" :key="c.id"
          :to="`/projects/${project.id}/tasks/${c.id}`" class="sb-tree__row sb-tree__row--child"
          :class="[
            { 'is-active': isActiveTask(c.id) },
            treeRowStateClass('task', project.id, c.id),
          ]"
          draggable="false"
          data-tree-kind="task"
          :data-task-id="c.id"
          :data-project-id="project.id"
          :data-pinned="c.pinned ? 'true' : 'false'"
          @dragstart.prevent
          @mouseleave="onRowLeave">
          <span class="sb-tree__name">{{ c.title }}</span>
          <div class="sb-tree__hover-tools" @click.stop>
            <button type="button" class="sb-icon-btn" :class="{ 'is-pinned': c.pinned }"
              :title="c.pinned ? '取消置顶' : '置顶'"
              :aria-label="c.pinned ? '取消置顶' : '置顶'"
              @click="onTaskPinClick($event, c.id)">
              <Pin :size="13" aria-hidden="true" />
            </button>
            <button type="button" class="sb-icon-btn" :class="{ 'is-confirming': confirmingId === c.id }"
              :title="confirmingId === c.id ? '确认归档，再点一次' : '归档'"
              :aria-label="confirmingId === c.id ? '确认归档，再点一次' : '归档'"
              @click="onArchiveClick($event, c.id)">
              <template v-if="confirmingId === c.id">确认</template>
              <Archive v-else :size="13" aria-hidden="true" />
            </button>
          </div>
        </RouterLink>
        <p v-if="listProjectConversations(project.id).length === 0" class="sb-tree__empty">
          还没有对话
        </p>
      </div>
    </div>
  </div>
</template>
