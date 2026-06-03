import { spawn, spawnSync } from "node:child_process";
import { createInterface } from "node:readline";
import { stringOrNull } from "../utils.mjs";

export function codexAppServerBinary(env = process.env, spawnSyncFn = spawnCodexCandidateSync) {
  const injected = stringOrNull(env.LILIA_CODEX_CLI_PATH)?.trim();
  if (!injected) {
    throw new Error(
      "Codex app-server 环境不满足：Lilia 未找到满足协议要求的 codex CLI。请升级 Codex CLI 到 0.128.0 或更新版本后重新检测。",
    );
  }
  const result = spawnSyncFn(injected, ["--version"], { stdio: "ignore" });
  if (result.error || result.status !== 0) {
    throw new Error(
      "Codex app-server 环境不满足：无法启动 Lilia 检测到的 codex CLI。请升级 Codex CLI 到 0.128.0 或更新版本后重新检测。",
    );
  }
  return injected;
}

export function isWindowsCommandScript(binary, platform = process.platform) {
  return platform === "win32" && /\.(cmd|bat)$/i.test(binary);
}

export function windowsCommandLineToken(value) {
  const text = String(value);
  if (!/[\s"&|<>^]/.test(text)) return text;
  return `"${text.replace(/"/g, '\\"')}"`;
}

export function windowsCommandLine(binary, args) {
  return [binary, ...args].map(windowsCommandLineToken).join(" ");
}

export function spawnCodexCandidateSync(binary, args, options = {}) {
  if (!isWindowsCommandScript(binary)) return spawnSync(binary, args, options);
  return spawnSync(process.env.ComSpec || "cmd.exe", [
    "/d",
    "/s",
    "/c",
    windowsCommandLine(binary, args),
  ], options);
}

export function spawnCodexAppServer(binary, options) {
  if (!isWindowsCommandScript(binary)) return spawn(binary, ["app-server"], options);
  return spawn(process.env.ComSpec || "cmd.exe", [
    "/d",
    "/s",
    "/c",
    windowsCommandLine(binary, ["app-server"]),
  ], options);
}

export function createCodexAppServer({
  env = process.env,
  resolveBinary = codexAppServerBinary,
  spawnServer = spawnCodexAppServer,
} = {}) {
  const binary = resolveBinary(env);
  const child = spawnServer(binary, {
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...env,
      ...(env.OPENAI_BASE_URL ? { OPENAI_BASE_URL: env.OPENAI_BASE_URL } : {}),
      ...(env.OPENAI_API_KEY ? { CODEX_API_KEY: env.OPENAI_API_KEY } : {}),
    },
  });
  const rl = createInterface({ input: child.stdout, crlfDelay: Infinity });
  const pending = new Map();
  const notifications = [];
  let seq = 1;
  let stderr = "";
  let closed = false;
  child.stderr?.on("data", (chunk) => {
    stderr += chunk.toString("utf8");
  });
  child.once("error", (err) => {
    closed = true;
    for (const { reject } of pending.values()) {
      reject(err);
    }
    pending.clear();
  });
  child.once("exit", () => {
    closed = true;
    for (const { reject } of pending.values()) {
      reject(new Error(`Codex app-server exited: ${stderr.trim()}`));
    }
    pending.clear();
  });
  rl.on("line", (line) => {
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (Object.prototype.hasOwnProperty.call(msg, "id")) {
      const entry = pending.get(msg.id);
      if (!entry) {
        notifications.push(msg);
        return;
      }
      pending.delete(msg.id);
      if (msg.error) entry.reject(new Error(msg.error.message || "Codex app-server request failed"));
      else entry.resolve(msg.result ?? null);
      return;
    }
    notifications.push(msg);
  });
  function request(method, params = {}) {
    if (closed || !child.stdin) {
      return Promise.reject(new Error("Codex app-server is not running"));
    }
    const id = seq++;
    const payload = { method, id, params };
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      child.stdin.write(`${JSON.stringify(payload)}\n`);
    });
  }
  function notify(method, params = {}) {
    if (!closed && child.stdin) child.stdin.write(`${JSON.stringify({ method, params })}\n`);
  }
  function respond(id, result) {
    if (!closed && child.stdin) child.stdin.write(`${JSON.stringify({ id, result })}\n`);
  }
  function drainNotifications() {
    return notifications.splice(0, notifications.length);
  }
  function close() {
    rl.close();
    try {
      child.kill();
    } catch {
      // noop
    }
  }
  return { binary, child, request, notify, respond, drainNotifications, close };
}
