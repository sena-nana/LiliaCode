<script setup lang="ts">
import { defineAsyncComponent, type Component } from "vue";
import { measurePerfAsync } from "../utils/perf";

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

function taskDetailPerfDetail() {
  return `${props.projectId ?? "orphan"}:${props.taskId}:${props.variant ?? "main"}`;
}

const TaskDetailPageContent = defineAsyncComponent({
  suspensible: false,
  loadingComponent: TaskDetailLoading,
  loader: () => measurePerfAsync(
    "task-detail.page-content.load",
    async () => (await import("./taskDetail/TaskDetailPageContent.vue")).default,
    { detail: taskDetailPerfDetail() },
  ),
});
</script>

<template>
  <TaskDetailPageContent
    :project-id="props.projectId"
    :task-id="props.taskId"
    :variant="props.variant"
  />
</template>
