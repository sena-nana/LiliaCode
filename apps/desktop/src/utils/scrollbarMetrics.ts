export interface ScrollbarMetricInput {
  domainSize: number;
  minThumbSize?: number;
  scrollOffset: number;
  trackSize: number;
  visibleSize: number;
}

export interface ScrollbarMetrics {
  domainSize: number;
  scrollable: boolean;
  scrollOffset: number;
  thumbOffset: number;
  thumbSize: number;
  trackSize: number;
  visibleSize: number;
}

export function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(Math.max(value, min), max);
}

export function maxScrollOffset(domainSize: number, visibleSize: number): number {
  return Math.max(0, domainSize - visibleSize);
}

export function readScrollbarMetrics(input: ScrollbarMetricInput): ScrollbarMetrics {
  const visibleSize = Math.max(0, input.visibleSize);
  const domainSize = Math.max(visibleSize, input.domainSize);
  const trackSize = Math.max(0, input.trackSize);
  const scrollOffset = clamp(input.scrollOffset, 0, maxScrollOffset(domainSize, visibleSize));
  const scrollable = domainSize - visibleSize > 1;
  const proportionalThumbSize = domainSize > 0 ? trackSize * visibleSize / domainSize : 0;
  const minThumbSize = input.minThumbSize ?? 0;
  const thumbSize = scrollable
    ? Math.min(trackSize, Math.max(minThumbSize, proportionalThumbSize))
    : 0;
  const thumbTrackSize = Math.max(0, trackSize - thumbSize);
  const thumbOffset = scrollable
    ? thumbTrackSize * scrollOffset / Math.max(1, maxScrollOffset(domainSize, visibleSize))
    : 0;

  return {
    domainSize,
    scrollable,
    scrollOffset,
    thumbOffset,
    thumbSize,
    trackSize,
    visibleSize,
  };
}

export function scrollOffsetForThumbDrag(
  startScrollOffset: number,
  pointerDelta: number,
  metrics: Pick<ScrollbarMetrics, "domainSize" | "thumbSize" | "trackSize" | "visibleSize">,
): number {
  const scrollRange = maxScrollOffset(metrics.domainSize, metrics.visibleSize);
  const thumbTrackSize = Math.max(1, metrics.trackSize - metrics.thumbSize);
  return clamp(
    startScrollOffset + pointerDelta * scrollRange / thumbTrackSize,
    0,
    scrollRange,
  );
}

