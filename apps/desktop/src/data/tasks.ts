/**
 * 任务 + 孤儿对话 store：所有数据经 Tauri IPC 走 SQLite 持久化。
 *
 * - OrphanConversation = `project_id IS NULL` 的 task，同一张表。
 * - 草稿（draft）留在前端内存，不落库；`promote` 后才 INSERT。
 * - 导出保持与旧版相同的函数签名，内部维护 reactive ref 缓存。
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ref } from "vue";
import type { Task } from "@lilia/contracts";
import {
  ensureProjectsLoaded,
  listProjects,
  registerProjectRemovalHandler,
} from "./projects";

// OrphanConversation 形状沿用 Task 的子集，project_id 为 null。
export interface OrphanConversation {
  id: string;
  sessionId: string;
  title: string;
  createdAt: number;
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

interface TasksChangedPayload {
  projectId?: string | null;
  project_id?: string | null;
}

export const TASKS = ref<Record<string, Task[]>>({});
export const ORPHAN_LIST = ref<OrphanConversation[]>([]);
export const PROJECT_TASKS_LOADED = ref<Record<string, boolean>>({});
export const ORPHANS_LOADED = ref(false);

const DRAFT_TASKS = new Map<string, Task>();
const DRAFT_ORPHANS = new Map<string, OrphanConversation>();
const DRAFT_TASK_PROMOTIONS = new Map<string, Promise<void>>();
const DRAFT_ORPHAN_PROMOTIONS = new Map<string, Promise<void>>();
const projectTaskLoads = new Map<string, Promise<void>>();
let orphanLoad: Promise<void> | null = null;

function rememberDraftPromotion(
  promotions: Map<string, Promise<void>>,
  id: string,
  run: () => Promise<void>,
): Promise<void> {
  const existing = promotions.get(id);
  if (existing) return existing;
  const promotion = run().finally(() => {
    if (promotions.get(id) === promotion) promotions.delete(id);
  });
  promotions.set(id, promotion);
  return promotion;
}

async function refreshTasks(projectId: string): Promise<void> {
  const rows = await invoke<TaskRow[]>("task_list", { projectId });
  TASKS.value = {
    ...TASKS.value,
    [projectId]: rows.map(rowToTask),
  };
  PROJECT_TASKS_LOADED.value = {
    ...PROJECT_TASKS_LOADED.value,
    [projectId]: true,
  };
}

async function refreshOrphans(): Promise<void> {
  const rows = await invoke<TaskRow[]>("task_list", { projectId: null });
  ORPHAN_LIST.value = rows.map(rowToOrphan);
  ORPHANS_LOADED.value = true;
}

function rowToTask(r: TaskRow): Task {
  return {
    id: r.id,
    projectId: r.projectId ?? "",
    sessionId: r.sessionId,
    title: r.title,
    status: r.status as Task["status"],
    createdAt: r.createdAt,
    pinned: r.pinned,
    parentId: r.parentId,
    dependsOn: r.dependsOn,
  };
}

function rowToOrphan(r: TaskRow): OrphanConversation {
  return {
    id: r.id,
    sessionId: r.sessionId,
    title: r.title,
    createdAt: r.createdAt,
    pinned: r.pinned,
  };
}

function upsertTaskRow(row: TaskRow): Task | OrphanConversation {
  if (row.projectId) {
    const task = rowToTask(row);
    const existing = TASKS.value[row.projectId] ?? [];
    const index = existing.findIndex((t) => t.id === task.id);
    const nextProjectTasks = [...existing];
    if (index === -1) {
      nextProjectTasks.unshift(task);
    } else {
      nextProjectTasks[index] = task;
    }
    TASKS.value = {
      ...TASKS.value,
      [row.projectId]: nextProjectTasks,
    };
    ORPHAN_LIST.value = ORPHAN_LIST.value.filter((o) => o.id !== task.id);
    return task;
  }

  const orphan = rowToOrphan(row);
  const index = ORPHAN_LIST.value.findIndex((o) => o.id === orphan.id);
  const nextOrphans = [...ORPHAN_LIST.value];
  if (index === -1) {
    nextOrphans.unshift(orphan);
  } else {
    nextOrphans[index] = orphan;
  }
  ORPHAN_LIST.value = nextOrphans;
  for (const [projectId, list] of Object.entries(TASKS.value)) {
    if (list.some((t) => t.id === orphan.id)) {
      TASKS.value = {
        ...TASKS.value,
        [projectId]: list.filter((t) => t.id !== orphan.id),
      };
    }
  }
  return orphan;
}

export function isProjectTasksLoaded(projectId: string): boolean {
  return PROJECT_TASKS_LOADED.value[projectId] === true ||
    Object.prototype.hasOwnProperty.call(TASKS.value, projectId);
}

export function areOrphansLoaded(): boolean {
  return ORPHANS_LOADED.value || ORPHAN_LIST.value.length > 0;
}

export function ensureOrphansLoaded(force = false): Promise<void> {
  if (!force && ORPHANS_LOADED.value) return Promise.resolve();
  if (!force && orphanLoad) return orphanLoad;
  orphanLoad = refreshOrphans().finally(() => {
    orphanLoad = null;
  });
  return orphanLoad;
}

export function ensureProjectTasksLoaded(projectId: string, force = false): Promise<void> {
  if (!projectId) return Promise.resolve();
  if (!force && isProjectTasksLoaded(projectId)) return Promise.resolve();
  const pending = projectTaskLoads.get(projectId);
  if (!force && pending) return pending;
  const load = refreshTasks(projectId).finally(() => {
    if (projectTaskLoads.get(projectId) === load) {
      projectTaskLoads.delete(projectId);
    }
  });
  projectTaskLoads.set(projectId, load);
  return load;
}

export async function ensureAllProjectTasksLoaded(): Promise<void> {
  await ensureProjectsLoaded();
  const projs = listProjects();
  await Promise.all(projs.map((p) => ensureProjectTasksLoaded(p.id)));
}

function readChangedProjectId(payload: TasksChangedPayload): string | null {
  const value = payload.projectId ?? payload.project_id ?? null;
  return typeof value === "string" && value.length > 0 ? value : null;
}

async function refreshChangedTasks(payload: TasksChangedPayload) {
  const projectId = readChangedProjectId(payload);
  if (projectId) {
    if (isProjectTasksLoaded(projectId)) {
      await ensureProjectTasksLoaded(projectId, true);
    }
  } else {
    if (areOrphansLoaded()) {
      await ensureOrphansLoaded(true);
    }
  }
}

export function installTasksChangedListener() {
  void listen<TasksChangedPayload>("tasks:changed", (event) => {
    void refreshChangedTasks(event.payload);
  }).catch((err) => {
    console.error("[tasks] listen tasks:changed failed", err);
  });
}

installTasksChangedListener();

export function listTasks(projectId: string): Task[] {
  return TASKS.value[projectId] ?? [];
}

export function getTask(projectId: string, taskId: string): Task | undefined {
  const persisted = (TASKS.value[projectId] ?? []).find((t) => t.id === taskId);
  const draft = DRAFT_TASKS.get(taskId);
  return draft?.projectId === projectId ? draft : persisted;
}

export async function ensureTaskLoaded(
  taskId: string,
  expectedProjectId?: string | null,
): Promise<Task | OrphanConversation | null> {
  if (expectedProjectId) {
    const existing = getTask(expectedProjectId, taskId);
    if (existing) return existing;
  } else if (expectedProjectId === null) {
    const existing = getOrphanConversation(taskId);
    if (existing) return existing;
  }

  const row = await invoke<TaskRow | null>("task_get", { id: taskId });
  if (!row) return null;
  if (expectedProjectId !== undefined && row.projectId !== expectedProjectId) return null;
  return upsertTaskRow(row);
}

export function listProjectConversations(projectId: string): Task[] {
  return listTasks(projectId);
}

export function isDraftTask(id: string): boolean {
  return DRAFT_TASKS.has(id);
}

export function resolveConversationRouteState(
  projectId: string | null | undefined,
  taskId: string | null | undefined,
) {
  if (!taskId) {
    return {
      isDraftRoute: false,
      isLiveDraft: false,
      isLostDraft: false,
    };
  }

  const projectScoped = !!projectId;
  const isDraftRoute = projectScoped
    ? taskId.startsWith("t-draft-")
    : taskId.startsWith("o-draft-");
  const isLiveDraft = projectScoped
    ? isDraftRoute && isDraftTask(taskId)
    : isDraftRoute && isDraftOrphan(taskId);
  const exists = projectScoped
    ? !!getTask(projectId, taskId)
    : !!getOrphanConversation(taskId);

  return {
    isDraftRoute,
    isLiveDraft,
    isLostDraft: isDraftRoute && !isLiveDraft && !exists,
  };
}

export function createDraftTask(projectId: string): Task {
  const id = `t-draft-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
  const draft: Task = {
    id,
    projectId,
    sessionId: id,
    title: "新对话",
    status: "draft",
    createdAt: Date.now(),
    pinned: false,
    parentId: null,
    dependsOn: [],
  };
  DRAFT_TASKS.set(id, draft);
  return draft;
}

export async function promoteDraftTask(id: string, title: string): Promise<void> {
  const draft = DRAFT_TASKS.get(id);
  if (!draft) return DRAFT_TASK_PROMOTIONS.get(id);
  return rememberDraftPromotion(DRAFT_TASK_PROMOTIONS, id, async () => {
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
    DRAFT_TASKS.delete(id);
  });
}

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

export async function toggleTaskPin(taskId: string): Promise<boolean> {
  const pinned = await invoke<boolean>("task_toggle_pin", { id: taskId });
  for (const [pid, list] of Object.entries(TASKS.value)) {
    if (list.some((t) => t.id === taskId)) {
      await refreshTasks(pid);
      return pinned;
    }
  }
  if (ORPHAN_LIST.value.some((o) => o.id === taskId)) {
    await refreshOrphans();
  }
  return pinned;
}

export function removeProjectTasks(projectId: string): void {
  const next = { ...TASKS.value };
  delete next[projectId];
  TASKS.value = next;
  const loaded = { ...PROJECT_TASKS_LOADED.value };
  delete loaded[projectId];
  PROJECT_TASKS_LOADED.value = loaded;
  for (const [draftId, draft] of DRAFT_TASKS) {
    if (draft.projectId === projectId) DRAFT_TASKS.delete(draftId);
  }
}

export async function detachProjectTasksToOrphans(projectId: string): Promise<void> {
  removeProjectTasks(projectId);
  await refreshOrphans();
}

registerProjectRemovalHandler(detachProjectTasksToOrphans);

export function listOrphanConversations(): OrphanConversation[] {
  return ORPHAN_LIST.value;
}

export function getOrphanConversation(id: string): OrphanConversation | undefined {
  const persisted = ORPHAN_LIST.value.find((o) => o.id === id);
  return DRAFT_ORPHANS.get(id) ?? persisted;
}

export function isDraftOrphan(id: string): boolean {
  return DRAFT_ORPHANS.has(id);
}

export function createDraftOrphan(): OrphanConversation {
  const id = `o-draft-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
  const draft: OrphanConversation = {
    id,
    sessionId: id,
    title: "新对话",
    createdAt: Date.now(),
    pinned: false,
  };
  DRAFT_ORPHANS.set(id, draft);
  return draft;
}

export async function promoteDraftOrphan(id: string, title: string): Promise<void> {
  const draft = DRAFT_ORPHANS.get(id);
  if (!draft) return DRAFT_ORPHAN_PROMOTIONS.get(id);
  return rememberDraftPromotion(DRAFT_ORPHAN_PROMOTIONS, id, async () => {
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
          pinned: row.pinned,
        },
        ...ORPHAN_LIST.value,
      ];
    }
    DRAFT_ORPHANS.delete(id);
  });
}

export async function reorderTasks(
  projectId: string | null,
  orderedIds: string[],
): Promise<void> {
  await invoke("task_reorder", { projectId, orderedIds });
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

export async function reparentTask(
  taskId: string,
  sourceProjectId: string | null,
  targetProjectId: string | null,
): Promise<void> {
  await invoke("task_reparent", { taskId, newProjectId: targetProjectId });
  if (sourceProjectId) {
    const list = TASKS.value[sourceProjectId] ?? [];
    TASKS.value = {
      ...TASKS.value,
      [sourceProjectId]: list.filter((t) => t.id !== taskId),
    };
  } else {
    ORPHAN_LIST.value = ORPHAN_LIST.value.filter((o) => o.id !== taskId);
  }
  if (targetProjectId) {
    await refreshTasks(targetProjectId);
  } else {
    await refreshOrphans();
  }
}
