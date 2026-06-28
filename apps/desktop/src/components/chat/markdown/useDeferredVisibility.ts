import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { beginPerfStage } from "../../../utils/perf";

type ElementGetter = () => Element | null;

export function useDeferredVisibility(options: {
  target: ElementGetter;
  perfName: string;
  detail?: () => string | null;
  rootMargin?: string;
  immediate?: boolean;
}) {
  const activated = ref(false);
  const stage = beginPerfStage(options.perfName, {
    detail: options.detail?.() ?? null,
  });
  const rootMargin = options.rootMargin ?? "240px 0px";
  const immediate = options.immediate ?? import.meta.env.MODE === "test";
  let observer: IntersectionObserver | null = null;
  let completed = false;

  function disconnectObserver() {
    observer?.disconnect();
    observer = null;
  }

  function finish(stageName: string) {
    if (completed) return;
    completed = true;
    disconnectObserver();
    stage.end(stageName);
  }

  function activate(stageName: string) {
    if (activated.value) return;
    activated.value = true;
    finish(stageName);
  }

  function observeTarget(element: Element) {
    if (activated.value) return;
    disconnectObserver();
    if (immediate || typeof IntersectionObserver !== "function") {
      activate(immediate ? "immediate" : "fallback");
      return;
    }
    observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting || entry.intersectionRatio > 0)) {
        activate("visible");
      }
    }, { rootMargin });
    observer.observe(element);
  }

  onMounted(() => {
    const element = options.target();
    if (element) observeTarget(element);
  });

  watch(
    options.target,
    (element) => {
      if (!element || activated.value) return;
      observeTarget(element);
    },
    { flush: "post" },
  );

  onBeforeUnmount(() => {
    if (!completed) {
      finish("cancelled");
    } else {
      disconnectObserver();
    }
  });

  return {
    activated,
  };
}

