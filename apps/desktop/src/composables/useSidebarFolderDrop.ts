import { computed, onScopeDispose, ref, type ComputedRef } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Project } from "@lilia/contracts";
import {
  ensureFolderProjects,
  reorderProjects,
} from "../services/projectsStore";
import {
  isPointInsideElement,
  normalizeDropPoint,
  readDropPayload,
  type DropPoint,
} from "./webviewDrop";
import type { TreeDropTarget } from "./useSidebarTreeDrag";

function sameOrder(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((id, index) => id === right[index]);
}

function orderedIdsWithInsertedProjects(
  projects: Project[],
  insertedProjects: Project[],
  target: TreeDropTarget,
): string[] {
  const insertedIds = Array.from(new Set(
    insertedProjects.filter((project) => !project.pinned).map((project) => project.id),
  ));
  if (insertedIds.length === 0) {
    return projects.filter((project) => !project.pinned).map((project) => project.id);
  }
  const inserted = new Set(insertedIds);
  const baseIds = projects
    .filter((project) => !project.pinned && !inserted.has(project.id))
    .map((project) => project.id);
  let insertIndex = baseIds.length;
  if (target.projectId && !inserted.has(target.projectId)) {
    const targetIndex = baseIds.indexOf(target.projectId);
    if (targetIndex >= 0) {
      insertIndex = target.position === "after" ? targetIndex + 1 : targetIndex;
    }
  }
  return [
    ...baseIds.slice(0, insertIndex),
    ...insertedIds,
    ...baseIds.slice(insertIndex),
  ];
}

export function useSidebarFolderDrop(options: {
  projects: ComputedRef<Project[]>;
  openProject: (projectId: string) => void;
  reportError: (message: string) => void;
}) {
  const folderDropTarget = ref<TreeDropTarget | null>(null);
  const folderDropZoneClass = computed(() => ({
    "is-tree-drop-target": folderDropTarget.value?.kind === "project" &&
      folderDropTarget.value.projectId === null &&
      folderDropTarget.value.valid,
  }));
  const appWindow = getCurrentWindow();
  let disposed = false;
  let unlistenDragDrop: UnlistenFn | null = null;
  let dragDropSeq = 0;

  function projectTree(): HTMLElement | null {
    const element = document.querySelector('[data-agent-id="sidebar.projects.tree"]');
    return element instanceof HTMLElement ? element : null;
  }

  function resolveDropTarget(point: DropPoint | null): TreeDropTarget | null {
    if (!point) return null;
    const tree = projectTree();
    if (!isPointInsideElement(point, tree)) return null;
    const element = document.elementFromPoint?.(point.x, point.y);
    const row = element?.closest<HTMLElement>("[data-tree-kind]") ?? null;
    if (row && !tree?.contains(row)) return null;

    if (row?.dataset.treeKind === "project") {
      const projectId = row.dataset.projectId?.trim() || null;
      if (!projectId) return null;
      const pinned = row.dataset.pinned === "true";
      const rect = row.getBoundingClientRect();
      return {
        kind: "project",
        projectId,
        taskId: null,
        pinned,
        position: point.y > rect.top + rect.height / 2 ? "after" : "before",
        valid: !pinned,
      };
    }

    if (row?.dataset.treeKind === "task") {
      return {
        kind: "task",
        projectId: row.dataset.projectId?.trim() || null,
        taskId: row.dataset.taskId?.trim() || null,
        pinned: row.dataset.pinned === "true",
        position: "inside",
        valid: false,
      };
    }

    if (options.projects.value.length === 0) {
      return {
        kind: "project",
        projectId: null,
        taskId: null,
        pinned: false,
        position: "inside",
        valid: true,
      };
    }

    return null;
  }

  async function applyFolderDrop(paths: string[], target: TreeDropTarget) {
    if (!target.valid) return;
    try {
      const ensuredProjects = await ensureFolderProjects(paths);
      if (disposed) return;
      if (ensuredProjects.length === 0) {
        options.reportError("没有可用的项目文件夹");
        return;
      }
      const insertedProjects = ensuredProjects.filter((project) => !project.pinned);
      if (insertedProjects.length > 0) {
        const currentOrder = options.projects.value
          .filter((project) => !project.pinned)
          .map((project) => project.id);
        const nextOrder = orderedIdsWithInsertedProjects(
          options.projects.value,
          insertedProjects,
          target,
        );
        if (!sameOrder(currentOrder, nextOrder)) {
          await reorderProjects(nextOrder);
          if (disposed) return;
        }
      }
      options.openProject(ensuredProjects[0].id);
    } catch (err) {
      if (!disposed) options.reportError(`添加项目失败：${String(err)}`);
    }
  }

  async function handleDragDropEvent(payload: unknown) {
    const drop = readDropPayload(payload);
    if (!drop) return;
    if (drop.type === "leave") {
      folderDropTarget.value = null;
      return;
    }
    const seq = ++dragDropSeq;
    const point = await normalizeDropPoint(drop.position, () => appWindow.scaleFactor());
    if (disposed || seq !== dragDropSeq) return;
    const target = resolveDropTarget(point);
    folderDropTarget.value = target;
    if (drop.type !== "drop") return;
    folderDropTarget.value = null;
    if (!target || drop.paths.length === 0) return;
    await applyFolderDrop(drop.paths, target);
  }

  void getCurrentWebview().onDragDropEvent((event) => {
    void handleDragDropEvent(event.payload);
  }).then((unlisten) => {
    if (disposed) {
      void unlisten();
      return;
    }
    unlistenDragDrop = unlisten;
  });

  onScopeDispose(() => {
    disposed = true;
    dragDropSeq += 1;
    folderDropTarget.value = null;
    if (unlistenDragDrop) {
      void unlistenDragDrop();
      unlistenDragDrop = null;
    }
  });

  return {
    folderDropTarget,
    folderDropZoneClass,
  };
}
