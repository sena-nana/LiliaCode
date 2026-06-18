import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import "./composables/useCornerStyle";
import "./composables/useTheme";
import { vContextMenu } from "./directives/contextMenu";
import "./styles/index.css";
import "./styles/components.css";
import "./styles/shell.css";
import { installPerfObservers, measurePerfAsync, scheduleAfterPaint } from "./utils/perf";

function scheduleGlobalInstallers() {
  scheduleAfterPaint(() => {
    void measurePerfAsync("bootstrap.context-menu.install", async () => {
      const { installContextMenu } = await import("./composables/useContextMenuInstall");
      installContextMenu();
    });
    void measurePerfAsync("bootstrap.scrollbar-overlay.install", async () => {
      const { installGlobalScrollbarVisibility } = await import("./composables/useGlobalScrollbarVisibility");
      installGlobalScrollbarVisibility();
    });
  });
}

export function mountLiliaApp(): void {
  installPerfObservers();

  const app = createApp(App);
  app.use(router);
  app.directive("context-menu", vContextMenu);
  app.mount("#root");

  scheduleGlobalInstallers();
}
