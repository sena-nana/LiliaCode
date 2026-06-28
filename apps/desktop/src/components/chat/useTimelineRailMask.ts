import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { WatchSource } from "vue";

interface RailGap {
  top: number;
  height: number;
}

export function useTimelineRailMask(measureSources: WatchSource<unknown>[]) {
  const timelineRef = ref<HTMLElement | null>(null);
  const railGaps = ref<RailGap[]>([]);
  let railResizeObserver: ResizeObserver | null = null;
  let railMeasureRaf = 0;
  let railMeasureSeq = 0;
  let disposed = false;

  const railLineStyle = computed<Record<string, string>>(() => {
    const style: Record<string, string> = {};
    const maskImage = railMaskImage(railGaps.value);
    if (!maskImage) return style;
    style.maskImage = maskImage;
    style.WebkitMaskImage = maskImage;
    return style;
  });

  watch(
    measureSources,
    () => scheduleRailMeasure(),
    { flush: "post" },
  );

  onMounted(() => {
    disposed = false;
    if (typeof ResizeObserver !== "undefined" && timelineRef.value) {
      railResizeObserver = new ResizeObserver(() => scheduleRailMeasure());
      railResizeObserver.observe(timelineRef.value);
    }
    scheduleRailMeasure();
  });

  onBeforeUnmount(() => {
    disposed = true;
    railMeasureSeq += 1;
    railResizeObserver?.disconnect();
    if (railMeasureRaf) cancelAnimationFrame(railMeasureRaf);
    railMeasureRaf = 0;
  });

  function scheduleRailMeasure() {
    if (disposed) return;
    if (railMeasureRaf) cancelAnimationFrame(railMeasureRaf);
    const seq = ++railMeasureSeq;
    railMeasureRaf = requestAnimationFrame(() => {
      railMeasureRaf = 0;
      void measureRailGaps(seq);
    });
  }

  async function measureRailGaps(seq: number) {
    await nextTick();
    if (disposed || seq !== railMeasureSeq) return;
    const timeline = timelineRef.value;
    if (!timeline) {
      railGaps.value = [];
      return;
    }

    const timelineRect = timeline.getBoundingClientRect();
    const nodes = [...timeline.querySelectorAll<HTMLElement>(".agent-timeline__node")]
      .filter(isMeasurableRailNode);
    const next = nodes
      .map((node): RailGap | null => {
        const rect = node.getBoundingClientRect();
        if (rect.height <= 0) return null;
        return {
          top: Math.round(rect.top - timelineRect.top),
          height: Math.round(rect.height),
        };
      })
      .filter((gap): gap is RailGap => gap !== null);

    if (railGapSignature(railGaps.value) !== railGapSignature(next)) {
      railGaps.value = next;
    }
  }

  return {
    railLineStyle,
    scheduleRailMeasure,
    timelineRef,
  };
}

function isMeasurableRailNode(node: HTMLElement): boolean {
  return !node.closest(".agent-timeline__process-collapse:not(.is-open)");
}

function railGapSignature(gaps: RailGap[]): string {
  return gaps.map((gap) => `${gap.top}:${gap.height}`).join("|");
}

function railMaskImage(gaps: RailGap[]): string {
  if (gaps.length === 0) return "";
  const stops: string[] = ["#000 0px"];
  for (const gap of mergeRailGaps(gaps)) {
    const top = `${gap.top}px`;
    const bottom = `${gap.top + gap.height}px`;
    stops.push(`#000 ${top}`, `transparent ${top}`, `transparent ${bottom}`, `#000 ${bottom}`);
  }
  stops.push("#000 100%");
  return `linear-gradient(to bottom, ${stops.join(", ")})`;
}

function mergeRailGaps(gaps: RailGap[]): RailGap[] {
  const sorted = [...gaps]
    .map((gap) => ({
      ...gap,
      top: Math.max(0, gap.top),
      height: Math.max(0, gap.height),
    }))
    .filter((gap) => gap.height > 0)
    .sort((a, b) => a.top - b.top);
  const merged: RailGap[] = [];
  for (const gap of sorted) {
    const last = merged[merged.length - 1];
    if (!last || gap.top > last.top + last.height) {
      merged.push(gap);
      continue;
    }
    const bottom = Math.max(last.top + last.height, gap.top + gap.height);
    last.height = bottom - last.top;
  }
  return merged;
}

