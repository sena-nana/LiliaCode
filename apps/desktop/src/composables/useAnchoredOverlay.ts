import {
  computed,
  nextTick,
  onBeforeUnmount,
  ref,
  watch,
  type ComputedRef,
  type Ref,
} from "vue";
import {
  createAnchoredRect,
  rectFromDomRect,
  resolveAnchoredOverlayLayout,
  resolveMenuTransformOrigin,
  type AnchoredMenuPlacement,
  type AnchoredRect,
} from "./menuMotion";
import { addDomEventListener, runUnlistenFns } from "../utils/eventListeners";

export interface OverlayAnchorPoint {
  x: number;
  y: number;
}

type MaybeAnchorRef = Ref<HTMLElement | null | undefined> | ComputedRef<HTMLElement | null | undefined>;

function pointRect(point: OverlayAnchorPoint): AnchoredRect {
  return createAnchoredRect(point.x, point.y, 0, 0);
}

export function useAnchoredOverlay(options: {
  open: Ref<boolean>;
  anchorEl?: MaybeAnchorRef;
  preferredPlacement: Ref<AnchoredMenuPlacement> | ComputedRef<AnchoredMenuPlacement>;
  offset?: number;
  matchAnchorWidth?: boolean;
}) {
  const overlayEl = ref<HTMLElement | null>(null);
  const overlayStyle = ref<Record<string, string>>({});
  const resolvedPlacement = ref<AnchoredMenuPlacement>(options.preferredPlacement.value);
  const anchorPoint = ref<OverlayAnchorPoint | null>(null);
  let viewportUnlisteners: Array<() => void> = [];
  let positionSeq = 0;
  let disposed = false;

  const activeAnchorRect = computed<AnchoredRect | null>(() => {
    if (options.anchorEl?.value) return rectFromDomRect(options.anchorEl.value.getBoundingClientRect());
    if (anchorPoint.value) return pointRect(anchorPoint.value);
    return null;
  });

  function setAnchorPoint(point: OverlayAnchorPoint | null) {
    positionSeq += 1;
    anchorPoint.value = point;
  }

  function containsTarget(target: EventTarget | null): boolean {
    const node = target instanceof Node ? target : null;
    if (!node) return false;
    return Boolean(
      overlayEl.value?.contains(node) ||
      options.anchorEl?.value?.contains(node),
    );
  }

  async function updatePosition() {
    if (disposed || !options.open.value) return;
    const seq = ++positionSeq;
    await nextTick();
    if (disposed || seq !== positionSeq || !options.open.value) return;
    const anchorRect = activeAnchorRect.value;
    const overlay = overlayEl.value;
    if (!anchorRect || !overlay) return;
    const overlayRect = overlay.getBoundingClientRect();
    const overlayWidth = options.matchAnchorWidth
      ? Math.max(anchorRect.width, 0)
      : (overlay.offsetWidth || overlayRect.width || 0);
    const overlayHeight = overlay.offsetHeight || overlayRect.height || 0;
    const layout = resolveAnchoredOverlayLayout({
      anchorRect,
      overlayWidth,
      overlayHeight,
      preferredPlacement: options.preferredPlacement.value,
      anchorPoint: anchorPoint.value,
      offset: options.offset,
    });
    const origin = resolveMenuTransformOrigin(layout, overlayWidth, overlayHeight);
    resolvedPlacement.value = `${layout.placement}-${layout.alignment}` as AnchoredMenuPlacement;
    overlayStyle.value = {
      left: `${layout.x}px`,
      top: `${layout.y}px`,
      "--sb-menu-origin-x": `${origin.x}px`,
      "--sb-menu-origin-y": `${origin.y}px`,
      ...(options.matchAnchorWidth ? { width: `${anchorRect.width}px` } : {}),
    };
  }

  function onViewportChange() {
    void updatePosition();
  }

  function clearViewportListeners() {
    runUnlistenFns(viewportUnlisteners.splice(0).reverse());
  }

  function installViewportListeners() {
    clearViewportListeners();
    viewportUnlisteners = [
      addDomEventListener(window, "resize", onViewportChange),
      addDomEventListener(window, "scroll", onViewportChange, true),
    ];
  }

  watch(
    () => [
      options.open.value,
      options.preferredPlacement.value,
      options.anchorEl?.value,
      anchorPoint.value?.x ?? -1,
      anchorPoint.value?.y ?? -1,
    ] as const,
    ([open]) => {
      if (!open) {
        positionSeq += 1;
        overlayStyle.value = {};
        return;
      }
      void updatePosition();
    },
    { immediate: true },
  );

  watch(
    () => options.open.value,
    (open) => {
      if (open) {
        installViewportListeners();
      } else {
        clearViewportListeners();
      }
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    disposed = true;
    positionSeq += 1;
    clearViewportListeners();
  });

  return {
    overlayEl,
    overlayStyle,
    resolvedPlacement,
    containsTarget,
    setAnchorPoint,
    updatePosition,
  };
}

