import { onBeforeUnmount, ref, type Ref } from "vue";

type ResizeEdge = "left" | "right";

export interface ResizablePaneOptions {
  storageKey: string;
  minWidth: number;
  maxWidth: number;
  defaultWidth: number;
  edge: ResizeEdge;
  disabled?: Ref<boolean>;
}

function readStoredWidth(
  storageKey: string,
  defaultWidth: number,
  clamp: (width: number) => number,
): number {
  try {
    const raw = localStorage.getItem(storageKey);
    const value = raw ? Number.parseFloat(raw) : NaN;
    return Number.isFinite(value) ? clamp(value) : defaultWidth;
  } catch {
    return defaultWidth;
  }
}

function writeStoredWidth(storageKey: string, width: number) {
  try {
    localStorage.setItem(storageKey, String(width));
  } catch {
    /* ignore quota / privacy mode errors */
  }
}

export function useResizablePane(options: ResizablePaneOptions) {
  const clampWidth = (width: number) =>
    Math.min(options.maxWidth, Math.max(options.minWidth, width));

  const width = ref(readStoredWidth(
    options.storageKey,
    options.defaultWidth,
    clampWidth,
  ));
  const isResizing = ref(false);

  let startX = 0;
  let startWidth = 0;

  function setWidth(nextWidth: number) {
    width.value = clampWidth(nextWidth);
  }

  function persistWidth() {
    writeStoredWidth(options.storageKey, width.value);
  }

  function resetWidth() {
    width.value = options.defaultWidth;
    writeStoredWidth(options.storageKey, options.defaultWidth);
  }

  function syncWidthFromStorage() {
    width.value = readStoredWidth(
      options.storageKey,
      options.defaultWidth,
      clampWidth,
    );
  }

  function onPointerMove(event: PointerEvent) {
    const delta = options.edge === "left"
      ? startX - event.clientX
      : event.clientX - startX;
    setWidth(startWidth + delta);
  }

  function onPointerUp(event: PointerEvent) {
    isResizing.value = false;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    (event.target as Element | null)?.releasePointerCapture?.(event.pointerId);
    persistWidth();
  }

  function startResize(event: PointerEvent) {
    if (options.disabled?.value || event.button !== 0) return;
    event.preventDefault();
    isResizing.value = true;
    startX = event.clientX;
    startWidth = width.value;
    (event.currentTarget as Element).setPointerCapture?.(event.pointerId);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  }

  onBeforeUnmount(() => {
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  });

  return {
    width,
    isResizing,
    minWidth: options.minWidth,
    maxWidth: options.maxWidth,
    setWidth,
    persistWidth,
    resetWidth,
    syncWidthFromStorage,
    startResize,
  };
}
