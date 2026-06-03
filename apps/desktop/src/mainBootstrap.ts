import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import "./composables/useTheme";
import { installContextMenu } from "./composables/useContextMenu";
import { vContextMenu } from "./directives/contextMenu";
import "./styles.css";

export function mountLiliaApp(): void {
  installContextMenu();

  const app = createApp(App);
  app.use(router);
  app.directive("context-menu", vContextMenu);
  app.mount("#root");
}
