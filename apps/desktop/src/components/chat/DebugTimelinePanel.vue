<script setup lang="ts">
import {
  FilePen,
  FileText,
  HelpCircle,
  KeyRound,
  ListChecks,
  ListOrdered,
  Rows3,
  SquareStack,
  Terminal,
} from "lucide-vue-next";
import { useDebugTimelineInteractions } from "../../composables/useDebugTimelineInteractions";

const props = defineProps<{
  taskId: string;
}>();

const debugTimeline = useDebugTimelineInteractions(props.taskId);

const groups = [
  {
    id: "interactive",
    label: "交互",
    actions: [
      { id: "plan", label: "计划", icon: ListOrdered, run: debugTimeline.emitPlan },
      { id: "ask-user", label: "单选提问", icon: HelpCircle, run: debugTimeline.emitAskUser },
      { id: "ask-user-multi", label: "多选提问", icon: SquareStack, run: debugTimeline.emitAskUserMulti },
      { id: "ask-user-preview", label: "示例提问", icon: Rows3, run: debugTimeline.emitAskUserPreview },
      { id: "ask-user-flow", label: "多题提问", icon: HelpCircle, run: debugTimeline.emitAskUserFlow },
      { id: "permission", label: "权限申请", icon: KeyRound, run: debugTimeline.emitPermission },
    ],
  },
  {
    id: "cards",
    label: "卡片",
    actions: [
      { id: "todo", label: "待办", icon: ListChecks, run: debugTimeline.emitTodo },
      { id: "command", label: "命令", icon: Terminal, run: debugTimeline.emitCommand },
      { id: "file-read", label: "读文件", icon: FileText, run: debugTimeline.emitFileRead },
      { id: "file-change", label: "改文件", icon: FilePen, run: debugTimeline.emitFileChange },
    ],
  },
];
</script>

<template>
  <div class="debug-panel">
    <section v-for="group in groups" :key="group.id" class="debug-panel__group">
      <div class="debug-panel__group-title">{{ group.label }}</div>
      <button
        v-for="action in group.actions"
        :key="action.id"
        type="button"
        class="debug-panel__button"
        @click="action.run"
      >
        <component :is="action.icon" :size="14" aria-hidden="true" />
        <span>{{ action.label }}</span>
      </button>
    </section>
  </div>
</template>
