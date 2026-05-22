<script setup lang="ts">
/**
 * TaskDetail 右侧抽屉：当前 Task 的 Todo 列表。
 *
 * - 默认折叠（localStorage key `lilia.todoDrawer.collapsed`），第一条 todo 出现时自动展开。
 * - 折叠态压成 36px 窄条，仅露出展开按钮 + 一个计数 pill；展开态 280px。
 * - 来源标识：user = 用户图标，agent = 火花图标（区分手动加 vs SDK TodoWrite 自动加）。
 * - 监听后端 `todo-changed`：仅当事件 taskId 匹配本面板才 refresh。
 */

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  ChevronsLeft,
  ChevronsRight,
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

const COLLAPSE_KEY = "lilia.todoDrawer.collapsed";

const todos = ref<TaskTodo[]>([]);
const newText = ref("");
const collapsed = ref<boolean>(loadCollapsed());
const loading = ref(false);
const error = ref<string | null>(null);
/** 用于 watch 0→>0 自动展开时，避免连续触发把用户手动折叠的状态又掀开。 */
let autoExpanded = false;

let unlistenTodoChanged: UnlistenFn | null = null;

function loadCollapsed(): boolean {
  try {
    const v = localStorage.getItem(COLLAPSE_KEY);
    if (v === null) return true; // 首次进入默认折叠
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
  // 用户手动操作后，重置 autoExpand 标记，让下一次列表 0→>0 仍能自动展开。
  autoExpanded = false;
}

const totalCount = computed(() => todos.value.length);
const doneCount = computed(() => todos.value.filter((t) => t.done).length);

async function refresh() {
  if (!props.taskId) return;
  loading.value = true;
  error.value = null;
  try {
    todos.value = await listTodos(props.taskId);
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
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
  const next = !todo.done;
  // 乐观更新，失败回滚。
  const original = todo.done;
  todo.done = next;
  try {
    await updateTodo(todo.id, { done: next });
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
    // 失败把那条放回去
    todos.value = [
      ...todos.value.slice(0, idx),
      removed,
      ...todos.value.slice(idx),
    ];
    error.value = String(e);
  }
}

// 0 → >0 自动展开（一次性）。用户手动 toggle 后 autoExpanded 重置，可再次自动展开。
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

// 切 task 时重拉。
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
    if (e.taskId === props.taskId) {
      refresh();
    }
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
  <aside class="todo-drawer" :class="{ 'is-collapsed': collapsed }">
    <header class="todo-drawer__head">
      <button
        type="button"
        class="todo-drawer__toggle"
        :title="collapsed ? '展开 Todo' : '折叠 Todo'"
        @click="toggle"
      >
        <ChevronsRight v-if="collapsed" :size="14" />
        <ChevronsLeft v-else :size="14" />
      </button>
      <template v-if="!collapsed">
        <span class="todo-drawer__title">Todo</span>
        <span v-if="totalCount > 0" class="todo-drawer__count">
          {{ doneCount }}/{{ totalCount }}
        </span>
      </template>
      <span
        v-else-if="totalCount > 0"
        class="todo-drawer__count todo-drawer__count--vertical"
        :title="`${doneCount}/${totalCount}`"
      >
        {{ doneCount }}/{{ totalCount }}
      </span>
    </header>

    <div v-if="!collapsed" class="todo-drawer__body">
      <ul v-if="todos.length" class="todo-drawer__list">
        <li
          v-for="t in todos"
          :key="t.id"
          class="todo-drawer__row"
          :class="{ 'is-done': t.done }"
        >
          <input
            type="checkbox"
            class="todo-drawer__check"
            :checked="t.done"
            @change="onToggleDone(t)"
          />
          <span class="todo-drawer__source" :title="t.source === 'agent' ? 'AI 自动添加' : '手动添加'">
            <Sparkles v-if="t.source === 'agent'" :size="11" />
            <User v-else :size="11" />
          </span>
          <span class="todo-drawer__text">{{ t.text }}</span>
          <button
            type="button"
            class="todo-drawer__del"
            title="删除"
            @click="onDelete(t)"
          >
            <Trash2 :size="12" />
          </button>
        </li>
      </ul>
      <p v-else-if="!loading" class="todo-drawer__empty">
        还没有 Todo。输入新增，或让 AI 用 TodoWrite 自动写入。
      </p>

      <form class="todo-drawer__add" @submit.prevent="onAdd">
        <input
          v-model="newText"
          type="text"
          class="todo-drawer__input"
          placeholder="添加 Todo…"
        />
        <button
          type="submit"
          class="todo-drawer__add-btn"
          :disabled="!newText.trim()"
          title="添加"
        >
          <Plus :size="14" />
        </button>
      </form>

      <p v-if="error" class="todo-drawer__error">{{ error }}</p>
    </div>
  </aside>
</template>

<style scoped>
.todo-drawer {
  flex: 0 0 auto;
  width: 280px;
  background: var(--bg-elev);
  border-left: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  min-height: 0;
  transition: width 0.18s ease;
}

.todo-drawer.is-collapsed {
  width: 36px;
}

.todo-drawer__head {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 8px;
  border-bottom: 1px solid var(--border-soft);
  flex: 0 0 auto;
}

.todo-drawer.is-collapsed .todo-drawer__head {
  flex-direction: column;
  height: auto;
  padding: 6px 4px;
  gap: 6px;
  border-bottom: 0;
}

.todo-drawer__toggle {
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.todo-drawer__toggle:hover {
  background: var(--bg-hover);
  color: var(--text);
  filter: none;
}

.todo-drawer__title {
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.4px;
  text-transform: uppercase;
  color: var(--text-muted);
}

.todo-drawer__count {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-muted);
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--bg-subtle);
  font-variant-numeric: tabular-nums;
}

.todo-drawer__count--vertical {
  margin-left: 0;
  font-size: 10px;
  padding: 1px 5px;
  writing-mode: vertical-rl;
}

.todo-drawer__body {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 8px 10px;
  gap: 8px;
  overflow-y: auto;
}

.todo-drawer__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.todo-drawer__row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 6px;
  border-radius: 4px;
  font-size: 12px;
  color: var(--text);
  position: relative;
}

.todo-drawer__row:hover {
  background: var(--bg-hover);
}

.todo-drawer__row.is-done .todo-drawer__text {
  text-decoration: line-through;
  color: var(--text-muted);
}

.todo-drawer__check {
  /* 覆盖全局 input 默认 32px 高度。 */
  width: 14px;
  height: 14px;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  flex: 0 0 auto;
  cursor: pointer;
}

.todo-drawer__source {
  flex: 0 0 auto;
  width: 16px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-faint);
}

.todo-drawer__text {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
  line-height: 1.4;
}

.todo-drawer__del {
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-faint);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.12s ease, background-color 0.12s ease, color 0.12s ease;
  flex: 0 0 auto;
}

.todo-drawer__row:hover .todo-drawer__del,
.todo-drawer__row:focus-within .todo-drawer__del {
  opacity: 1;
}

.todo-drawer__del:hover {
  background: var(--err-soft);
  color: var(--err);
}

.todo-drawer__empty {
  margin: 8px 4px;
  font-size: 12px;
  color: var(--text-faint);
  line-height: 1.5;
}

.todo-drawer__add {
  margin-top: auto;
  display: flex;
  gap: 4px;
  padding-top: 6px;
  border-top: 1px solid var(--border-soft);
}

.todo-drawer__input {
  flex: 1;
  height: 28px;
  padding: 0 8px;
  font-size: 12px;
  background: var(--bg-subtle);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text);
  font-weight: 500;
}

.todo-drawer__input:focus {
  outline: none;
  border-color: var(--accent);
  background: var(--bg);
}

.todo-drawer__add-btn {
  width: 28px;
  height: 28px;
  padding: 0;
  border-radius: 6px;
  border: 0;
  background: var(--accent);
  color: var(--accent-text);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.todo-drawer__add-btn:disabled {
  background: var(--bg-active);
  color: var(--text-faint);
  cursor: not-allowed;
  opacity: 0.7;
}

.todo-drawer__error {
  margin: 0;
  font-size: 11px;
  color: var(--err);
}
</style>
