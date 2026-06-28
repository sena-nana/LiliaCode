import { computed, markRaw, reactive, type Component } from "vue";

const STORAGE_KEY = "lilia.chatSidebar.open";
const WIDTH_STORAGE_KEY = "lilia.chatSidebar.width";

export const CHAT_SIDEBAR_WIDTH_STORAGE_KEY = WIDTH_STORAGE_KEY;
export const CHAT_SIDEBAR_MIN_WIDTH = 180;
export const CHAT_SIDEBAR_MAX_WIDTH = 520;
export const CHAT_SIDEBAR_DEFAULT_WIDTH = 340;

export interface ChatSidebarContext {
  taskId: string;
  projectId?: string;
  projectCwd: string | null;
}

export interface ChatSidebarPanel {
  id: string;
  title: string;
  icon?: Component;
  order?: number;
  loader?: () => Promise<Component>;
  component?: Component;
}

interface RegisteredChatSidebarPanel extends ChatSidebarPanel {
  loader: () => Promise<Component>;
  token: symbol;
}

interface ChatSidebarState {
  open: boolean;
  activePanelId: string | null;
  pendingPanelId: string | null;
  panels: RegisteredChatSidebarPanel[];
}

function readStoredOpen(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function persistOpen(open: boolean) {
  try {
    localStorage.setItem(STORAGE_KEY, open ? "1" : "0");
  } catch {
    /* ignore quota / privacy mode errors */
  }
}

const state = reactive<ChatSidebarState>({
  open: readStoredOpen(),
  activePanelId: null,
  pendingPanelId: null,
  panels: [],
});

function comparePanels(
  a: RegisteredChatSidebarPanel,
  b: RegisteredChatSidebarPanel,
): number {
  return (a.order ?? 0) - (b.order ?? 0) ||
    a.title.localeCompare(b.title) ||
    a.id.localeCompare(b.id);
}

function sortedPanels(): RegisteredChatSidebarPanel[] {
  return [...state.panels].sort(comparePanels);
}

function reconcileActivePanel() {
  if (state.pendingPanelId && state.panels.some((panel) => panel.id === state.pendingPanelId)) {
    state.activePanelId = state.pendingPanelId;
    state.pendingPanelId = null;
    return;
  }
  if (state.activePanelId && state.panels.some((panel) => panel.id === state.activePanelId)) {
    return;
  }
  state.activePanelId = sortedPanels()[0]?.id ?? null;
}

function requestActivePanel(panelId?: string) {
  if (panelId) {
    state.pendingPanelId = panelId;
  }
  reconcileActivePanel();
}

function setOpen(open: boolean) {
  state.open = open;
  persistOpen(open);
}

function syncFromStorage() {
  state.open = readStoredOpen();
}

export const chatSidebarPanels = computed(() => sortedPanels());

export const activeChatSidebarPanel = computed(() => {
  const panels = chatSidebarPanels.value;
  if (!state.activePanelId) return panels[0] ?? null;
  return panels.find((panel) => panel.id === state.activePanelId) ?? panels[0] ?? null;
});

function normalizePanelLoader(panel: ChatSidebarPanel): () => Promise<Component> {
  if (panel.loader) {
    return panel.loader;
  }
  if (panel.component) {
    const component = markRaw(panel.component);
    return () => Promise.resolve(component);
  }
  throw new Error(`chat sidebar panel "${panel.id}" 缺少 loader`);
}

export function registerChatSidebarPanel(panel: ChatSidebarPanel): () => void {
  const token = Symbol(panel.id);
  const registered: RegisteredChatSidebarPanel = {
    ...panel,
    token,
    icon: panel.icon ? markRaw(panel.icon) : undefined,
    component: panel.component ? markRaw(panel.component) : undefined,
    loader: normalizePanelLoader(panel),
  };
  const existingIndex = state.panels.findIndex((item) => item.id === panel.id);
  if (existingIndex >= 0) {
    state.panels.splice(existingIndex, 1, registered);
  } else {
    state.panels.push(registered);
  }
  reconcileActivePanel();

  return () => {
    const index = state.panels.findIndex((item) =>
      item.id === panel.id && item.token === token
    );
    if (index < 0) return;
    const wasActive = state.activePanelId === panel.id;
    state.panels.splice(index, 1);
    if (wasActive) {
      state.activePanelId = null;
    }
    if (state.pendingPanelId === panel.id) {
      state.pendingPanelId = null;
    }
    reconcileActivePanel();
  };
}

export function openChatSidebar(panelId?: string) {
  requestActivePanel(panelId);
  setOpen(true);
}

export function closeChatSidebar() {
  setOpen(false);
}

export function toggleChatSidebar(panelId?: string) {
  if (state.open && (!panelId || panelId === state.activePanelId)) {
    closeChatSidebar();
    return;
  }
  openChatSidebar(panelId);
}

export function setActiveChatSidebarPanel(panelId: string) {
  if (!state.panels.some((panel) => panel.id === panelId)) return;
  state.pendingPanelId = null;
  state.activePanelId = panelId;
}

export function useChatSidebar() {
  syncFromStorage();
  return {
    state,
    minWidth: CHAT_SIDEBAR_MIN_WIDTH,
    maxWidth: CHAT_SIDEBAR_MAX_WIDTH,
    panels: chatSidebarPanels,
    activePanel: activeChatSidebarPanel,
    open: openChatSidebar,
    close: closeChatSidebar,
    toggle: toggleChatSidebar,
    setActivePanel: setActiveChatSidebarPanel,
  };
}

