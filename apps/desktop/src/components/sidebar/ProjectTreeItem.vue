<script setup lang="ts">
import { nextTick, ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import {
  Folder,
  FolderOpen,
  MoreHorizontal,
  Plus,
  Pencil,
  Archive,
  Trash2,
  Code2,
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
} from "../../services/projectsStore";
import { archiveTask, listProjectConversations } from "../../services/tasksStore";
import { openInFileManager, openInVSCode } from "../../services/projects";

const props = defineProps<{
  project: Project;
  isExpanded: boolean;
}>();

const emit = defineEmits<{
  toggle: [projectId: string];
  newChat: [projectId: string];
  error: [msg: string];
  deleted: [];
  archived: [];
}>();

const route = useRoute();

// --- Inline rename ---
const editingId = ref<string | null>(null);
const editingValue = ref("");
const editingInput = ref<HTMLInputElement | null>(null);

// --- Session 归档确认 ---
const confirmingId = ref<string | null>(null);

function onArchiveClick(e: MouseEvent, taskId: string) {
  e.preventDefault();
  e.stopPropagation();
  if (confirmingId.value === taskId) {
    archiveTask(taskId);
    confirmingId.value = null;
  } else {
    confirmingId.value = taskId;
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

// --- Actions ---
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
      String(route.params.projectId) === props.project.id
    ) {
      emit("archived");
    }
  } catch (err) {
    emit("error", `归档所有对话失败：${String(err)}`);
  }
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

// --- Context menu ---
function buildMenu(): ContextMenuItem[] {
  const hasCwd = !!props.project.cwd;
  return [
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
      :class="{ 'is-open': isExpanded, 'is-editing': editingId === project.id }"
      :role="editingId === project.id ? undefined : 'button'"
      :tabindex="editingId === project.id ? -1 : 0"
      :aria-expanded="isExpanded"
      v-context-menu="() => buildMenu()"
      @click="editingId === project.id ? null : emit('toggle', project.id)"
      @keydown.enter.prevent="editingId === project.id ? null : emit('toggle', project.id)"
      @keydown.space.prevent="editingId === project.id ? null : emit('toggle', project.id)">
      <Folder :size="14" aria-hidden="true" />
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
      <span v-else class="sb-tree__name">{{ project.name }}</span>
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
          :class="{ 'is-active': isActiveTask(c.id) }"
          @mouseleave="onRowLeave">
          <span class="sb-tree__name">{{ c.title }}</span>
          <div class="sb-tree__hover-tools" @click.stop>
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
