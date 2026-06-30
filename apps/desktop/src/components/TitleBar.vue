<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import {
  ChevronRight,
  PanelRightClose,
  PanelRightOpen,
} from "@lucide/vue";
import { TitleBar as LiliaTitleBar } from "@lilia/ui";
import { toggleChatSidebar, useChatSidebar } from "../composables/useChatSidebar";
import { useTitleBarCrumbs } from "../composables/useTitleBarCrumbs";

defineProps<{
  leftSidebarCollapsed?: boolean;
  sidebarTogglesDisabled?: boolean;
}>();

defineEmits<{
  toggleLeftSidebar: [];
}>();

const route = useRoute();
const chatSidebar = useChatSidebar();
const { projectId, taskId, crumbs } = useTitleBarCrumbs();

const canToggleChatSidebar = computed(() =>
  !!taskId.value &&
  (route.path.startsWith("/chats/") || !!projectId.value) &&
  chatSidebar.panels.value.length > 0,
);

const leafCrumb = computed(() => {
  const arr = crumbs.value;
  return arr.length > 0 ? arr[arr.length - 1] : null;
});

const nonLeafCrumbs = computed(() => crumbs.value.slice(0, -1));
const prefixKey = computed(() => nonLeafCrumbs.value.map((c) => c.text).join("|"));
const leafKey = computed(() => `${prefixKey.value}|${leafCrumb.value?.text ?? ""}`);

function onToggleChatSidebar() {
  toggleChatSidebar();
}
</script>

<template>
  <LiliaTitleBar
    :left-sidebar-collapsed="leftSidebarCollapsed"
    :sidebar-toggles-disabled="sidebarTogglesDisabled"
    @toggle-left-sidebar="$emit('toggleLeftSidebar')"
  >
    <template #center>
      <div class="titlebar__crumbs" data-agent-id="titlebar.crumbs" data-tauri-drag-region>
        <Transition name="tb-crumbs" mode="out-in">
          <span
            v-if="nonLeafCrumbs.length > 0"
            :key="prefixKey"
            class="titlebar__crumbs-prefix"
            data-tauri-drag-region
          >
            <template v-for="(c, i) in nonLeafCrumbs" :key="i">
              <span
                class="titlebar__crumb"
                :class="{ 'titlebar__crumb--muted': c.muted }"
                :title="c.text"
                data-tauri-drag-region
              >{{ c.text }}</span>
              <ChevronRight
                class="titlebar__crumb-sep"
                :size="12"
                aria-hidden="true"
                data-tauri-drag-region
              />
            </template>
          </span>
        </Transition>

        <Transition name="tb-crumbs" mode="out-in">
          <span
            v-if="leafCrumb"
            :key="leafKey"
            class="titlebar__crumb"
            :class="{
              'titlebar__crumb--muted': leafCrumb.muted,
              'titlebar__crumb--leaf': !leafCrumb.muted,
            }"
            :title="leafCrumb.text"
            data-tauri-drag-region
          >{{ leafCrumb.text }}</span>
        </Transition>
      </div>
    </template>

    <template #right-actions>
      <button
        v-if="canToggleChatSidebar"
        type="button"
        class="titlebar__btn titlebar__chat-sidebar-btn"
        data-agent-id="titlebar.chat-sidebar.toggle"
        :aria-label="chatSidebar.state.open ? '关闭对话侧栏' : '打开对话侧栏'"
        :title="chatSidebar.state.open ? '关闭对话侧栏' : '打开对话侧栏'"
        :aria-pressed="chatSidebar.state.open"
        :disabled="sidebarTogglesDisabled"
        @click="onToggleChatSidebar"
      >
        <PanelRightClose
          v-if="chatSidebar.state.open"
          :size="15"
          aria-hidden="true"
        />
        <PanelRightOpen
          v-else
          :size="15"
          aria-hidden="true"
        />
      </button>
    </template>
  </LiliaTitleBar>
</template>
