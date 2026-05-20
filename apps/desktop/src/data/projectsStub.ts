import type { Project, Task } from "@lilia/contracts";

const PROJECTS: Project[] = [
  {
    id: "lilia",
    name: "Lilia",
    cwd: "c:\\Files\\workspace\\Lilia",
    sessionCount: 2,
  },
  {
    id: "momo",
    name: "Momo",
    cwd: "c:\\Files\\workspace\\Momo",
    sessionCount: 5,
  },
];

const TASKS: Record<string, Task[]> = {
  lilia: [
    {
      id: "t-001",
      projectId: "lilia",
      sessionId: "0192-aaaa-0001",
      title: "搭建 Tauri + Vue 工程骨架",
      status: "running",
      createdAt: Date.now() - 1000 * 60 * 60 * 2,
      parentId: null,
      dependsOn: [],
    },
    {
      id: "t-002",
      projectId: "lilia",
      sessionId: "0192-aaaa-0002",
      title: "接入 Claude Code 会话发现",
      status: "waiting",
      createdAt: Date.now() - 1000 * 60 * 30,
      parentId: null,
      dependsOn: ["t-001"],
    },
  ],
  momo: [
    {
      id: "m-001",
      projectId: "momo",
      sessionId: "0192-bbbb-0001",
      title: "Widget 拖拽优化",
      status: "done",
      createdAt: Date.now() - 1000 * 60 * 60 * 24 * 3,
      parentId: null,
      dependsOn: [],
    },
  ],
};

/**
 * 侧边栏第三区域用的「零散对话」：还没绑定到任何项目的 Session/Task。
 * 数据形状沿用 Task，只是 projectId 为 null —— 用单独的视图类型表示。
 */
export interface OrphanConversation {
  id: string;
  sessionId: string;
  title: string;
  createdAt: number;
}

const ORPHAN_CONVERSATIONS: OrphanConversation[] = [
  {
    id: "o-001",
    sessionId: "0192-zzzz-0001",
    title: "随手问问 Claude：tsconfig paths",
    createdAt: Date.now() - 1000 * 60 * 12,
  },
  {
    id: "o-002",
    sessionId: "0192-zzzz-0002",
    title: "整理 Yarn 4 workspaces 笔记",
    createdAt: Date.now() - 1000 * 60 * 60 * 6,
  },
];

export function listProjects(): Project[] {
  return PROJECTS;
}

export function getProject(id: string): Project | undefined {
  return PROJECTS.find((p) => p.id === id);
}

export function listTasks(projectId: string): Task[] {
  return TASKS[projectId] ?? [];
}

export function getTask(projectId: string, taskId: string): Task | undefined {
  return (TASKS[projectId] ?? []).find((t) => t.id === taskId);
}

/** 侧边栏项目树里，挂在每个 Project 下面的对话节点。 */
export function listProjectConversations(projectId: string): Task[] {
  return TASKS[projectId] ?? [];
}

/** 侧边栏第三区域的零散对话。 */
export function listOrphanConversations(): OrphanConversation[] {
  return ORPHAN_CONVERSATIONS;
}
