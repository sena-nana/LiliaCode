/**
 * Todo 服务层：把 Tauri `todo_*` 命令 + `todo-changed` 事件包成 typed 函数。
 *
 * Rust 端字段已 `rename_all = "camelCase"`，前端不需要再做 key 映射。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { TaskTodo } from "@lilia/contracts";

export type { TaskTodo };

export function listTodos(taskId: string): Promise<TaskTodo[]> {
  return invoke<TaskTodo[]>("todo_list", { taskId });
}

export function createTodo(taskId: string, text: string): Promise<TaskTodo> {
  return invoke<TaskTodo>("todo_create", { taskId, text });
}

export interface TodoPatch {
  text?: string;
  done?: boolean;
  order?: number;
}

/**
 * 部分字段更新。未传的字段保持原样；text/done/order 任一传入都会刷新 updatedAt。
 */
export function updateTodo(id: string, patch: TodoPatch): Promise<void> {
  return invoke<void>("todo_update", {
    id,
    text: patch.text ?? null,
    done: patch.done ?? null,
    order: patch.order ?? null,
  });
}

export function deleteTodo(id: string): Promise<void> {
  return invoke<void>("todo_delete", { id });
}

export interface TodoChangedEvent {
  taskId: string;
}

/**
 * 后端在 SDK 的 TodoWrite 工具事件落库后会 emit `todo-changed`，
 * 这里订阅以让 TodoDrawer 自动 refresh。
 */
export function onTodoChanged(
  handler: (e: TodoChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<TodoChangedEvent>("todo-changed", (event) => handler(event.payload));
}
