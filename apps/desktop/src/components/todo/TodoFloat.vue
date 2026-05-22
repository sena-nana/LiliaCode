<script setup lang="ts">
/**
 * Composer 顶贴附 Todo 卡片：替代旧的右侧抽屉。
 *
 * 三态：
 *  - todos.length === 0     → 整块 v-if 不渲染，零脚印。
 *  - todos>0 && collapsed   → 右对齐 chip，显示圆环进度 + 数字 + chevron。
 *  - todos>0 && !collapsed  → 与 composer 同视觉语言的卡片：列表 + 添加输入。
 *
 * 首次 0→>0 自动展开一次；之后保留用户偏好（localStorage）。
 * 切 task 时重置 autoExpanded 闸门 + 重拉。
 */

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  ChevronDown,
  ChevronUp,
  Plus,
  Sparkles,
  Trash2,
  User,
} from "lucide-vue-next";
import {
  createTodo,
  deleteTodo,
  listTodos,
  onTodoChanged,
  updateTodo,
} from "../../services/todos";
import type { TaskTodo } from "@lilia/contracts";
import type { UnlistenFn } from "@tauri-apps/api/event";

const props = defineProps<{ taskId: string }>();

const COLLAPSE_KEY = "lilia.todoFloat.collapsed";

const todos = ref<TaskTodo[]>([]);
const newText = ref("");
const collapsed = ref<boolean>(loadCollapsed());
const error = ref<string | null>(null);
/** 0→>0 自动展开闸门：用户手动 toggle 后重置，可再次触发。 */
let autoExpanded = false;

let unlistenTodoChanged: UnlistenFn | null = null;

function loadCollapsed(): boolean {
  try {
    const v = localStorage.getItem(COLLAPSE_KEY);
    if (v === null) return true;
    return v === "1";
  } catch {
    return true;
  }
}

function persistCollapsed(v: boolean) {
  try {
    localStorage.setItem(COLLAPSE_KEY, v ? "1" : "0");
  } catch {
    /* ignore quota / privacy mode errors */
  }
}

function toggle() {
  collapsed.value = !collapsed.value;
  persistCollapsed(collapsed.value);
  autoExpanded = false;
}

const totalCount = computed(() => todos.value.length);
const doneCount = computed(() => todos.value.filter((t) => t.done).length);
const RING_CIRC = 2 * Math.PI * 7;
const ringDash = computed(() => {
  if (totalCount.value === 0) return `0 ${RING_CIRC}`;
  const done = (doneCount.value / totalCount.value) * RING_CIRC;
  return `${done} ${RING_CIRC}`;
});

async function refresh() {
  if (!props.taskId) return;
  try {
    todos.value = await listTodos(props.taskId);
  } catch (e) {
    error.value = String(e);
  }
}

async function onAdd() {
  const text = newText.value.trim();
  if (!text) return;
  try {
    const created = await createTodo(props.taskId, text);
    todos.value = [...todos.value, created];
    newText.value = "";
  } catch (e) {
    error.value = String(e);
  }
}

async function onToggleDone(todo: TaskTodo) {
  const original = todo.done;
  todo.done = !original;
  try {
    await updateTodo(todo.id, { done: todo.done });
  } catch (e) {
    todo.done = original;
    error.value = String(e);
  }
}

async function onDelete(todo: TaskTodo) {
  const idx = todos.value.findIndex((t) => t.id === todo.id);
  if (idx < 0) return;
  const removed = todos.value[idx];
  todos.value = todos.value.filter((t) => t.id !== todo.id);
  try {
    await deleteTodo(todo.id);
  } catch (e) {
    todos.value = [
      ...todos.value.slice(0, idx),
      removed,
      ...todos.value.slice(idx),
    ];
    error.value = String(e);
  }
}

watch(
  () => todos.value.length,
  (next, prev) => {
    if (prev === 0 && next > 0 && collapsed.value && !autoExpanded) {
      collapsed.value = false;
      persistCollapsed(false);
      autoExpanded = true;
    }
  },
);

watch(
  () => props.taskId,
  () => {
    autoExpanded = false;
    refresh();
  },
);

onMounted(async () => {
  await refresh();
  unlistenTodoChanged = await onTodoChanged((e) => {
    if (e.taskId === props.taskId) refresh();
  });
});

onUnmounted(async () => {
  if (unlistenTodoChanged) {
    try { await unlistenTodoChanged(); } catch { /* ignore */ }
    unlistenTodoChanged = null;
  }
});
</script>

<template>
  <div
    v-if="totalCount > 0"
    class="todo-float"
    :class="{ 'is-collapsed': collapsed }"
  >
    <!-- 折叠态：单 chip -->
    <button
      v-if="collapsed"
      type="button"
      class="todo-float__chip"
      :title="`Todo 进度 ${doneCount}/${totalCount}`"
      @click="toggle"
    >
      <svg class="todo-float__ring" viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="7" class="todo-float__ring-track" />
        <circle
          cx="8"
          cy="8"
          r="7"
          class="todo-float__ring-done"
          :stroke-dasharray="ringDash"
        />
      </svg>
      <span class="todo-float__chip-count">{{ doneCount }} / {{ totalCount }}</span>
      <ChevronUp :size="12" aria-hidden="true" />
    </button>

    <!-- 展开态：卡片 -->
    <template v-else>
      <header class="todo-float__head">
        <span class="todo-float__title">TODO</span>
        <span class="todo-float__count">{{ doneCount }} / {{ totalCount }} done</span>
        <button
          type="button"
          class="todo-float__collapse"
          title="折叠"
          @click="toggle"
        >
          <ChevronDown :size="14" aria-hidden="true" />
        </button>
      </header>

      <ul class="todo-float__list">
        <li
          v-for="t in todos"
          :key="t.id"
          class="todo-float__row"
          :class="{ 'is-done': t.done }"
        >
          <input
            type="checkbox"
            class="todo-float__check"
            :checked="t.done"
            @change="onToggleDone(t)"
          />
          <span
            class="todo-float__source"
            :title="t.source === 'agent' ? 'AI 自动添加' : '手动添加'"
          >
            <Sparkles v-if="t.source === 'agent'" :size="11" />
            <User v-else :size="11" />
          </span>
          <span class="todo-float__text">{{ t.text }}</span>
          <button
            type="button"
            class="todo-float__del"
            title="删除"
            @click="onDelete(t)"
          >
            <Trash2 :size="12" />
          </button>
        </li>
      </ul>

      <form class="todo-float__add" @submit.prevent="onAdd">
        <input
          v-model="newText"
          type="text"
          class="todo-float__input"
          placeholder="添加 Todo…"
        />
        <button
          type="submit"
          class="todo-float__add-btn"
          :disabled="!newText.trim()"
          title="添加"
        >
          <Plus :size="14" />
        </button>
      </form>

      <p v-if="error" class="todo-float__error">{{ error }}</p>
    </template>
  </div>
</template>
