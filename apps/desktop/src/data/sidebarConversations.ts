import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { shallowRef } from "vue";
import type { SidebarConversationSummary } from "@lilia/contracts";
import { measurePerfAsync } from "../utils/perf";

export const SIDEBAR_CONVERSATIONS = shallowRef<SidebarConversationSummary[]>([]);
const SIDEBAR_CONVERSATIONS_BY_KEY = shallowRef<Map<string, SidebarConversationSummary>>(new Map());

const loaded = shallowRef(false);
let loadPromise: Promise<SidebarConversationSummary[]> | null = null;
let listenerInstalled = false;
let revision = 0;

async function refreshSidebarConversations(): Promise<SidebarConversationSummary[]> {
  const rows = await measurePerfAsync(
    "sidebar.unified.fetch",
    () => invoke<SidebarConversationSummary[]>("task_list_sidebar_conversations"),
    { detail: "task_list_sidebar_conversations" },
  );
  SIDEBAR_CONVERSATIONS.value = rows;
  SIDEBAR_CONVERSATIONS_BY_KEY.value = new Map(
    rows.map((row) => [sidebarConversationKey(row.projectId, row.taskId), row] as const),
  );
  loaded.value = true;
  revision += 1;
  return rows;
}

function sidebarConversationKey(projectId: string | null, taskId: string): string {
  return `${projectId ?? ""}\x1f${taskId}`;
}

function installSidebarConversationListener() {
  if (listenerInstalled) return;
  listenerInstalled = true;
  void listen("tasks:changed", () => {
    if (!loaded.value) return;
    void refreshSidebarConversations().catch((err) => {
      console.error("[sidebar-conversations] refresh failed", err);
    });
  }).catch((err) => {
    console.error("[sidebar-conversations] listen tasks:changed failed", err);
  });
}

installSidebarConversationListener();

export function areSidebarConversationsLoaded(): boolean {
  return loaded.value;
}

export function listSidebarConversations(): SidebarConversationSummary[] {
  return SIDEBAR_CONVERSATIONS.value;
}

export function findSidebarConversation(
  taskId: string,
  projectId: string | null = null,
): SidebarConversationSummary | null {
  if (!taskId) return null;
  return SIDEBAR_CONVERSATIONS_BY_KEY.value.get(sidebarConversationKey(projectId, taskId)) ?? null;
}

export function getSidebarConversationRevision(): number {
  return revision;
}

export function ensureSidebarConversationsLoaded(force = false): Promise<SidebarConversationSummary[]> {
  if (!force && loaded.value) return Promise.resolve(SIDEBAR_CONVERSATIONS.value);
  if (!force && loadPromise) return loadPromise;
  loadPromise = refreshSidebarConversations().finally(() => {
    loadPromise = null;
  });
  return loadPromise;
}
