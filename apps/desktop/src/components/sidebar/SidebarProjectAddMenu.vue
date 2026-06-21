<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from "vue";
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
  SB_MENU_POP_TRANSITION_MS,
  type AnchoredMenuPosition,
} from "../../composables/menuMotion";
import { useAnchoredOverlay } from "../../composables/useAnchoredOverlay";

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
const openState = computed(() => props.open);
const preferredPlacement = computed(() => "bottom-start" as const);
const {
  overlayEl: menuEl,
  overlayStyle,
  setAnchorPoint,
  updatePosition,
} = useAnchoredOverlay({
  open: openState,
  preferredPlacement,
  offset: 0,
});
void menuEl;

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
    setAnchorPoint({
      x: props.position.anchorX,
      y: props.position.anchorY,
    });
    void updatePosition();
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
        :style="overlayStyle"
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
