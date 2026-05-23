/**
 * 项目 store：所有数据经 Tauri IPC 走 SQLite 持久化。
 *
 * 导出保持与旧版相同的函数签名（仅 `createProject` 参数微调），
 * 内部维护 `PROJECTS` reactive ref 作为 UI 缓存。
 */
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { Project } from "@lilia/contracts";

interface ProjectRow {
  id: string;
  name: string;
  cwd: string | null;
  sessionCount: number;
  sortOrder: number;
  pinned: boolean;
}

export const PROJECTS = ref<Project[]>([]);
let onProjectRemoved:
  | ((projectId: string) => void | Promise<void>)
  | null = null;

export function registerProjectRemovalHandler(
  handler: (projectId: string) => void | Promise<void>,
): void {
  onProjectRemoved = handler;
}

async function refresh(): Promise<void> {
  const rows = await invoke<ProjectRow[]>("project_list");
  PROJECTS.value = rows.map((r) => ({
    id: r.id,
    name: r.name,
    cwd: r.cwd,
    sessionCount: r.sessionCount,
    pinned: r.pinned,
  }));
}

// 模块加载时立刻拉一次；调用方可 await 确保数据就绪。
export const projectsReady: Promise<void> = refresh();

export function listProjects(): Project[] {
  return PROJECTS.value;
}

export function getProject(id: string): Project | undefined {
  return PROJECTS.value.find((p) => p.id === id);
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
  const row = await invoke<ProjectRow>("project_create", {
    name: trimmedName || "未命名项目",
    cwd: input.cwd && input.cwd.trim() ? input.cwd.trim() : null,
  });
  const project: Project = {
    id: row.id,
    name: row.name,
    cwd: row.cwd,
    sessionCount: row.sessionCount,
    pinned: row.pinned,
  };
  PROJECTS.value = [...PROJECTS.value, project];
  return project;
}

/** 更新项目名称；trim 后为空时不改动。返回是否真正更新。 */
export async function renameProject(id: string, nextName: string): Promise<boolean> {
  const updated = await invoke<boolean>("project_rename", { id, nextName });
  if (updated) await refresh();
  return updated;
}

/**
 * 「移除项目」：从侧栏摘掉项目本身，它的 tasks 变成孤儿（project_id → null）。
 * 不动磁盘上的 cwd 目录。
 */
export async function removeProject(id: string): Promise<boolean> {
  const removed = await invoke<boolean>("project_remove", { id });
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
  const pinned = await invoke<boolean>("project_toggle_pin", { id });
  await refresh();
  return pinned;
}

/** 项目列表拖拽排序后调用。`orderedIds` 按显示顺序传入。 */
export async function reorderProjects(orderedIds: string[]): Promise<void> {
  await invoke("project_reorder", { orderedIds });
  // 本地缓存按新顺序重排
  const byId = new Map(PROJECTS.value.map((p) => [p.id, p]));
  PROJECTS.value = orderedIds.map((id) => byId.get(id)).filter(Boolean) as Project[];
}
