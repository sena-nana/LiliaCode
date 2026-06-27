/// <reference types="vitest" />
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath, URL } from "node:url";

// @ts-expect-error process 是 Node.js 全局对象
const host = process.env.TAURI_DEV_HOST;
// @ts-expect-error process 是 Node.js 全局对象
const port = Number.parseInt(process.env.LILIA_DEV_PORT ?? "1420", 10);
// @ts-expect-error process 是 Node.js 全局对象
const strictPort = process.env.LILIA_DEV_STRICT_PORT === "1";

const devMockAliases = {
  "@tauri-apps/api/core": fileURLToPath(
    new URL("./src/tauri/devMock.ts", import.meta.url),
  ),
  "@tauri-apps/api/event": fileURLToPath(
    new URL("./src/tauri/devMock.ts", import.meta.url),
  ),
  "@tauri-apps/api/window": fileURLToPath(
    new URL("./src/tauri/devMock.ts", import.meta.url),
  ),
  "@tauri-apps/api/path": fileURLToPath(
    new URL("./src/tauri/devMock.ts", import.meta.url),
  ),
  "@tauri-apps/api/webview": fileURLToPath(
    new URL("./src/tauri/devMock.ts", import.meta.url),
  ),
  "@tauri-apps/api/dpi": fileURLToPath(
    new URL("./src/tauri/devMock.ts", import.meta.url),
  ),
};

function isMermaidParserModule(id: string): boolean {
  const normalized = id.replace(/\\/g, "/");
  return normalized.includes("/node_modules/@mermaid-js/parser/");
}

function chunkFileNames(chunk: { moduleIds?: string[] }): string {
  if (chunk.moduleIds?.some(isMermaidParserModule)) {
    return "assets/mermaid-parser-[hash].js";
  }
  return "assets/[name]-[hash].js";
}

// https://vite.dev/config/
export default defineConfig(async ({ command, mode }) => ({
  plugins: [vue()],

  resolve: {
    alias: {
      ...(command === "serve" && mode !== "test" && !strictPort ? devMockAliases : {}),
      "@lilia/contracts/agentInteractionContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/agentInteractionContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/askUserContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/askUserContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/architectureContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/architectureContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/claudePlanContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/claudePlanContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/claudeTools.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/claudeTools.mjs", import.meta.url),
      ),
      "@lilia/contracts/conversationContextContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/conversationContextContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/historyImportContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/historyImportContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/liliaAgentProtocol.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/liliaAgentProtocol.mjs", import.meta.url),
      ),
      "@lilia/contracts/liliaWorkflowContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/liliaWorkflowContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/permissionModes.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/permissionModes.vite.mjs", import.meta.url),
      ),
      "@lilia/contracts/promptContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/promptContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/provider.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/providerRuntime.mjs", import.meta.url),
      ),
      "@lilia/contracts/quotaContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/quotaContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/runnerProtocolContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/runnerProtocolContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/runtimeCommandContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/runtimeCommandContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/sessionManagementContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/sessionManagementContract.mjs", import.meta.url),
      ),
      "@lilia/contracts/timelineContract.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/timelineContract.mjs", import.meta.url),
      ),
      "@lilia/contracts": fileURLToPath(
        new URL("../../packages/contracts/src/index.ts", import.meta.url),
      ),
    },
  },

  // 这些 Vite 选项面向 Tauri 开发，只在 `tauri dev` 或 `tauri build` 中生效
  clearScreen: false,
  server: {
    port: Number.isFinite(port) ? port : 1420,
    strictPort,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setupTests.ts"],
  },
  build: {
    rollupOptions: {
      output: {
        chunkFileNames,
      },
    },
  },
}));
