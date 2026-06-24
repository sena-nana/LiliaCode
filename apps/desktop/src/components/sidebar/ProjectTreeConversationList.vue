<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import type { Task } from "@lilia/contracts";
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
  archived: [taskId: string];
  error: [message: string];
}>();

const route = useRoute();
const PROJECT_CONVERSATION_COLLAPSE_LIMIT = 4;
const overflowExpanded = ref(false);

const projectConversations = computed(() => listProjectConversations(props.projectId));
const projectTasksLoaded = computed(() => isProjectTasksLoaded(props.projectId));

interface ConversationTreeRow {
  task: Task;
  depth: number;
  metaParts: string[];
}

const activeTaskId = computed(() => {
  const taskId = route.params.taskId;
  return String(route.params.projectId ?? "") === props.projectId &&
    typeof taskId === "string"
    ? taskId
    : null;
});

function taskMetaParts(task: Task, childCount: number): string[] {
  const parts: string[] = [];
  if (task.parentId) parts.push("子任务");
  if (task.status === "blocked") parts.push("阻塞");
  if (task.dependsOn.length > 0) parts.push(`${task.dependsOn.length} 依赖`);
  if (childCount > 0) parts.push(`${childCount} 子任务`);
  return parts;
}

const projectConversationRows = computed<ConversationTreeRow[]>(() => {
  const conversations = projectConversations.value;
  const byParent = new Map<string | null, Task[]>();
  for (const task of conversations) {
    const parentId = conversations.some((candidate) => candidate.id === task.parentId)
      ? task.parentId
      : null;
    const list = byParent.get(parentId) ?? [];
    list.push(task);
    byParent.set(parentId, list);
  }

  const out: ConversationTreeRow[] = [];
  const seen = new Set<string>();
  function visit(parentId: string | null, depth: number) {
    for (const task of byParent.get(parentId) ?? []) {
      if (seen.has(task.id)) continue;
      seen.add(task.id);
      const childCount = byParent.get(task.id)?.length ?? 0;
      out.push({ task, depth, metaParts: taskMetaParts(task, childCount) });
      visit(task.id, depth + 1);
    }
  }

  visit(null, 0);
  for (const task of conversations) {
    if (!seen.has(task.id)) {
      out.push({ task, depth: 0, metaParts: taskMetaParts(task, 0) });
    }
  }
  return out;
});

function collapsedConversations() {
  const rows = projectConversationRows.value;
  if (rows.length <= PROJECT_CONVERSATION_COLLAPSE_LIMIT) {
    return rows;
  }
  const first = rows.slice(0, PROJECT_CONVERSATION_COLLAPSE_LIMIT);
  const currentActiveTaskId = activeTaskId.value;
  if (!currentActiveTaskId || first.some((row) => row.task.id === currentActiveTaskId)) {
    return first;
  }
  const active = rows.find((row) => row.task.id === currentActiveTaskId);
  if (!active) return first;
  return [
    ...first.slice(0, PROJECT_CONVERSATION_COLLAPSE_LIMIT - 1),
    active,
  ];
}

const visibleProjectConversations = computed(() =>
  overflowExpanded.value
    ? projectConversationRows.value
    : collapsedConversations()
);

const showConversationOverflow = computed(() =>
  !overflowExpanded.value &&
  visibleProjectConversations.value.length < projectConversationRows.value.length
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
    :key="conversation.task.id"
    :task="conversation.task"
    :depth="conversation.depth"
    :meta-parts="conversation.metaParts"
    :project-id="projectId"
    :activity="activityForTask(conversation.task.id)"
    row-kind="child"
    :data-agent-id="`sidebar.task.${conversation.task.id}`"
    :active="isActiveTask(conversation.task.id)"
    :archive="archive"
    :tree-row-state-class="treeRowStateClass"
    @archived="emit('archived', $event)"
    @open="open"
    @error="emit('error', $event)"
  />
  <button
    v-if="showConversationOverflow"
    type="button"
    class="sb-tree__row sb-tree__row--child sb-tree__row--more"
    :data-agent-id="`sidebar.project.${projectId}.show-more`"
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
