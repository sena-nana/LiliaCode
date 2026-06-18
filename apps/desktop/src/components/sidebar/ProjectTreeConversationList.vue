<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import type { ConversationActivity } from "../../composables/useConversationActivity";
import { isProjectTasksLoaded, listProjectConversations } from "../../services/tasksStore";
import type { TreeDragKind } from "../../composables/useSidebarTreeDrag";
import SidebarTaskRow from "./SidebarTaskRow.vue";

const props = defineProps<{
  activityForTask: (taskId: string) => ConversationActivity | null;
  archive: (taskId: string) => Promise<boolean>;
  open: (taskId: string) => void;
  projectId: string;
  treeRowStateClass: (
    kind: TreeDragKind,
    projectId: string | null,
    taskId: string | null,
  ) => Record<string, boolean>;
}>();

const emit = defineEmits<{
  error: [message: string];
}>();

const route = useRoute();
const PROJECT_CONVERSATION_COLLAPSE_LIMIT = 4;
const overflowExpanded = ref(false);

const projectConversations = computed(() => listProjectConversations(props.projectId));
const projectTasksLoaded = computed(() => isProjectTasksLoaded(props.projectId));

const activeTaskId = computed(() => {
  const taskId = route.params.taskId;
  return String(route.params.projectId ?? "") === props.projectId &&
    typeof taskId === "string"
    ? taskId
    : null;
});

function collapsedConversations() {
  const conversations = projectConversations.value;
  if (conversations.length <= PROJECT_CONVERSATION_COLLAPSE_LIMIT) {
    return conversations;
  }
  const first = conversations.slice(0, PROJECT_CONVERSATION_COLLAPSE_LIMIT);
  const currentActiveTaskId = activeTaskId.value;
  if (!currentActiveTaskId || first.some((conversation) => conversation.id === currentActiveTaskId)) {
    return first;
  }
  const active = conversations.find((conversation) => conversation.id === currentActiveTaskId);
  if (!active) return first;
  return [
    ...first.slice(0, PROJECT_CONVERSATION_COLLAPSE_LIMIT - 1),
    active,
  ];
}

const visibleProjectConversations = computed(() =>
  overflowExpanded.value
    ? projectConversations.value
    : collapsedConversations()
);

const showConversationOverflow = computed(() =>
  !overflowExpanded.value &&
  visibleProjectConversations.value.length < projectConversations.value.length
);

watch(
  () => projectConversations.value.length,
  (conversationCount) => {
    if (conversationCount <= PROJECT_CONVERSATION_COLLAPSE_LIMIT) {
      overflowExpanded.value = false;
    }
  },
);

function revealConversationOverflow() {
  overflowExpanded.value = true;
}

function isActiveTask(taskId: string) {
  return route.path === `/projects/${props.projectId}/tasks/${taskId}`;
}
</script>

<template>
  <SidebarTaskRow
    v-for="conversation in visibleProjectConversations"
    :key="conversation.id"
    :task="conversation"
    :project-id="projectId"
    :activity="activityForTask(conversation.id)"
    row-kind="child"
    :active="isActiveTask(conversation.id)"
    :archive="archive"
    :tree-row-state-class="treeRowStateClass"
    @open="open"
    @error="emit('error', $event)"
  />
  <button
    v-if="showConversationOverflow"
    type="button"
    class="sb-tree__row sb-tree__row--child sb-tree__row--more"
    title="显示剩余对话"
    aria-label="显示剩余对话"
    @click="revealConversationOverflow"
  >
    ...
  </button>
  <p v-if="!projectTasksLoaded" class="sb-tree__empty">
    加载中…
  </p>
  <p v-else-if="projectConversations.length === 0" class="sb-tree__empty">
    还没有对话
  </p>
</template>
