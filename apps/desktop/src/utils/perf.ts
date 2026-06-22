type PerfStageHandle = {
  end: (stage?: string) => void;
};

type PerfMeasureOptions = {
  detail?: string | null;
};

let perfSeq = 0;
let longTaskObserverInstalled = false;

function currentPerformance(): Performance | null {
  return typeof performance === "object" && performance ? performance : null;
}

function isPerfEnabled(): boolean {
  if (import.meta.env.DEV) return true;
  try {
    return localStorage.getItem("lilia.perf") === "1";
  } catch {
    return false;
  }
}

function logPerf(name: string, duration: number, detail?: string | null) {
  if (!isPerfEnabled()) return;
  const suffix = detail ? ` ${detail}` : "";
  console.info(`[perf] ${name} ${duration.toFixed(1)}ms${suffix}`);
}

function createMeasureName(name: string, stage: string) {
  return stage === "done" ? name : `${name}:${stage}`;
}

function recordMeasure(
  name: string,
  stage: string,
  startMark: string,
  endMark: string,
  detail?: string | null,
) {
  const perf = currentPerformance();
  if (!perf) return;
  const measureName = createMeasureName(name, stage);
  try {
    perf.measure(measureName, startMark, endMark);
  } catch {
    return;
  }
  const entry = perf.getEntriesByName(measureName, "measure").at(-1);
  if (entry) {
    logPerf(measureName, entry.duration, detail);
  }
  perf.clearMeasures(measureName);
}

export function beginPerfStage(name: string, options: PerfMeasureOptions = {}): PerfStageHandle {
  const perf = currentPerformance();
  const detail = options.detail ?? null;
  if (!perf) {
    return { end: () => {} };
  }
  const token = `${name}:${++perfSeq}`;
  const startMark = `${token}:start`;
  let ended = false;
  perf.mark(startMark);
  return {
    end(stage = "done") {
      if (ended) return;
      ended = true;
      const endMark = `${token}:${stage}`;
      perf.mark(endMark);
      recordMeasure(name, stage, startMark, endMark, detail);
      perf.clearMarks(startMark);
      perf.clearMarks(endMark);
    },
  };
}

export async function measurePerfAsync<T>(
  name: string,
  run: () => Promise<T>,
  options: PerfMeasureOptions = {},
): Promise<T> {
  const stage = beginPerfStage(name, options);
  try {
    return await run();
  } finally {
    stage.end();
  }
}

export function measurePerfSync<T>(
  name: string,
  run: () => T,
  options: PerfMeasureOptions = {},
): T {
  const stage = beginPerfStage(name, options);
  try {
    return run();
  } finally {
    stage.end();
  }
}

export function scheduleAfterPaint(callback: () => void): () => void {
  let active = true;
  let frameHandle: number | null = null;
  let timeoutHandle: ReturnType<typeof globalThis.setTimeout> | null = null;
  const runCallback = () => {
    timeoutHandle = null;
    if (active) callback();
  };
  const scheduleTimeout = () => {
    if (!active) return;
    timeoutHandle = globalThis.setTimeout(runCallback, 0);
  };
  if (typeof requestAnimationFrame === "function") {
    frameHandle = requestAnimationFrame(() => {
      frameHandle = null;
      scheduleTimeout();
    });
  } else {
    scheduleTimeout();
  }
  return () => {
    active = false;
    if (frameHandle !== null && typeof cancelAnimationFrame === "function") {
      cancelAnimationFrame(frameHandle);
      frameHandle = null;
    }
    if (timeoutHandle !== null) {
      globalThis.clearTimeout(timeoutHandle);
      timeoutHandle = null;
    }
  }
}

type IdleWindow = typeof globalThis & {
  requestIdleCallback?: (callback: () => void) => number;
  cancelIdleCallback?: (handle: number) => void;
};

export function runWhenIdle(callback: () => void): number {
  const idleWindow = globalThis as IdleWindow;
  if (typeof idleWindow.requestIdleCallback === "function") {
    return idleWindow.requestIdleCallback(callback);
  }
  return globalThis.setTimeout(callback, 1);
}

export function cancelIdleRun(handle: number) {
  const idleWindow = globalThis as IdleWindow;
  if (typeof idleWindow.cancelIdleCallback === "function") {
    idleWindow.cancelIdleCallback(handle);
    return;
  }
  globalThis.clearTimeout(handle);
}

export function installPerfObservers() {
  if (
    longTaskObserverInstalled ||
    !isPerfEnabled() ||
    typeof PerformanceObserver !== "function"
  ) {
    return;
  }
  longTaskObserverInstalled = true;
  try {
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        logPerf("longtask", entry.duration, entry.name || "main-thread");
      }
    });
    observer.observe({ entryTypes: ["longtask"] });
  } catch {
    longTaskObserverInstalled = false;
  }
}
