import type { ChatAttachment } from "./chat";

export type TaskTodoSource = "lilia" | "agent";
export type TaskTodoPriority = "high" | "normal" | "low";
export type TaskTodoGuideStatus = "pending" | "queued" | "sent";

export interface TaskTodo {
  id: string;
  taskId: string;
  text: string;
  done: boolean;
  order: number;
  source: TaskTodoSource;
  priority: TaskTodoPriority;
  guideStatus: TaskTodoGuideStatus | null;
  attachments?: ChatAttachment[];
  createdAt: number;
  updatedAt: number;
}
