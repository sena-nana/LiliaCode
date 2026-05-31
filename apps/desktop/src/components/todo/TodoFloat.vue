<script setup lang="ts">
/**
 * Composer 顶部紧凑 Todo / 引导列表。
 *
 * - Todo：agent 原生 TodoWrite/todo_list 的只读镜像，只展示未完成项。
 * - 引导：Lilia 自己维护的待插入用户消息，用户可编辑、删除、调整优先级。
 */

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  Bot,
  Send,
  Sparkles,
  Trash2,
  X,
} from "lucide-vue-next";
import {
  deleteTodo,
  listTodos,
  onTodoChanged,
  updateTodo,
  type TaskTodoPriority,
} from "../../services/todos";
import type { TaskTodo } from "@lilia/contracts";
import type { UnlistenFn } from "@tauri-apps/api/event";

const props = defineProps<{ taskId: string }>();

const emit = defineEmits<{
  "insert-guide": [todo: TaskTodo];
}>();

const todos = ref<TaskTodo[]>([]);
const error = ref<string | null>(null);
const editingId = ref<string | null>(null);
const editText = ref("");
const savingId = ref<string | null>(null);

let unlistenTodoChanged: UnlistenFn | null = null;

const priorityOptions: Array<{ value: TaskTodoPriority; label: string }> = [
  { value: "high", label: "高" },
  { value: "normal", label: "中" },
  { value: "low", label: "低" },
];

const agentTodos = computed(() =>
  todos.value.filter((todo) => todo.source === "agent" && !todo.done),
);

const guides = computed(() =>
  todos.value.filter((todo) => todo.source === "lilia"),
);

const hasVisibleTodos = computed(() =>
  agentTodos.value.length > 0 || guides.value.length > 0,
);

function priorityLabel(priority: TaskTodoPriority): string {
  return priorityOptions.find((option) => option.value === priority)?.label ?? "中";
}

function guideStatusLabel(todo: TaskTodo): string | null {
  if (todo.guideStatus === "queued") return "已排队";
  if (todo.guideStatus === "sent") return "已插入";
  return null;
}

async function refresh() {
  if (!props.taskId) return;
  try {
    todos.value = await listTodos(props.taskId);
    error.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

function beginEdit(todo: TaskTodo) {
  if (todo.source !== "lilia" || savingId.value) return;
  editingId.value = todo.id;
  editText.value = todo.text;
}

function cancelEdit() {
  editingId.value = null;
  editText.value = "";
}

async function saveEdit(todo: TaskTodo) {
  if (todo.source !== "lilia" || editingId.value !== todo.id) return;
  const text = editText.value.trim();
  if (!text) {
    cancelEdit();
    return;
  }
  if (text === todo.text) {
    cancelEdit();
    return;
  }
  savingId.value = todo.id;
  try {
    await updateTodo(todo.id, { text });
    cancelEdit();
  } catch (e) {
    error.value = String(e);
  } finally {
    savingId.value = null;
  }
}

async function setPriority(todo: TaskTodo, priority: TaskTodoPriority) {
  if (todo.source !== "lilia" || todo.priority === priority || savingId.value) return;
  savingId.value = todo.id;
  try {
    await updateTodo(todo.id, { priority });
  } catch (e) {
    error.value = String(e);
  } finally {
    savingId.value = null;
  }
}

async function onDelete(todo: TaskTodo) {
  if (todo.source !== "lilia" || todo.guideStatus === "queued" || savingId.value) return;
  savingId.value = todo.id;
  try {
    await deleteTodo(todo.id);
  } catch (e) {
    error.value = String(e);
  } finally {
    savingId.value = null;
  }
}

function onInsertGuide(todo: TaskTodo) {
  if (todo.source !== "lilia" || todo.guideStatus !== "pending" || savingId.value) return;
  emit("insert-guide", todo);
}

watch(
  () => props.taskId,
  () => {
    cancelEdit();
    refresh();
  },
);

onMounted(async () => {
  unlistenTodoChanged = await onTodoChanged((e) => {
    if (e.taskId === props.taskId) refresh();
  });
  await refresh();
});

onUnmounted(async () => {
  if (unlistenTodoChanged) {
    try { await unlistenTodoChanged(); } catch { /* ignore */ }
    unlistenTodoChanged = null;
  }
});
</script>

<template>
  <div v-if="hasVisibleTodos" class="todo-float" aria-label="Todo 与引导">
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
            {{ priorityLabel(todo.priority) }}
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
          :class="`is-${todo.guideStatus ?? 'pending'}`"
        >
          <span class="todo-float__source" title="Lilia 引导">
            <Sparkles :size="12" aria-hidden="true" />
          </span>

          <input
            v-if="editingId === todo.id"
            v-model="editText"
            class="todo-float__edit"
            type="text"
            aria-label="编辑引导"
            autofocus
            @keydown.enter.prevent="saveEdit(todo)"
            @keydown.esc.prevent="cancelEdit"
            @blur="saveEdit(todo)"
          />
          <button
            v-else
            type="button"
            class="todo-float__text todo-float__text-btn"
            :title="todo.text"
            @click="beginEdit(todo)"
          >
            {{ todo.text }}
          </button>

          <span v-if="guideStatusLabel(todo)" class="todo-float__status">
            {{ guideStatusLabel(todo) }}
          </span>

          <div class="todo-float__priority-group" aria-label="引导优先级">
            <button
              v-for="option in priorityOptions"
              :key="option.value"
              type="button"
              class="todo-float__priority-choice"
              :class="{
                'is-active': todo.priority === option.value,
                [`todo-float__priority-choice--${option.value}`]: true,
              }"
              :aria-pressed="todo.priority === option.value"
              :title="`设为${option.label}优先级`"
              @click="setPriority(todo, option.value)"
            >
              {{ option.label }}
            </button>
          </div>

          <button
            type="button"
            class="todo-float__icon-btn"
            :disabled="todo.guideStatus !== 'pending'"
            title="立即插入"
            :aria-label="`立即插入引导：${todo.text}`"
            @click="onInsertGuide(todo)"
          >
            <Send :size="12" aria-hidden="true" />
          </button>
          <button
            type="button"
            class="todo-float__icon-btn todo-float__icon-btn--danger"
            :disabled="todo.guideStatus === 'queued'"
            title="删除引导"
            :aria-label="`删除引导：${todo.text}`"
            @click="onDelete(todo)"
          >
            <Trash2 :size="12" aria-hidden="true" />
          </button>
        </li>
      </ul>
    </section>

    <p v-if="error" class="todo-float__error">
      <X :size="11" aria-hidden="true" />
      {{ error }}
    </p>
  </div>
</template>
