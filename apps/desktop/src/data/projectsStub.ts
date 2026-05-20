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
