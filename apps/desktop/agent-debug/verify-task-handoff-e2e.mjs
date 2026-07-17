import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  createAgentDebugChildEnv,
  createAgentDebugDevServerPlan,
  findAvailablePort,
} from "./dev-server.mjs";

const repoRoot = path.resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const runId = new Date().toISOString().replace(/[:.]/g, "-");
const runDir = path.join(repoRoot, "agent-debug-runs", `${runId}-task-handoff`);
const liliaHome = path.join(runDir, "lilia-home");
const handoffPath = path.join(runDir, "workflow-handoff.json");
const receiptPath = `${handoffPath}.receipt.json`;
const appBinary = path.join(
  repoRoot,
  "apps",
  "desktop",
  "src-tauri",
  "target",
  "debug",
  process.platform === "win32" ? "lilia.exe" : "lilia",
);
const tauriConfigPath = path.join(repoRoot, "apps", "desktop", "src-tauri", "tauri.conf.json");
const cargoManifestPath = path.join(repoRoot, "apps", "desktop", "src-tauri", "Cargo.toml");
const viteBin = path.join(repoRoot, "node_modules", "vite", "bin", "vite.js");
const debugIdentifier = `com.lilia.desktop.agent-debug.${runId.toLowerCase()}`;
const handoffId = `issue-26-e2e-${runId}`;
let devServer = null;
let app = null;
let duplicateApp = null;
let sessionId = null;
const devServerOutput = [];
const appOutput = [];
const duplicateOutput = [];

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function stopProcess(child) {
  if (!child || child.killed || child.exitCode !== null) return;
  if (process.platform === "win32") {
    spawn("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
    return;
  }
  child.kill();
}

function trackOutput(child, output) {
  child.stdout?.on("data", (chunk) => output.push(chunk.toString("utf8")));
  child.stderr?.on("data", (chunk) => output.push(chunk.toString("utf8")));
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const output = [];
    const child = spawn(command, args, {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
      ...options,
    });
    trackOutput(child, output);
    child.once("error", reject);
    child.once("exit", (code) => {
      const text = output.join("");
      if (code === 0) resolve(text);
      else reject(new Error(`${command} ${args.join(" ")} failed with exit code ${code}\n${text}`));
    });
  });
}

function request(driverUrl, method, pathname, body) {
  const payload = body === undefined ? null : JSON.stringify(body);
  return new Promise((resolve, reject) => {
    const req = http.request(
      `${driverUrl}${pathname}`,
      {
        method,
        headers: payload
          ? { "content-type": "application/json", "content-length": Buffer.byteLength(payload) }
          : undefined,
      },
      (res) => {
        const chunks = [];
        res.on("data", (chunk) => chunks.push(chunk));
        res.on("end", () => {
          const text = Buffer.concat(chunks).toString("utf8");
          let value = null;
          try {
            value = text ? JSON.parse(text) : null;
          } catch {
            // Raw response text is included in the error below.
          }
          if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) resolve(value);
          else reject(new Error(`${method} ${pathname} failed: ${res.statusCode} ${text}`));
        });
      },
    );
    req.once("error", reject);
    if (payload) req.write(payload);
    req.end();
  });
}

function getUrl(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      res.resume();
      res.once("end", () => {
        if (res.statusCode && res.statusCode >= 200 && res.statusCode < 400) resolve();
        else reject(new Error(`GET ${url} failed: ${res.statusCode}`));
      });
    });
    req.once("error", reject);
    req.setTimeout(1_000, () => req.destroy(new Error(`GET ${url} timed out`)));
  });
}

async function waitUntil(label, timeoutMs, check) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const value = await check();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await sleep(250);
  }
  throw new Error(`${label} did not become ready within ${timeoutMs}ms${lastError ? `: ${lastError}` : ""}`);
}

async function execute(driverUrl, script, args = []) {
  const result = await request(
    driverUrl,
    "POST",
    `/session/${sessionId}/execute/sync`,
    { script, args },
  );
  return result?.value;
}

async function writeJson(name, value) {
  const target = path.join(runDir, name);
  await writeFile(target, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  return target;
}

async function readReceipt() {
  if (!existsSync(receiptPath)) return null;
  return JSON.parse(await readFile(receiptPath, "utf8"));
}

function assertReceipt(receipt) {
  if (
    receipt?.protocol !== "lilia-code-task-handoff" ||
    receipt?.version !== 1 ||
    receipt?.handoffId !== handoffId ||
    receipt?.status !== "accepted" ||
    !receipt?.taskId ||
    !receipt?.projectId ||
    receipt?.resultRoute !== `/projects/${receipt.projectId}/tasks/${receipt.taskId}`
  ) {
    throw new Error(`Invalid task handoff receipt: ${JSON.stringify(receipt)}`);
  }
}

function requiredPromptSemantics(handoff) {
  return [
    handoff.problem,
    handoff.repository.fullName,
    handoff.repository.worktreePath,
    handoff.repository.branch,
    handoff.repository.remoteUrl,
    handoff.source.route,
    handoff.source.objectUrl,
    ...handoff.relatedFiles,
    handoff.logSummary,
    ...handoff.acceptanceCriteria,
    handoff.workflow?.workflowName,
    handoff.workflow?.runUrl,
    handoff.workflow ? String(handoff.workflow.runId) : null,
  ].filter((value) => typeof value === "string" && value.length > 0);
}

async function buildDebugApp(devUrl) {
  const config = JSON.parse(await readFile(tauriConfigPath, "utf8"));
  config.identifier = debugIdentifier;
  config.build = { ...config.build, devUrl };
  const features = ["agent-debug-webdriver"];
  if (process.platform === "darwin") features.push("tauri/macos-private-api");
  const output = await run(
    "cargo",
    ["build", "--manifest-path", cargoManifestPath, "--features", features.join(",")],
    {
      env: {
        ...process.env,
        TAURI_CONFIG: JSON.stringify(config),
      },
    },
  );
  await writeFile(path.join(runDir, "debug-app-build.log"), output, "utf8");
}

async function main() {
  await mkdir(liliaHome, { recursive: true });
  const devServerPlan = await createAgentDebugDevServerPlan(process.env);
  const driverPort = await findAvailablePort(
    Number.parseInt(process.env.LILIA_TASK_HANDOFF_DRIVER_PORT ?? "4444", 10),
  );
  const driverUrl = `http://127.0.0.1:${driverPort}`;
  const childEnv = {
    ...createAgentDebugChildEnv(process.env, devServerPlan.devUrl, devServerPlan.port),
    LILIA_HOME: liliaHome,
    TAURI_WEBDRIVER_PORT: String(driverPort),
  };
  const handoff = {
    protocol: "lilia-code-task-handoff",
    version: 1,
    id: handoffId,
    createdAt: new Date().toISOString(),
    title: "修复 Desktop CI 构建失败",
    kind: "workflowFailure",
    repository: {
      fullName: "sena-nana/LiliaCode",
      worktreePath: repoRoot,
      branch: "main",
      remoteUrl: "https://github.com/sena-nana/LiliaCode.git",
    },
    source: {
      application: "LiliaGithub",
      route: "/repos/LiliaCode?projectTab=actions&run=2601",
      objectUrl: "https://github.com/sena-nana/LiliaCode/actions/runs/2601",
    },
    problem: "修复 CI 中稳定复现的桌面端构建失败，并保留失败上下文。",
    relatedFiles: ["apps/desktop/src-tauri/src/lib.rs", "apps/desktop/src/services/taskHandoff.ts"],
    logSummary: "error[E0425]: cannot find function `task_handoff_get` in this scope",
    acceptanceCriteria: ["真实桌面应用打开精确任务路由", "失败日志与相关文件自动进入 composer"],
    pullRequest: null,
    workflow: {
      runId: 2601,
      runUrl: "https://github.com/sena-nana/LiliaCode/actions/runs/2601",
      workflowName: "Desktop CI",
    },
  };
  const expectedPromptFragments = requiredPromptSemantics(handoff);
  await writeJson("workflow-handoff.json", handoff);
  await writeJson("preflight.json", {
    runId,
    runDir,
    devUrl: devServerPlan.devUrl,
    driverUrl,
    appBinary,
    liliaHome,
    debugIdentifier,
    webdriverProvider: "embedded",
  });

  await buildDebugApp(devServerPlan.devUrl);
  if (!existsSync(appBinary)) throw new Error(`Debug app binary not found: ${appBinary}`);

  try {
    try {
      await getUrl(devServerPlan.devUrl);
    } catch {
      devServer = spawn(process.execPath, [viteBin, "--host", "127.0.0.1"], {
        cwd: path.join(repoRoot, "apps", "desktop"),
        stdio: ["ignore", "pipe", "pipe"],
        env: childEnv,
      });
      trackOutput(devServer, devServerOutput);
      await waitUntil("LiliaCode dev server", 30_000, async () => {
        if (devServer.exitCode !== null) {
          throw new Error(devServerOutput.join("") || `exit ${devServer.exitCode}`);
        }
        try {
          await getUrl(devServerPlan.devUrl);
          return true;
        } catch {
          return false;
        }
      });
    }

    app = spawn(appBinary, ["--task-handoff", handoffPath], {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
      env: childEnv,
    });
    trackOutput(app, appOutput);
    await waitUntil("embedded Tauri WebDriver", 20_000, async () => {
      if (app.exitCode !== null) throw new Error(appOutput.join("") || `exit ${app.exitCode}`);
      try {
        await request(driverUrl, "GET", "/status");
        return true;
      } catch {
        return false;
      }
    });
    const session = await request(driverUrl, "POST", "/session", {
      capabilities: { alwaysMatch: { browserName: "tauri" } },
    });
    sessionId = session?.value?.sessionId;
    if (!sessionId) throw new Error(`WebDriver session did not return an id: ${JSON.stringify(session)}`);

    const firstReceipt = await waitUntil("task handoff receipt", 20_000, readReceipt);
    assertReceipt(firstReceipt);
    let lastUiState = null;
    let firstUi;
    try {
      firstUi = await waitUntil("imported handoff task UI", 30_000, async () => {
        lastUiState = await execute(
          driverUrl,
          `const composer = document.querySelector('[data-agent-id="chat.composer.input"]');
           const debug = window.__liliaAgentDebug?.observe?.();
           return {
             route: window.location.pathname + window.location.search,
             composerText: composer?.innerText ?? composer?.textContent ?? "",
             bodyText: document.body?.innerText?.slice(0, 1500) ?? "",
             agentIds: (debug?.elements ?? []).map((element) => element.id || element.agentId).filter(Boolean),
             recentErrors: window.__liliaAgentDebug?.getRecentErrors?.() ?? [],
             hasDebugApi: Boolean(window.__liliaAgentDebug),
           };`,
        );
        if (lastUiState?.route !== firstReceipt.resultRoute) return null;
        if (!expectedPromptFragments.every((fragment) => lastUiState.composerText.includes(fragment))) return null;
        return lastUiState;
      });
    } catch (error) {
      throw new Error(`${error instanceof Error ? error.message : String(error)}; last UI state: ${JSON.stringify(lastUiState)}`);
    }
    const screenshot = await request(driverUrl, "GET", `/session/${sessionId}/screenshot`);
    const screenshotPath = path.join(runDir, "task-handoff.png");
    await writeFile(screenshotPath, Buffer.from(screenshot.value, "base64"));

    await execute(driverUrl, "window.history.pushState({}, '', '/'); window.dispatchEvent(new PopStateEvent('popstate')); return true;");
    await waitUntil("route reset before duplicate handoff", 5_000, async () =>
      (await execute(driverUrl, "return window.location.pathname;")) === "/"
    );
    await sleep(500);
    duplicateApp = spawn(appBinary, ["--task-handoff", handoffPath], {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
      env: childEnv,
    });
    trackOutput(duplicateApp, duplicateOutput);
    const duplicateReceipt = await waitUntil("duplicate task handoff receipt", 15_000, async () => {
      const receipt = await readReceipt();
      return Number(receipt?.updatedAt) > Number(firstReceipt.updatedAt) ? receipt : null;
    });
    assertReceipt(duplicateReceipt);
    if (
      duplicateReceipt.taskId !== firstReceipt.taskId ||
      duplicateReceipt.projectId !== firstReceipt.projectId ||
      duplicateReceipt.resultRoute !== firstReceipt.resultRoute
    ) {
      throw new Error("Duplicate handoff created a different project, task, or result route");
    }
    const duplicateRoute = await waitUntil("duplicate handoff exact route", 15_000, async () => {
      const route = await execute(driverUrl, "return window.location.pathname + window.location.search;");
      return route === firstReceipt.resultRoute ? route : null;
    });

    await writeJson("summary.json", {
      status: "passed",
      runId,
      artifact: screenshotPath,
      firstImport: {
        receipt: firstReceipt,
        route: firstUi.route,
        promptFragments: expectedPromptFragments,
        debugApiEnabled: firstUi.hasDebugApi,
      },
      duplicateReopen: {
        receipt: duplicateReceipt,
        route: duplicateRoute,
      },
      isolation: { liliaHome, debugIdentifier },
      webdriverProvider: "embedded",
    });
  } finally {
    if (sessionId) {
      await request(driverUrl, "DELETE", `/session/${sessionId}`).catch(() => undefined);
    }
    stopProcess(duplicateApp);
    stopProcess(app);
    stopProcess(devServer);
    await writeFile(path.join(runDir, "dev-server.log"), devServerOutput.join(""), "utf8");
    await writeFile(path.join(runDir, "app.log"), appOutput.join(""), "utf8");
    await writeFile(path.join(runDir, "duplicate-app.log"), duplicateOutput.join(""), "utf8");
  }
}

await mkdir(runDir, { recursive: true });
try {
  await main();
} catch (error) {
  await writeJson("summary.json", {
    status: "failed",
    runId,
    message: error instanceof Error ? error.message : String(error),
    runDir,
  });
  throw error;
}
