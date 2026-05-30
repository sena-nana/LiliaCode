<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import {
  useChatSidebar,
  type ChatSidebarContext,
} from "../../composables/useChatSidebar";

const props = defineProps<ChatSidebarContext>();

const sidebar = useChatSidebar();
const panels = sidebar.panels;
const activePanel = sidebar.activePanel;
const sidebarState = sidebar.state;
const isResizing = ref(false);

let startX = 0;
let startWidth = 0;

const sidebarContext = computed<ChatSidebarContext>(() => ({
  taskId: props.taskId,
  projectId: props.projectId,
  projectCwd: props.projectCwd,
}));

function onPointerMove(event: PointerEvent) {
  sidebar.setWidth(startWidth + (startX - event.clientX));
}

function onPointerUp(event: PointerEvent) {
  isResizing.value = false;
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  (event.target as Element | null)?.releasePointerCapture?.(event.pointerId);
  sidebar.persistWidth();
}

function startResize(event: PointerEvent) {
  if (!sidebarState.open || event.button !== 0) return;
  event.preventDefault();
  isResizing.value = true;
  startX = event.clientX;
  startWidth = sidebarState.width;
  (event.currentTarget as Element).setPointerCapture?.(event.pointerId);
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
}

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
});
</script>

<template>
  <aside
    class="chat-sidebar"
    :class="{ 'is-open': sidebarState.open, 'is-resizing': isResizing }"
    :style="{ '--chat-sidebar-width': sidebarState.width + 'px' }"
    aria-label="对话侧栏"
    :aria-hidden="sidebarState.open ? undefined : 'true'"
    :inert="sidebarState.open ? undefined : true"
  >
    <div
      class="chat-sidebar__resizer"
      role="separator"
      aria-orientation="vertical"
      :aria-disabled="sidebarState.open ? undefined : 'true'"
      :aria-valuenow="sidebarState.width"
      :aria-valuemin="sidebar.minWidth"
      :aria-valuemax="sidebar.maxWidth"
      title="拖动调整对话侧栏宽度（双击恢复默认）"
      @pointerdown="startResize"
      @dblclick="sidebar.resetWidth"
    />
    <div class="chat-sidebar__inner">
      <header
        v-if="panels.length > 1"
        class="chat-sidebar__head"
      >
        <div
          class="chat-sidebar__tabs"
          role="tablist"
          aria-label="对话侧栏内容"
        >
          <button
            v-for="panel in panels"
            :key="panel.id"
            type="button"
            class="chat-sidebar__tab"
            :class="{ 'is-active': activePanel?.id === panel.id }"
            role="tab"
            :aria-selected="activePanel?.id === panel.id"
            :title="panel.title"
            @click="sidebar.setActivePanel(panel.id)"
          >
            <component
              :is="panel.icon"
              v-if="panel.icon"
              :size="14"
              aria-hidden="true"
            />
            <span>{{ panel.title }}</span>
          </button>
        </div>
      </header>

      <section class="chat-sidebar__body">
        <component
          :is="activePanel.component"
          v-if="activePanel"
          v-bind="sidebarContext"
        />
        <div v-else class="chat-sidebar__empty">
          暂无内容
        </div>
      </section>
    </div>
  </aside>
</template>
