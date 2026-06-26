import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  AgentDebugRunArtifacts,
  SmokeBlockedError,
  writeRunSummary,
} from "../agent-debug/verify-agent-debug.mjs";

async function withRunDir(fn) {
  const runDir = await mkdtemp(path.join(os.tmpdir(), "lilia-agent-debug-artifacts-"));
  try {
    return await fn(runDir);
  } finally {
    await rm(runDir, { recursive: true, force: true });
  }
}

async function readJson(runDir, name) {
  return JSON.parse(await readFile(path.join(runDir, name), "utf8"));
}

describe("agent debug run artifacts", () => {
  it("writes stable artifact paths for preflight blocked runs", async () => {
    await withRunDir(async (runDir) => {
      const artifacts = new AgentDebugRunArtifacts({ runDir, runId: "blocked-run" });
      const preflight = { tauriDriver: false, edgeDriver: true, appBinaryExists: false };

      const summary = await writeRunSummary(artifacts, {
        status: "blocked",
        preflight,
        reason: "missing debug binary",
        nextStep: "Fix the preflight error, build the debug desktop binary, or set LILIA_AGENT_DEBUG_APP.",
      });

      expect(summary).toMatchObject({
        status: "blocked",
        runId: "blocked-run",
        reason: "missing debug binary",
        preflight,
        failureScreenshotPath: null,
        screenshots: {
          before: null,
          after: null,
          failure: null,
        },
        scenarios: [],
        replay: [],
      });
      expect(await readJson(runDir, "summary.json")).toEqual(summary);
      expect(await readJson(runDir, "logs.json")).toEqual([]);
      expect(await readJson(runDir, "replay.json")).toEqual([]);
      expect(await readJson(runDir, "scenario-results.json")).toEqual([]);
      expect(summary.logsPath).toBe(path.join(runDir, "logs.json"));
      expect(summary.scenarioResultsPath).toBe(path.join(runDir, "scenario-results.json"));
      expect(summary.driverLogPath).toBe(path.join(runDir, "tauri-driver.log"));
      expect(summary.devServerLogPath).toBe(path.join(runDir, "dev-server.log"));
    });
  });

  it("preserves provider-not-ready scenario screenshots in blocked summaries", async () => {
    await withRunDir(async (runDir) => {
      const artifacts = new AgentDebugRunArtifacts({ runDir, runId: "provider-blocked-run" });
      const scenario = {
        id: "ordinary-send",
        label: "普通发送",
        artifact: "scenario-ordinary-send.png",
      };
      const blocked = new SmokeBlockedError("Provider is not ready", {
        scenario: scenario.id,
        command: "chat_send_message",
      });

      artifacts.beginScenario(scenario);
      artifacts.appendScenarioResult(scenario, "blocked", {
        route: "/chats/debug",
        screenshotPath: path.join(runDir, scenario.artifact),
        reason: "provider is not ready for agent debug smoke",
        commandStatus: "error",
      });
      const summary = await writeRunSummary(artifacts, {
        status: "blocked",
        preflight: { tauriDriver: true, edgeDriver: true, appBinaryExists: true },
        reason: blocked.message,
        message: blocked.message,
        details: blocked.details,
        failureScreenshotPath: path.join(runDir, "failure.png"),
      });

      expect(summary.status).toBe("blocked");
      expect(summary.reason).toBe("Provider is not ready");
      expect(summary.details).toEqual(blocked.details);
      expect(summary.scenarios).toEqual([
        expect.objectContaining({
          id: scenario.id,
          status: "blocked",
          screenshotPath: path.join(runDir, scenario.artifact),
        }),
      ]);
      expect(await readJson(runDir, "scenario-results.json")).toEqual(summary.scenarios);
      expect(summary.screenshots.failure).toBe(path.join(runDir, "failure.png"));
    });
  });

  it("records the active scenario for partial-run failures", async () => {
    await withRunDir(async (runDir) => {
      const artifacts = new AgentDebugRunArtifacts({ runDir, runId: "partial-run" });
      const scenario = {
        id: "permission-pending-action",
        label: "permission pending action",
        artifact: "scenario-permission-pending-action.png",
      };
      const failureScreenshotPath = path.join(runDir, "failure.png");

      artifacts.beginScenario(scenario);
      artifacts.ensureActiveScenarioResult("failed", {
        route: "/chats/debug",
        screenshotPath: failureScreenshotPath,
        message: "enabled target did not become ready",
      });
      const summary = await writeRunSummary(artifacts, {
        status: "failed",
        preflight: { tauriDriver: true, edgeDriver: true, appBinaryExists: true },
        message: "enabled target did not become ready",
        logs: [{ command: "chat_send_message", status: "success" }],
        failureScreenshotPath,
      });

      expect(summary.status).toBe("failed");
      expect(summary.scenarios).toEqual([
        expect.objectContaining({
          id: scenario.id,
          status: "failed",
          route: "/chats/debug",
          screenshotPath: failureScreenshotPath,
        }),
      ]);
      expect(await readJson(runDir, "logs.json")).toEqual([
        { command: "chat_send_message", status: "success" },
      ]);
      expect(await readJson(runDir, "scenario-results.json")).toEqual(summary.scenarios);
    });
  });
});
