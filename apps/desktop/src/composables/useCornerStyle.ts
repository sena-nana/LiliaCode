import { ref, watch } from "vue";

export type CornerStyle = "smooth" | "round";

const STYLE_STORAGE_KEY = "lilia.corners";
const RADIUS_STORAGE_KEY = "lilia.cornerRadius";
const DEFAULT_STYLE: CornerStyle = "smooth";
export const CORNER_RADIUS_MIN = 0;
export const CORNER_RADIUS_MAX = 20;
export const DEFAULT_CORNER_RADIUS = 8;

function normalizeStyle(value: unknown): CornerStyle {
  return value === "round" ? "round" : DEFAULT_STYLE;
}

function clampRadius(value: number): number {
  return Math.min(CORNER_RADIUS_MAX, Math.max(CORNER_RADIUS_MIN, value));
}

function loadInitialStyle(): CornerStyle {
  try {
    return normalizeStyle(localStorage.getItem(STYLE_STORAGE_KEY));
  } catch {
    return DEFAULT_STYLE;
  }
}

function loadInitialRadius(): number {
  try {
    const parsed = Number.parseFloat(localStorage.getItem(RADIUS_STORAGE_KEY) ?? "");
    if (Number.isFinite(parsed)) return clampRadius(parsed);
  } catch {}
  return DEFAULT_CORNER_RADIUS;
}

function apply(style: CornerStyle, radius: number): void {
  const normalizedRadius = clampRadius(radius);
  document.documentElement.dataset.corners = style;
  document.documentElement.style.setProperty("--app-corner-radius", `${normalizedRadius}px`);
  try {
    localStorage.setItem(STYLE_STORAGE_KEY, style);
    localStorage.setItem(RADIUS_STORAGE_KEY, String(normalizedRadius));
  } catch {}
}

const cornerStyle = ref<CornerStyle>(loadInitialStyle());
const cornerRadius = ref(loadInitialRadius());

watch([cornerStyle, cornerRadius], ([style, radius]) => apply(style, radius), {
  flush: "sync",
  immediate: true,
});

export function useCornerStyle() {
  apply(cornerStyle.value, cornerRadius.value);

  return {
    cornerStyle,
    cornerRadius,
    setCornerStyle(next: CornerStyle) {
      cornerStyle.value = normalizeStyle(next);
    },
    setCornerRadius(next: number) {
      cornerRadius.value = clampRadius(next);
    },
  };
}

