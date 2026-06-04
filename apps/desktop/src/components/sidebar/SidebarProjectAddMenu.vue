<script setup lang="ts">
import { defineAsyncComponent, ref } from "vue";
import {
  FolderOpen,
  FolderPlus,
  Github,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import {
  createProject,
  deriveProjectName,
} from "../../services/projectsStore";
import { pickFolder } from "../../services/projects";

defineProps<{
  open: boolean;
  position: { x: number; y: number };
}>();

const emit = defineEmits<{
  close: [];
  created: [project: Project];
  error: [message: string];
}>();

const CloneRepoDialog = defineAsyncComponent(
  () => import("./CloneRepoDialog.vue"),
);
const CategoryDialog = defineAsyncComponent(
  () => import("./CategoryDialog.vue"),
);

const cloneOpen = ref(false);
const categoryOpen = ref(false);

async function pickLocalFolder() {
  emit("close");
  try {
    const picked = await pickFolder({ title: "选择项目根目录" });
    if (!picked) return;
    const project = await createProject({
      name: deriveProjectName(picked) || "新项目",
      cwd: picked,
    });
    emit("created", project);
  } catch (err) {
    emit("error", `选择文件夹失败：${String(err)}`);
  }
}

function openClone() {
  emit("close");
  cloneOpen.value = true;
}

function openCategory() {
  emit("close");
  categoryOpen.value = true;
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="sb-menu" role="menu"
      :style="{ left: `${position.x}px`, top: `${position.y}px` }">
      <button type="button" class="sb-menu__item" role="menuitem" @click="pickLocalFolder">
        <FolderOpen :size="13" aria-hidden="true" />
        <span class="sb-menu__label">使用本地文件夹</span>
      </button>
      <button type="button" class="sb-menu__item" role="menuitem" @click="openClone">
        <Github :size="13" aria-hidden="true" />
        <span class="sb-menu__label">从 GitHub clone</span>
      </button>
      <button type="button" class="sb-menu__item" role="menuitem" @click="openCategory">
        <FolderPlus :size="13" aria-hidden="true" />
        <span class="sb-menu__label">创建空分类</span>
      </button>
    </div>
  </Teleport>

  <CloneRepoDialog
    v-if="cloneOpen"
    @close="cloneOpen = false"
    @cloned="emit('created', $event)"
    @error="emit('error', $event)"
  />

  <CategoryDialog
    v-if="categoryOpen"
    @close="categoryOpen = false"
    @created="emit('created', $event)"
  />
</template>
