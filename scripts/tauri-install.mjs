import { fileURLToPath } from "node:url";
import { runTauriInstall } from "@lilia/build";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));

runTauriInstall(projectRoot, process.env, {
  appName: "lilia",
  appDir: "apps/desktop",
  dryRunEnvKey: "TAURI_TEMPLATE_INSTALL_DRY_RUN",
});
