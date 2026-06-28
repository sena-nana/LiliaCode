import { onBeforeUnmount, ref, watch } from "vue";
import {
  cancelIdleRun,
  measurePerfAsync,
  measurePerfSync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../../utils/perf";
import { createLazyLoadState } from "../../../utils/lazyLoadState";
import { useDeferredVisibility } from "./useDeferredVisibility";

const mathRenderModuleLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "markdown.math.module.load",
    async () => await import("./mathRender"),
  )
);

async function loadMathRenderModule() {
  return mathRenderModuleLoad.load();
}

export function useDeferredMathRender(options: {
  source: () => string;
  target: () => Element | null;
  displayMode: boolean;
  perfName: string;
  visibilityPerfName: string;
}) {
  const renderedHtml = ref<string | null>(null);
  let renderSeq = 0;
  let idleHandle: number | null = null;
  let cancelPaintRender: (() => void) | null = null;
  const { activated } = useDeferredVisibility({
    target: options.target,
    perfName: options.visibilityPerfName,
    detail: () => `${options.source().length} chars`,
  });

  function cancelScheduledRender() {
    cancelPaintRender?.();
    cancelPaintRender = null;
    if (idleHandle === null) return;
    cancelIdleRun(idleHandle);
    idleHandle = null;
  }

  async function renderNow(seq: number) {
    const source = options.source();
    if (!source.trim()) {
      renderedHtml.value = null;
      return;
    }
    const module = await loadMathRenderModule();
    if (seq !== renderSeq) return;
    renderedHtml.value = measurePerfSync(
      options.perfName,
      () => module.renderMathToHtml(source, options.displayMode),
      { detail: `${source.length} chars` },
    );
  }

  function scheduleRender() {
    const seq = ++renderSeq;
    cancelScheduledRender();
    const source = options.source();
    if (!source.trim()) {
      renderedHtml.value = null;
      return;
    }
    if (!activated.value) {
      renderedHtml.value = null;
      return;
    }
    if (import.meta.env.MODE === "test") {
      void renderNow(seq);
      return;
    }
    cancelPaintRender = scheduleAfterPaint(() => {
      cancelPaintRender = null;
      if (seq !== renderSeq) return;
      idleHandle = runWhenIdle(() => {
        idleHandle = null;
        if (seq !== renderSeq) return;
        void renderNow(seq);
      });
    });
  }

  watch(
    () => [options.source(), activated.value] as const,
    scheduleRender,
    { immediate: true },
  );

  onBeforeUnmount(() => {
    renderSeq += 1;
    cancelScheduledRender();
  });

  return {
    renderedHtml,
  };
}

