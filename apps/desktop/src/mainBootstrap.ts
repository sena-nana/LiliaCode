import { createApp } from "vue";
import "@lilia/ui/styles.css";
import { installPerfObservers } from "@lilia/ui/diagnostics";
import {
  installCornerStyle,
  installGlobalScrollbarVisibility,
  installLiliaContextMenu,
  installNativeAppearance,
} from "@lilia/ui/runtime";
import { provideLiliaSettings } from "@lilia/ui/settings";
import { setLiliaUiConfig } from "@lilia/ui/shell";
import App from "./App.vue";
import { router } from "./router";
import {
  installAgentDebugHarness,
  isAgentDebugFrontendEnabled,
} from "./agentDebug/harness";
import { appConfig, settingsModel } from "./app.config";
import "./styles/components.css";
import "./styles/shell.css";

export function mountLiliaApp(): void {
  installPerfObservers();

  const app = createApp(App);
  setLiliaUiConfig(appConfig);
  provideLiliaSettings(app, settingsModel);
  app.use(router);
  installLiliaContextMenu(app);
  installGlobalScrollbarVisibility();
  installCornerStyle();
  installNativeAppearance();
  app.mount("#root");

  if (isAgentDebugFrontendEnabled()) installAgentDebugHarness(router);
}
