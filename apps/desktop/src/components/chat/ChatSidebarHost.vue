<script setup lang="ts">
import { computed, markRaw, ref, shallowRef, watch, type Component } from "vue";
import {
  CHAT_SIDEBAR_DEFAULT_WIDTH,
  CHAT_SIDEBAR_MAX_WIDTH,
  CHAT_SIDEBAR_MIN_WIDTH,
  CHAT_SIDEBAR_WIDTH_STORAGE_KEY,
  useChatSidebar,
  type ChatSidebarContext,
} from "../../composables/useChatSidebar";
import { useResizablePane } from "../../composables/useResizablePane";
import { withComponentEpoch } from "../../composables/useComponentEpoch";
import { measurePerfAsync } from "../../utils/perf";

const props = defineProps<ChatSidebarContext>();

const sidebar = useChatSidebar();
const panels = sidebar.panels;
const activePanel = sidebar.activePanel;
const sidebarState = sidebar.state;
const sidebarWidth = useResizablePane({
  storageKey: CHAT_SIDEBAR_WIDTH_STORAGE_KEY,
  minWidth: CHAT_SIDEBAR_MIN_WIDTH,
  maxWidth: CHAT_SIDEBAR_MAX_WIDTH,
  defaultWidth: CHAT_SIDEBAR_DEFAULT_WIDTH,
  edge: "left",
  disabled: computed(() => !sidebarState.open),
});

const sidebarContext = computed<ChatSidebarContext>(() => ({
  taskId: props.taskId,
  projectId: props.projectId,
  projectCwd: props.projectCwd,
}));

const panelComponentCache = new Map<string, Component>();
const activePanelComponent = shallowRef<Component | null>(null);
const activePanelLoading = ref(false);
const panelLoadEpoch = withComponentEpoch();

watch(
  () => [sidebarState.open, activePanel.value?.id ?? ""] as const,
  async ([open, panelId]) => {
    const seq = panelLoadEpoch.nextEpoch();
    if (!open || !panelId) {
      activePanelComponent.value = null;
      activePanelLoading.value = false;
      return;
    }
    const panel = activePanel.value;
    if (!panel) {
      activePanelComponent.value = null;
      activePanelLoading.value = false;
      return;
    }
    const cached = panelComponentCache.get(panel.id);
    if (cached) {
      if (!panelLoadEpoch.assertAlive(seq)) return;
      activePanelComponent.value = cached;
      activePanelLoading.value = false;
      return;
    }
    activePanelLoading.value = true;
    try {
      const component = markRaw(await measurePerfAsync(
        "chat-sidebar.panel.load",
        () => panel.loader(),
        { detail: panel.id },
      ));
      if (!panelLoadEpoch.assertAlive(seq) || !sidebarState.open || activePanel.value?.id !== panel.id) return;
      panelComponentCache.set(panel.id, component);
      activePanelComponent.value = component;
    } catch (err) {
      if (!panelLoadEpoch.assertAlive(seq)) return;
      console.error("[chat-sidebar] load panel failed", panel.id, err);
      if (sidebarState.open && activePanel.value?.id === panel.id) {
        activePanelComponent.value = null;
      }
    } finally {
      if (panelLoadEpoch.assertAlive(seq) && sidebarState.open && activePanel.value?.id === panel.id) {
        activePanelLoading.value = false;
      }
    }
  },
  { immediate: true },
);

</script>

<template>
  <aside
    class="chat-sidebar"
    :class="{ 'is-open': sidebarState.open, 'is-resizing': sidebarWidth.isResizing.value }"
    :style="{ '--chat-sidebar-width': sidebarWidth.width.value + 'px' }"
    aria-label="对话侧栏"
    :aria-hidden="sidebarState.open ? undefined : 'true'"
    :inert="sidebarState.open ? undefined : true"
  >
    <div
      class="chat-sidebar__resizer"
      role="separator"
      aria-orientation="vertical"
      :aria-disabled="sidebarState.open ? undefined : 'true'"
      :aria-valuenow="sidebarWidth.width.value"
      :aria-valuemin="sidebarWidth.minWidth"
      :aria-valuemax="sidebarWidth.maxWidth"
      title="拖动调整对话侧栏宽度（双击恢复默认）"
      @pointerdown="sidebarWidth.startResize"
      @dblclick="sidebarWidth.resetWidth"
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
          :is="activePanelComponent"
          v-if="activePanel && activePanelComponent"
          v-bind="sidebarContext"
        />
        <div v-else-if="activePanelLoading" class="chat-sidebar__empty">
          正在加载...
        </div>
        <div v-else class="chat-sidebar__empty">
          暂无内容
        </div>
      </section>
    </div>
  </aside>
</template>
