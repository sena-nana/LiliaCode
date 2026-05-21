import { ref, watch } from "vue";

export type Theme = "dark" | "light";

const STORAGE_KEY = "lilia.theme";
const DEFAULT_THEME: Theme = "dark";

function loadInitial(): Theme {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "light" || stored === "dark") return stored;
  } catch {
    /* localStorage 不可用：落到默认值。 */
  }
  return DEFAULT_THEME;
}

function apply(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
}

// 模块级单例：所有 useTheme() 调用共享同一个 ref。
const theme = ref<Theme>(loadInitial());
apply(theme.value);

watch(theme, (next) => {
  apply(next);
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // ignore
  }
});

export function useTheme() {
  return {
    theme,
    setTheme(next: Theme) {
      theme.value = next;
    },
    toggleTheme() {
      theme.value = theme.value === "dark" ? "light" : "dark";
    },
  };
}
