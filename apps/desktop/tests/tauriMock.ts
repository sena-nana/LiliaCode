import { vi } from "vitest";

interface ProjectRow {
  id: string;
  name: string;
  cwd: string | null;
  sessionCount: number;
  sortOrder: number;
  pinned: boolean;
}

interface TaskRow {
  id: string;
  projectId: string | null;
  sessionId: string;
  title: string;
  status: string;
  createdAt: number;
  parentId: string | null;
  dependsOn: string[];
  sortOrder: number;
  pinned: boolean;
}

const baseProjects: ProjectRow[] = [
  {
    id: "lilia",
    name: "Lilia",
    cwd: "D:\\PROJECT\\workspace\\Lilia",
    sessionCount: 2,
    sortOrder: 0,
    pinned: false,
  },
  {
    id: "tools",
    name: "工具箱",
    cwd: "D:\\PROJECT\\workspace\\tools",
    sessionCount: 1,
    sortOrder: 1,
    pinned: false,
  },
];

const baseTasks: TaskRow[] = [
  {
    id: "t-001",
    projectId: "lilia",
    sessionId: "0192-lilia-0001",
    title: "接入 Claude Code 会话发现",
    status: "done",
    createdAt: 1000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
  {
    id: "t-002",
    projectId: "lilia",
    sessionId: "0192-lilia-0002",
    title: "打通 tsconfig paths 搜索",
    status: "running",
    createdAt: 2000,
    parentId: null,
    dependsOn: ["t-001"],
    sortOrder: 1,
    pinned: false,
  },
  {
    id: "t-003",
    projectId: "tools",
    sessionId: "0192-tools-0001",
    title: "整理窗口快捷键",
    status: "waiting",
    createdAt: 3000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
  {
    id: "o-001",
    projectId: null,
    sessionId: "0192-orphan-0001",
    title: "随手问问 Claude：tsconfig paths",
    status: "running",
    createdAt: 4000,
    parentId: null,
    dependsOn: [],
    sortOrder: 0,
    pinned: false,
  },
];

let projects: ProjectRow[] = [];
let tasks: TaskRow[] = [];

function cloneProject(row: ProjectRow): ProjectRow {
  return { ...row };
}

function cloneTask(row: TaskRow): TaskRow {
  return { ...row, dependsOn: [...row.dependsOn] };
}

function refreshSessionCounts() {
  projects = projects.map((project) => ({
    ...project,
    sessionCount: tasks.filter((task) =>
      task.projectId === project.id && task.status !== "cancelled"
    ).length,
  }));
}

export function resetTauriMockData() {
  projects = baseProjects.map(cloneProject);
  tasks = baseTasks.map(cloneTask);
  refreshSessionCounts();
  mockInvoke.mockClear();
}

export const mockInvoke = vi.fn(async (cmd: string, args: Record<string, unknown> = {}) => {
  switch (cmd) {
    case "project_list":
      refreshSessionCounts();
      return projects
        .map(cloneProject)
        .sort((a, b) => a.sortOrder - b.sortOrder);

    case "project_get": {
      const id = String(args.id);
      refreshSessionCounts();
      return projects.find((project) => project.id === id) ?? null;
    }

    case "project_create": {
      const name = String(args.name || "未命名项目");
      const cwd = typeof args.cwd === "string" ? args.cwd : null;
      const project: ProjectRow = {
        id: `p-${projects.length + 1}`,
        name,
        cwd,
        sessionCount: 0,
        sortOrder: projects.length,
        pinned: false,
      };
      projects.push(project);
      return cloneProject(project);
    }

    case "project_remove": {
      const id = String(args.id);
      const before = projects.length;
      projects = projects.filter((project) => project.id !== id);
      tasks = tasks.map((task) =>
        task.projectId === id ? { ...task, projectId: null } : task
      );
      refreshSessionCounts();
      return projects.length !== before;
    }

    case "task_list": {
      const projectId = args.projectId as string | null | undefined;
      return tasks
        .filter((task) => task.projectId === (projectId ?? null))
        .map(cloneTask)
        .sort((a, b) =>
          Number(b.pinned) - Number(a.pinned) || a.sortOrder - b.sortOrder
        );
    }

    case "task_toggle_pin": {
      const id = String(args.id);
      let pinned = false;
      tasks = tasks.map((task) => {
        if (task.id !== id) return task;
        pinned = !task.pinned;
        return { ...task, pinned };
      });
      return pinned;
    }

    case "task_archive_project": {
      const projectId = String(args.projectId);
      const count = tasks.filter((task) => task.projectId === projectId).length;
      tasks = tasks.filter((task) => task.projectId !== projectId);
      refreshSessionCounts();
      return count;
    }

    default:
      throw new Error(`未配置的 Tauri mock 命令：${cmd}`);
  }
});

resetTauriMockData();

