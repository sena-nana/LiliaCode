import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { runAgentTurn } from "../agent-runner/core.mjs";
import { createInteractionBroker } from "../agent-runner/interactions.mjs";
import { createProtocolEmitter } from "../agent-runner/protocol.mjs";
import { mapClaudeInitialPermission } from "../agent-runner/claude/permissions.mjs";
import {
  createCodexRunContext,
  mapCodexEventToNdjson,
  normalizeCodexAppServerEvent,
  normalizeCodexPlanSteps,
} from "../agent-runner/codex/timeline.mjs";

const testsDir = dirname(fileURLToPath(import.meta.url));
const runnerSource = readFileSync(join(testsDir, "..", "agent-runner.mjs"), "utf8");
const packageManifest = readFileSync(join(testsDir, "..", "package.json"), "utf8");

function captureProtocol() {
  const lines: string[] = [];
  const protocol = createProtocolEmitter({ write: (line) => lines.push(line.trimEnd()) });
  return {
    protocol,
    lines,
    json: () => lines.map((line) => JSON.parse(line)),
  };
}

describe("agent runner entry", () => {
  it("入口保持薄 CLI，真实实现从 runner core 进入", () => {
    expect(runnerSource).toContain("runAgentTurn");
    expect(runnerSource).toContain("createRunnerContext");
    expect(runnerSource).not.toContain("function runClaude");
    expect(runnerSource).not.toContain("function runCodex");
    expect(packageManifest).not.toContain("@openai/codex-sdk");
  });
});

describe("runner core", () => {
  it("缺失或空 prompt 时输出 error 且不进入 backend", async () => {
    const { protocol, json } = captureProtocol();
    const result = await runAgentTurn({}, {
      protocol,
      env: {},
      runClaude: async () => {
        throw new Error("should not run");
      },
    });

    expect(result).toEqual({ ok: false, exitCode: 1 });
    expect(json()).toEqual([{ type: "error", message: "missing prompt" }]);
  });

  it("按 backend 路由，并把附件路径注入 prompt", async () => {
    const { protocol } = captureProtocol();
    let seen: any = null;
    const result = await runAgentTurn({
      backend: "codex",
      prompt: "请读附件",
      attachments: [{
        path: "C:/tmp/a.txt",
        name: "a.txt",
        kind: "file",
        mime: "text/plain",
        size: 2048,
      }, {
        path: "C:/tmp/src",
        name: "src",
        kind: "directory",
        size: null,
        directory: { fileCount: 3, directoryCount: 1, totalSize: 100, truncated: true },
      }],
    }, {
      protocol,
      env: {},
      runCodex: async (cmd: any) => {
        seen = cmd;
      },
      runClaude: async () => {
        throw new Error("wrong backend");
      },
    });

    expect(result.ok).toBe(true);
    expect(seen.prompt).toContain("用户随本轮消息附加的本地路径");
    expect(seen.prompt).toContain("C:/tmp/a.txt");
    expect(seen.prompt).toContain("file, text/plain, 2 KB");
    expect(seen.prompt).toContain("directory, unknown size, 3 files, 1 dirs, truncated");
    expect(seen.prompt).toContain("不要假设已经读取了内容");
  });

  it("dry-run 分支不启动真实后端", async () => {
    const { protocol, json } = captureProtocol();
    const result = await runAgentTurn({ backend: "claude", prompt: "hi" }, {
      protocol,
      env: { LILIA_AGENT_DRY_RUN: "1" },
      runDryRun: async (cmd: any, context: any) => {
        context.protocol.emit({ type: "done", sessionId: `dry-${cmd.backend}` });
      },
      runClaude: async () => {
        throw new Error("should not run");
      },
    });

    expect(result.ok).toBe(true);
    expect(json()).toEqual([{ type: "done", sessionId: "dry-claude" }]);
  });
});

describe("protocol emitter", () => {
  it("timeline payload 只保留事实字段并能安全 JSON 化", () => {
    const { protocol, json } = captureProtocol();
    const circular: any = { ok: true, taskId: "drop", nested: { turn_id: "drop" } };
    circular.self = circular;
    protocol.emitTimeline({
      kind: "tool",
      status: "running",
      title: "Tool",
      summary: "summary",
      payload: {
        circular,
        value: 1n,
        error: new Error("boom"),
      },
      sourceId: "s1",
    });

    const event = json()[0].event;
    expect(event).toMatchObject({
      kind: "tool",
      status: "running",
      title: "Tool",
      summary: "summary",
      sourceId: "s1",
    });
    expect(event.payload.circular.taskId).toBeUndefined();
    expect(event.payload.circular.nested.turn_id).toBeUndefined();
    expect(event.payload.circular.self).toBe("[Circular]");
    expect(event.payload.value).toBe("1");
    expect(event.payload.error.message).toBe("boom");
  });
});

describe("interaction broker", () => {
  it("关联 consent/ask-user 请求与 stdin 响应", async () => {
    const { protocol, json } = captureProtocol();
    const timelineCalls: any[] = [];
    const broker = createInteractionBroker({
      protocol,
      emitToolConsentTimeline: (...args: any[]) => timelineCalls.push(["consent", ...args]),
      emitAskUserTimeline: (...args: any[]) => timelineCalls.push(["ask", ...args]),
    });

    const consent = broker.requestUserConsent({ toolName: "Bash", input: { command: "pwd" } });
    expect(json()[0]).toMatchObject({
      type: "consent_request",
      id: "consent-1",
      toolName: "Bash",
    });
    broker.handleControlLine(JSON.stringify({
      type: "consent_response",
      id: "consent-1",
      decision: "allow",
      message: "ok",
      updatedInput: { command: "ls" },
    }));
    expect(await consent).toMatchObject({
      id: "consent-1",
      decision: "allow",
      updatedInput: { command: "ls" },
    });

    const ask = broker.requestAskUser({
      title: "Confirm",
      questions: [{ id: "q1", question: "Go?", options: [] }],
    });
    broker.handleControlLine("not json");
    broker.handleControlLine(JSON.stringify({ type: "ask_user_response", id: "missing" }));
    broker.handleControlLine(JSON.stringify({
      type: "ask_user_response",
      id: "ask-1",
      result: { answers: { q1: { value: "yes" } } },
    }));
    expect(await ask).toMatchObject({
      cancelled: false,
      answers: { q1: { value: "yes" } },
    });
    expect(timelineCalls.map((call) => call[0])).toEqual(["consent", "ask", "ask"]);
  });
});

describe("Claude helpers", () => {
  it("plan mode 初始进入 Claude plan，确认后恢复原执行权限映射", () => {
    expect(mapClaudeInitialPermission("ask", true).permissionMode).toBe("plan");
    expect(mapClaudeInitialPermission("readonly", false).permissionMode).toBe("default");
    expect(mapClaudeInitialPermission("full", false)).toMatchObject({
      permissionMode: "bypassPermissions",
      allowDangerouslySkipPermissions: true,
    });
  });
});

describe("Codex app-server mapping", () => {
  it("normalizes app-server turn and plan events", () => {
    expect(normalizeCodexAppServerEvent({
      method: "turn/completed",
      params: { turn: { status: "failed", error: { message: "bad" } } },
    })).toMatchObject({
      type: "turn.failed",
      error: { message: "bad" },
    });
    expect(normalizeCodexPlanSteps([
      { step: "读代码", status: "completed" },
      { text: "写测试", status: "pending" },
    ])).toEqual([
      { text: "读代码", completed: true, status: "completed" },
      { text: "写测试", completed: false, status: "pending" },
    ]);
  });

  it("maps Codex item events to stable NDJSON/timeline facts", () => {
    const { protocol, json } = captureProtocol();
    const ctx: any = createCodexRunContext({ permission: "ask" }, protocol, "thread-1");

    mapCodexEventToNdjson({
      type: "item.started",
      item: {
        id: "cmd-1",
        type: "commandExecution",
        command: "yarn test",
      },
    }, ctx);

    expect(json()).toEqual([
      {
        type: "timeline",
        event: expect.objectContaining({
          kind: "command",
          status: "started",
          title: "yarn test",
          summary: "yarn test",
          sourceId: "cmd-1",
        }),
      },
      {
        type: "tool_use",
        name: "commandExecution",
        input: { id: "cmd-1", command: "yarn test" },
      },
    ]);
  });
});
