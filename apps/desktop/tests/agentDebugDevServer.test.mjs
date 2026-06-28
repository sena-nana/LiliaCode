import { describe, expect, it } from "vitest";
import {
  createAgentDebugChildEnv,
  createAgentDebugDevServerPlan,
  devUrlForPort,
  findAvailablePort,
  parsePort,
} from "../agent-debug/dev-server.mjs";

describe("agent debug dev server planning", () => {
  it("skips a busy default port instead of reusing another dev server", async () => {
    const checked = [];
    const port = await findAvailablePort(1420, {
      isPortAvailableFn: async (candidate) => {
        checked.push(candidate);
        return candidate === 1421;
      },
    });

    expect(port).toBe(1421);
    expect(checked).toEqual([1420, 1421]);
  });

  it("creates a dynamic dev URL from the first available port", async () => {
    const plan = await createAgentDebugDevServerPlan(
      {},
      { findAvailablePortFn: async (startPort) => startPort + 2 },
    );

    expect(plan).toMatchObject({
      devUrl: "http://localhost:1422",
      explicit: false,
      port: 1422,
      reason: "available-port",
    });
  });

  it("keeps an explicit dev URL and propagates it to child process env", async () => {
    const plan = await createAgentDebugDevServerPlan({
      LILIA_AGENT_DEBUG_DEV_URL: "http://localhost:1515",
    });
    const env = createAgentDebugChildEnv({ PATH: "base-path" }, plan.devUrl, plan.port);

    expect(plan).toMatchObject({
      devUrl: "http://localhost:1515",
      explicit: true,
      port: 1515,
      reason: "env:LILIA_AGENT_DEBUG_DEV_URL",
    });
    expect(env).toMatchObject({
      PATH: "base-path",
      LILIA_AGENT_DEBUG: "1",
      LILIA_AGENT_DEBUG_DEV_URL: "http://localhost:1515",
      LILIA_DEV_PORT: "1515",
      LILIA_DEV_STRICT_PORT: "1",
      VITE_LILIA_AGENT_DEBUG: "1",
    });
  });

  it("normalizes invalid ports to the default dev port", () => {
    expect(parsePort("not-a-port")).toBe(1420);
    expect(devUrlForPort(1530)).toBe("http://localhost:1530");
  });
});

