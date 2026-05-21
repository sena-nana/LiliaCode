import { ref } from "vue";
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

const ORPHAN_LIST = ref<OrphanConversation[]>([
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
]);

/**
 * 草稿对话：点了「新对话」但还没发出第一条消息的会话。
 * 不进侧栏，只在内存里活着；首条发送成功后会被 promote 进 ORPHAN_LIST。
 */
const DRAFTS = new Map<string, OrphanConversation>();

function makeId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
}

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
  return ORPHAN_LIST.value;
}

export function getOrphanConversation(id: string): OrphanConversation | undefined {
  return DRAFTS.get(id) ?? ORPHAN_LIST.value.find((o) => o.id === id);
}

export function isDraftOrphan(id: string): boolean {
  return DRAFTS.has(id);
}

/** 点「新对话」按钮时调用：产出一条只活在内存里的草稿。 */
export function createDraftOrphan(): OrphanConversation {
  const id = makeId("o-draft");
  const draft: OrphanConversation = {
    id,
    sessionId: id,
    title: "新对话",
    createdAt: Date.now(),
  };
  DRAFTS.set(id, draft);
  return draft;
}

/**
 * 草稿发出第一条消息后调用：把它从 DRAFTS 移到 ORPHAN_LIST，
 * title 用首条消息预览代替占位。若同 id 已经在列表里则忽略。
 */
export function promoteDraftOrphan(id: string, title: string): void {
  const draft = DRAFTS.get(id);
  if (!draft) return;
  DRAFTS.delete(id);
  if (ORPHAN_LIST.value.some((o) => o.id === id)) return;
  ORPHAN_LIST.value = [
    {
      ...draft,
      title: title || draft.title,
      createdAt: Date.now(),
    },
    ...ORPHAN_LIST.value,
  ];
}
