type PerfStageHandle = {
  end: (stage?: string) => void;
  context: PerfLogContext;
};

type PerfMeasureOptions = {
  detail?: string | null;
  feature?: string | null;
  id?: number | string | null;
  route?: string | null;
  seq?: number | string | null;
};

type PerfLogContext = {
  feature: string;
  id: string;
  route: string;
  seq: string;
  detail?: string;
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

function stringValue(value: number | string | null | undefined): string | null {
  if (value === null || value === undefined || value === "") return null;
  return String(value);
}

function currentRoute(): string {
  try {
    const location = globalThis.location;
    if (!location) return "-";
    const path = `${location.pathname}${location.search}${location.hash}`;
    return path || location.href || "-";
  } catch {
    return "-";
  }
}

function createPerfContext(
  name: string,
  options: PerfMeasureOptions,
  generatedSeq: number,
): PerfLogContext {
  const detail = stringValue(options.detail);
  const seq = stringValue(options.seq) ?? String(generatedSeq);
  const route = stringValue(options.route) ??
    (detail?.startsWith("/") ? detail : null) ??
    currentRoute();
  const id = stringValue(options.id) ?? detail ?? `${name}:${seq}`;
  return {
    feature: stringValue(options.feature) ?? name,
    id,
    route,
    seq,
    ...(detail ? { detail } : {}),
  };
}

function formatPerfContext(context: PerfLogContext): string {
  const parts: Array<[string, string]> = [
    ["feature", context.feature],
    ["route", context.route],
    ["id", context.id],
    ["seq", context.seq],
  ];
  if (context.detail && context.detail !== context.id && context.detail !== context.route) {
    parts.push(["detail", context.detail]);
  }
  return parts
    .map(([key, value]) => `${key}=${JSON.stringify(value)}`)
    .join(" ");
}

function logPerf(name: string, duration: number, context: PerfLogContext) {
  if (!isPerfEnabled()) return;
  console.info(
    `[perf] ${name} ${duration.toFixed(1)}ms ${formatPerfContext(context)}`,
  );
}

function logPerfFailure(name: string, context: PerfLogContext, err: unknown) {
  console.error(`[perf] ${name}:failed ${formatPerfContext(context)}`, err);
}

function createMeasureName(name: string, stage: string) {
  return stage === "done" ? name : `${name}:${stage}`;
}

function recordMeasure(
  name: string,
  stage: string,
  startMark: string,
  endMark: string,
  context: PerfLogContext,
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
    logPerf(measureName, entry.duration, context);
  }
  perf.clearMeasures(measureName);
}

export function beginPerfStage(name: string, options: PerfMeasureOptions = {}): PerfStageHandle {
  const perf = currentPerformance();
  const seq = ++perfSeq;
  const context = createPerfContext(name, options, seq);
  if (!perf) {
    return { context, end: () => {} };
  }
  const activePerf = perf;
  const token = `${name}:${seq}`;
  const startMark = `${token}:start`;
  let ended = false;
  activePerf.mark(startMark);
  function finish(stage: string) {
    if (ended) return;
    ended = true;
    const endMark = `${token}:${stage}`;
    activePerf.mark(endMark);
    recordMeasure(name, stage, startMark, endMark, context);
    activePerf.clearMarks(startMark);
    activePerf.clearMarks(endMark);
  }
  return {
    context,
    end(stage = "done") {
      finish(stage);
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
    const result = await run();
    stage.end();
    return result;
  } catch (err) {
    stage.end("failed");
    logPerfFailure(name, stage.context, err);
    throw err;
  }
}

export function measurePerfSync<T>(
  name: string,
  run: () => T,
  options: PerfMeasureOptions = {},
): T {
  const stage = beginPerfStage(name, options);
  try {
    const result = run();
    stage.end();
    return result;
  } catch (err) {
    stage.end("failed");
    logPerfFailure(name, stage.context, err);
    throw err;
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
        const detail = entry.name || "main-thread";
        logPerf(
          "longtask",
          entry.duration,
          createPerfContext("longtask", {
            detail,
            feature: "longtask",
            id: detail,
            seq: "observer",
          }, 0),
        );
      }
    });
    observer.observe({ entryTypes: ["longtask"] });
  } catch {
    longTaskObserverInstalled = false;
  }
}

