<script setup lang="ts">
/**
 * Composer 顶部紧凑 Todo / 引导列表。
 *
 * - Todo：agent 原生 TodoWrite/todo_list 的只读镜像，只展示未完成项。
 * - 引导：Lilia 自己维护的待插入用户消息，用户可立即发送或删除。
 */

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  Bot,
  Goal,
  Pencil,
  RefreshCw,
  Send,
  Sparkles,
  Trash2,
  X,
} from "@lucide/vue";
import {
  deleteTodo,
  listTodos,
  onTodoChanged,
} from "../../services/todos";
import { runUnlistenFns } from "@lilia/ui";
import {
  PENDING_TASK_TODO_GUIDE_STATUS,
  QUEUED_TASK_TODO_GUIDE_STATUS,
  SENT_TASK_TODO_GUIDE_STATUS,
  liliaGoalStatusLabel,
  taskTodoPriorityLabel,
  type LiliaThreadGoal,
  type TaskTodo,
} from "@lilia/contracts";
import type { UnlistenFn } from "@tauri-apps/api/event";

const props = withDefaults(defineProps<{
  taskId: string;
  showGoal?: boolean;
  goal?: LiliaThreadGoal | null;
  goalDisabled?: boolean;
}>(), {
  showGoal: false,
  goal: null,
  goalDisabled: false,
});

const emit = defineEmits<{
  "insert-guide": [todo: TaskTodo];
  "set-lilia-goal": [objective: string];
  "refresh-lilia-goal": [];
  "clear-lilia-goal": [];
}>();

const todos = ref<TaskTodo[]>([]);
const error = ref<string | null>(null);
const savingId = ref<string | null>(null);

let unlistenTodoChanged: UnlistenFn | null = null;
let disposed = false;
let refreshSeq = 0;

const agentTodos = computed(() =>
  todos.value.filter((todo) => todo.source === "agent" && !todo.done),
);

const guides = computed(() =>
  todos.value.filter((todo) =>
    todo.source === "lilia" && todo.guideStatus !== SENT_TASK_TODO_GUIDE_STATUS
  ),
);

const visibleGoal = computed(() =>
  props.showGoal && props.goal?.objective.trim() ? props.goal : null,
);

const hasVisibleTodos = computed(() =>
  visibleGoal.value !== null || agentTodos.value.length > 0 || guides.value.length > 0,
);

const goalText = computed(() =>
  visibleGoal.value?.objective.trim() ?? "",
);

const goalMeta = computed(() => {
  const goal = visibleGoal.value;
  if (!goal) return "";
  const status = liliaGoalStatusLabel(goal.status);
  const used = Number.isFinite(goal.tokensUsed) ? goal.tokensUsed : 0;
  const budget = goal.tokenBudget;
  const tokenText = typeof budget === "number" && Number.isFinite(budget)
    ? `${used}/${budget}`
    : `${used}`;
  return `${status} · ${tokenText} tokens`;
});

async function refresh() {
  const taskId = props.taskId;
  if (!taskId) return;
  const seq = ++refreshSeq;
  try {
    const nextTodos = await listTodos(taskId);
    if (disposed || seq !== refreshSeq || taskId !== props.taskId) return;
    todos.value = nextTodos;
    error.value = null;
  } catch (e) {
    if (disposed || seq !== refreshSeq || taskId !== props.taskId) return;
    error.value = String(e);
  }
}

async function onDelete(todo: TaskTodo) {
  if (todo.source !== "lilia" || todo.guideStatus === QUEUED_TASK_TODO_GUIDE_STATUS || savingId.value) {
    return;
  }
  const taskId = props.taskId;
  savingId.value = todo.id;
  try {
    await deleteTodo(todo.id);
  } catch (e) {
    if (!disposed && taskId === props.taskId) error.value = String(e);
  } finally {
    if (!disposed && taskId === props.taskId && savingId.value === todo.id) savingId.value = null;
  }
}

function onInsertGuide(todo: TaskTodo) {
  if (todo.source !== "lilia" || todo.guideStatus !== PENDING_TASK_TODO_GUIDE_STATUS || savingId.value) {
    return;
  }
  emit("insert-guide", todo);
}

function onSetGoal() {
  if (props.goalDisabled) return;
  const next = window.prompt("Lilia Goal", props.goal?.objective ?? "")?.trim();
  if (!next) return;
  emit("set-lilia-goal", next);
}

function onRefreshGoal() {
  if (props.goalDisabled) return;
  emit("refresh-lilia-goal");
}

function onClearGoal() {
  if (props.goalDisabled || !props.goal) return;
  emit("clear-lilia-goal");
}

watch(
  () => props.taskId,
  () => {
    void refresh();
  },
);

onMounted(async () => {
  disposed = false;
  try {
    const unlisten = await onTodoChanged((e) => {
      if (e.taskId === props.taskId) void refresh();
    });
    if (disposed) {
      runUnlistenFns([unlisten]);
      return;
    }
    unlistenTodoChanged = unlisten;
  } catch (e) {
    if (!disposed) error.value = String(e);
  }
  await refresh();
});

onUnmounted(() => {
  disposed = true;
  refreshSeq += 1;
  if (unlistenTodoChanged) {
    runUnlistenFns([unlistenTodoChanged]);
    unlistenTodoChanged = null;
  }
});
</script>

<template>
  <div v-if="hasVisibleTodos" class="todo-float" aria-label="Todo 与引导" data-agent-id="todo.float">
    <section v-if="visibleGoal" class="todo-float__section todo-float__section--goal">
      <ul class="todo-float__list">
        <li class="todo-float__row todo-float__row--goal">
          <span class="todo-float__source todo-float__source--goal" title="Lilia Goal">
            <Goal :size="12" aria-hidden="true" />
          </span>
          <span class="todo-float__text" :title="goalText">{{ goalText }}</span>
          <span class="todo-float__priority todo-float__priority--goal">{{ goalMeta }}</span>
          <div class="todo-float__actions">
            <button
              type="button"
              class="todo-float__icon-btn"
              data-agent-id="todo.goal.set"
              :disabled="goalDisabled"
              title="设置 Goal"
              aria-label="设置 Lilia Goal"
              @click="onSetGoal"
            >
              <Pencil :size="12" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="todo-float__icon-btn"
              data-agent-id="todo.goal.refresh"
              :disabled="goalDisabled"
              title="刷新 Goal"
              aria-label="刷新 Lilia Goal"
              @click="onRefreshGoal"
            >
              <RefreshCw :size="12" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="todo-float__icon-btn todo-float__icon-btn--danger"
              data-agent-id="todo.goal.clear"
              :disabled="goalDisabled || !goal"
              title="清除 Goal"
              aria-label="清除 Lilia Goal"
              @click="onClearGoal"
            >
              <Trash2 :size="12" aria-hidden="true" />
            </button>
          </div>
        </li>
      </ul>
    </section>

    <section v-if="agentTodos.length" class="todo-float__section">
      <ul class="todo-float__list">
        <li
          v-for="todo in agentTodos"
          :key="todo.id"
          class="todo-float__row todo-float__row--agent"
        >
          <span class="todo-float__source" title="Agent 原生 Todo">
            <Bot :size="12" aria-hidden="true" />
          </span>
          <span class="todo-float__text">{{ todo.text }}</span>
          <span
            class="todo-float__priority"
            :class="`todo-float__priority--${todo.priority}`"
          >
            {{ taskTodoPriorityLabel(todo.priority) }}
          </span>
        </li>
      </ul>
    </section>

    <section v-if="guides.length" class="todo-float__section">
      <ul class="todo-float__list">
        <li
          v-for="todo in guides"
          :key="todo.id"
          class="todo-float__row todo-float__row--guide"
          :class="`is-${todo.guideStatus ?? PENDING_TASK_TODO_GUIDE_STATUS}`"
        >
          <span class="todo-float__source" title="Lilia 引导">
            <Sparkles :size="12" aria-hidden="true" />
          </span>
          <span class="todo-float__text" :title="todo.text">
            {{ todo.text }}
          </span>
          <span
            class="todo-float__priority"
            :class="`todo-float__priority--${todo.priority}`"
          >
            {{ taskTodoPriorityLabel(todo.priority) }}
          </span>
          <div class="todo-float__actions">
            <button
              type="button"
              class="todo-float__icon-btn"
              :data-agent-id="`todo.guide.${todo.id}.insert`"
              :disabled="todo.guideStatus !== PENDING_TASK_TODO_GUIDE_STATUS"
              title="立即插入"
              :aria-label="`立即插入引导：${todo.text}`"
              @click="onInsertGuide(todo)"
            >
              <Send :size="12" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="todo-float__icon-btn todo-float__icon-btn--danger"
              :data-agent-id="`todo.guide.${todo.id}.delete`"
              :disabled="todo.guideStatus === QUEUED_TASK_TODO_GUIDE_STATUS"
              title="删除引导"
              :aria-label="`删除引导：${todo.text}`"
              @click="onDelete(todo)"
            >
              <Trash2 :size="12" aria-hidden="true" />
            </button>
          </div>
        </li>
      </ul>
    </section>

    <p v-if="error" class="todo-float__error">
      <X :size="11" aria-hidden="true" />
      {{ error }}
    </p>
  </div>
</template>

