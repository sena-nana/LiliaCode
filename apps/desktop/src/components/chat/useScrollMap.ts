import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { CSSProperties, ComputedRef, Ref } from "vue";
import {
  clamp as clampValue,
  maxScrollOffset,
  readScrollbarMetrics,
  scrollOffsetForThumbDrag,
} from "../../utils/scrollbarMetrics";
import { addDomEventListener, runUnlistenFns } from "@lilia/ui";

export { clamp } from "../../utils/scrollbarMetrics";

export interface ScrollMapGeometry {
  bottomOffset?: number;
  domainHeight: number;
  visibleHeight: number;
}

export interface ScrollMapMetrics {
  bottomOffset: number;
  domainHeight: number;
  scrollable: boolean;
  thumbHeight: number;
  thumbTop: number;
  trackHeight: number;
  visibleHeight: number;
}

export interface ScrollMapController {
  isDragging: Ref<boolean>;
  metrics: Ref<ScrollMapMetrics>;
  scheduleMeasure: () => void;
  scrollTo: (top: number, behavior: ScrollBehavior) => void;
  setTrackElement: (element: unknown) => void;
  shouldRender: ComputedRef<boolean>;
  thumbStyle: ComputedRef<CSSProperties>;
  onThumbPointerDown: (event: PointerEvent) => void;
  onTrackPointerDown: (event: PointerEvent) => void;
}

interface ScrollDragState {
  pointerId: number;
  startScrollTop: number;
  startY: number;
  metrics: ScrollMapMetrics;
}

interface UseScrollMapOptions {
  enabled?: Ref<boolean> | ComputedRef<boolean>;
  observeTargets?: (scroller: HTMLElement) => HTMLElement[];
  readGeometry?: (scroller: HTMLElement) => ScrollMapGeometry;
  scheduleMode?: "frame" | "tick";
  scroller: Ref<HTMLElement | null> | ComputedRef<HTMLElement | null>;
  trackEdgePadding?: number;
}

interface UseScrollMapVisibilityOptions {
  enabled?: Ref<boolean> | ComputedRef<boolean>;
  hideDelay?: number;
  hotZone?: number;
  hoverTarget?: Ref<HTMLElement | null> | ComputedRef<HTMLElement | null>;
  isDragging?: Ref<boolean> | ComputedRef<boolean>;
  scroller: Ref<HTMLElement | null> | ComputedRef<HTMLElement | null>;
}

const DEFAULT_HIDE_DELAY = 180;
const DEFAULT_HOT_ZONE = 18;
const DEFAULT_TRACK_EDGE_PADDING = 8;

const EMPTY_METRICS: ScrollMapMetrics = {
  bottomOffset: 0,
  domainHeight: 0,
  scrollable: false,
  thumbHeight: 0,
  thumbTop: 0,
  trackHeight: 0,
  visibleHeight: 0,
};

export function useScrollMap(options: UseScrollMapOptions): ScrollMapController {
  const track = ref<HTMLElement | null>(null);
  const metrics = ref<ScrollMapMetrics>({ ...EMPTY_METRICS });
  const isDragging = ref(false);
  const observedTargets = new Set<HTMLElement>();

  let frameId: number | null = null;
  let measurePending = false;
  let resizeObserver: ResizeObserver | null = null;
  let dragState: ScrollDragState | null = null;
  let scrollerUnlisten: (() => void) | null = null;
  let dragUnlisteners: Array<() => void> = [];
  let measureSeq = 0;
  let disposed = false;

  const thumbStyle = computed<CSSProperties>(() => ({
    transform: `translateY(${metrics.value.thumbTop}px)`,
    height: `${metrics.value.thumbHeight}px`,
  }));
  const shouldRender = computed(() =>
    metrics.value.scrollable && metrics.value.thumbHeight > 0,
  );

  watch(
    () => [options.scroller.value, isEnabled()] as const,
    (nextState) => {
      const [next, enabled] = nextState;
      clearScrollerListener();
      disconnectResizeObserver();

      if (next && enabled) {
        scrollerUnlisten = addDomEventListener(next, "scroll", scheduleMeasure, { passive: true });
        if (typeof ResizeObserver !== "undefined") {
          resizeObserver = new ResizeObserver(scheduleMeasure);
          resizeObserver.observe(next);
          refreshResizeTargets(next);
        }
        scheduleMeasure();
      } else {
        resetMeasurements();
      }
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    disposed = true;
    measureSeq += 1;
    measurePending = false;
    if (frameId !== null) window.cancelAnimationFrame(frameId);
    frameId = null;
    clearScrollerListener();
    clearDragListeners();
    disconnectResizeObserver();
  });

  function isEnabled(): boolean {
    return options.enabled?.value ?? true;
  }

  function disconnectResizeObserver() {
    resizeObserver?.disconnect();
    resizeObserver = null;
    observedTargets.clear();
  }

  function clearScrollerListener() {
    scrollerUnlisten?.();
    scrollerUnlisten = null;
  }

  function refreshResizeTargets(scroller: HTMLElement) {
    if (!resizeObserver || !options.observeTargets) return;

    const nextTargets = new Set(options.observeTargets(scroller));
    for (const target of observedTargets) {
      if (!nextTargets.has(target)) resizeObserver.unobserve(target);
    }
    for (const target of nextTargets) {
      if (!observedTargets.has(target)) resizeObserver.observe(target);
    }
    observedTargets.clear();
    for (const target of nextTargets) observedTargets.add(target);
  }

  function scheduleMeasure() {
    if (disposed || measurePending) return;
    measurePending = true;
    const seq = ++measureSeq;
    if (options.scheduleMode === "frame") {
      frameId = window.requestAnimationFrame(() => {
        frameId = null;
        void nextTick(() => runPendingMeasure(seq));
      });
      return;
    }

    void nextTick(() => runPendingMeasure(seq));
  }

  function runPendingMeasure(seq: number) {
    if (disposed || seq !== measureSeq) return;
    measurePending = false;
    measure();
  }

  function measure() {
    if (disposed) return;
    const scroller = options.scroller.value;
    if (!scroller || !isEnabled()) {
      resetMeasurements();
      return;
    }

    refreshResizeTargets(scroller);
    const geometry = options.readGeometry?.(scroller) ?? readDefaultGeometry(scroller);
    const visibleHeight = Math.max(0, geometry.visibleHeight);
    const domainHeight = Math.max(visibleHeight, geometry.domainHeight);
    const trackHeight = readTrackHeight(visibleHeight);
    const scrollMetrics = readScrollbarMetrics({
      domainSize: domainHeight,
      scrollOffset: scroller.scrollTop,
      trackSize: trackHeight,
      visibleSize: visibleHeight,
    });

    metrics.value = {
      bottomOffset: geometry.bottomOffset ?? 0,
      domainHeight,
      scrollable: scrollMetrics.scrollable,
      thumbHeight: scrollMetrics.thumbSize,
      thumbTop: scrollMetrics.thumbOffset,
      trackHeight: scrollMetrics.trackSize,
      visibleHeight,
    };
  }

  function resetMeasurements() {
    metrics.value = { ...EMPTY_METRICS };
    isDragging.value = false;
  }

  function readTrackHeight(visibleHeight: number): number {
    const rectHeight = track.value?.getBoundingClientRect().height ?? 0;
    if (rectHeight > 0) return rectHeight;
    return Math.max(0, visibleHeight - (options.trackEdgePadding ?? DEFAULT_TRACK_EDGE_PADDING) * 2);
  }

  function setTrackElement(element: unknown) {
    if (disposed) return;
    const next = element instanceof HTMLElement ? element : null;
    if (track.value === next) return;
    track.value = next;
    scheduleMeasure();
  }

  function scrollTo(top: number, behavior: ScrollBehavior) {
    if (disposed) return;
    const scroller = options.scroller.value;
    if (!scroller) return;

    const nextTop = clampValue(top, 0, maxScrollTop(scroller));
    if (typeof scroller.scrollTo === "function") {
      scroller.scrollTo({ top: nextTop, behavior });
    } else {
      scroller.scrollTop = nextTop;
    }
    scheduleMeasure();
  }

  function onTrackPointerDown(event: PointerEvent) {
    if (disposed) return;
    const scroller = options.scroller.value;
    const trackElement = track.value;
    if (!scroller || !trackElement) return;

    const currentMetrics = metrics.value;
    if (!currentMetrics.scrollable) return;

    const trackRect = trackElement.getBoundingClientRect();
    const y = event.clientY - trackRect.top;
    const thumbBottom = currentMetrics.thumbTop + currentMetrics.thumbHeight;
    if (y >= currentMetrics.thumbTop && y <= thumbBottom) return;

    event.preventDefault();
    scrollTo(
      scroller.scrollTop + (y < currentMetrics.thumbTop
        ? -currentMetrics.visibleHeight
        : currentMetrics.visibleHeight),
      "auto",
    );
  }

  function onThumbPointerDown(event: PointerEvent) {
    if (disposed) return;
    const scroller = options.scroller.value;
    const currentMetrics = metrics.value;
    if (!scroller || !currentMetrics.scrollable || currentMetrics.trackHeight <= 0) return;

    event.preventDefault();
    event.stopPropagation();
    clearDragListeners();
    dragState = {
      pointerId: event.pointerId,
      startScrollTop: scroller.scrollTop,
      startY: event.clientY,
      metrics: currentMetrics,
    };
    isDragging.value = true;
    dragUnlisteners = [
      addDomEventListener(window, "pointermove", onWindowPointerMove),
      addDomEventListener(window, "pointerup", onWindowPointerEnd),
      addDomEventListener(window, "pointercancel", onWindowPointerEnd),
    ];
  }

  function onWindowPointerMove(event: PointerEvent) {
    if (disposed) return;
    const scroller = options.scroller.value;
    if (!scroller || !dragState || event.pointerId !== dragState.pointerId) return;

    event.preventDefault();
    const deltaY = event.clientY - dragState.startY;
    scroller.scrollTop = scrollOffsetForThumbDrag(dragState.startScrollTop, deltaY, {
      domainSize: dragState.metrics.domainHeight,
      thumbSize: dragState.metrics.thumbHeight,
      trackSize: dragState.metrics.trackHeight,
      visibleSize: dragState.metrics.visibleHeight,
    });
    scheduleMeasure();
  }

  function onWindowPointerEnd(event: PointerEvent) {
    if (dragState && event.pointerId !== dragState.pointerId) return;
    clearDragListeners();
  }

  function clearDragListeners() {
    dragState = null;
    isDragging.value = false;
    runUnlistenFns(dragUnlisteners.splice(0).reverse());
  }

  return {
    isDragging,
    metrics,
    scheduleMeasure,
    scrollTo,
    setTrackElement,
    shouldRender,
    thumbStyle,
    onThumbPointerDown,
    onTrackPointerDown,
  };
}

export function useScrollMapVisibility(options: UseScrollMapVisibilityOptions) {
  const isVisible = ref(false);
  const isPointerInZone = ref(false);
  let hideTimer: ReturnType<typeof window.setTimeout> | null = null;
  let scrollerUnlisteners: Array<() => void> = [];
  let hoverUnlisteners: Array<() => void> = [];

  watch(
    () => [options.scroller.value, isEnabled()] as const,
    ([next, enabled]) => {
      clearScrollerListeners();
      if (next && enabled) {
        scrollerUnlisteners = [
          addDomEventListener(next, "scroll", onScroll, { passive: true }),
          addDomEventListener(next, "scrollend", onScrollEnd),
        ];
      }
    },
    { immediate: true },
  );

  watch(
    () => [options.hoverTarget?.value ?? options.scroller.value, isEnabled()] as const,
    ([next, enabled]) => {
      clearHoverListeners();
      if (next && enabled) {
        hoverUnlisteners = [
          addDomEventListener(next, "mousemove", onMouseMove),
          addDomEventListener(next, "mouseleave", onMouseLeave),
        ];
      }
    },
    { immediate: true },
  );

  watch(
    () => isEnabled(),
    (enabled) => {
      if (!enabled) {
        isPointerInZone.value = false;
        hide();
      }
    },
  );

  watch(
    () => options.isDragging?.value ?? false,
    (dragging, wasDragging) => {
      if (dragging) {
        show();
        return;
      }
      if (wasDragging && !isPointerInZone.value) hideSoon();
    },
  );

  onBeforeUnmount(() => {
    clearScrollerListeners();
    clearHoverListeners();
    clearHideTimer();
  });

  function isEnabled(): boolean {
    return options.enabled?.value ?? true;
  }

  function clearHideTimer() {
    if (hideTimer === null) return;
    window.clearTimeout(hideTimer);
    hideTimer = null;
  }

  function clearScrollerListeners() {
    runUnlistenFns(scrollerUnlisteners.splice(0).reverse());
  }

  function clearHoverListeners() {
    runUnlistenFns(hoverUnlisteners.splice(0).reverse());
  }

  function show() {
    clearHideTimer();
    isVisible.value = true;
  }

  function hide() {
    clearHideTimer();
    isVisible.value = false;
  }

  function hideSoon() {
    if (hideTimer !== null) return;
    hideTimer = window.setTimeout(() => {
      if (!isPointerInZone.value && !(options.isDragging?.value ?? false)) {
        isVisible.value = false;
      }
      hideTimer = null;
    }, options.hideDelay ?? DEFAULT_HIDE_DELAY);
  }

  function onScroll() {
    show();
  }

  function onScrollEnd() {
    if (!isPointerInZone.value) hideSoon();
  }

  function onMouseMove(event: MouseEvent) {
    const inZone = isInZone(event);
    isPointerInZone.value = inZone;
    if (inZone) {
      show();
      return;
    }
    if (isVisible.value) hideSoon();
  }

  function onMouseLeave() {
    isPointerInZone.value = false;
    if (isVisible.value) hideSoon();
  }

  function isInZone(event: MouseEvent): boolean {
    const scroller = options.scroller.value;
    if (!scroller) return false;
    const rect = scroller.getBoundingClientRect();
    const hotZone = options.hotZone ?? DEFAULT_HOT_ZONE;
    return event.clientX >= rect.right - hotZone && event.clientX <= rect.right;
  }

  return {
    hide,
    isVisible,
    show,
  };
}

export function markerTopForElement(
  scroller: HTMLElement,
  element: HTMLElement,
  metrics: ScrollMapMetrics,
): number {
  const scrollerRect = scroller.getBoundingClientRect();
  const elementRect = element.getBoundingClientRect();
  const contentTop = elementRect.top - scrollerRect.top + scroller.scrollTop;
  return clampValue(metrics.trackHeight * contentTop / metrics.domainHeight, 0, metrics.trackHeight);
}

function readDefaultGeometry(scroller: HTMLElement): ScrollMapGeometry {
  const visibleHeight = scroller.clientHeight;
  return {
    domainHeight: Math.max(visibleHeight, scroller.scrollHeight),
    visibleHeight,
  };
}

function maxScrollTop(scroller: HTMLElement): number {
  return maxScrollOffset(scroller.scrollHeight, scroller.clientHeight);
}

