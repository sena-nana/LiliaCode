import { spawn, spawnSync } from "node:child_process";
import { copyFile, mkdir, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const runsRoot = path.join(repoRoot, "agent-debug-runs");
const runId = new Date().toISOString().replace(/[:.]/g, "-");
const runDir = path.join(runsRoot, runId);
const driverPort = Number.parseInt(process.env.LILIA_AGENT_DEBUG_DRIVER_PORT ?? "4444", 10);
const driverUrl = `http://127.0.0.1:${driverPort}`;
const devUrl = process.env.LILIA_AGENT_DEBUG_DEV_URL ?? "http://localhost:1420";
const appBinary = process.env.LILIA_AGENT_DEBUG_APP ??
  path.join(repoRoot, "apps", "desktop", "src-tauri", "target", "debug", process.platform === "win32" ? "lilia.exe" : "lilia");
const toolDir = process.env.LILIA_AGENT_DEBUG_TOOL_DIR ??
  path.join(os.homedir(), ".lilia", "agent-debug", "bin");
const replay = [];

function commandExists(command, args = ["--version"]) {
  const result = spawnSync(command, args, { stdio: "ignore" });
  return result.status === 0;
}

function prependPath(dir) {
  const delimiter = process.platform === "win32" ? ";" : ":";
  const current = process.env.PATH ?? "";
  if (current.split(delimiter).some((entry) => path.resolve(entry) === path.resolve(dir))) return;
  process.env.PATH = `${dir}${delimiter}${current}`;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    ...options,
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed: ${result.stderr || result.stdout || result.error?.message || result.status}`);
  }
  return result.stdout.trim();
}

function runPowerShell(script) {
  return run("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script]);
}

async function downloadFile(url, target) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Download failed ${response.status}: ${url}`);
  }
  await writeFile(target, Buffer.from(await response.arrayBuffer()));
}

function parseVersion(text) {
  return text.match(/\d+\.\d+\.\d+\.\d+/)?.[0] ?? null;
}

function versionPrefix(version) {
  return version.split(".").slice(0, 3).join(".");
}

function edgeDriverVersion() {
  const result = spawnSync("msedgedriver", ["--version"], { encoding: "utf8" });
  if (result.status !== 0) return null;
  return parseVersion(`${result.stdout}\n${result.stderr}`);
}

function edgeVersion() {
  if (process.env.LILIA_AGENT_DEBUG_EDGE_VERSION) return process.env.LILIA_AGENT_DEBUG_EDGE_VERSION;
  if (process.platform !== "win32") return null;
  const script = [
    "$paths = @(",
    "'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',",
    "'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe'",
    ");",
    "foreach ($path in $paths) {",
    "  $item = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue;",
    "  if ($item) { $item.VersionInfo.ProductVersion; exit 0 }",
    "}",
    "exit 1",
  ].join(" ");
  try {
    return parseVersion(runPowerShell(script));
  } catch {
    return null;
  }
}

async function installEdgeDriver(version, setup) {
  if (process.platform !== "win32") {
    throw new Error("Automatic EdgeDriver install is currently supported on Windows only.");
  }
  await mkdir(toolDir, { recursive: true });
  const tempDir = path.join(os.tmpdir(), `lilia-edgedriver-${version}-${Date.now()}`);
  const zipPath = path.join(tempDir, "edgedriver_win64.zip");
  const extractDir = path.join(tempDir, "extract");
  await mkdir(extractDir, { recursive: true });
  try {
    const url = `https://msedgedriver.microsoft.com/${version}/edgedriver_win64.zip`;
    await downloadFile(url, zipPath);
    runPowerShell(`Expand-Archive -LiteralPath '${zipPath.replace(/'/g, "''")}' -DestinationPath '${extractDir.replace(/'/g, "''")}' -Force`);
    const extracted = runPowerShell(`Get-ChildItem -Path '${extractDir.replace(/'/g, "''")}' -Recurse -Filter 'msedgedriver.exe' | Select-Object -First 1 -ExpandProperty FullName`);
    await copyFile(extracted, path.join(toolDir, "msedgedriver.exe"));
    prependPath(toolDir);
    setup.push(`installed EdgeDriver ${version}`);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function ensureAgentDebugTools() {
  const setup = [];
  const cargoBin = path.join(os.homedir(), ".cargo", "bin");
  prependPath(cargoBin);
  prependPath(toolDir);

  if (!commandExists("tauri-driver", ["--help"])) {
    if (!commandExists("cargo")) {
      throw new Error("cargo is required to auto-install tauri-driver.");
    }
    run("cargo", ["install", "tauri-driver"], { stdio: "pipe" });
    prependPath(cargoBin);
    setup.push("installed tauri-driver");
  }

  const browserVersion = edgeVersion();
  const driverVersion = edgeDriverVersion();
  const matchesEdge = browserVersion && driverVersion &&
    versionPrefix(browserVersion) === versionPrefix(driverVersion);
  if (!matchesEdge) {
    if (!browserVersion) {
      throw new Error("Microsoft Edge version could not be detected. Set LILIA_AGENT_DEBUG_EDGE_VERSION to install EdgeDriver automatically.");
    }
    await installEdgeDriver(browserVersion, setup);
  }

  return {
    setup,
    edgeVersion: browserVersion,
    edgeDriverVersion: edgeDriverVersion(),
    toolDir,
  };
}

async function writeJson(name, value) {
  const target = path.join(runDir, name);
  await writeFile(target, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  return target;
}

function request(method, pathname, body) {
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
          let json = null;
          try {
            json = text ? JSON.parse(text) : null;
          } catch {
            // Keep raw text in the error below.
          }
          if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) {
            resolve(json);
          } else {
            reject(new Error(`${method} ${pathname} failed: ${res.statusCode} ${text}`));
          }
        });
      },
    );
    req.on("error", reject);
    if (payload) req.write(payload);
    req.end();
  });
}

function getUrl(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      res.resume();
      res.on("end", () => {
        if (res.statusCode && res.statusCode >= 200 && res.statusCode < 400) {
          resolve();
        } else {
          reject(new Error(`GET ${url} failed: ${res.statusCode}`));
        }
      });
    });
    req.on("error", reject);
    req.setTimeout(1000, () => {
      req.destroy(new Error(`GET ${url} timed out`));
    });
  });
}

async function waitForUrl(url, timeoutMs, label, child = null, output = []) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (child && child.exitCode !== null) {
      throw new Error(`${label} exited before ready: ${output.join("").trim() || child.exitCode}`);
    }
    try {
      await getUrl(url);
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  throw new Error(`${label} did not become ready within ${timeoutMs}ms`);
}

async function isUrlReady(url) {
  try {
    await getUrl(url);
    return true;
  } catch {
    return false;
  }
}

function stopProcessTree(child) {
  if (!child || child.killed) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
    return;
  }
  child.kill();
}

function spawnYarn(args, options) {
  if (process.platform === "win32") {
    return spawn("cmd.exe", ["/d", "/s", "/c", "yarn", ...args], options);
  }
  return spawn("yarn", args, options);
}

async function waitForDriver() {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      await request("GET", "/status");
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
  }
  throw new Error("tauri-driver did not become ready within 15s");
}

async function execute(sessionId, script, args = []) {
  const result = await request("POST", `/session/${sessionId}/execute/sync`, { script, args });
  return result?.value;
}

async function waitForDebugApi(sessionId) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const ready = await execute(
      sessionId,
      "return Boolean(window.__liliaAgentDebug?.observe?.().enabled);",
    ).catch(() => false);
    if (ready) return;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error("window.__liliaAgentDebug did not become enabled within 30s");
}

async function waitForDebugUi(sessionId) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const ready = await execute(
      sessionId,
      `const observe = window.__liliaAgentDebug?.observe?.();
       return Boolean(observe?.elements?.some((element) =>
         element.id === "app.shell" ||
         element.id === "sidebar" ||
         element.id === "titlebar"
       ));`,
    ).catch(() => false);
    if (ready) return;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error("agent debug UI tree did not become ready within 30s");
}

async function screenshot(sessionId, name) {
  const result = await request("GET", `/session/${sessionId}/screenshot`);
  const target = path.join(runDir, name);
  await writeFile(target, Buffer.from(result.value, "base64"));
  return target;
}

async function main() {
  await mkdir(runDir, { recursive: true });
  let setupInfo = null;
  let setupError = null;
  try {
    setupInfo = await ensureAgentDebugTools();
  } catch (error) {
    setupError = error?.message ?? String(error);
  }
  const preflight = {
    runId,
    runDir,
    appBinary,
    devUrl,
    autoSetup: setupInfo?.setup ?? [],
    setupError,
    edgeVersion: setupInfo?.edgeVersion ?? null,
    edgeDriverVersion: setupInfo?.edgeDriverVersion ?? edgeDriverVersion(),
    toolDir,
    tauriDriver: commandExists("tauri-driver", ["--help"]),
    edgeDriver: commandExists("msedgedriver") || commandExists("MicrosoftWebDriver"),
    appBinaryExists: existsSync(appBinary),
  };
  await writeJson("preflight.json", preflight);
  if (!preflight.tauriDriver || !preflight.edgeDriver || !preflight.appBinaryExists) {
    await writeJson("summary.json", {
      status: "blocked",
      reason: setupError ?? "Missing tauri-driver, EdgeDriver, or debug app binary.",
      preflight,
      nextStep: "Fix the preflight error, build the debug desktop binary, or set LILIA_AGENT_DEBUG_APP.",
    });
    process.exitCode = 2;
    return;
  }

  let devServer = null;
  const devServerOutput = [];
  let driver = null;
  const driverOutput = [];
  let sessionId = null;
  try {
    if (!(await isUrlReady(devUrl))) {
      devServer = spawnYarn(["--cwd", "apps/desktop", "dev", "--host", "127.0.0.1"], {
        cwd: repoRoot,
        stdio: ["ignore", "pipe", "pipe"],
        env: {
          ...process.env,
          LILIA_DEV_STRICT_PORT: "1",
          VITE_LILIA_AGENT_DEBUG: "1",
        },
      });
      devServer.stdout.on("data", (chunk) => devServerOutput.push(chunk.toString("utf8")));
      devServer.stderr.on("data", (chunk) => devServerOutput.push(chunk.toString("utf8")));
      await waitForUrl(devUrl, 30_000, "Vite dev server", devServer, devServerOutput);
    }

    driver = spawn("tauri-driver", ["--port", String(driverPort)], {
      stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...process.env,
        LILIA_AGENT_DEBUG: "1",
        MSEDGEDRIVER_TELEMETRY_OPTOUT: "1",
        VITE_LILIA_AGENT_DEBUG: "1",
      },
    });
    driver.stdout.on("data", (chunk) => driverOutput.push(chunk.toString("utf8")));
    driver.stderr.on("data", (chunk) => driverOutput.push(chunk.toString("utf8")));

    await waitForDriver();
    const session = await request("POST", "/session", {
      capabilities: {
        alwaysMatch: {
          browserName: "wry",
          "tauri:options": {
            application: appBinary,
            args: [],
            env: {
              LILIA_AGENT_DEBUG: "1",
              VITE_LILIA_AGENT_DEBUG: "1",
            },
          },
        },
      },
    });
    sessionId = session.value.sessionId;
    await waitForDebugApi(sessionId);
    await waitForDebugUi(sessionId);
    const beforeScreenshotPath = await screenshot(sessionId, "before.png");
    const observe = await execute(sessionId, "return window.__liliaAgentDebug?.observe?.() ?? null;");
    if (!observe?.enabled) {
      throw new Error("window.__liliaAgentDebug is not enabled in the app session");
    }
    await writeJson("observe.json", observe);
    if (!observe.elements.some((element) => element.id === "sidebar.new-chat" || element.id === "app.shell")) {
      throw new Error("Expected sidebar.new-chat or app.shell in agent debug element tree");
    }
    await writeJson("missing-agent-ids.json", observe.missingAgentIds ?? []);
    if (observe.missingAgentIds?.length) {
      throw new Error(`Visible interactive elements without data-agent-id: ${observe.missingAgentIds.length}`);
    }
    replay.push({ type: "observe", route: observe.route, capturedAt: observe.capturedAt });
    const typeResult = await execute(
      sessionId,
      "return window.__liliaAgentDebug.act({ type: 'mark', label: 'verify-agent-debug-smoke', data: { source: 'verify:agent-debug' } });",
    );
    replay.push({ type: "mark", label: "verify-agent-debug-smoke" });
    await writeJson("after-mark-observe.json", typeResult);
    const missingTargetError = await execute(
      sessionId,
      "return window.__liliaAgentDebug.act({ type: 'click', target: 'missing.debug.target' }).then(() => null, err => String(err && err.message || err));",
    );
    if (!String(missingTargetError).includes("missing.debug.target")) {
      throw new Error("Missing target scenario did not return a useful diagnostic");
    }
    replay.push({ type: "click", target: "missing.debug.target", expectedError: true });
    const logs = await execute(sessionId, "return window.__liliaAgentDebug?.observe?.().invokes ?? [];");
    const afterScreenshotPath = await screenshot(sessionId, "after.png");
    await writeJson("logs.json", logs);
    await writeJson("replay.json", replay);
    await writeJson("summary.json", {
      status: "passed",
      runId,
      preflight,
      beforeScreenshotPath,
      afterScreenshotPath,
      observePath: path.join(runDir, "observe.json"),
      logsPath: path.join(runDir, "logs.json"),
      replayPath: path.join(runDir, "replay.json"),
    });
  } catch (error) {
    let failureDiagnosticsPath = null;
    let failureScreenshotPath = null;
    if (sessionId) {
      failureScreenshotPath = await screenshot(sessionId, "failure.png").catch(() => null);
      const diagnostics = await execute(
        sessionId,
        `return {
          location: window.location.href,
          title: document.title,
          readyState: document.readyState,
          bodyText: document.body?.innerText?.slice(0, 2000) ?? "",
          hasAgentDebug: Boolean(window.__liliaAgentDebug),
        };`,
      ).catch((diagnosticError) => ({ diagnosticError: String(diagnosticError?.message ?? diagnosticError) }));
      failureDiagnosticsPath = await writeJson("failure-diagnostics.json", diagnostics).catch(() => null);
    }
    await writeJson("summary.json", {
      status: "failed",
      runId,
      preflight,
      message: error?.message ?? String(error),
      driverOutput,
      devServerOutput,
      failureScreenshotPath,
      failureDiagnosticsPath,
      replay,
    });
    process.exitCode = 1;
  } finally {
    if (sessionId) {
      await request("DELETE", `/session/${sessionId}`).catch(() => undefined);
    }
    driver?.kill();
    stopProcessTree(devServer);
    await writeFile(path.join(runDir, "tauri-driver.log"), driverOutput.join(""), "utf8");
    await writeFile(path.join(runDir, "dev-server.log"), devServerOutput.join(""), "utf8");
  }
}

await main();
