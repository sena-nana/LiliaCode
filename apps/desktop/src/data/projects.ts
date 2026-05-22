import { ref } from "vue";
import type { Project } from "@lilia/contracts";
import { makeId } from "./shared";

export const PROJECTS = ref<Project[]>([
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
 * 「移除项目」：从侧栏摘掉项目本身，连带把它的 TASKS / 草稿任务一并清掉。
 * 不动磁盘上的 cwd 目录（哪怕是 clone 进来的也只是「从 Lilia 视野里移除」）。
 *
 * 使用动态 import 避免与 tasks.ts 的循环依赖（tasks → projects.listProjects）。
 */
export async function removeProject(id: string): Promise<boolean> {
  const before = PROJECTS.value.length;
  PROJECTS.value = PROJECTS.value.filter((p) => p.id !== id);
  if (PROJECTS.value.length === before) return false;
  const { removeProjectTasks } = await import("./tasks");
  removeProjectTasks(id);
  return true;
}

/** 从绝对路径取末尾段作为项目名候选；Windows / Unix 分隔符都吃。 */
export function deriveProjectName(absPath: string): string {
  const cleaned = absPath.trim().replace(/[\\/]+$/, "");
  if (!cleaned) return "";
  const parts = cleaned.split(/[\\/]/);
  return parts[parts.length - 1] ?? cleaned;
}
