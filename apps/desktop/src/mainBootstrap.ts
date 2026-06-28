import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import { installAgentDebugHarness } from "./agentDebug/harness";
import "./composables/useCornerStyle";
import "./composables/useTheme";
import { vContextMenu } from "./directives/contextMenu";
import "./styles/index.css";
import "./styles/components.css";
import "./styles/shell.css";
import { runUnlistenFns } from "./utils/eventListeners";
import { createLazyLoadState } from "./utils/lazyLoadState";
import { installPerfObservers, measurePerfAsync, scheduleAfterPaint } from "./utils/perf";

const contextMenuInstallerLoad = createLazyLoadState(() =>
  measurePerfAsync("bootstrap.context-menu.module.load", async () =>
    await import("./composables/useContextMenuInstall")
  )
);
const scrollbarInstallerLoad = createLazyLoadState(() =>
  measurePerfAsync("bootstrap.scrollbar-overlay.module.load", async () =>
    await import("./composables/useGlobalScrollbarVisibility")
  )
);

export function scheduleGlobalInstallers(): () => void {
  let cancelled = false;
  const cleanups: Array<() => void> = [];

  function keepCleanup(cleanup: (() => void) | void) {
    if (!cleanup) return;
    if (cancelled) {
      cleanup();
      return;
    }
    cleanups.push(cleanup);
  }

  const cancelPaint = scheduleAfterPaint(() => {
    void measurePerfAsync("bootstrap.context-menu.install", async () => {
      const { installContextMenu } = await contextMenuInstallerLoad.load();
      keepCleanup(installContextMenu());
    });
    void measurePerfAsync("bootstrap.scrollbar-overlay.install", async () => {
      const { installGlobalScrollbarVisibility } = await scrollbarInstallerLoad.load();
      keepCleanup(installGlobalScrollbarVisibility());
    });
  });

  return () => {
    cancelled = true;
    cancelPaint();
    runUnlistenFns(cleanups.splice(0).reverse());
  };
}

export function mountLiliaApp(): void {
  installPerfObservers();

  const app = createApp(App);
  app.use(router);
  app.directive("context-menu", vContextMenu);
  app.mount("#root");

  installAgentDebugHarness(router);
  scheduleGlobalInstallers();
}

