<script setup lang="ts">
import { computed, ref } from "vue";
import { RouterLink } from "vue-router";
import { AlertTriangle, Archive, Check, CircleHelp, ExternalLink, Loader2, MessageSquarePlus, Pin } from "lucide-vue-next";
import type { Task } from "@lilia/contracts";
import type { ConversationActivity } from "../../composables/useConversationActivity";
import { clearConversationActivityNotice } from "../../composables/useConversationActivity";
import type { ContextMenuItem } from "../../composables/useContextMenu";
import type { TreeDragKind } from "../../composables/useSidebarTreeDrag";
import { scheduleTaskDetailPreload } from "../../router";
import { archiveTask, toggleTaskPin } from "../../services/tasksStore";
import { openPopupChildQuestion, openPopupTask } from "../../services/popupWindows";

type SidebarTask = Pick<Task, "id" | "title" | "pinned">;

const props = defineProps<{
  active: boolean;
  archive?: (taskId: string) => Promise<boolean>;
  activity?: ConversationActivity | null;
  depth?: number;
  metaParts?: string[];
  projectLabel?: string | null;
  projectId: string | null;
  readonly?: boolean;
  rowKind: "child" | "orphan" | "unified";
  task: SidebarTask;
  to?: string;
  treeRowStateClass: (
    kind: TreeDragKind,
    projectId: string | null,
    taskId: string | null,
  ) => Record<string, boolean>;
}>();

const emit = defineEmits<{
  archived: [taskId: string];
  error: [message: string];
  open: [taskId: string];
}>();

const confirming = ref(false);
const taskDepthStyle = computed(() => ({
  "--task-tree-depth": String(Math.min(Math.max(props.depth ?? 0, 0), 6)),
}));

const activityLabel: Record<ConversationActivity, string> = {
  running: "对话中",
  requires_action: "等待交互",
  completed: "对话完成",
  error: "发生错误",
};

async function archiveCurrentTask() {
  try {
    const archived = await (props.archive ?? archiveTask)(props.task.id);
    confirming.value = false;
    if (archived) emit("archived", props.task.id);
  } catch (err) {
    confirming.value = false;
    emit("error", `归档对话失败：${String(err)}`);
  }
}

async function toggleCurrentTaskPin() {
  try {
    await toggleTaskPin(props.task.id);
  } catch (err) {
    emit("error", `切换对话置顶失败：${String(err)}`);
  }
}

async function onArchiveClick(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (!confirming.value) {
    confirming.value = true;
    return;
  }

  await archiveCurrentTask();
}

async function onPinClick(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  await toggleCurrentTaskPin();
}

function onRowLeave() {
  confirming.value = false;
}

function preloadConversationDetail() {
  if (!props.to) return;
  scheduleTaskDetailPreload(`sidebar-task:${props.task.id}`);
}

async function openInPopup() {
  try {
    await openPopupTask(props.task.id, props.projectId);
  } catch (err) {
    emit("error", `打开弹出窗口对话失败：${String(err)}`);
  }
}

async function askInPopup() {
  try {
    await openPopupChildQuestion(props.task.id, props.projectId);
  } catch (err) {
    emit("error", `创建弹出窗口子对话失败：${String(err)}`);
  }
}

function onAuxClick(e: MouseEvent) {
  if (e.button !== 1) return;
  e.preventDefault();
  e.stopPropagation();
  void openInPopup();
}

function buildMenu(): ContextMenuItem[] {
  return [
    {
      id: "continue-popup-task",
      label: "在弹出窗口继续",
      icon: ExternalLink,
      onSelect: () => openInPopup(),
    },
    {
      id: "ask-popup-task",
      label: "在弹出窗口询问",
      icon: MessageSquarePlus,
      onSelect: () => askInPopup(),
    },
    {
      id: "pin-task",
      label: props.task.pinned ? "取消置顶" : "置顶",
      icon: Pin,
      onSelect: () => toggleCurrentTaskPin(),
    },
    {
      id: "archive-task",
      label: "归档",
      icon: Archive,
      danger: true,
      confirmLabel: "确认归档？再点一次",
      onSelect: () => archiveCurrentTask(),
    },
  ];
}

function onClick() {
  clearConversationActivityNotice(props.task.id);
  if (props.readonly) {
    emit("open", props.task.id);
    return;
  }
  if (props.to) return;
  emit("open", props.task.id);
}
</script>

<template>
  <component
    :is="to ? RouterLink : 'button'"
    :to="to"
    type="button"
    class="sb-tree__row"
    :style="taskDepthStyle"
    :class="[
      `sb-tree__row--${rowKind}`,
      { 'is-active': active },
      treeRowStateClass('task', projectId, task.id),
    ]"
    draggable="false"
    data-tree-kind="task"
    :data-task-id="task.id"
    :data-project-id="projectId ?? ''"
    :data-pinned="task.pinned ? 'true' : 'false'"
    v-context-menu="readonly ? null : () => buildMenu()"
    @click="onClick"
    @dragstart.prevent
    @auxclick="onAuxClick"
    @mouseenter="preloadConversationDetail"
    @focusin="preloadConversationDetail"
    @mouseleave="onRowLeave"
  >
    <span class="sb-tree__name">{{ task.title }}</span>
    <span v-if="metaParts?.length" class="sb-tree__meta">{{ metaParts.join(" · ") }}</span>
    <span v-if="projectLabel" class="sb-tree__project-label">{{ projectLabel }}</span>
    <span
      v-if="activity && !active"
      class="sb-tree__activity"
      :class="`sb-tree__activity--${activity}`"
      :aria-label="activityLabel[activity]"
      :title="activityLabel[activity]"
    >
      <Loader2
        v-if="activity === 'running'"
        :size="13"
        class="sb-tree__activity-icon sb-tree__activity-icon--spin"
        aria-hidden="true"
      />
      <CircleHelp
        v-else-if="activity === 'requires_action'"
        :size="13"
        class="sb-tree__activity-icon"
        aria-hidden="true"
      />
      <AlertTriangle
        v-else-if="activity === 'error'"
        :size="13"
        class="sb-tree__activity-icon"
        aria-hidden="true"
      />
      <Check
        v-else
        :size="13"
        class="sb-tree__activity-icon"
        aria-hidden="true"
      />
    </span>
    <div v-if="!readonly" class="sb-tree__hover-tools" @click.stop>
      <button
        type="button"
        class="sb-icon-btn"
        :class="{ 'is-pinned': task.pinned }"
        :title="task.pinned ? '取消置顶' : '置顶'"
        :aria-label="task.pinned ? '取消置顶' : '置顶'"
        @click="onPinClick"
      >
        <Pin :size="13" aria-hidden="true" />
      </button>
      <button
        type="button"
        class="sb-icon-btn"
        :class="{ 'is-confirming': confirming }"
        :title="confirming ? '确认归档，再点一次' : '归档'"
        :aria-label="confirming ? '确认归档，再点一次' : '归档'"
        @click="onArchiveClick"
      >
        <template v-if="confirming">确认</template>
        <Archive v-else :size="13" aria-hidden="true" />
      </button>
    </div>
  </component>
</template>
