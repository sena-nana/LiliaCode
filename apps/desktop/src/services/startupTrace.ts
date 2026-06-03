declare const __LILIA_STARTUP_TRACE__: boolean | undefined;

const startupTraceEnabled =
  (import.meta.env.DEV && import.meta.env.MODE !== "test") ||
  (typeof __LILIA_STARTUP_TRACE__ !== "undefined" && __LILIA_STARTUP_TRACE__);

const startupStart =
  typeof performance !== "undefined" ? performance.now() : Date.now();

interface PendingStartupMark {
  label: string;
  elapsed: number;
}

type TauriInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

let tauriInvoke: TauriInvoke | null = null;
let pendingTauriMarks: PendingStartupMark[] = [];
let flushingTauriMarks = false;
let tauriTraceDisabled = false;

function hasTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function loadTauriInvoke(): Promise<TauriInvoke> {
  if (tauriInvoke) return tauriInvoke;
  const api = await import("@tauri-apps/api/core");
  tauriInvoke = api.invoke as TauriInvoke;
  return tauriInvoke;
}

async function flushTauriMarks(): Promise<void> {
  if (!hasTauriRuntime() || tauriTraceDisabled || flushingTauriMarks) return;
  flushingTauriMarks = true;
  try {
    const invoke = await loadTauriInvoke();
    while (pendingTauriMarks.length > 0) {
      const marks = pendingTauriMarks;
      pendingTauriMarks = [];
      for (const mark of marks) {
        void invoke("startup_trace_frontend_mark", {
          label: mark.label,
          frontendElapsedMs: mark.elapsed,
        });
      }
    }
  } catch {
    tauriTraceDisabled = true;
    pendingTauriMarks = [];
  } finally {
    flushingTauriMarks = false;
    if (pendingTauriMarks.length > 0) void flushTauriMarks();
  }
}

export function markStartup(label: string): void {
  const now = typeof performance !== "undefined" ? performance.now() : Date.now();
  const elapsed = Math.round(now - startupStart);
  if (startupTraceEnabled) {
    console.info(`[startup] +${elapsed}ms ${label}`);
  }
  if (startupTraceEnabled && hasTauriRuntime()) {
    pendingTauriMarks.push({ label, elapsed });
    void flushTauriMarks();
  }
}
