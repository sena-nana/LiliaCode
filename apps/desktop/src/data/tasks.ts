/**
 * 任务 + 孤儿对话 store：所有数据经 Tauri IPC 走 SQLite 持久化。
 *
 * - OrphanConversation = `project_id IS NULL` 的 task，同一张表。
 * - 草稿（draft）留在前端内存，不落库；`promote` 后才 INSERT。
 * - 导出保持与旧版相同的函数签名，内部维护 reactive ref 缓存。
 */
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { Task } from "@lilia/contracts";
import {
  listProjects,
  projectsReady,
  registerProjectRemovalHandler,
} from "./projects";

// OrphanConversation 形状沿用 Task 的子集，project_id 为 null。
export interface OrphanConversation {
  id: string;
  sessionId: string;
  title: string;
  createdAt: number;
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
}

// ---------- Reactive 缓存 ----------

export const TASKS = ref<Record<string, Task[]>>({});
export const ORPHAN_LIST = ref<OrphanConversation[]>([]);

// 内存草稿（不落库）
const DRAFT_TASKS = new Map<string, Task>();
const DRAFT_ORPHANS = new Map<string, OrphanConversation>();

// ---------- 数据加载 ----------

async function refreshTasks(projectId: string): Promise<void> {
  const rows = await invoke<TaskRow[]>("task_list", { projectId });
  TASKS.value = {
    ...TASKS.value,
    [projectId]: rows.map(rowToTask),
  };
}

async function refreshOrphans(): Promise<void> {
  const rows = await invoke<TaskRow[]>("task_list", { projectId: null });
  ORPHAN_LIST.value = rows.map((r) => ({
    id: r.id,
    sessionId: r.sessionId,
    title: r.title,
    createdAt: r.createdAt,
  }));
}

function rowToTask(r: TaskRow): Task {
  return {
    id: r.id,
    projectId: r.projectId ?? "",
    sessionId: r.sessionId,
    title: r.title,
    status: r.status as Task["status"],
    createdAt: r.createdAt,
    parentId: r.parentId,
    dependsOn: r.dependsOn,
  };
}

// 模块加载时先等 projects 初始化，再拉所有项目的 tasks 和 orphans。
export const allTasksReady: Promise<void> = projectsReady.then(async () => {
  await refreshOrphans();
  await refreshAllProjectTasks();
});

// 首次加载时把所有项目的 tasks 一并拉下来。
async function refreshAllProjectTasks(): Promise<void> {
  const projs = listProjects();
  await Promise.all(projs.map((p) => refreshTasks(p.id)));
}

// ---------- Task CRUD ----------

export function listTasks(projectId: string): Task[] {
  return TASKS.value[projectId] ?? [];
}

export function getTask(projectId: string, taskId: string): Task | undefined {
  const draft = DRAFT_TASKS.get(taskId);
  if (draft && draft.projectId === projectId) return draft;
  return (TASKS.value[projectId] ?? []).find((t) => t.id === taskId);
}

/** 供侧栏直接绑定用。 */
export function listProjectConversations(projectId: string): Task[] {
  return listTasks(projectId);
}

export function isDraftTask(id: string): boolean {
  return DRAFT_TASKS.has(id);
}

/** 点项目行「+」时调用：产出一条绑定到该项目的草稿任务，首条消息发出前不落库。 */
export function createDraftTask(projectId: string): Task {
  const id = `t-draft-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
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
 * 草稿发出第一条消息后调用：落库 → 从 DRAFT_TASKS 移到 TASKS 缓存。
 */
export async function promoteDraftTask(id: string, title: string): Promise<void> {
  const draft = DRAFT_TASKS.get(id);
  if (!draft) return;
  DRAFT_TASKS.delete(id);
  const row = await invoke<TaskRow>("task_promote", {
    id,
    projectId: draft.projectId,
    title: title || draft.title,
    dependsOn: draft.dependsOn,
  });
  const task = rowToTask(row);
  const existing = TASKS.value[draft.projectId] ?? [];
  if (!existing.some((t) => t.id === id)) {
    TASKS.value = {
      ...TASKS.value,
      [draft.projectId]: [task, ...existing],
    };
  }
}

/** 归档单条对话：软删除（archived = 1），并从缓存移除。 */
export async function archiveTask(taskId: string): Promise<boolean> {
  const ok = await invoke<boolean>("task_archive", { id: taskId });
  if (!ok) return false;
  for (const [pid, list] of Object.entries(TASKS.value)) {
    const idx = list.findIndex((t) => t.id === taskId);
    if (idx !== -1) {
      const next = [...list];
      next.splice(idx, 1);
      TASKS.value = { ...TASKS.value, [pid]: next };
      return true;
    }
  }
  ORPHAN_LIST.value = ORPHAN_LIST.value.filter((o) => o.id !== taskId);
  return true;
}

/**
 * 「归档所有对话」：软删除该项目下所有 Task + 清空草稿。
 * 返回清掉的数量（含草稿），方便调用方做提示。
 */
export async function archiveProjectConversations(projectId: string): Promise<number> {
  const dbCount = await invoke<number>("task_archive_project", { projectId });
  const next = { ...TASKS.value };
  delete next[projectId];
  TASKS.value = next;
  let draftCleared = 0;
  for (const [draftId, draft] of DRAFT_TASKS) {
    if (draft.projectId === projectId) {
      DRAFT_TASKS.delete(draftId);
      draftCleared += 1;
    }
  }
  return dbCount + draftCleared;
}

/** 供 projects.removeProject 调用：清理该项目关联的草稿任务（DB 侧由 project_remove 处理）。 */
export function removeProjectTasks(projectId: string): void {
  const next = { ...TASKS.value };
  delete next[projectId];
  TASKS.value = next;
  for (const [draftId, draft] of DRAFT_TASKS) {
    if (draft.projectId === projectId) DRAFT_TASKS.delete(draftId);
  }
}

/**
 * 项目被移除时 DB 会把该项目下的 task project_id 置 NULL。
 * 前端缓存需要同步清掉原项目列表并重新拉取收集箱。
 */
export async function detachProjectTasksToOrphans(projectId: string): Promise<void> {
  removeProjectTasks(projectId);
  await refreshOrphans();
}

registerProjectRemovalHandler(detachProjectTasksToOrphans);

// ---------- Orphan Conversation CRUD ----------

export function listOrphanConversations(): OrphanConversation[] {
  return ORPHAN_LIST.value;
}

export function getOrphanConversation(id: string): OrphanConversation | undefined {
  return DRAFT_ORPHANS.get(id) ?? ORPHAN_LIST.value.find((o) => o.id === id);
}

export function isDraftOrphan(id: string): boolean {
  return DRAFT_ORPHANS.has(id);
}

/** 点「新对话」时调用：产出一条只活在内存里的草稿。 */
export function createDraftOrphan(): OrphanConversation {
  const id = `o-draft-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
  const draft: OrphanConversation = {
    id,
    sessionId: id,
    title: "新对话",
    createdAt: Date.now(),
  };
  DRAFT_ORPHANS.set(id, draft);
  return draft;
}

/**
 * 草稿发出第一条消息后调用：落库 → 从 DRAFT_ORPHANS 移到 ORPHAN_LIST 缓存。
 */
export async function promoteDraftOrphan(id: string, title: string): Promise<void> {
  const draft = DRAFT_ORPHANS.get(id);
  if (!draft) return;
  DRAFT_ORPHANS.delete(id);
  const row = await invoke<TaskRow>("task_promote", {
    id,
    projectId: null,
    title: title || draft.title,
    dependsOn: [],
  });
  if (!ORPHAN_LIST.value.some((o) => o.id === id)) {
    ORPHAN_LIST.value = [
      {
        id: row.id,
        sessionId: row.sessionId,
        title: row.title,
        createdAt: row.createdAt,
      },
      ...ORPHAN_LIST.value,
    ];
  }
}

// ---------- 拖拽排序 / 跨项目移动 ----------

/** 项目内 session 列表拖拽排序后调用。`orderedIds` 按显示顺序传入。 */
export async function reorderTasks(
  projectId: string | null,
  orderedIds: string[],
): Promise<void> {
  await invoke("task_reorder", { projectId, orderedIds });
  // 本地缓存按新顺序重排
  if (projectId) {
    const list = TASKS.value[projectId] ?? [];
    const byId = new Map(list.map((t) => [t.id, t]));
    TASKS.value = {
      ...TASKS.value,
      [projectId]: orderedIds.map((id) => byId.get(id)).filter(Boolean) as Task[],
    };
  } else {
    const byId = new Map(ORPHAN_LIST.value.map((o) => [o.id, o]));
    ORPHAN_LIST.value = orderedIds
      .map((id) => byId.get(id))
      .filter(Boolean) as OrphanConversation[];
  }
}

/**
 * 跨项目移动 session。从 `sourceProjectId` 移到 `targetProjectId`。
 * `sourceProjectId` 或 `targetProjectId` 为 null 表示孤儿收集箱。
 */
export async function reparentTask(
  taskId: string,
  sourceProjectId: string | null,
  targetProjectId: string | null,
): Promise<void> {
  await invoke("task_reparent", { taskId, newProjectId: targetProjectId });
  // 从源列表移除
  if (sourceProjectId) {
    const list = TASKS.value[sourceProjectId] ?? [];
    TASKS.value = {
      ...TASKS.value,
      [sourceProjectId]: list.filter((t) => t.id !== taskId),
    };
  } else {
    ORPHAN_LIST.value = ORPHAN_LIST.value.filter((o) => o.id !== taskId);
  }
  // 刷新目标列表
  if (targetProjectId) {
    await refreshTasks(targetProjectId);
  } else {
    await refreshOrphans();
  }
}
