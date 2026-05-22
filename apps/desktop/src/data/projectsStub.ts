import { ref } from "vue";
import type { Project, Task } from "@lilia/contracts";

const PROJECTS = ref<Project[]>([
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
]);

const TASKS = ref<Record<string, Task[]>>({
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
});

/**
 * 「收集箱」：还没绑定到任何项目的 Session/Task。形状沿用 Task，projectId 为 null。
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
 * 草稿：点了「新对话」但还没发出第一条消息的会话。不进侧栏，首条发送成功后 promote 进 ORPHAN_LIST。
 */
const DRAFTS = new Map<string, OrphanConversation>();

/**
 * 项目内草稿：点了项目行的「+」按钮创建的会话。同样首条发送前不进 TASKS。
 */
const DRAFT_TASKS = new Map<string, Task>();

function makeId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
}

export function listProjects(): Project[] {
  return PROJECTS.value;
}

export function getProject(id: string): Project | undefined {
  return PROJECTS.value.find((p) => p.id === id);
}

export function listTasks(projectId: string): Task[] {
  return TASKS.value[projectId] ?? [];
}

export function getTask(projectId: string, taskId: string): Task | undefined {
  const draft = DRAFT_TASKS.get(taskId);
  if (draft && draft.projectId === projectId) return draft;
  return (TASKS.value[projectId] ?? []).find((t) => t.id === taskId);
}

/** 侧边栏项目树里挂在每个 Project 下面的对话节点。 */
export function listProjectConversations(projectId: string): Task[] {
  return TASKS.value[projectId] ?? [];
}

/** 侧边栏第三区域的收集箱对话。 */
export function listOrphanConversations(): OrphanConversation[] {
  return ORPHAN_LIST.value;
}

export function getOrphanConversation(id: string): OrphanConversation | undefined {
  return DRAFTS.get(id) ?? ORPHAN_LIST.value.find((o) => o.id === id);
}

export function isDraftOrphan(id: string): boolean {
  return DRAFTS.has(id);
}

/** 点「新对话」时调用：产出一条只活在内存里的草稿。 */
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

/** 点项目行「+」时调用：产出一条绑定到该项目的草稿任务，首条消息发出前不进 TASKS。 */
export function createDraftTask(projectId: string): Task | undefined {
  if (!PROJECTS.value.some((p) => p.id === projectId)) return undefined;
  const id = makeId("t-draft");
  const draft: Task = {
    id,
    projectId,
    sessionId: id,
    title: "新对话",
    status: "draft",
    createdAt: Date.now(),
    parentId: null,
    dependsOn: [],
  };
  DRAFT_TASKS.set(id, draft);
  return draft;
}

export function isDraftTask(id: string): boolean {
  return DRAFT_TASKS.has(id);
}

/**
 * 草稿发出第一条消息后调用：从 DRAFTS 移到 ORPHAN_LIST，title 用首条消息预览代替占位。
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

/**
 * 项目内草稿发出第一条消息后调用：从 DRAFT_TASKS 移到对应项目的 TASKS 头部。
 */
export function promoteDraftTask(id: string, title: string): void {
  const draft = DRAFT_TASKS.get(id);
  if (!draft) return;
  DRAFT_TASKS.delete(id);
  const existing = TASKS.value[draft.projectId] ?? [];
  if (existing.some((t) => t.id === id)) return;
  TASKS.value = {
    ...TASKS.value,
    [draft.projectId]: [
      {
        ...draft,
        title: title || draft.title,
        status: "running",
        createdAt: Date.now(),
      },
      ...existing,
    ],
  };
}

/**
 * 侧栏「添加项目」入口：本地文件夹 / clone / 空分类三类都进这里。
 * cwd 传 null 表示「分类型」项目，仅做侧栏归类用。
 */
export function createProject(input: { name: string; cwd: string | null }): Project {
  const trimmedName = input.name.trim();
  const project: Project = {
    id: makeId("p"),
    name: trimmedName || "未命名项目",
    cwd: input.cwd && input.cwd.trim() ? input.cwd.trim() : null,
    sessionCount: 0,
  };
  PROJECTS.value = [...PROJECTS.value, project];
  return project;
}

/** 更新项目名称；trim 后为空时不改动。返回是否真正更新。 */
export function renameProject(id: string, nextName: string): boolean {
  const name = nextName.trim();
  if (!name) return false;
  const idx = PROJECTS.value.findIndex((p) => p.id === id);
  if (idx < 0) return false;
  if (PROJECTS.value[idx].name === name) return false;
  const next = [...PROJECTS.value];
  next[idx] = { ...next[idx], name };
  PROJECTS.value = next;
  return true;
}

/**
 * 「归档所有对话」：把该项目下的全部 Task + 草稿 Task 清空。
 * 当前 stub 没有「已归档」状态字段，行为退化为「从侧栏隐藏」——
 * 接 SQLite 后改成给 Task 打 archived flag，sidebar 默认过滤。
 * 返回清掉的数量（含草稿），方便调用方做提示。
 */
export function archiveProjectConversations(projectId: string): number {
  const existing = TASKS.value[projectId] ?? [];
  const cleared = existing.length;
  if (cleared > 0) {
    TASKS.value = { ...TASKS.value, [projectId]: [] };
  }
  let draftCleared = 0;
  for (const [draftId, draft] of DRAFT_TASKS) {
    if (draft.projectId === projectId) {
      DRAFT_TASKS.delete(draftId);
      draftCleared += 1;
    }
  }
  return cleared + draftCleared;
}

/**
 * 「移除项目」：从侧栏摘掉项目本身，连带把它的 TASKS / 草稿任务一并清掉。
 * 不动磁盘上的 cwd 目录（哪怕是 clone 进来的也只是「从 Lilia 视野里移除」）。
 */
export function removeProject(id: string): boolean {
  const before = PROJECTS.value.length;
  PROJECTS.value = PROJECTS.value.filter((p) => p.id !== id);
  if (PROJECTS.value.length === before) return false;
  if (TASKS.value[id]) {
    const next = { ...TASKS.value };
    delete next[id];
    TASKS.value = next;
  }
  for (const [draftId, draft] of DRAFT_TASKS) {
    if (draft.projectId === id) DRAFT_TASKS.delete(draftId);
  }
  return true;
}

/** 从绝对路径取末尾段作为项目名候选；Windows / Unix 分隔符都吃。 */
export function deriveProjectName(absPath: string): string {
  const cleaned = absPath.trim().replace(/[\\/]+$/, "");
  if (!cleaned) return "";
  const parts = cleaned.split(/[\\/]/);
  return parts[parts.length - 1] ?? cleaned;
}
