<script setup lang="ts">
import { defineAsyncComponent, nextTick, ref, watch } from "vue";
import {
  FolderOpen,
  FolderPlus,
  Github,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import CategoryDialog from "./CategoryDialog.vue";
import {
  createProject,
  deriveProjectName,
} from "../../services/projectsStore";
import { pickFolder } from "../../services/projects";
import {
  resolveMenuTransformOrigin,
  SB_MENU_POP_TRANSITION_MS,
  type AnchoredMenuPosition,
} from "../../composables/menuMotion";

const props = defineProps<{
  open: boolean;
  position: AnchoredMenuPosition;
}>();

const emit = defineEmits<{
  close: [];
  created: [project: Project];
  error: [message: string];
}>();

const CloneRepoDialog = defineAsyncComponent(
  () => import("./CloneRepoDialog.vue"),
);

const cloneOpen = ref(false);
const categoryOpen = ref(false);
const menuEl = ref<HTMLElement | null>(null);
const origin = ref(resolveMenuTransformOrigin(props.position));

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

function onCloneCreated(project: Project) {
  emit("created", project);
  cloneOpen.value = false;
}

function onCategoryCreated(project: Project) {
  emit("created", project);
  categoryOpen.value = false;
}

async function confirmCategory(name: string) {
  try {
    const project = await createProject({ name, cwd: null });
    onCategoryCreated(project);
  } catch (err) {
    emit("error", `创建空分类失败：${String(err)}`);
  }
}

async function updateOrigin() {
  origin.value = resolveMenuTransformOrigin(props.position);
  await nextTick();
  const el = menuEl.value;
  if (!el) return;
  origin.value = resolveMenuTransformOrigin(props.position, el.offsetWidth, el.offsetHeight);
}

watch(
  () => [
    props.open,
    props.position.x,
    props.position.y,
    props.position.anchorX,
    props.position.anchorY,
  ] as const,
  ([open]) => {
    if (!open) return;
    void updateOrigin();
  },
  { immediate: true },
);
</script>

<template>
  <Teleport to="body">
    <Transition name="sb-menu-pop" :duration="SB_MENU_POP_TRANSITION_MS">
      <div
        v-if="open"
        ref="menuEl"
        class="sb-menu"
        role="menu"
        :style="{
          left: `${position.x}px`,
          top: `${position.y}px`,
          '--sb-menu-origin-x': `${origin.x}px`,
          '--sb-menu-origin-y': `${origin.y}px`,
        }"
      >
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
    </Transition>
  </Teleport>

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
