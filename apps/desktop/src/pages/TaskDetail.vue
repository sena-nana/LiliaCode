<script setup lang="ts">
import { defineAsyncComponent, type Component } from "vue";
import { loadTaskDetailPageContent } from "./taskDetail/taskDetailLazyLoaders";

const props = withDefaults(defineProps<{
  projectId?: string;
  taskId: string;
  variant?: "main" | "popup";
}>(), {
  variant: "main",
});

const TaskDetailLoading = {
  name: "TaskDetailLoading",
  template: "<div aria-live=\"polite\">正在打开对话…</div>",
} satisfies Component;

const TaskDetailPageContent = defineAsyncComponent({
  suspensible: false,
  loadingComponent: TaskDetailLoading,
  loader: () => loadTaskDetailPageContent(),
});
</script>

<template>
  <TaskDetailPageContent
    :project-id="props.projectId"
    :task-id="props.taskId"
    :variant="props.variant"
  />
</template>

