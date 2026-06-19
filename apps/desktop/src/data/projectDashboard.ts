import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { shallowRef } from "vue";
import type { ProjectDashboardSummary } from "@lilia/contracts";

export const PROJECT_DASHBOARD_SUMMARIES = shallowRef<ProjectDashboardSummary[]>([]);

const loaded = shallowRef(false);
let loadPromise: Promise<ProjectDashboardSummary[]> | null = null;
let listenerInstalled = false;

async function refreshProjectDashboard(): Promise<ProjectDashboardSummary[]> {
  const rows = await invoke<ProjectDashboardSummary[]>("project_dashboard_list");
  PROJECT_DASHBOARD_SUMMARIES.value = rows;
  loaded.value = true;
  return rows;
}

function installProjectDashboardListener() {
  if (listenerInstalled) return;
  listenerInstalled = true;
  void listen("tasks:changed", () => {
    if (!loaded.value) return;
    void refreshProjectDashboard().catch((err) => {
      console.error("[project-dashboard] refresh failed", err);
    });
  }).catch((err) => {
    console.error("[project-dashboard] listen tasks:changed failed", err);
  });
}

installProjectDashboardListener();

export function listProjectDashboardSummaries(): ProjectDashboardSummary[] {
  return PROJECT_DASHBOARD_SUMMARIES.value;
}

export function ensureProjectDashboardLoaded(
  force = false,
): Promise<ProjectDashboardSummary[]> {
  if (!force && loaded.value) return Promise.resolve(PROJECT_DASHBOARD_SUMMARIES.value);
  if (!force && loadPromise) return loadPromise;
  loadPromise = refreshProjectDashboard().finally(() => {
    loadPromise = null;
  });
  return loadPromise;
}
