/**
 * Todo 服务层：把 Tauri `todo_*` 命令 + `todo-changed` 事件包成 typed 函数。
 *
 * Rust 端字段已 `rename_all = "camelCase"`，前端不需要再做 key 映射。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ChatAttachment,
  TaskTodo,
  TaskTodoGuideStatus,
  TaskTodoPriority,
} from "@lilia/contracts";

export type { TaskTodo };
export type { TaskTodoGuideStatus, TaskTodoPriority };

export interface AgentTodoInput {
  content?: string;
  text?: string;
  title?: string;
  description?: string;
  status?: string;
  completed?: boolean;
  done?: boolean;
  priority?: string;
}

export function listTodos(taskId: string): Promise<TaskTodo[]> {
  return invoke<TaskTodo[]>("todo_list", { taskId });
}

export function createTodo(
  taskId: string,
  text: string,
  priority: TaskTodoPriority = "normal",
  attachments: ChatAttachment[] = [],
): Promise<TaskTodo> {
  return invoke<TaskTodo>("todo_create", { taskId, text, priority, attachments });
}

export interface TodoPatch {
  text?: string;
  done?: boolean;
  order?: number;
  priority?: TaskTodoPriority;
  guideStatus?: TaskTodoGuideStatus;
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
    priority: patch.priority ?? null,
    guideStatus: patch.guideStatus ?? null,
  });
}

export function deleteTodo(id: string): Promise<void> {
  return invoke<void>("todo_delete", { id });
}

export function applyAgentTodoEvent(
  taskId: string,
  todos: AgentTodoInput[],
): Promise<TaskTodo[]> {
  return invoke<TaskTodo[]>("todo_apply_agent_event", { taskId, todos });
}

export interface TodoChangedEvent {
  taskId: string;
}

/**
 * 后端在 SDK 的 TodoWrite 工具事件落库后会 emit `todo-changed`，
 * 这里订阅以让 TodoFloat 自动 refresh。
 */
export function onTodoChanged(
  handler: (e: TodoChangedEvent) => void,
): Promise<UnlistenFn> {
  return listen<TodoChangedEvent>("todo-changed", (event) => handler(event.payload));
}
