import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { shallowRef } from "vue";
import type { ProjectDashboardSummary } from "@lilia/contracts";
import { PROJECT_DASHBOARD_LIST_COMMAND, TASKS_CHANGED_EVENT_NAME } from "@lilia/contracts";

export const PROJECT_DASHBOARD_SUMMARIES = shallowRef<ProjectDashboardSummary[]>([]);

const loaded = shallowRef(false);
let loadPromise: Promise<ProjectDashboardSummary[]> | null = null;
let listenerInstalled = false;
let listenerInstallPromise: Promise<void> | null = null;

async function refreshProjectDashboard(): Promise<ProjectDashboardSummary[]> {
  const rows = await invoke<ProjectDashboardSummary[]>(PROJECT_DASHBOARD_LIST_COMMAND);
  PROJECT_DASHBOARD_SUMMARIES.value = rows;
  loaded.value = true;
  return rows;
}

function installProjectDashboardListener() {
  if (listenerInstalled || listenerInstallPromise) return;
  listenerInstallPromise = listen(TASKS_CHANGED_EVENT_NAME, () => {
    if (!loaded.value) return;
    void refreshProjectDashboard().catch((err) => {
      console.error("[project-dashboard] refresh failed", err);
    });
  })
    .then(() => {
      listenerInstalled = true;
    })
    .catch((err) => {
      console.error(`[project-dashboard] listen ${TASKS_CHANGED_EVENT_NAME} failed`, err);
    })
    .finally(() => {
      listenerInstallPromise = null;
    });
}

installProjectDashboardListener();

export function listProjectDashboardSummaries(): ProjectDashboardSummary[] {
  return PROJECT_DASHBOARD_SUMMARIES.value;
}

export function ensureProjectDashboardLoaded(
  force = false,
): Promise<ProjectDashboardSummary[]> {
  installProjectDashboardListener();
  if (!force && loaded.value) return Promise.resolve(PROJECT_DASHBOARD_SUMMARIES.value);
  if (!force && loadPromise) return loadPromise;
  loadPromise = refreshProjectDashboard().finally(() => {
    loadPromise = null;
  });
  return loadPromise;
}
