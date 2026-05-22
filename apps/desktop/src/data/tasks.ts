import { ref } from "vue";
import type { Task } from "@lilia/contracts";
import { makeId } from "./shared";
import { listProjects } from "./projects";

export const TASKS = ref<Record<string, Task[]>>({
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
 * 项目内草稿：点了项目行的「+」按钮创建的会话。同样首条发送前不进 TASKS。
 */
const DRAFT_TASKS = new Map<string, Task>();

export function listTasks(projectId: string): Task[] {
  return TASKS.value[projectId] ?? [];
}

export function getTask(projectId: string, taskId: string): Task | undefined {
  const draft = DRAFT_TASKS.get(taskId);
  if (draft && draft.projectId === projectId) return draft;
  return (TASKS.value[projectId] ?? []).find((t) => t.id === taskId);
}

export function listProjectConversations(projectId: string): Task[] {
  return TASKS.value[projectId] ?? [];
}

export function isDraftTask(id: string): boolean {
  return DRAFT_TASKS.has(id);
}

/** 点项目行「+」时调用：产出一条绑定到该项目的草稿任务，首条消息发出前不进 TASKS。 */
export function createDraftTask(projectId: string): Task | undefined {
  if (!listProjects().some((p) => p.id === projectId)) return undefined;
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

/** 供 projects.removeProject 调用：清理该项目关联的任务和草稿任务。 */
export function removeProjectTasks(projectId: string): void {
  if (TASKS.value[projectId]) {
    const next = { ...TASKS.value };
    delete next[projectId];
    TASKS.value = next;
  }
  for (const [draftId, draft] of DRAFT_TASKS) {
    if (draft.projectId === projectId) DRAFT_TASKS.delete(draftId);
  }
}
