import { computed, markRaw, reactive, type Component } from "vue";

const STORAGE_KEY = "lilia.chatSidebar.open";
const WIDTH_STORAGE_KEY = "lilia.chatSidebar.width";
const MIN_WIDTH = 180;
const MAX_WIDTH = 520;
const DEFAULT_WIDTH = 340;

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
  component: Component;
}

interface RegisteredChatSidebarPanel extends ChatSidebarPanel {
  token: symbol;
}

interface ChatSidebarState {
  open: boolean;
  width: number;
  activePanelId: string | null;
  panels: RegisteredChatSidebarPanel[];
}

function clampWidth(width: number): number {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width));
}

function readStoredOpen(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function readStoredWidth(): number {
  try {
    const raw = localStorage.getItem(WIDTH_STORAGE_KEY);
    const value = raw ? Number.parseFloat(raw) : NaN;
    return Number.isFinite(value) ? clampWidth(value) : DEFAULT_WIDTH;
  } catch {
    return DEFAULT_WIDTH;
  }
}

function persistOpen(open: boolean) {
  try {
    localStorage.setItem(STORAGE_KEY, open ? "1" : "0");
  } catch {
    /* ignore quota / privacy mode errors */
  }
}

function persistWidth(width: number) {
  try {
    localStorage.setItem(WIDTH_STORAGE_KEY, String(width));
  } catch {
    /* ignore quota / privacy mode errors */
  }
}

const state = reactive<ChatSidebarState>({
  open: readStoredOpen(),
  width: readStoredWidth(),
  activePanelId: null,
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

function ensureActivePanel(panelId?: string) {
  if (panelId && state.panels.some((panel) => panel.id === panelId)) {
    state.activePanelId = panelId;
    return;
  }
  if (state.activePanelId && state.panels.some((panel) => panel.id === state.activePanelId)) {
    return;
  }
  state.activePanelId = sortedPanels()[0]?.id ?? null;
}

function setOpen(open: boolean) {
  state.open = open;
  persistOpen(open);
}

function syncFromStorage() {
  state.open = readStoredOpen();
  state.width = readStoredWidth();
}

export const chatSidebarPanels = computed(() => sortedPanels());

export const activeChatSidebarPanel = computed(() => {
  const panels = chatSidebarPanels.value;
  if (!state.activePanelId) return panels[0] ?? null;
  return panels.find((panel) => panel.id === state.activePanelId) ?? panels[0] ?? null;
});

export function registerChatSidebarPanel(panel: ChatSidebarPanel): () => void {
  const token = Symbol(panel.id);
  const registered: RegisteredChatSidebarPanel = {
    ...panel,
    token,
    icon: panel.icon ? markRaw(panel.icon) : undefined,
    component: markRaw(panel.component),
  };
  const existingIndex = state.panels.findIndex((item) => item.id === panel.id);
  if (existingIndex >= 0) {
    state.panels.splice(existingIndex, 1, registered);
  } else {
    state.panels.push(registered);
  }
  ensureActivePanel(panel.id);

  return () => {
    const index = state.panels.findIndex((item) =>
      item.id === panel.id && item.token === token
    );
    if (index < 0) return;
    const wasActive = state.activePanelId === panel.id;
    state.panels.splice(index, 1);
    if (wasActive) {
      state.activePanelId = sortedPanels()[0]?.id ?? null;
    }
  };
}

export function openChatSidebar(panelId?: string) {
  ensureActivePanel(panelId);
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
  state.activePanelId = panelId;
}

export function setChatSidebarWidth(width: number) {
  state.width = clampWidth(width);
}

export function persistChatSidebarWidth() {
  persistWidth(state.width);
}

export function resetChatSidebarWidth() {
  state.width = DEFAULT_WIDTH;
  persistWidth(DEFAULT_WIDTH);
}

export function useChatSidebar() {
  syncFromStorage();
  return {
    state,
    minWidth: MIN_WIDTH,
    maxWidth: MAX_WIDTH,
    panels: chatSidebarPanels,
    activePanel: activeChatSidebarPanel,
    open: openChatSidebar,
    close: closeChatSidebar,
    toggle: toggleChatSidebar,
    setActivePanel: setActiveChatSidebarPanel,
    setWidth: setChatSidebarWidth,
    persistWidth: persistChatSidebarWidth,
    resetWidth: resetChatSidebarWidth,
  };
}
