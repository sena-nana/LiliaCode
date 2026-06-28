import { invoke } from "../tauri/runtime";
import { ref } from "vue";
import {
  MILESTONE_CREATE_COMMAND,
  MILESTONE_DELETE_COMMAND,
  MILESTONE_LIST_COMMAND,
  MILESTONE_REORDER_COMMAND,
  MILESTONE_SET_TASKS_COMMAND,
  MILESTONE_UPDATE_COMMAND,
  type Milestone,
  type MilestoneUpdatePatch,
  type ProjectRoadmap,
  type TaskMilestoneLink,
} from "@lilia/contracts";

export const MILESTONES = ref<Record<string, Milestone[]>>({});
export const MILESTONE_LINKS = ref<Record<string, TaskMilestoneLink[]>>({});
export const PROJECT_ROADMAP_LOADED = ref<Record<string, boolean>>({});

const roadmapLoads = new Map<string, Promise<void>>();

async function refreshRoadmap(projectId: string): Promise<void> {
  const snapshot = await invoke<ProjectRoadmap>(MILESTONE_LIST_COMMAND, { projectId });
  MILESTONES.value = {
    ...MILESTONES.value,
    [projectId]: snapshot.milestones,
  };
  MILESTONE_LINKS.value = {
    ...MILESTONE_LINKS.value,
    [projectId]: snapshot.links,
  };
  PROJECT_ROADMAP_LOADED.value = {
    ...PROJECT_ROADMAP_LOADED.value,
    [projectId]: true,
  };
}

export function isProjectRoadmapLoaded(projectId: string): boolean {
  return PROJECT_ROADMAP_LOADED.value[projectId] === true ||
    Object.prototype.hasOwnProperty.call(MILESTONES.value, projectId);
}

export function ensureProjectRoadmapLoaded(projectId: string, force = false): Promise<void> {
  if (!projectId) return Promise.resolve();
  if (!force && isProjectRoadmapLoaded(projectId)) return Promise.resolve();
  const pending = roadmapLoads.get(projectId);
  if (!force && pending) return pending;
  const load = refreshRoadmap(projectId).finally(() => {
    if (roadmapLoads.get(projectId) === load) roadmapLoads.delete(projectId);
  });
  roadmapLoads.set(projectId, load);
  return load;
}

export function listProjectMilestones(projectId: string): Milestone[] {
  return MILESTONES.value[projectId] ?? [];
}

export function listProjectMilestoneLinks(projectId: string): TaskMilestoneLink[] {
  return MILESTONE_LINKS.value[projectId] ?? [];
}

export async function createMilestone(projectId: string, title: string): Promise<Milestone> {
  const milestone = await invoke<Milestone>(MILESTONE_CREATE_COMMAND, {
    projectId,
    title,
  });
  MILESTONES.value = {
    ...MILESTONES.value,
    [projectId]: [...(MILESTONES.value[projectId] ?? []), milestone],
  };
  PROJECT_ROADMAP_LOADED.value = {
    ...PROJECT_ROADMAP_LOADED.value,
    [projectId]: true,
  };
  return milestone;
}

export async function updateMilestone(
  projectId: string,
  id: string,
  patch: MilestoneUpdatePatch,
): Promise<void> {
  const { dueDate, ...rest } = patch;
  await invoke(MILESTONE_UPDATE_COMMAND, {
    id,
    ...rest,
    ...(dueDate === undefined
      ? {}
      : dueDate === null
        ? { clearDueDate: true }
        : { dueDate }),
  });
  await ensureProjectRoadmapLoaded(projectId, true);
}

export async function deleteMilestone(projectId: string, id: string): Promise<void> {
  await invoke(MILESTONE_DELETE_COMMAND, { id });
  await ensureProjectRoadmapLoaded(projectId, true);
}

export async function reorderMilestones(
  projectId: string,
  orderedIds: string[],
): Promise<void> {
  await invoke(MILESTONE_REORDER_COMMAND, { projectId, orderedIds });
  await ensureProjectRoadmapLoaded(projectId, true);
}

export async function setMilestoneTasks(
  projectId: string,
  milestoneId: string,
  taskIds: string[],
): Promise<void> {
  await invoke(MILESTONE_SET_TASKS_COMMAND, { milestoneId, taskIds });
  await ensureProjectRoadmapLoaded(projectId, true);
}

