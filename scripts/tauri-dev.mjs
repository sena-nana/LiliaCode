import { fileURLToPath } from "node:url";
import { runTauriDev } from "@lilia/build";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));

await runTauriDev(projectRoot, process.argv.slice(2), process.env, {
  appName: "lilia",
  appDir: "apps/desktop",
  dryRunEnvKey: "LILIA_TAURI_DEV_DRY_RUN",
  dryRunEnvKeys: ["VITE_LILIA_AGENT_DEBUG"],
  extraEnv: (env) => ({
    VITE_LILIA_AGENT_DEBUG: env.LILIA_AGENT_DEBUG === "1"
      ? "1"
      : env.VITE_LILIA_AGENT_DEBUG,
  }),
});
