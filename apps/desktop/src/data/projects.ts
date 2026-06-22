/**
 * 项目 store：所有数据经 Tauri IPC 走 SQLite 持久化。
 * 内部维护 `PROJECTS` reactive ref 作为 UI 缓存。
 */
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import {
  PROJECT_CREATE_COMMAND,
  PROJECT_GET_COMMAND,
  PROJECT_LIST_COMMAND,
  PROJECT_REMOVE_COMMAND,
  PROJECT_RENAME_COMMAND,
  PROJECT_REORDER_COMMAND,
  PROJECT_TOGGLE_PIN_COMMAND,
  type Project,
} from "@lilia/contracts";

interface ProjectRow {
  id: string;
  name: string;
  cwd: string | null;
  sessionCount: number;
  sortOrder: number;
  pinned: boolean;
}

export const PROJECTS = ref<Project[]>([]);
const projectsLoaded = ref(false);
let projectLoad: Promise<void> | null = null;
const projectLoads = new Map<string, Promise<Project | null>>();
let onProjectRemoved:
  | ((projectId: string) => void | Promise<void>)
  | null = null;

export function registerProjectRemovalHandler(
  handler: (projectId: string) => void | Promise<void>,
): void {
  onProjectRemoved = handler;
}

async function refresh(): Promise<void> {
  const rows = await invoke<ProjectRow[]>(PROJECT_LIST_COMMAND);
  PROJECTS.value = rows.map(projectRowToProject);
  projectsLoaded.value = true;
}

function projectRowToProject(r: ProjectRow): Project {
  return {
    id: r.id,
    name: r.name,
    cwd: r.cwd,
    sessionCount: r.sessionCount,
    pinned: r.pinned,
  };
}

function upsertProject(project: Project): Project {
  const index = PROJECTS.value.findIndex((p) => p.id === project.id);
  if (index === -1) {
    PROJECTS.value = [...PROJECTS.value, project];
    return project;
  }
  const next = [...PROJECTS.value];
  next[index] = project;
  PROJECTS.value = next;
  return project;
}

function shouldDeferInitialRefresh(): boolean {
  return typeof window !== "undefined" && window.location.hash.startsWith("#/popup");
}

export function ensureProjectsLoaded(force = false): Promise<void> {
  if (shouldDeferInitialRefresh()) return Promise.resolve();
  if (!force && projectsLoaded.value) return Promise.resolve();
  if (!force && projectLoad) return projectLoad;
  projectLoad = refresh().finally(() => {
    projectLoad = null;
  });
  return projectLoad;
}

export function listProjects(): Project[] {
  return PROJECTS.value;
}

export function getProject(id: string): Project | undefined {
  return PROJECTS.value.find((p) => p.id === id);
}

export async function ensureProjectLoaded(id: string): Promise<Project | null> {
  const existing = getProject(id);
  if (existing) return existing;
  const pending = projectLoads.get(id);
  if (pending) return pending;
  const load = invoke<ProjectRow | null>(PROJECT_GET_COMMAND, { id })
    .then((row) => row ? upsertProject(projectRowToProject(row)) : null)
    .finally(() => {
      if (projectLoads.get(id) === load) {
        projectLoads.delete(id);
      }
    });
  projectLoads.set(id, load);
  return load;
}

/**
 * 侧栏「添加项目」入口：本地文件夹 / clone / 空分类三类都进这里。
 * cwd 传 null 表示「分类型」项目，仅做侧栏归类用。
 */
export async function createProject(input: {
  name: string;
  cwd: string | null;
}): Promise<Project> {
  const trimmedName = input.name.trim();
  const row = await invoke<ProjectRow>(PROJECT_CREATE_COMMAND, {
    name: trimmedName || "未命名项目",
    cwd: input.cwd && input.cwd.trim() ? input.cwd.trim() : null,
  });
  return upsertProject(projectRowToProject(row));
}

/** 更新项目名称；trim 后为空时不改动。返回是否真正更新。 */
export async function renameProject(id: string, nextName: string): Promise<boolean> {
  const updated = await invoke<boolean>(PROJECT_RENAME_COMMAND, { id, nextName });
  if (updated) await refresh();
  return updated;
}

/**
 * 「移除项目」：从侧栏摘掉项目本身，它的 tasks 变成孤儿（project_id → null）。
 * 不动磁盘上的 cwd 目录。
 */
export async function removeProject(id: string): Promise<boolean> {
  const removed = await invoke<boolean>(PROJECT_REMOVE_COMMAND, { id });
  if (removed) {
    await refresh();
    await onProjectRemoved?.(id);
  }
  return removed;
}

/** 从绝对路径取末尾段作为项目名候选；Windows / Unix 分隔符都吃。 */
export function deriveProjectName(absPath: string): string {
  const cleaned = absPath.trim().replace(/[\\/]+$/, "");
  if (!cleaned) return "";
  const parts = cleaned.split(/[\\/]/);
  return parts[parts.length - 1] ?? cleaned;
}

/** 切换项目置顶状态。 */
export async function toggleProjectPin(id: string): Promise<boolean> {
  const pinned = await invoke<boolean>(PROJECT_TOGGLE_PIN_COMMAND, { id });
  await refresh();
  return pinned;
}

/** 项目列表拖拽排序后调用。`orderedIds` 按显示顺序传入。 */
export async function reorderProjects(orderedIds: string[]): Promise<void> {
  await invoke(PROJECT_REORDER_COMMAND, { orderedIds });
  // 本地缓存只重排参与本次拖动的项目；其它 pinned 分组保持原位。
  const byId = new Map(PROJECTS.value.map((p) => [p.id, p]));
  const reordered = orderedIds
    .map((id) => byId.get(id))
    .filter((project): project is Project => Boolean(project));
  if (reordered.length === 0) return;
  let nextIndex = 0;
  const affected = new Set(orderedIds);
  PROJECTS.value = PROJECTS.value.map((project) => {
    if (!affected.has(project.id)) return project;
    return reordered[nextIndex++] ?? project;
  });
}
