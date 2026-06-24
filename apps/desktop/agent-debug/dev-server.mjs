import net from "node:net";

export const DEFAULT_AGENT_DEBUG_DEV_PORT = 1420;
export const LOCALHOST_CHECK_HOSTS = ["127.0.0.1", "::1"];

export function parsePort(value, fallback = DEFAULT_AGENT_DEBUG_DEV_PORT) {
  if (!value) return fallback;
  const port = Number.parseInt(value, 10);
  return Number.isInteger(port) && port > 0 && port < 65536 ? port : fallback;
}

export function parsePortFromUrl(value) {
  try {
    const url = new URL(value);
    return url.port ? parsePort(url.port, null) : null;
  } catch {
    return null;
  }
}

export function devUrlForPort(port) {
  return `http://localhost:${port}`;
}

export function canListen(host, port) {
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

export async function isPortAvailable(
  port,
  { hosts = LOCALHOST_CHECK_HOSTS, canListenFn = canListen } = {},
) {
  for (const host of hosts) {
    if (!(await canListenFn(host, port))) return false;
  }
  return true;
}

export async function findAvailablePort(
  startPort,
  { isPortAvailableFn = isPortAvailable } = {},
) {
  for (let port = startPort; port < 65536; port += 1) {
    if (await isPortAvailableFn(port)) return port;
  }
  throw new Error(`No available localhost port found from ${startPort}.`);
}

export async function createAgentDebugDevServerPlan(
  env = process.env,
  { findAvailablePortFn = findAvailablePort } = {},
) {
  const explicitDevUrl = env.LILIA_AGENT_DEBUG_DEV_URL;
  if (explicitDevUrl) {
    const port = parsePortFromUrl(explicitDevUrl) ??
      parsePort(env.LILIA_AGENT_DEBUG_DEV_PORT ?? env.LILIA_DEV_PORT);
    return {
      devUrl: explicitDevUrl,
      explicit: true,
      port,
      reason: "env:LILIA_AGENT_DEBUG_DEV_URL",
    };
  }

  const startPort = parsePort(env.LILIA_AGENT_DEBUG_DEV_PORT ?? env.LILIA_DEV_PORT);
  const port = await findAvailablePortFn(startPort);
  return {
    devUrl: devUrlForPort(port),
    explicit: false,
    port,
    reason: port === startPort ? "default-port" : "available-port",
  };
}

export function createAgentDebugChildEnv(baseEnv, devUrl, port) {
  return {
    ...baseEnv,
    LILIA_AGENT_DEBUG: "1",
    LILIA_AGENT_DEBUG_DEV_URL: devUrl,
    LILIA_DEV_PORT: String(port),
    LILIA_DEV_STRICT_PORT: "1",
    VITE_LILIA_AGENT_DEBUG: "1",
  };
}
