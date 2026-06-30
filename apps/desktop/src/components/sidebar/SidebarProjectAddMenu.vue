<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, ref } from "vue";
import {
  FolderOpen,
  FolderPlus,
  GitBranch,
} from "@lucide/vue";
import type { Project } from "@lilia/contracts";
import CategoryDialog from "./CategoryDialog.vue";
import {
  createProject,
  deriveProjectName,
} from "../../services/projectsStore";
import { pickFolder } from "../../services/projects";
import {
  ActionMenuItem,
  AnchoredActionMenu,
  type AnchoredMenuPosition,
} from "@lilia/ui";
import { createLazyLoadState } from "@lilia/ui";

const props = defineProps<{
  open: boolean;
  position: AnchoredMenuPosition;
}>();

const emit = defineEmits<{
  close: [];
  created: [project: Project];
  error: [message: string];
}>();

const cloneRepoDialogLoad = createLazyLoadState(
  async () => (await import("./CloneRepoDialog.vue")).default,
);
const CloneRepoDialog = defineAsyncComponent(
  () => cloneRepoDialogLoad.load(),
);

const cloneOpen = ref(false);
const categoryOpen = ref(false);
let disposed = false;

async function pickLocalFolder() {
  if (disposed) return;
  emit("close");
  try {
    const picked = await pickFolder({ title: "选择项目根目录" });
    if (disposed || !picked) return;
    const project = await createProject({
      name: deriveProjectName(picked) || "新项目",
      cwd: picked,
    });
    if (disposed) return;
    emit("created", project);
  } catch (err) {
    if (disposed) return;
    emit("error", `选择文件夹失败：${String(err)}`);
  }
}

function openClone() {
  if (disposed) return;
  emit("close");
  cloneOpen.value = true;
}

function openCategory() {
  if (disposed) return;
  emit("close");
  categoryOpen.value = true;
}

function onCloneCreated(project: Project) {
  if (disposed) return;
  emit("created", project);
  cloneOpen.value = false;
}

function onCategoryCreated(project: Project) {
  if (disposed) return;
  emit("created", project);
  categoryOpen.value = false;
}

async function confirmCategory(name: string) {
  if (disposed) return;
  try {
    const project = await createProject({ name, cwd: null });
    if (disposed) return;
    onCategoryCreated(project);
  } catch (err) {
    if (disposed) return;
    emit("error", `创建空分类失败：${String(err)}`);
  }
}

onBeforeUnmount(() => {
  disposed = true;
});
</script>

<template>
  <AnchoredActionMenu
    :open="open"
    :position="position"
  >
    <ActionMenuItem
      :icon="FolderOpen"
      agent-id="sidebar.project-add.local-folder"
      @click="pickLocalFolder"
    >
      使用本地文件夹
    </ActionMenuItem>
    <ActionMenuItem
      :icon="GitBranch"
      agent-id="sidebar.project-add.clone"
      @click="openClone"
    >
      从 GitHub clone
    </ActionMenuItem>
    <ActionMenuItem
      :icon="FolderPlus"
      agent-id="sidebar.project-add.category"
      @click="openCategory"
    >
      创建空分类
    </ActionMenuItem>
  </AnchoredActionMenu>

  <CloneRepoDialog
    v-if="cloneOpen"
    @close="cloneOpen = false"
    @cloned="onCloneCreated"
    @error="emit('error', $event)"
  />

  <CategoryDialog
    v-if="categoryOpen"
    @close="categoryOpen = false"
    @confirm="confirmCategory"
  />
</template>

