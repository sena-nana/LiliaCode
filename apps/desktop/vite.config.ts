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

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  resolve: {
    alias: {
      "@lilia/contracts/claudeTools.mjs": fileURLToPath(
        new URL("../../packages/contracts/src/claudeTools.mjs", import.meta.url),
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
}));
