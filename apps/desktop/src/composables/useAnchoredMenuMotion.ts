import { nextTick, ref, type Ref } from "vue";
import { resolveMenuTransformOrigin, type MenuPlacement, type MenuTransformOrigin } from "./menuMotion";

interface AnchorPoint {
  x: number;
  y: number;
}

function resolveAnchorFromEvent(
  event: MouseEvent,
  triggerRect: DOMRect,
  placement: MenuPlacement,
): AnchorPoint {
  return {
    x: Number.isFinite(event.clientX) && event.clientX > 0
      ? event.clientX
      : triggerRect.left + triggerRect.width / 2,
    y: Number.isFinite(event.clientY) && event.clientY > 0
      ? event.clientY
      : (placement === "bottom" ? triggerRect.bottom : triggerRect.top),
  };
}

function resolveMenuSize(menuEl: HTMLElement, menuRect: DOMRect) {
  return {
    width: menuEl.offsetWidth || menuRect.width || Number.POSITIVE_INFINITY,
    height: menuEl.offsetHeight || menuRect.height || Number.POSITIVE_INFINITY,
  };
}

export function useAnchoredMenuMotion(
  placement: Ref<MenuPlacement>,
) {
  const rootEl = ref<HTMLElement | null>(null);
  const triggerEl = ref<HTMLElement | null>(null);
  const menuEl = ref<HTMLElement | null>(null);
  const origin = ref<MenuTransformOrigin>({ x: 0, y: 0 });
  const anchor = ref<AnchorPoint | null>(null);

  function captureAnchor(event: MouseEvent) {
    const trigger = triggerEl.value ?? rootEl.value;
    if (!trigger) return;
    anchor.value = resolveAnchorFromEvent(event, trigger.getBoundingClientRect(), placement.value);
  }

  function resolveInitialOrigin() {
    const root = rootEl.value;
    if (!root) return { x: 0, y: 0 };
    const rootRect = root.getBoundingClientRect();
    const anchorX = anchor.value?.x ?? (rootRect.left + rootRect.width / 2);
    origin.value = {
      x: Math.max(0, anchorX - rootRect.left),
      y: placement.value === "bottom" ? 0 : Number.POSITIVE_INFINITY,
    };
  }

  async function updateOrigin() {
    const root = rootEl.value;
    const trigger = triggerEl.value ?? root;
    if (!root || !trigger) return;
    await nextTick();
    const menu = menuEl.value;
    if (!menu) return;
    const triggerRect = trigger.getBoundingClientRect();
    const menuRect = menu.getBoundingClientRect();
    const { width, height } = resolveMenuSize(menu, menuRect);
    origin.value = resolveMenuTransformOrigin(
      {
        x: menuRect.left || root.getBoundingClientRect().left,
        y: menuRect.top,
        anchorX: anchor.value?.x ?? (triggerRect.left + triggerRect.width / 2),
        anchorY: anchor.value?.y ?? (placement.value === "bottom" ? triggerRect.bottom : triggerRect.top),
      },
      width,
      height,
    );
  }

  return {
    rootEl,
    triggerEl,
    menuEl,
    origin,
    captureAnchor,
    resolveInitialOrigin,
    updateOrigin,
  };
}
