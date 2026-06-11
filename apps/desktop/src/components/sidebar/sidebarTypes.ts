import type { Task } from "@lilia/contracts";

export interface UnifiedSidebarConversation {
  task: Pick<Task, "id" | "title" | "pinned" | "createdAt">;
  projectId: string | null;
  projectName: string | null;
  route: string;
}
