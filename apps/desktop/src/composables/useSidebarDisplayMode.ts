import { ref, watch } from "vue";

export type SidebarDisplayMode = "grouped" | "unified";

const STORAGE_KEY = "lilia.sidebarDisplayMode";
const DEFAULT_MODE: SidebarDisplayMode = "grouped";

function normalizeSidebarDisplayMode(value: unknown): SidebarDisplayMode {
  return value === "unified" ? "unified" : DEFAULT_MODE;
}

function loadInitial(): SidebarDisplayMode {
  try {
    return normalizeSidebarDisplayMode(localStorage.getItem(STORAGE_KEY));
  } catch {
    return DEFAULT_MODE;
  }
}

const sidebarDisplayMode = ref<SidebarDisplayMode>(loadInitial());

watch(sidebarDisplayMode, (next) => {
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {}
});

export function useSidebarDisplayMode() {
  return {
    sidebarDisplayMode,
    setSidebarDisplayMode(next: SidebarDisplayMode) {
      sidebarDisplayMode.value = normalizeSidebarDisplayMode(next);
    },
  };
}
