import { spawn } from "node:child_process";
import net from "node:net";
import { fileURLToPath } from "node:url";

const DEFAULT_PORT = 1420;
const LOCALHOST_CHECK_HOSTS = ["127.0.0.1", "::1"];

function parsePort(value) {
  if (!value) return DEFAULT_PORT;
  const port = Number.parseInt(value, 10);
  if (Number.isInteger(port) && port > 0 && port < 65536) return port;
  return DEFAULT_PORT;
}

function canListen(host, port) {
  return new Promise((resolve) => {
    const server = net.createServer();

    server.once("error", (error) => {
      resolve(error.code === "EAFNOSUPPORT" || error.code === "EADDRNOTAVAIL");
    });
    server.once("listening", () => {
      server.close(() => resolve(true));
    });
    server.listen({ host, port });
  });
}

async function isPortAvailable(port) {
  for (const host of LOCALHOST_CHECK_HOSTS) {
    if (!(await canListen(host, port))) return false;
  }
  return true;
}

async function findAvailablePort(startPort) {
  for (let port = startPort; port < 65536; port += 1) {
    if (await isPortAvailable(port)) return port;
  }

  throw new Error(`No available localhost port found from ${startPort}.`);
}

function yarnSpawn(args) {
  if (process.platform === "win32") {
    return {
      command: "cmd.exe",
      args: ["/d", "/s", "/c", "yarn.cmd", ...args],
    };
  }

  return {
    command: "yarn",
    args,
  };
}

const startPort = parsePort(process.env.LILIA_DEV_PORT);
const port = await findAvailablePort(startPort);
const devUrl = `http://localhost:${port}`;
const config = JSON.stringify({
  build: {
    devUrl,
  },
});
const args = [
  "--cwd",
  "apps/desktop",
  "tauri",
  "dev",
  "--config",
  config,
  ...process.argv.slice(2),
];
const env = {
  ...process.env,
  LILIA_DEV_PORT: String(port),
  LILIA_DEV_STRICT_PORT: "1",
};

if (process.env.LILIA_TAURI_DEV_DRY_RUN === "1") {
  console.log(
    JSON.stringify({
      args,
      devUrl,
      env: {
        LILIA_DEV_PORT: env.LILIA_DEV_PORT,
        LILIA_DEV_STRICT_PORT: env.LILIA_DEV_STRICT_PORT,
      },
    }),
  );
  process.exit(0);
}

console.log(`[lilia] Starting Tauri dev server at ${devUrl}`);

const yarn = yarnSpawn(args);
const child = spawn(yarn.command, yarn.args, {
  cwd: fileURLToPath(new URL("..", import.meta.url)),
  env,
  shell: false,
  stdio: "inherit",
});

child.on("error", (error) => {
  console.error(`[lilia] Failed to start Tauri dev: ${error.message}`);
  process.exitCode = 1;
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exitCode = code ?? 1;
});
