<script setup lang="ts">
import { defineAsyncComponent, nextTick, onBeforeUnmount, ref, watch, type Component } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  Archive,
  Code2,
  Folder,
  FolderOpen,
  LayoutGrid,
  MessageSquarePlus,
  MoreHorizontal,
  Pencil,
  Pin,
  Plus,
  Trash2,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import type { ConversationActivity } from "../../composables/useConversationActivity";
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
import { archiveTask } from "../../services/tasksStore";
import type {
  TreeDragKind,
  TreeDragSource,
  TreeDropTarget,
} from "../../composables/useSidebarTreeDrag";
import { openInFileManager, openInVSCode } from "../../services/projects";
import { openPopupNewChat } from "../../services/popupWindows";
import {
  beginPerfStage,
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../utils/perf";

const ProjectTreeConversationList = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "sidebar.project-conversation-list.load",
    async () => (await import("./ProjectTreeConversationList.vue")).default as Component,
  ),
});

const props = defineProps<{
  activityForTask: (taskId: string) => ConversationActivity | null;
  project: Project;
  isExpanded: boolean;
  dragSource?: TreeDragSource | null;
  dropTarget?: TreeDropTarget | null;
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
const conversationListReady = ref(false);
let conversationListRevealHandle: number | null = null;
let cancelConversationListRevealPaint: (() => void) | null = null;
let conversationListRevealSeq = 0;
let renameSeq = 0;
let disposed = false;

async function startRename() {
  if (disposed) return;
  const seq = ++renameSeq;
  editingId.value = props.project.id;
  editingValue.value = props.project.name;
  await nextTick();
  if (disposed || seq !== renameSeq || editingId.value !== props.project.id) return;
  const input = editingInput.value;
  input?.focus();
  input?.select();
}

function commitRename() {
  const id = editingId.value;
  if (!id) return;
  const next = editingValue.value.trim();
  if (next) renameProject(id, next);
  editingId.value = null;
  editingValue.value = "";
  renameSeq += 1;
}

function cancelRename() {
  editingId.value = null;
  editingValue.value = "";
  renameSeq += 1;
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
  if (disposed || !props.project.cwd) return;
  try {
    await openInFileManager(props.project.cwd);
  } catch (err) {
    if (!disposed) emit("error", `在资源管理器中打开失败：${String(err)}`);
  }
}

async function openWithVSCode() {
  if (disposed || !props.project.cwd) return;
  try {
    await openInVSCode(props.project.cwd);
  } catch (err) {
    if (!disposed) emit("error", `用 VSCode 打开失败：${String(err)}`);
  }
}

async function archiveAllConversations() {
  if (disposed) return;
  try {
    await archiveProjectConversations(props.project.id);
    if (disposed) return;
    if (
      route.params.projectId &&
      route.params.taskId &&
      String(route.params.projectId) === props.project.id
    ) {
      emit("archived");
    }
  } catch (err) {
    if (!disposed) emit("error", `归档所有对话失败：${String(err)}`);
  }
}

async function togglePin() {
  if (disposed) return;
  await toggleProjectPin(props.project.id);
}

async function deleteProject() {
  if (disposed) return;
  await removeProject(props.project.id);
  if (disposed) return;
  if (
    route.params.projectId &&
    String(route.params.projectId) === props.project.id
  ) {
    emit("deleted");
  }
}

async function openProjectChatInPopup() {
  if (disposed) return;
  try {
    await openPopupNewChat(props.project.id);
  } catch (err) {
    if (!disposed) emit("error", `创建弹出窗口对话失败：${String(err)}`);
  }
}

function onProjectAuxClick(e: MouseEvent) {
  if (e.button !== 1) return;
  e.preventDefault();
  e.stopPropagation();
  void openProjectChatInPopup();
}

async function archiveProjectTask(taskId: string): Promise<boolean> {
  if (disposed) return false;
  try {
    const archived = await archiveTask(taskId);
    if (disposed) return false;
    if (archived) onProjectTaskArchived(taskId);
    return archived;
  } catch (err) {
    if (!disposed) emit("error", `归档对话失败：${String(err)}`);
    return false;
  }
}

function onProjectTaskArchived(taskId: string) {
  if (route.path === `/projects/${props.project.id}/tasks/${taskId}`) {
    emit("archived");
  }
}

function openTask(taskId: string) {
  void router.push(`/projects/${props.project.id}/tasks/${taskId}`);
}

function isActiveProject() {
  return String(route.params.projectId ?? "") === props.project.id &&
    !route.params.taskId;
}

function isSameTreeRow(
  marker: TreeDragSource | TreeDropTarget | null | undefined,
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
      id: "open-popup-new-chat",
      label: "在弹出窗口中创建对话",
      icon: MessageSquarePlus,
      onSelect: () => openProjectChatInPopup(),
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

function scheduleConversationListReveal() {
  if (disposed || conversationListReady.value || !props.isExpanded) return;
  cancelConversationListRevealPaint?.();
  cancelConversationListRevealPaint = null;
  const seq = ++conversationListRevealSeq;
  const stage = beginPerfStage("sidebar.project-conversations.reveal", {
    detail: props.project.id,
  });
  cancelConversationListRevealPaint = scheduleAfterPaint(() => {
    cancelConversationListRevealPaint = null;
    if (disposed || seq !== conversationListRevealSeq || conversationListReady.value || !props.isExpanded) {
      stage.end("cancelled");
      return;
    }
    conversationListRevealHandle = runWhenIdle(() => {
      conversationListRevealHandle = null;
      if (disposed || seq !== conversationListRevealSeq || conversationListReady.value || !props.isExpanded) {
        stage.end("cancelled");
        return;
      }
      conversationListReady.value = true;
      stage.end("idle");
    });
  });
}

watch(
  () => props.isExpanded,
  (isExpanded) => {
    if (isExpanded) {
      scheduleConversationListReveal();
      return;
    }
    conversationListRevealSeq += 1;
    cancelConversationListRevealPaint?.();
    cancelConversationListRevealPaint = null;
    if (conversationListRevealHandle !== null) {
      cancelIdleRun(conversationListRevealHandle);
      conversationListRevealHandle = null;
    }
  },
);

if (props.isExpanded) {
  scheduleConversationListReveal();
}

onBeforeUnmount(() => {
  disposed = true;
  renameSeq += 1;
  cancelConversationListRevealPaint?.();
  cancelConversationListRevealPaint = null;
  if (conversationListRevealHandle !== null) {
    cancelIdleRun(conversationListRevealHandle);
    conversationListRevealHandle = null;
  }
  conversationListRevealSeq += 1;
});
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
      @auxclick="onProjectAuxClick"
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
        <ProjectTreeConversationList
          v-if="conversationListReady && isExpanded"
          :project-id="project.id"
          :activity-for-task="activityForTask"
          :archive="archiveProjectTask"
          :open="openTask"
          :tree-row-state-class="treeRowStateClass"
          @archived="onProjectTaskArchived"
          @error="emit('error', $event)"
        />
        <p v-else-if="isExpanded" class="sb-tree__empty">
          准备对话列表…
        </p>
      </div>
    </div>
  </div>
</template>
