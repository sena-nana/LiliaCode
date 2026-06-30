import { createApp } from "vue";
import "@lilia/ui/styles.css";
import { installLiliaAppRuntime } from "@lilia/ui";
import App from "./App.vue";
import { router } from "./router";
import { installAgentDebugHarness } from "./agentDebug/harness";
import { appConfig } from "./app.config";
import "./styles/components.css";
import "./styles/shell.css";
import { installPerfObservers } from "@lilia/ui";

export function mountLiliaApp(): void {
  installPerfObservers();

  const app = createApp(App);
  installLiliaAppRuntime({ app, config: appConfig });
  app.use(router);
  app.mount("#root");

  installAgentDebugHarness(router);
}

