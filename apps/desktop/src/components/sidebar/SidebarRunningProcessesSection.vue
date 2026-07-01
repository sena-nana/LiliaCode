<script setup lang="ts">
import Loader2 from "@lucide/vue/dist/esm/icons/loader-circle.mjs";
import Square from "@lucide/vue/dist/esm/icons/square.mjs";
import type { SidebarRunningProcessItem } from "./sidebarTypes";

defineProps<{
  items: SidebarRunningProcessItem[];
  stoppingTaskIds: string[];
}>();

const emit = defineEmits<{
  open: [item: SidebarRunningProcessItem];
  stop: [taskId: string];
}>();

function openFromKey(item: SidebarRunningProcessItem, event: KeyboardEvent) {
  if (event.key !== "Enter" && event.key !== " ") return;
  event.preventDefault();
  emit("open", item);
}
</script>

<template>
  <section v-if="items.length" class="sb-section sb-section--running-processes">
    <div class="sb-section__header">
      <span class="sb-section__title">进行中</span>
    </div>

    <div class="sb-tree sb-tree--running-processes">
      <div
        v-for="item in items"
        :key="item.taskId"
        class="sb-tree__row sb-tree__row--running-process"
        :data-agent-id="`sidebar.running-process.open.${item.taskId}`"
        role="button"
        tabindex="0"
        @click="emit('open', item)"
        @keydown="openFromKey(item, $event)"
      >
        <span class="sb-tree__running-process-icon" aria-hidden="true">
          <Loader2 :size="13" class="sb-tree__activity-icon--spin" />
        </span>
        <span class="sb-tree__name">{{ item.title }}</span>
        <span v-if="item.projectName" class="sb-tree__project-label">{{ item.projectName }}</span>
        <button
      type="button"
      class="sb-icon-btn sb-icon-btn--danger sb-tree__running-process-stop"
      :data-agent-id="`sidebar.running-process.stop.${item.taskId}`"
      :disabled="stoppingTaskIds.includes(item.taskId)"
          :title="stoppingTaskIds.includes(item.taskId) ? '正在停止进程' : '强行停止进程'"
          :aria-label="stoppingTaskIds.includes(item.taskId) ? '正在停止进程' : '强行停止进程'"
          @click.stop="emit('stop', item.taskId)"
        >
          <Loader2
            v-if="stoppingTaskIds.includes(item.taskId)"
            :size="13"
            class="sb-tree__activity-icon--spin"
            aria-hidden="true"
          />
          <Square v-else :size="13" aria-hidden="true" />
        </button>
      </div>
    </div>
  </section>
</template>

