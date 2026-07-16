import {
  computed,
  onBeforeUnmount,
  reactive,
  ref,
  watch,
  type ComputedRef,
} from "vue";
import { useRoute } from "vue-router";
import type { Project } from "@lilia/contracts";
import {
  ensureProjectsLoaded,
} from "../services/projectsStore";
import {
  areOrphansLoaded,
  ensureOrphansLoaded,
  ensureProjectTasksLoaded,
} from "../services/tasksStore";
import { beginPerfStage, cancelIdleRun, measurePerfAsync, runWhenIdle, scheduleAfterPaint } from "@lilia/ui/diagnostics";

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

export function useProjectTreeExpansion(
  projects: ComputedRef<Project[]>,
  reportError: (message: string) => void,
) {
  const route = useRoute();
  const savedTreeExpansion = loadProjectTreeExpansion();
  const expanded = reactive<Record<string, boolean>>({});
  const orphansExpanded = ref(savedTreeExpansion.orphansExpanded ?? true);
  const sidebarDataReady = ref(false);
  let expandedProjectHydrationHandle: number | null = null;
  let cancelExpandedProjectHydrationPaint: (() => void) | null = null;
  let expandedProjectHydrationStage: ReturnType<typeof beginPerfStage> | null = null;
  let expandedProjectHydrationQueue: string[] = [];
  let expandedProjectHydrationSeq = 0;
  let orphanHydrationHandle: number | null = null;
  let cancelOrphanHydrationPaint: (() => void) | null = null;
  let orphanHydrationSeq = 0;
  let initialSidebarDataSeq = 0;
  let disposed = false;

  function isProjectExpanded(projectId: string): boolean {
    if (String(route.params.projectId ?? "") === projectId) return true;
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

  async function loadProjectTasks(projectId: string, source = "sidebar.project") {
    if (disposed) return;
    try {
      await measurePerfAsync(
        "sidebar.project-tasks.load",
        () => ensureProjectTasksLoaded(projectId),
        { detail: `${source}:${projectId}` },
      );
    } catch (err) {
      if (disposed) return;
      reportError(`加载项目对话失败：${String(err)}`);
    }
  }

  function finishExpandedProjectHydration(stage: string) {
    expandedProjectHydrationStage?.end(stage);
    expandedProjectHydrationStage = null;
  }

  function cancelExpandedProjectHydration(stage = "superseded") {
    expandedProjectHydrationSeq += 1;
    cancelExpandedProjectHydrationPaint?.();
    cancelExpandedProjectHydrationPaint = null;
    if (expandedProjectHydrationHandle !== null) {
      cancelIdleRun(expandedProjectHydrationHandle);
      expandedProjectHydrationHandle = null;
    }
    expandedProjectHydrationQueue = [];
    finishExpandedProjectHydration(stage);
  }

  function cancelOrphanHydration() {
    orphanHydrationSeq += 1;
    cancelOrphanHydrationPaint?.();
    cancelOrphanHydrationPaint = null;
    if (orphanHydrationHandle !== null) {
      cancelIdleRun(orphanHydrationHandle);
      orphanHydrationHandle = null;
    }
  }

  function scheduleOrphanHydration(source: string) {
    if (disposed) return;
    cancelOrphanHydration();
    if (areOrphansLoaded()) return;
    const seq = orphanHydrationSeq;
    cancelOrphanHydrationPaint = scheduleAfterPaint(() => {
      cancelOrphanHydrationPaint = null;
      if (disposed || seq !== orphanHydrationSeq) return;
      orphanHydrationHandle = runWhenIdle(() => {
        orphanHydrationHandle = null;
        if (disposed || seq !== orphanHydrationSeq) return;
        void measurePerfAsync(
          "sidebar.orphans.load",
          () => ensureOrphansLoaded(),
          { detail: source },
        ).catch((err) => {
          if (disposed || seq !== orphanHydrationSeq) return;
          reportError(`加载收集箱对话失败：${String(err)}`);
        });
      });
    });
  }

  function listExpandedProjectIds(): string[] {
    return projects.value
      .filter((project) => isProjectExpanded(project.id))
      .map((project) => project.id);
  }

  function pumpExpandedProjectHydration(seq: number) {
    if (disposed || seq !== expandedProjectHydrationSeq) return;
    if (expandedProjectHydrationQueue.length === 0) {
      finishExpandedProjectHydration("idle");
      return;
    }
    expandedProjectHydrationHandle = runWhenIdle(async () => {
      expandedProjectHydrationHandle = null;
      if (disposed || seq !== expandedProjectHydrationSeq) return;
      const nextProjectId = expandedProjectHydrationQueue.shift();
      if (nextProjectId) {
        await loadProjectTasks(nextProjectId, "sidebar.expanded-project");
      }
      if (disposed || seq !== expandedProjectHydrationSeq) return;
      if (expandedProjectHydrationQueue.length === 0) {
        finishExpandedProjectHydration("idle");
        return;
      }
      pumpExpandedProjectHydration(seq);
    });
  }

  function scheduleExpandedProjectHydration(source: string) {
    if (disposed) return;
    const expandedProjectIds = listExpandedProjectIds();
    const currentProjectId =
      typeof route.params.projectId === "string" && expandedProjectIds.includes(route.params.projectId)
        ? route.params.projectId
        : null;
    cancelExpandedProjectHydration();
    if (currentProjectId) {
      void loadProjectTasks(currentProjectId, `${source}:route`);
    }
    const deferredIds = expandedProjectIds.filter((projectId) => projectId !== currentProjectId);
    if (deferredIds.length === 0) return;
    const seq = expandedProjectHydrationSeq;
    expandedProjectHydrationQueue = deferredIds;
    expandedProjectHydrationStage = beginPerfStage("sidebar.expanded-projects.hydrate", {
      detail: `${source}:${deferredIds.length}`,
    });
    cancelExpandedProjectHydrationPaint = scheduleAfterPaint(() => {
      cancelExpandedProjectHydrationPaint = null;
      if (disposed || seq !== expandedProjectHydrationSeq) return;
      pumpExpandedProjectHydration(seq);
    });
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
        expanded[project.id] = savedTreeExpansion.projects?.[project.id] ?? false;
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
      if (sidebarDataReady.value) {
        scheduleExpandedProjectHydration("sidebar.projects.sync");
      }
    },
    { immediate: true },
  );

  async function loadInitialSidebarData() {
    const seq = ++initialSidebarDataSeq;
    await measurePerfAsync(
      "sidebar.projects.load",
      () => ensureProjectsLoaded(),
      { detail: "sidebar.initial" },
    );
    if (disposed || seq !== initialSidebarDataSeq) return;
    sidebarDataReady.value = true;
    scheduleExpandedProjectHydration("sidebar.initial");
    scheduleOrphanHydration("sidebar.initial");
  }

  watch(
    () => route.params.projectId,
    (projectId) => {
      if (
        sidebarDataReady.value &&
        typeof projectId === "string" &&
        projectId.length > 0
      ) {
        void loadProjectTasks(projectId, "sidebar.route");
        scheduleExpandedProjectHydration("sidebar.route.sync");
      }
    },
    { immediate: true },
  );

  function toggle(projectId: string) {
    if (disposed) return;
    expanded[projectId] = !isProjectExpanded(projectId);
    persistProjectTreeExpansion();
    if (isProjectExpanded(projectId)) {
      void loadProjectTasks(projectId, "sidebar.toggle");
      scheduleExpandedProjectHydration("sidebar.toggle.sync");
      return;
    }
    scheduleExpandedProjectHydration("sidebar.toggle.sync");
  }

  const allExpanded = computed(
    () =>
      projects.value.length > 0 &&
      projects.value.every((p) => isProjectExpanded(p.id)),
  );

  function toggleAll() {
    if (disposed) return;
    const target = !allExpanded.value;
    for (const p of projects.value) expanded[p.id] = target;
    persistProjectTreeExpansion();
    scheduleExpandedProjectHydration("sidebar.toggle-all");
  }

  function rememberExpanded(projectId: string) {
    if (disposed) return;
    expanded[projectId] = true;
    persistProjectTreeExpansion();
  }

  function forgetProject(projectId: string) {
    if (disposed) return;
    delete expanded[projectId];
    persistProjectTreeExpansion();
  }

  function rememberCurrentExpansion() {
    if (disposed) return;
    persistProjectTreeExpansion();
  }

  if (typeof window !== "undefined") {
    window.addEventListener("beforeunload", rememberCurrentExpansion);
  }

  onBeforeUnmount(() => {
    disposed = true;
    initialSidebarDataSeq += 1;
    cancelExpandedProjectHydration("disposed");
    cancelOrphanHydration();
    window.removeEventListener("beforeunload", rememberCurrentExpansion);
    persistProjectTreeExpansion();
  });

  function toggleOrphans() {
    if (disposed) return;
    orphansExpanded.value = !orphansExpanded.value;
    persistProjectTreeExpansion();
  }

  return {
    allExpanded,
    forgetProject,
    isProjectExpanded,
    loadInitialSidebarData,
    orphansExpanded,
    rememberExpanded,
    toggle,
    toggleAll,
    toggleOrphans,
  };
}
