import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import { markStartup } from "./services/startupTrace";
import "./composables/useTheme";
import { installContextMenu } from "./composables/useContextMenu";
import { vContextMenu } from "./directives/contextMenu";
import "./styles.css";

markStartup("bootstrap module start");

export function mountLiliaApp(): void {
  installContextMenu();
  markStartup("context menu installed");

  const app = createApp(App);
  markStartup("createApp");
  app.use(router);
  markStartup("router install");
  app.directive("context-menu", vContextMenu);
  app.mount("#root");
  markStartup("mount");
}
