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
  ensureOrphansLoaded,
  ensureProjectTasksLoaded,
} from "../services/tasksStore";

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

  function loadProjectTasks(projectId: string) {
    void ensureProjectTasksLoaded(projectId).catch((err) => {
      reportError(`加载项目对话失败：${String(err)}`);
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

  function loadExpandedProjectTasks() {
    for (const project of projects.value) {
      if (isProjectExpanded(project.id)) {
        loadProjectTasks(project.id);
      }
    }
  }

  watch(
    projects,
    (nextProjects) => {
      if (syncProjectExpansion(nextProjects)) {
        persistProjectTreeExpansion();
      }
      if (sidebarDataReady.value) {
        loadExpandedProjectTasks();
      }
    },
    { immediate: true },
  );

  async function loadInitialSidebarData() {
    await Promise.all([ensureProjectsLoaded(), ensureOrphansLoaded()]);
    sidebarDataReady.value = true;
    window.setTimeout(loadExpandedProjectTasks, 0);
  }

  watch(
    () => route.params.projectId,
    (projectId) => {
      if (
        sidebarDataReady.value &&
        typeof projectId === "string" &&
        projectId.length > 0
      ) {
        loadProjectTasks(projectId);
      }
    },
    { immediate: true },
  );

  function toggle(projectId: string) {
    expanded[projectId] = !isProjectExpanded(projectId);
    persistProjectTreeExpansion();
    if (isProjectExpanded(projectId)) {
      loadProjectTasks(projectId);
    }
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

  function forgetProject(projectId: string) {
    delete expanded[projectId];
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

  function toggleOrphans() {
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
