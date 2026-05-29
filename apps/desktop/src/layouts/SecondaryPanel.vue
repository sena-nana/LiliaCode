<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import {
  Settings,
  MessageSquarePlus,
  Puzzle,
  Plus,
  ChevronsDownUp,
  ChevronsUpDown,
  AlertTriangle,
  FolderOpen,
  FolderPlus,
  Github,
  Sparkles,
  X,
  Archive,
  Pin,
  LayoutGrid,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import {
  createProject,
  deriveProjectName,
  listProjects,
  reorderProjects,
} from "../services/projectsStore";
import {
  archiveTask,
  createDraftOrphan,
  createDraftTask,
  listOrphanConversations,
  listProjectConversations,
  reorderTasks,
  reparentTask,
  toggleTaskPin,
} from "../services/tasksStore";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import { pickFolder } from "../services/projects";

import ProjectTreeItem from "../components/sidebar/ProjectTreeItem.vue";
import CloneRepoDialog from "../components/sidebar/CloneRepoDialog.vue";
import CategoryDialog from "../components/sidebar/CategoryDialog.vue";

const route = useRoute();
const router = useRouter();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const { statusFor } = useConnectionStatus();

// ── Connection status badge ──

const primaryStatus = computed(() => {
  const claude = statusFor("claude");
  const codex = statusFor("codex");
  if (claude && claude.connectionMode !== "unconfigured") {
    return { backend: "claude" as const, status: claude };
  }
  if (codex && codex.connectionMode !== "unconfigured") {
    return { backend: "codex" as const, status: codex };
  }
  return claude || codex
    ? { backend: "claude" as const, status: claude ?? codex! }
    : null;
});

const backendLabel = computed(() =>
  primaryStatus.value?.backend === "codex" ? "Codex" : "Claude",
);

const isUnconfigured = computed(
  () => primaryStatus.value?.status.connectionMode === "unconfigured" ||
    primaryStatus.value === null,
);

const connectionTooltip = computed(() => {
  const ps = primaryStatus.value;
  if (!ps) return "正在检测 agent 连接…";
  const s = ps.status;
  if (s.connectionMode === "unconfigured") {
    return "CC-Switch 代理不可达。点击进入设置。";
  }
  return `${backendLabel.value} · ${s.effectiveUrl ?? "—"}`;
});

// ── Project tree expansion ──

const TREE_EXPANSION_KEY = "lilia.projectTree.expansion";

interface ProjectTreeExpansionSnapshot {
  projects: Record<string, boolean>;
  orphansExpanded: boolean;
}

function loadProjectTreeExpansion(): Partial<ProjectTreeExpansionSnapshot> {
  try {
    const raw = localStorage.getItem(TREE_EXPANSION_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Partial<ProjectTreeExpansionSnapshot>;
    const projectEntries =
      parsed.projects && typeof parsed.projects === "object"
        ? Object.entries(parsed.projects).filter(
            ([, value]) => typeof value === "boolean",
          )
        : [];
    return {
      projects: Object.fromEntries(projectEntries),
      orphansExpanded:
        typeof parsed.orphansExpanded === "boolean"
          ? parsed.orphansExpanded
          : undefined,
    };
  } catch {
    return {};
  }
}

const savedTreeExpansion = loadProjectTreeExpansion();
const expanded = reactive<Record<string, boolean>>({});
const orphansExpanded = ref(savedTreeExpansion.orphansExpanded ?? true);

function isProjectExpanded(projectId: string): boolean {
  return expanded[projectId] !== false;
}

function persistProjectTreeExpansion() {
  try {
    const projectSnapshot: Record<string, boolean> = {};
    for (const project of projects.value) {
      projectSnapshot[project.id] = isProjectExpanded(project.id);
    }
    localStorage.setItem(
      TREE_EXPANSION_KEY,
      JSON.stringify({
        projects: projectSnapshot,
        orphansExpanded: orphansExpanded.value,
      } satisfies ProjectTreeExpansionSnapshot),
    );
  } catch {
    /* localStorage 不可用或配额满时忽略。 */
  }
}

function syncProjectExpansion(nextProjects: Project[]): boolean {
  let changed = false;
  const liveIds = new Set(nextProjects.map((project) => project.id));
  for (const projectId of Object.keys(expanded)) {
    if (!liveIds.has(projectId)) {
      delete expanded[projectId];
      changed = true;
    }
  }
  for (const project of nextProjects) {
    if (!Object.prototype.hasOwnProperty.call(expanded, project.id)) {
      expanded[project.id] = savedTreeExpansion.projects?.[project.id] ?? true;
      changed = true;
    }
  }
  return changed;
}

watch(
  projects,
  (nextProjects) => {
    if (syncProjectExpansion(nextProjects)) {
      persistProjectTreeExpansion();
    }
  },
  { immediate: true },
);

function toggle(projectId: string) {
  expanded[projectId] = !isProjectExpanded(projectId);
  persistProjectTreeExpansion();
}

const allExpanded = computed(
  () =>
    projects.value.length > 0 &&
    projects.value.every((p) => isProjectExpanded(p.id)),
);

function toggleAll() {
  const target = !allExpanded.value;
  for (const p of projects.value) expanded[p.id] = target;
  persistProjectTreeExpansion();
}

function rememberExpanded(projectId: string) {
  expanded[projectId] = true;
  persistProjectTreeExpansion();
}

function rememberCurrentExpansion() {
  persistProjectTreeExpansion();
}

if (typeof window !== "undefined") {
  window.addEventListener("beforeunload", rememberCurrentExpansion);
}

onBeforeUnmount(() => {
  window.removeEventListener("beforeunload", rememberCurrentExpansion);
  persistProjectTreeExpansion();
});

// ── Orphan inbox ──

function toggleOrphans() {
  orphansExpanded.value = !orphansExpanded.value;
  persistProjectTreeExpansion();
}

// --- 孤儿 session 归档确认 ---
const confirmingOrphanId = ref<string | null>(null);

function onOrphanArchiveClick(e: MouseEvent, orphanId: string) {
  e.preventDefault();
  e.stopPropagation();
  if (confirmingOrphanId.value === orphanId) {
    archiveTask(orphanId);
    confirmingOrphanId.value = null;
  } else {
    confirmingOrphanId.value = orphanId;
  }
}

async function onOrphanPinClick(e: MouseEvent, orphanId: string) {
  e.preventDefault();
  e.stopPropagation();
  try {
    await toggleTaskPin(orphanId);
  } catch (err) {
    projectError.value = `切换对话置顶失败：${String(err)}`;
  }
}

function onOrphanRowLeave() {
  confirmingOrphanId.value = null;
}

// ── Navigation helpers ──

function isActiveOrphan(taskId: string) {
  return route.path === `/chats/${taskId}`;
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

// ── New chat ──

function newChat() {
  const draft = createDraftOrphan();
  router.push(`/chats/${draft.id}`);
}

function newProjectChat(projectId: string) {
  openProjectChat(projectId);
}

function openProjectChat(projectId: string) {
  const draft = createDraftTask(projectId);
  if (!draft) return;
  rememberExpanded(projectId);
  router.push(`/projects/${projectId}/tasks/${draft.id}`);
}

function openProjectsOverview() {
  router.push("/projects");
}

// ── Add project menu ──

const addMenuOpen = ref(false);
const menuPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const MENU_W = 200;
const MENU_H_EST = 132;
const projectError = ref<string | null>(null);

function dismissError() {
  projectError.value = null;
}

function openAddMenu(e: MouseEvent) {
  const x = Math.min(e.clientX, window.innerWidth - MENU_W - 4);
  const y = Math.min(e.clientY, window.innerHeight - MENU_H_EST - 4);
  menuPos.value = { x: Math.max(4, x), y: Math.max(4, y) };
  addMenuOpen.value = true;
}

function closeAddMenu() {
  addMenuOpen.value = false;
}

function onDocPointer(e: PointerEvent) {
  const target = e.target as HTMLElement | null;
  if (target && target.closest && target.closest(".sb-menu")) return;
  closeAddMenu();
}

function onDocKey(e: KeyboardEvent) {
  if (e.key === "Escape" && addMenuOpen.value) {
    closeAddMenu();
    e.stopPropagation();
  }
}

watch(addMenuOpen, async (v) => {
  if (v) {
    await nextTick();
    document.addEventListener("pointerdown", onDocPointer, true);
    document.addEventListener("keydown", onDocKey);
  } else {
    document.removeEventListener("pointerdown", onDocPointer, true);
    document.removeEventListener("keydown", onDocKey);
  }
});

// ── Local folder picker ──

async function pickLocalFolder() {
  closeAddMenu();
  projectError.value = null;
  try {
    const picked = await pickFolder({ title: "选择项目根目录" });
    if (!picked) return;
    const project = await createProject({
      name: deriveProjectName(picked) || "新项目",
      cwd: picked,
    });
    openProjectChat(project.id);
  } catch (err) {
    projectError.value = `选择文件夹失败：${String(err)}`;
  }
}

// ── Clone dialog ──

const cloneOpen = ref(false);
const cloneDialogRef = ref<InstanceType<typeof CloneRepoDialog> | null>(null);

function openClone() {
  closeAddMenu();
  cloneOpen.value = true;
  nextTick(() => cloneDialogRef.value?.init());
}

function onCloneCreated(p: Project) {
  openProjectChat(p.id);
}

// ── Category dialog ──

const categoryOpen = ref(false);
const categoryDialogRef = ref<InstanceType<typeof CategoryDialog> | null>(null);

function openCategory() {
  closeAddMenu();
  categoryOpen.value = true;
  nextTick(() => categoryDialogRef.value?.init());
}

function onCategoryCreated(p: Project) {
  openProjectChat(p.id);
}

// ── Project tree item handlers ──

function onProjectArchived(projectId: string) {
  openProjectChat(projectId);
}

function onProjectDeleted(projectId: string) {
  router.push("/");
  delete expanded[projectId];
  persistProjectTreeExpansion();
}

// ── Project tree pointer drag ──

type TreeDragKind = "project" | "task";
type TreeDropPosition = "before" | "after" | "inside";

interface TreeDragSource {
  kind: TreeDragKind;
  pointerId: number;
  startX: number;
  startY: number;
  active: boolean;
  projectId: string | null;
  taskId: string | null;
  pinned: boolean;
}

interface TreeDropTarget {
  kind: "project" | "task" | "orphans";
  projectId: string | null;
  taskId: string | null;
  pinned: boolean | null;
  position: TreeDropPosition;
  valid: boolean;
}

const DRAG_THRESHOLD = 4;
const treeDrag = ref<TreeDragSource | null>(null);
const treeDropTarget = ref<TreeDropTarget | null>(null);
const suppressTreeClick = ref(false);

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

function positionForElement(event: PointerEvent, element: HTMLElement): Exclude<TreeDropPosition, "inside"> {
  const rect = element.getBoundingClientRect();
  return event.clientY > rect.top + rect.height / 2 ? "after" : "before";
}

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

function insertRelative(
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
    projectError.value = `调整项目树失败：${String(err)}`;
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
</script>

<template>
  <aside
    class="secondary-panel"
    :class="{ 'is-tree-dragging': treeDrag?.active }"
    @pointerdown="onTreePointerDown"
    @click.capture="onTreeClickCapture"
  >
    <div class="sb-section sb-section--actions">
      <button
        type="button"
        class="sb-primary-btn"
        title="新对话"
        aria-label="新对话"
        @click="newChat"
      >
        <MessageSquarePlus :size="15" aria-hidden="true" />
        <span class="sb-primary-btn__label">新对话</span>
      </button>
      <button type="button" class="sb-icon-action" title="项目" aria-label="项目" @click="openProjectsOverview">
        <LayoutGrid :size="15" aria-hidden="true" />
      </button>
    </div>

    <div class="sb-section">
      <div class="sb-section__header">
        <span class="sb-section__title">项目</span>
        <div class="sb-section__tools">
          <button type="button" class="sb-icon-btn"
            :title="allExpanded ? '全部折叠' : '全部展开'"
            :aria-label="allExpanded ? '全部折叠' : '全部展开'"
            @click="toggleAll">
            <ChevronsDownUp v-if="allExpanded" :size="14" aria-hidden="true" />
            <ChevronsUpDown v-else :size="14" aria-hidden="true" />
          </button>
          <button type="button" class="sb-icon-btn" title="添加项目" aria-label="添加项目"
            :aria-expanded="addMenuOpen" :aria-haspopup="true"
            @click="openAddMenu">
            <Plus :size="14" aria-hidden="true" />
          </button>
        </div>
      </div>

      <div v-if="projectError" class="sb-banner sb-banner--err">
        <AlertTriangle :size="12" aria-hidden="true" />
        <span class="sb-banner__msg">{{ projectError }}</span>
        <button type="button" class="sb-icon-btn" @click="dismissError" aria-label="忽略错误">
          <X :size="12" aria-hidden="true" />
        </button>
      </div>

      <div class="sb-tree">
        <ProjectTreeItem
          v-for="p in projects"
          :key="p.id"
          :project="p"
          :is-expanded="isProjectExpanded(p.id)"
          :drag-source="treeDrag"
          :drop-target="treeDropTarget"
          @toggle="toggle"
          @new-chat="newProjectChat"
          @error="(msg: string) => projectError = msg"
          @archived="onProjectArchived(p.id)"
          @deleted="onProjectDeleted(p.id)"
        />

        <p v-if="projects.length === 0" class="sb-tree__empty">暂无项目</p>
      </div>
    </div>

    <div class="sb-section">
      <div class="sb-section__header">
        <span class="sb-section__title">收集箱</span>
        <div class="sb-section__tools">
          <button type="button" class="sb-icon-btn"
            :title="orphansExpanded ? '折叠收集箱' : '展开收集箱'"
            :aria-label="orphansExpanded ? '折叠收集箱' : '展开收集箱'"
            @click="toggleOrphans">
            <ChevronsDownUp v-if="orphansExpanded" :size="14" aria-hidden="true" />
            <ChevronsUpDown v-else :size="14" aria-hidden="true" />
          </button>
          <button type="button" class="sb-icon-btn" title="新对话" aria-label="新对话" @click="newChat">
            <Plus :size="14" aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="sb-collapse" :class="{ 'is-open': orphansExpanded }" :aria-hidden="!orphansExpanded">
        <div class="sb-collapse__inner">
          <div class="sb-tree" :class="orphanDropZoneClass" data-tree-drop-zone="orphans">
            <RouterLink v-for="o in orphans" :key="o.id" :to="`/chats/${o.id}`" class="sb-tree__row sb-tree__row--orphan"
              :class="[
                { 'is-active': isActiveOrphan(o.id) },
                treeRowStateClass('task', null, o.id),
              ]"
              draggable="false"
              data-tree-kind="task"
              :data-task-id="o.id"
              data-project-id=""
              :data-pinned="o.pinned ? 'true' : 'false'"
              @dragstart.prevent
              @mouseleave="onOrphanRowLeave">
              <span class="sb-tree__name">{{ o.title }}</span>
              <div class="sb-tree__hover-tools" @click.stop>
                <button type="button" class="sb-icon-btn" :class="{ 'is-pinned': o.pinned }"
                  :title="o.pinned ? '取消置顶' : '置顶'"
                  :aria-label="o.pinned ? '取消置顶' : '置顶'"
                  @click="onOrphanPinClick($event, o.id)">
                  <Pin :size="13" aria-hidden="true" />
                </button>
                <button type="button" class="sb-icon-btn" :class="{ 'is-confirming': confirmingOrphanId === o.id }"
                  :title="confirmingOrphanId === o.id ? '确认归档，再点一次' : '归档'"
                  :aria-label="confirmingOrphanId === o.id ? '确认归档，再点一次' : '归档'"
                  @click="onOrphanArchiveClick($event, o.id)">
                  <template v-if="confirmingOrphanId === o.id">确认</template>
                  <Archive v-else :size="13" aria-hidden="true" />
                </button>
              </div>
            </RouterLink>
            <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
          </div>
        </div>
      </div>
    </div>

    <div class="sb-footer">
      <RouterLink to="/settings" class="sb-footer__btn" active-class="is-active" title="设置" aria-label="设置">
        <Settings :size="14" aria-hidden="true" />
      </RouterLink>

      <RouterLink to="/plugins" class="sb-footer__btn" active-class="is-active" title="插件 / 技能" aria-label="插件 / 技能">
        <Puzzle :size="14" aria-hidden="true" />
      </RouterLink>

      <RouterLink to="/settings" class="sb-conn" :class="isUnconfigured ? 'sb-conn--warn' : 'sb-conn--ok'"
        :title="connectionTooltip" :aria-label="connectionTooltip">
        <template v-if="isUnconfigured">
          <AlertTriangle :size="12" aria-hidden="true" />
          <span class="sb-conn__label">未连接</span>
        </template>
        <template v-else-if="primaryStatus">
          <Sparkles :size="12" aria-hidden="true" />
          <span class="sb-conn__label">{{ backendLabel }}</span>
        </template>
        <template v-else>
          <span class="sb-conn__label sb-conn__label--probing">检测中…</span>
        </template>
      </RouterLink>
    </div>

    <!-- Add project context menu -->
    <Teleport to="body">
      <div v-if="addMenuOpen" class="sb-menu" role="menu"
        :style="{ left: `${menuPos.x}px`, top: `${menuPos.y}px` }">
        <button type="button" class="sb-menu__item" role="menuitem" @click="pickLocalFolder">
          <FolderOpen :size="13" aria-hidden="true" />
          <span class="sb-menu__label">使用本地文件夹</span>
        </button>
        <button type="button" class="sb-menu__item" role="menuitem" @click="openClone">
          <Github :size="13" aria-hidden="true" />
          <span class="sb-menu__label">从 GitHub clone</span>
        </button>
        <button type="button" class="sb-menu__item" role="menuitem" @click="openCategory">
          <FolderPlus :size="13" aria-hidden="true" />
          <span class="sb-menu__label">创建空分类</span>
        </button>
      </div>
    </Teleport>

    <!-- Clone dialog -->
    <CloneRepoDialog
      ref="cloneDialogRef"
      :open="cloneOpen"
      @close="cloneOpen = false"
      @cloned="onCloneCreated"
      @error="(msg: string) => projectError = msg"
    />

    <!-- Category dialog -->
    <CategoryDialog
      ref="categoryDialogRef"
      :open="categoryOpen"
      @close="categoryOpen = false"
      @created="onCategoryCreated"
    />
  </aside>
</template>
