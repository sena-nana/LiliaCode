import {
  readScrollbarMetrics,
  scrollOffsetForThumbDrag,
} from "../utils/scrollbarMetrics";
import { addDomEventListener, runUnlistenFns } from "../utils/eventListeners";

const DEFAULT_HIDE_DELAY = 480;
const HOVER_HOT_ZONE = 12;
const TRACK_EDGE_PADDING = 4;
const NO_GLOBAL_OVERLAY_ATTRIBUTE = "data-no-global-scrollbar-overlay";
const SCROLLABLE_OVERFLOW_VALUES = new Set(["auto", "scroll", "overlay"]);

type ScrollbarVisibilityTarget = {
  key: Element;
  scroller: Element | Window;
};

type ScrollbarAxis = "vertical" | "horizontal";

type DragState = {
  axis: ScrollbarAxis;
  metrics: ReturnType<typeof readScrollbarMetrics>;
  pointerId: number | null;
  startX: number;
  startY: number;
  startScrollLeft: number;
  startScrollTop: number;
  target: ScrollbarVisibilityTarget;
};

let installed = false;
const hideTimers = new WeakMap<Element, ReturnType<typeof window.setTimeout>>();
const overlayCleanupTimers = new WeakMap<Element, ReturnType<typeof window.setTimeout>>();
const overlays = new Map<Element, { vertical: HTMLDivElement; horizontal: HTMLDivElement }>();
const overlayTargets = new WeakMap<HTMLDivElement, { axis: ScrollbarAxis; target: ScrollbarVisibilityTarget }>();
let hoverTarget: ScrollbarVisibilityTarget | null = null;
let dragState: DragState | null = null;
let globalUnlisteners: Array<() => void> = [];
let dragUnlisteners: Array<() => void> = [];

function isVertical(axis: ScrollbarAxis): boolean {
  return axis === "vertical";
}

function isDocumentScrollTarget(target: EventTarget | null): boolean {
  return target === window ||
    target === document ||
    target === document.documentElement ||
    target === document.body;
}

function resolveScrollTarget(target: EventTarget | null): ScrollbarVisibilityTarget | null {
  if (typeof document === "undefined") return null;
  if (isDocumentScrollTarget(target)) {
    return {
      key: document.documentElement,
      scroller: window,
    };
  }
  if (target instanceof Element) {
    return {
      key: target,
      scroller: target,
    };
  }
  return null;
}

function shouldSkipTarget(target: ScrollbarVisibilityTarget): boolean {
  return (
    target.scroller instanceof Element &&
    target.scroller.closest(`[${NO_GLOBAL_OVERLAY_ATTRIBUTE}]`) !== null
  );
}

function clearOverlayCleanupTimer(target: Element) {
  const timer = overlayCleanupTimers.get(target);
  if (timer === undefined) return;
  window.clearTimeout(timer);
  overlayCleanupTimers.delete(target);
}

function clearHideTimer(target: Element) {
  const timer = hideTimers.get(target);
  if (timer === undefined) return;
  window.clearTimeout(timer);
  hideTimers.delete(target);
}

function removeOverlay(target: Element) {
  const overlay = overlays.get(target);
  if (!overlay) return;
  overlayTargets.delete(overlay.vertical);
  overlayTargets.delete(overlay.horizontal);
  overlay.vertical.remove();
  overlay.horizontal.remove();
  overlays.delete(target);
}

function clearGlobalListeners() {
  runUnlistenFns(globalUnlisteners.splice(0).reverse());
}

function clearDragListeners() {
  runUnlistenFns(dragUnlisteners.splice(0).reverse());
}

function ensureOverlay(target: Element) {
  const existing = overlays.get(target);
  if (existing) return existing;
  const vertical = document.createElement("div");
  const horizontal = document.createElement("div");
  vertical.className = "global-scrollbar-overlay global-scrollbar-overlay--vertical";
  horizontal.className = "global-scrollbar-overlay global-scrollbar-overlay--horizontal";
  vertical.addEventListener("pointerdown", onOverlayPointerDown);
  horizontal.addEventListener("pointerdown", onOverlayPointerDown);
  document.body.append(vertical, horizontal);
  const overlay = { vertical, horizontal };
  overlays.set(target, overlay);
  return overlay;
}

function readMetrics(target: ScrollbarVisibilityTarget) {
  if (target.scroller === window) {
    const scroller = document.scrollingElement ?? document.documentElement;
    return {
      rect: {
        top: 0,
        right: window.innerWidth,
        bottom: window.innerHeight,
        left: 0,
        width: window.innerWidth,
        height: window.innerHeight,
      },
      scrollTop: scroller.scrollTop || window.scrollY,
      scrollLeft: scroller.scrollLeft || window.scrollX,
      scrollHeight: scroller.scrollHeight,
      scrollWidth: scroller.scrollWidth,
      clientHeight: window.innerHeight,
      clientWidth: window.innerWidth,
    };
  }
  const element = target.scroller as Element;
  const rect = element.getBoundingClientRect();
  return {
    rect,
    scrollTop: element.scrollTop,
    scrollLeft: element.scrollLeft,
    scrollHeight: element.scrollHeight,
    scrollWidth: element.scrollWidth,
    clientHeight: element.clientHeight,
    clientWidth: element.clientWidth,
  };
}

function isAxisScrollable(
  target: ScrollbarVisibilityTarget,
  metrics: ReturnType<typeof readMetrics>,
  axis: ScrollbarAxis,
): boolean {
  const overflowing = axis === "vertical"
    ? metrics.scrollHeight > metrics.clientHeight + 1
    : metrics.scrollWidth > metrics.clientWidth + 1;
  if (!overflowing) return false;
  if (target.scroller === window) return true;
  const element = target.scroller as Element;
  const style = window.getComputedStyle(element);
  const overflowValue = (axis === "vertical" ? style.overflowY : style.overflowX).toLowerCase();
  return SCROLLABLE_OVERFLOW_VALUES.has(overflowValue);
}

function isScrollable(
  target: ScrollbarVisibilityTarget,
  metrics: ReturnType<typeof readMetrics>,
): boolean {
  return isAxisScrollable(target, metrics, "vertical") ||
    isAxisScrollable(target, metrics, "horizontal");
}

function isPointerInScrollHotZone(
  target: ScrollbarVisibilityTarget,
  metrics: ReturnType<typeof readMetrics>,
  clientX: number,
  clientY: number,
): boolean {
  if (
    clientX < metrics.rect.left ||
    clientX > metrics.rect.right ||
    clientY < metrics.rect.top ||
    clientY > metrics.rect.bottom
  ) {
    return false;
  }
  const verticalHotZone = isAxisScrollable(target, metrics, "vertical") &&
    clientX >= metrics.rect.right - HOVER_HOT_ZONE;
  const horizontalHotZone = isAxisScrollable(target, metrics, "horizontal") &&
    clientY >= metrics.rect.bottom - HOVER_HOT_ZONE;
  return verticalHotZone || horizontalHotZone;
}

function findScrollableHoverTarget(event: PointerEvent): ScrollbarVisibilityTarget | null {
  const path = event.composedPath();
  for (const node of path) {
    if (!(node instanceof Element)) continue;
    const target = resolveScrollTarget(node);
    if (!target) continue;
    if (shouldSkipTarget(target)) continue;
    const metrics = readMetrics(target);
    if (
      isScrollable(target, metrics) &&
      isPointerInScrollHotZone(target, metrics, event.clientX, event.clientY)
    ) {
      return target;
    }
  }
  const documentTarget = resolveScrollTarget(document);
  if (documentTarget) {
    const metrics = readMetrics(documentTarget);
    if (
      isScrollable(documentTarget, metrics) &&
      isPointerInScrollHotZone(documentTarget, metrics, event.clientX, event.clientY)
    ) {
      return documentTarget;
    }
  }
  return null;
}

function updateOverlay(target: ScrollbarVisibilityTarget) {
  if (shouldSkipTarget(target)) return;
  const metrics = readMetrics(target);
  const verticalScrollable = isAxisScrollable(target, metrics, "vertical");
  const horizontalScrollable = isAxisScrollable(target, metrics, "horizontal");
  if (!verticalScrollable && !horizontalScrollable) {
    removeOverlay(target.key);
    return;
  }

  clearOverlayCleanupTimer(target.key);
  const overlay = ensureOverlay(target.key);
  overlayTargets.set(overlay.vertical, { axis: "vertical", target });
  overlayTargets.set(overlay.horizontal, { axis: "horizontal", target });
  const verticalThumbSize = verticalScrollable
    ? readScrollbarMetrics({
      domainSize: metrics.scrollHeight,
      minThumbSize: 24,
      scrollOffset: metrics.scrollTop,
      trackSize: Math.max(0, metrics.rect.height - TRACK_EDGE_PADDING * 2),
      visibleSize: metrics.clientHeight,
    })
    : null;

  overlay.vertical.style.top = `${metrics.rect.top + TRACK_EDGE_PADDING + (verticalThumbSize?.thumbOffset ?? 0)}px`;
  overlay.vertical.style.right = `${Math.max(0, window.innerWidth - metrics.rect.right)}px`;
  overlay.vertical.style.height = `${verticalThumbSize?.thumbSize ?? 0}px`;
  overlay.vertical.classList.toggle("is-visible", verticalScrollable);

  const horizontalThumbSize = horizontalScrollable
    ? readScrollbarMetrics({
      domainSize: metrics.scrollWidth,
      minThumbSize: 24,
      scrollOffset: metrics.scrollLeft,
      trackSize: Math.max(0, metrics.rect.width - TRACK_EDGE_PADDING * 2),
      visibleSize: metrics.clientWidth,
    })
    : null;

  overlay.horizontal.style.left = `${metrics.rect.left + TRACK_EDGE_PADDING + (horizontalThumbSize?.thumbOffset ?? 0)}px`;
  overlay.horizontal.style.bottom = `${Math.max(0, window.innerHeight - metrics.rect.bottom)}px`;
  overlay.horizontal.style.width = `${horizontalThumbSize?.thumbSize ?? 0}px`;
  overlay.horizontal.classList.toggle("is-visible", horizontalScrollable);
}

function readAxisMetrics(target: ScrollbarVisibilityTarget, axis: ScrollbarAxis) {
  const metrics = readMetrics(target);
  const vertical = isVertical(axis);
  return {
    metrics,
    scrollbar: readScrollbarMetrics({
      domainSize: vertical ? metrics.scrollHeight : metrics.scrollWidth,
      minThumbSize: 24,
      scrollOffset: vertical ? metrics.scrollTop : metrics.scrollLeft,
      trackSize: Math.max(0, (vertical ? metrics.rect.height : metrics.rect.width) - TRACK_EDGE_PADDING * 2),
      visibleSize: vertical ? metrics.clientHeight : metrics.clientWidth,
    }),
  };
}

function hideOverlay(target: Element) {
  const overlay = overlays.get(target);
  if (!overlay) return;
  overlay.vertical.classList.remove("is-visible");
  overlay.horizontal.classList.remove("is-visible");
  clearOverlayCleanupTimer(target);
  overlayCleanupTimers.set(
    target,
    window.setTimeout(() => {
      removeOverlay(target);
      overlayCleanupTimers.delete(target);
    }, DEFAULT_HIDE_DELAY),
  );
}

function hideSoon(target: ScrollbarVisibilityTarget) {
  clearHideTimer(target.key);
  hideTimers.set(
    target.key,
    window.setTimeout(() => {
      hideOverlay(target.key);
      hideTimers.delete(target.key);
    }, DEFAULT_HIDE_DELAY),
  );
}

function show(target: ScrollbarVisibilityTarget) {
  clearHideTimer(target.key);
  updateOverlay(target);
}

function setScrollPosition(target: ScrollbarVisibilityTarget, axis: ScrollbarAxis, value: number) {
  const metrics = readMetrics(target);
  if (target.scroller === window) {
    const nextLeft = axis === "horizontal" ? value : metrics.scrollLeft;
    const nextTop = axis === "vertical" ? value : metrics.scrollTop;
    window.scrollTo(nextLeft, nextTop);
    return;
  }
  const element = target.scroller as Element;
  if (axis === "vertical") {
    element.scrollTop = value;
  } else {
    element.scrollLeft = value;
  }
}

function onOverlayPointerDown(event: PointerEvent) {
  const overlayTarget = overlayTargets.get(event.currentTarget as HTMLDivElement);
  if (!overlayTarget) return;
  const { metrics, scrollbar } = readAxisMetrics(overlayTarget.target, overlayTarget.axis);
  event.preventDefault();
  event.stopPropagation();
  show(overlayTarget.target);
  clearDragListeners();
  dragState = {
    axis: overlayTarget.axis,
    metrics: scrollbar,
    pointerId: Number.isFinite(event.pointerId) ? event.pointerId : null,
    startX: event.clientX,
    startY: event.clientY,
    startScrollLeft: metrics.scrollLeft,
    startScrollTop: metrics.scrollTop,
    target: overlayTarget.target,
  };
  dragUnlisteners = [
    addDomEventListener(window, "pointerup", onDragPointerEnd, true),
    addDomEventListener(window, "pointercancel", onDragPointerEnd, true),
  ];
}

function onDragPointerMove(event: PointerEvent) {
  if (!dragState || (dragState.pointerId !== null && event.pointerId !== dragState.pointerId)) return;
  const vertical = isVertical(dragState.axis);
  const delta = vertical
    ? event.clientY - dragState.startY
    : event.clientX - dragState.startX;
  const startScroll = vertical
    ? dragState.startScrollTop
    : dragState.startScrollLeft;
  const nextScroll = scrollOffsetForThumbDrag(startScroll, delta, dragState.metrics);
  event.preventDefault();
  setScrollPosition(dragState.target, dragState.axis, nextScroll);
  show(dragState.target);
}

function onDragPointerEnd(event: PointerEvent) {
  if (!dragState || (dragState.pointerId !== null && event.pointerId !== dragState.pointerId)) return;
  const target = dragState.target;
  dragState = null;
  hideSoon(target);
  clearDragListeners();
}

function onScroll(event: Event) {
  const target = resolveScrollTarget(event.target);
  if (!target || shouldSkipTarget(target)) return;
  show(target);
  hideSoon(target);
}

function onScrollEnd(event: Event) {
  const target = resolveScrollTarget(event.target);
  if (!target || shouldSkipTarget(target)) return;
  hideSoon(target);
}

function onPointerMove(event: PointerEvent) {
  if (dragState) {
    onDragPointerMove(event);
    return;
  }
  const target = findScrollableHoverTarget(event);
  if (target) {
    if (hoverTarget?.key && hoverTarget.key !== target.key) {
      hideSoon(hoverTarget);
    }
    hoverTarget = target;
    show(target);
    return;
  }
  if (hoverTarget) {
    hideSoon(hoverTarget);
    hoverTarget = null;
  }
}

function onPointerLeave() {
  if (!hoverTarget) return;
  hideSoon(hoverTarget);
  hoverTarget = null;
}

export function uninstallGlobalScrollbarVisibility() {
  if (!installed || typeof window === "undefined") return;
  installed = false;
  clearGlobalListeners();
  clearDragListeners();
  hoverTarget = null;
  dragState = null;
  overlays.forEach((_overlay, target) => {
    clearHideTimer(target);
    clearOverlayCleanupTimer(target);
    removeOverlay(target);
  });
}

export function installGlobalScrollbarVisibility() {
  if (installed || typeof window === "undefined") return uninstallGlobalScrollbarVisibility;
  installed = true;
  globalUnlisteners = [
    addDomEventListener(window, "scroll", onScroll, { capture: true, passive: true }),
    addDomEventListener(window, "scrollend", onScrollEnd, { capture: true }),
    addDomEventListener(window, "pointermove", onPointerMove, { capture: true }),
    addDomEventListener(window, "pointerleave", onPointerLeave),
  ];
  return uninstallGlobalScrollbarVisibility;
}

