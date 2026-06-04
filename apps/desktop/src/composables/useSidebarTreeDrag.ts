import { computed, onBeforeUnmount, ref, type ComputedRef } from "vue";
import type { Project } from "@lilia/contracts";
import { reorderProjects } from "../services/projectsStore";
import {
  listProjectConversations,
  reorderTasks,
  reparentTask,
  type OrphanConversation,
} from "../services/tasksStore";

export type TreeDragKind = "project" | "task";
export type TreeDropPosition = "before" | "after" | "inside";

export interface TreeDragSource {
  kind: TreeDragKind;
  pointerId: number;
  startX: number;
  startY: number;
  active: boolean;
  projectId: string | null;
  taskId: string | null;
  pinned: boolean;
}

export interface TreeDropTarget {
  kind: "project" | "task" | "orphans";
  projectId: string | null;
  taskId: string | null;
  pinned: boolean | null;
  position: TreeDropPosition;
  valid: boolean;
}

const DRAG_THRESHOLD = 4;

function normalizeDatasetId(value: string | undefined): string | null {
  return value && value.length > 0 ? value : null;
}

function isInteractiveTreeTarget(target: HTMLElement): boolean {
  return !!target.closest(
    "button,input,textarea,select,.sb-tree__hover-tools,.sb-menu,[data-no-tree-drag]",
  );
}

function sourceFromTreeRow(row: HTMLElement, event: PointerEvent): TreeDragSource | null {
  const kind = row.dataset.treeKind;
  const pinned = row.dataset.pinned === "true";
  if (kind === "project") {
    const projectId = normalizeDatasetId(row.dataset.projectId);
    if (!projectId) return null;
    return {
      kind,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
      projectId,
      taskId: null,
      pinned,
    };
  }
  if (kind === "task") {
    const taskId = normalizeDatasetId(row.dataset.taskId);
    if (!taskId) return null;
    return {
      kind,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
      projectId: normalizeDatasetId(row.dataset.projectId),
      taskId,
      pinned,
    };
  }
  return null;
}

function closestTreeDropElement(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement)) return null;
  return target.closest<HTMLElement>("[data-tree-kind],[data-tree-drop-zone]");
}

function elementAtPointer(event: PointerEvent): HTMLElement | null {
  const direct = closestTreeDropElement(event.target);
  if (direct) return direct;
  const fromPoint = document.elementFromPoint?.(event.clientX, event.clientY);
  return closestTreeDropElement(fromPoint);
}

function positionForElement(
  event: PointerEvent,
  element: HTMLElement,
): Exclude<TreeDropPosition, "inside"> {
  const rect = element.getBoundingClientRect();
  return event.clientY > rect.top + rect.height / 2 ? "after" : "before";
}

function isSameTreeRow(
  marker: TreeDragSource | TreeDropTarget | null | undefined,
  kind: TreeDragKind,
  projectId: string | null,
  taskId: string | null,
): boolean {
  return !!marker &&
    marker.kind === kind &&
    marker.projectId === projectId &&
    marker.taskId === taskId;
}

export function insertRelative(
  ids: string[],
  sourceId: string,
  targetId: string,
  position: Exclude<TreeDropPosition, "inside">,
): string[] {
  const withoutSource = ids.filter((id) => id !== sourceId);
  const targetIndex = withoutSource.indexOf(targetId);
  if (targetIndex < 0) return ids;
  const insertIndex = position === "after" ? targetIndex + 1 : targetIndex;
  return [
    ...withoutSource.slice(0, insertIndex),
    sourceId,
    ...withoutSource.slice(insertIndex),
  ];
}

export function useSidebarTreeDrag(
  projects: ComputedRef<Project[]>,
  orphans: ComputedRef<OrphanConversation[]>,
  reportError: (message: string) => void,
) {
  const treeDrag = ref<TreeDragSource | null>(null);
  const treeDropTarget = ref<TreeDropTarget | null>(null);
  const suppressTreeClick = ref(false);

  function resolveTreeDropTarget(event: PointerEvent): TreeDropTarget | null {
    const source = treeDrag.value;
    if (!source) return null;
    const element = elementAtPointer(event);
    if (!element) return null;

    if (element.dataset.treeDropZone === "orphans") {
      return source.kind === "task"
        ? {
            kind: "orphans",
            projectId: null,
            taskId: null,
            pinned: null,
            position: "inside",
            valid: true,
          }
        : null;
    }

    if (element.dataset.treeKind === "project") {
      const projectId = normalizeDatasetId(element.dataset.projectId);
      if (!projectId) return null;
      const pinned = element.dataset.pinned === "true";
      if (source.kind === "project") {
        return {
          kind: "project",
          projectId,
          taskId: null,
          pinned,
          position: positionForElement(event, element),
          valid: source.projectId !== projectId && source.pinned === pinned,
        };
      }
      return {
        kind: "project",
        projectId,
        taskId: null,
        pinned,
        position: "inside",
        valid: source.projectId !== projectId,
      };
    }

    if (element.dataset.treeKind === "task") {
      const taskId = normalizeDatasetId(element.dataset.taskId);
      if (!taskId || source.kind !== "task") return null;
      const projectId = normalizeDatasetId(element.dataset.projectId);
      const pinned = element.dataset.pinned === "true";
      return {
        kind: "task",
        projectId,
        taskId,
        pinned,
        position: positionForElement(event, element),
        valid: source.taskId !== taskId && source.pinned === pinned,
      };
    }

    return null;
  }

  function treeRowStateClass(
    kind: TreeDragKind,
    projectId: string | null,
    taskId: string | null,
  ) {
    const isSource = treeDrag.value?.active === true &&
      isSameTreeRow(treeDrag.value, kind, projectId, taskId);
    const isTarget = isSameTreeRow(treeDropTarget.value, kind, projectId, taskId);
    const position = isTarget ? treeDropTarget.value?.position : null;
    const valid = isTarget && treeDropTarget.value?.valid === true;
    return {
      "is-tree-drag-source": isSource,
      "is-tree-drop-target": isTarget,
      "is-tree-drop-invalid": isTarget && !valid,
      "is-tree-drop-before": valid && position === "before",
      "is-tree-drop-after": valid && position === "after",
      "is-tree-drop-inside": valid && position === "inside",
    };
  }

  const orphanDropZoneClass = computed(() => {
    const target = treeDropTarget.value;
    const valid = target?.kind === "orphans" && target.valid;
    return {
      "is-tree-drop-target": valid,
      "is-tree-drop-inside": valid,
    };
  });

  function taskIdsForProject(projectId: string | null): string[] {
    return projectId
      ? listProjectConversations(projectId).map((task) => task.id)
      : orphans.value.map((orphan) => orphan.id);
  }

  async function applyProjectDrop(source: TreeDragSource, target: TreeDropTarget) {
    if (!source.projectId || !target.projectId || target.kind !== "project" || !target.valid) {
      return;
    }
    const orderedIds = insertRelative(
      projects.value
        .filter((project) => project.pinned === source.pinned)
        .map((project) => project.id),
      source.projectId,
      target.projectId,
      target.position === "inside" ? "after" : target.position,
    );
    await reorderProjects(orderedIds);
  }

  async function applyTaskDrop(source: TreeDragSource, target: TreeDropTarget) {
    if (!source.taskId || !target.valid) return;
    const sourceProjectId = source.projectId;
    const targetProjectId = target.projectId;
    if (sourceProjectId === targetProjectId) {
      if (target.kind !== "task" || !target.taskId || target.position === "inside") return;
      const orderedIds = insertRelative(
        taskIdsForProject(sourceProjectId),
        source.taskId,
        target.taskId,
        target.position,
      );
      await reorderTasks(sourceProjectId, orderedIds);
      return;
    }

    await reparentTask(source.taskId, sourceProjectId, targetProjectId);
    const targetIds = taskIdsForProject(targetProjectId);
    const orderedIds = target.kind === "task" && target.taskId && target.position !== "inside"
      ? insertRelative(targetIds, source.taskId, target.taskId, target.position)
      : targetIds;
    if (orderedIds.includes(source.taskId)) {
      await reorderTasks(targetProjectId, orderedIds);
    }
  }

  async function applyTreeDrop() {
    const source = treeDrag.value;
    const target = treeDropTarget.value;
    if (!source || !target || !target.valid) return;
    try {
      if (source.kind === "project") {
        await applyProjectDrop(source, target);
      } else {
        await applyTaskDrop(source, target);
      }
    } catch (err) {
      reportError(`调整项目树失败：${String(err)}`);
    }
  }

  function onTreePointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    if (isInteractiveTreeTarget(target)) return;
    const row = target.closest<HTMLElement>("[data-tree-kind]");
    if (!row) return;
    const source = sourceFromTreeRow(row, event);
    if (!source) return;
    treeDrag.value = source;
    treeDropTarget.value = null;
    window.addEventListener("pointermove", onTreePointerMove);
    window.addEventListener("pointerup", onTreePointerUp, { once: true });
  }

  function onTreePointerMove(event: PointerEvent) {
    const source = treeDrag.value;
    if (!source || source.pointerId !== event.pointerId) return;
    const moved = Math.hypot(event.clientX - source.startX, event.clientY - source.startY);
    if (!source.active && moved < DRAG_THRESHOLD) return;
    if (!source.active) {
      treeDrag.value = { ...source, active: true };
      suppressTreeClick.value = true;
    }
    event.preventDefault();
    treeDropTarget.value = resolveTreeDropTarget(event);
  }

  async function onTreePointerUp(event: PointerEvent) {
    const source = treeDrag.value;
    window.removeEventListener("pointermove", onTreePointerMove);
    if (!source || source.pointerId !== event.pointerId) {
      treeDrag.value = null;
      treeDropTarget.value = null;
      return;
    }
    if (source.active) {
      event.preventDefault();
      treeDropTarget.value = resolveTreeDropTarget(event);
      await applyTreeDrop();
    }
    treeDrag.value = null;
    treeDropTarget.value = null;
  }

  function onTreeClickCapture(event: MouseEvent) {
    if (!suppressTreeClick.value) return;
    suppressTreeClick.value = false;
    event.preventDefault();
    event.stopPropagation();
  }

  onBeforeUnmount(() => {
    window.removeEventListener("pointermove", onTreePointerMove);
    window.removeEventListener("pointerup", onTreePointerUp);
  });

  return {
    orphanDropZoneClass,
    onTreeClickCapture,
    onTreePointerDown,
    treeDrag,
    treeDropTarget,
    treeRowStateClass,
  };
}
