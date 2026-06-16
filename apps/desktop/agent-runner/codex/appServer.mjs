import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createInterface } from "node:readline";
import { stringOrNull } from "../utils.mjs";

export function codexAppServerBinary(env = process.env) {
  const injected = stringOrNull(env.LILIA_CODEX_CLI_PATH)?.trim();
  if (!injected) {
    throw new Error(
      "Codex app-server 环境不满足：Lilia 未找到满足协议要求的 codex CLI。请升级 Codex CLI 到 0.128.0 或更新版本后重新检测。",
    );
  }
  return injected;
}

export function isWindowsCommandScript(binary, platform = process.platform) {
  return platform === "win32" && /\.(cmd|bat)$/i.test(binary);
}

export function resolveWindowsCommandScript(
  binary,
  {
    platform = process.platform,
    fileExists = existsSync,
  } = {},
) {
  if (platform !== "win32") return null;
  if (isWindowsCommandScript(binary, platform)) return binary;
  if (/\.[^\\/]+$/i.test(binary)) return null;
  for (const candidate of [`${binary}.cmd`, `${binary}.bat`]) {
    if (fileExists(candidate)) return candidate;
  }
  return null;
}

export function windowsCommandLineToken(value) {
  const text = String(value);
  if (!/[\s"&|<>^]/.test(text)) return text;
  return `"${text.replace(/"/g, '\\"')}"`;
}

export function windowsCommandLine(binary, args) {
  return [binary, ...args].map(windowsCommandLineToken).join(" ");
}

export function codexAppServerSpawnCommand(
  binary,
  {
    env = process.env,
    platform = process.platform,
    fileExists = existsSync,
  } = {},
) {
  const commandScript = resolveWindowsCommandScript(binary, { platform, fileExists });
  if (!commandScript) return { command: binary, args: ["app-server"] };
  return {
    command: env.ComSpec || "cmd.exe",
    args: [
      "/d",
      "/s",
      "/c",
      windowsCommandLine(commandScript, ["app-server"]),
    ],
  };
}

export function spawnCodexAppServer(binary, options) {
  const { command, args } = codexAppServerSpawnCommand(binary);
  return spawn(command, args, options);
}

export function codexAppServerExitError(stderr, code, signal) {
  const status = signal ? `signal ${signal}` : `code ${code ?? "unknown"}`;
  const detail = stderr.trim();
  if (detail) return new Error(`Codex app-server exited (${status}): ${detail}`);
  return new Error(
    `Codex app-server exited (${status})，但没有输出 stderr；请检查 Codex CLI 配置或认证状态。`,
  );
}

export function codexAppServerClosedError() {
  return new Error("Codex app-server request was cancelled because Lilia closed the app-server.");
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
  let closeRequested = false;
  let closeTimer = null;
  let closeForceKillMs = 30_000;
  let shutdownStarted = false;
  child.stderr?.on("data", (chunk) => {
    stderr += chunk.toString("utf8");
  });
  child.once("error", (err) => {
    closed = true;
    clearCloseTimer();
    for (const { reject } of pending.values()) {
      reject(err);
    }
    pending.clear();
  });
  child.once("exit", (code, signal) => {
    closed = true;
    if (closeRequested) {
      return;
    }
    for (const { reject } of pending.values()) {
      reject(codexAppServerExitError(stderr, code, signal));
    }
    pending.clear();
  });
  child.once("close", () => {
    closed = true;
    clearCloseTimer();
    if (closeRequested) rejectPendingRequests();
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
      if (closeRequested && pending.size === 0) shutdown({ forceKillMs: closeForceKillMs });
      return;
    }
    notifications.push(msg);
  });
  function request(method, params = null) {
    if (closed || !child.stdin) {
      return Promise.reject(new Error("Codex app-server is not running"));
    }
    const id = seq++;
    const payload = { method, id, params: params ?? null };
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

  function clearCloseTimer() {
    if (!closeTimer) return;
    clearTimeout(closeTimer);
    closeTimer = null;
  }

  function rejectPendingRequests() {
    for (const { reject } of pending.values()) {
      reject(codexAppServerClosedError());
    }
    pending.clear();
  }

  function shutdown({ forceKillMs = 30_000, cancelPending = false } = {}) {
    if (shutdownStarted) return;
    shutdownStarted = true;
    closeRequested = true;
    closed = true;
    clearCloseTimer();
    if (cancelPending) rejectPendingRequests();
    try {
      child.stdin?.end?.();
    } catch {
      // noop
    }
    if (forceKillMs <= 0) {
      try {
        child.kill();
      } catch {
        // noop
      }
      return;
    }
    closeTimer = setTimeout(() => {
      if (pending.size > 0) rejectPendingRequests();
      try {
        child.kill();
      } catch {
        // noop
      }
    }, forceKillMs);
    closeTimer.unref?.();
  }

  function close({ forceKillMs = 30_000 } = {}) {
    if (closeRequested) return;
    closeRequested = true;
    closed = true;
    closeForceKillMs = forceKillMs;
    if (pending.size === 0 || forceKillMs <= 0) {
      shutdown({ forceKillMs, cancelPending: pending.size > 0 });
    } else {
      closeTimer = setTimeout(() => {
        shutdown({ forceKillMs: 0, cancelPending: true });
      }, forceKillMs);
      closeTimer.unref?.();
    }
  }
  return { binary, child, request, notify, respond, drainNotifications, close };
}
