<script setup lang="ts">
import { FolderPlus } from "@lucide/vue";
import type { Project } from "@lilia/contracts";
import { useAnchoredActionMenu } from "@lilia/ui";
import SidebarProjectAddMenu from "../sidebar/SidebarProjectAddMenu.vue";

defineProps<{
  projects: Project[];
}>();

const emit = defineEmits<{
  "select-project": [projectId: string];
  "created-project": [project: Project];
  error: [message: string];
}>();

const {
  open: addMenuOpen,
  close: closeAddMenu,
  position: menuPos,
  openAtEvent: openAddMenu,
} = useAnchoredActionMenu();

function onSelect(event: Event) {
  const target = event.target as HTMLSelectElement | null;
  if (!target) return;
  const projectId = target.value;
  if (!projectId) return;
  emit("select-project", projectId);
  target.value = "";
}
</script>

<template>
  <div class="composer-project-picker" aria-label="选择对话项目">
    <select
      class="composer-project-picker__select"
      data-agent-id="chat.composer.project-picker.select"
      aria-label="选择项目"
      @change="onSelect"
    >
      <option value="">选择项目工作...</option>
      <option
        v-for="project in projects"
        :key="project.id"
        :value="project.id"
      >
        {{ project.name }}
      </option>
    </select>
    <button
      type="button"
      class="composer-project-picker__new"
      data-agent-id="chat.composer.project-picker.new"
      title="打开新项目"
      aria-label="打开新项目"
      @click="openAddMenu"
    >
      <FolderPlus :size="14" aria-hidden="true" />
      打开新项目
    </button>
    <SidebarProjectAddMenu
      :open="addMenuOpen"
      :position="menuPos"
      @close="closeAddMenu"
      @created="emit('created-project', $event)"
      @error="emit('error', $event)"
    />
  </div>
</template>

