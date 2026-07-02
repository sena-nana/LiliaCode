import { invoke } from "../tauri/runtime";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { shallowRef } from "vue";
import type { ProjectDashboardSummary } from "@lilia/contracts";
import { PROJECT_DASHBOARD_LIST_COMMAND } from "@lilia/contracts/projectCommandsContract.mjs";
import { TASKS_CHANGED_EVENT_NAME } from "@lilia/contracts/taskEventsContract.mjs";
import { addDomEventListener } from "@lilia/ui";

export const PROJECT_DASHBOARD_SUMMARIES = shallowRef<ProjectDashboardSummary[]>([]);

const loaded = shallowRef(false);
let loadPromise: Promise<ProjectDashboardSummary[]> | null = null;
let listenerInstalled = false;
let listenerInstallPromise: Promise<void> | null = null;
let listenerUnlisten: UnlistenFn | null = null;
let beforeUnloadUnlisten: UnlistenFn | null = null;

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
    .then((unlisten) => {
      listenerUnlisten = unlisten;
      listenerInstalled = true;
      installProjectDashboardBeforeUnloadCleanup();
    })
    .catch((err) => {
      console.error(`[project-dashboard] listen ${TASKS_CHANGED_EVENT_NAME} failed`, err);
    })
    .finally(() => {
      listenerInstallPromise = null;
    });
}

function installProjectDashboardBeforeUnloadCleanup() {
  if (beforeUnloadUnlisten || typeof window === "undefined") return;
  beforeUnloadUnlisten = addDomEventListener(
    window,
    "beforeunload",
    disposeProjectDashboardListener,
    { once: true },
  );
}

export function disposeProjectDashboardListener() {
  listenerUnlisten?.();
  listenerUnlisten = null;
  listenerInstalled = false;
  listenerInstallPromise = null;
  beforeUnloadUnlisten?.();
  beforeUnloadUnlisten = null;
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

