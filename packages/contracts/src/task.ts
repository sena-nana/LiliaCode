export type TaskStatus =
  | "draft"
  | "waiting"
  | "running"
  | "blocked"
  | "done"
  | "cancelled";

export interface Task {
  id: string;
  projectId: string;
  sessionId: string;
  title: string;
  status: TaskStatus;
  createdAt: number;
  pinned: boolean;
  parentId: string | null;
  dependsOn: string[];
}

export interface TaskGraph {
  tasks: Task[];
  childrenByParent: Record<string, string[]>;
}

export type MemoryScope = "user" | "project";

export interface Memory {
  id: string;
  scope: MemoryScope;
  projectId: string | null;
  title: string;
  body: string;
  tags: string[];
  enabled: boolean;
  sourceTaskId: string | null;
  createdAt: number;
  updatedAt: number;
}

export type MilestoneStatus =
  | "upcoming"
  | "in-progress"
  | "done"
  | "abandoned";

export interface Milestone {
  id: string;
  projectId: string;
  title: string;
  description: string;
  status: MilestoneStatus;
  dueDate: number | null;
  order: number;
  createdAt: number;
}

export interface TaskMilestoneLink {
  taskId: string;
  milestoneId: string;
}
