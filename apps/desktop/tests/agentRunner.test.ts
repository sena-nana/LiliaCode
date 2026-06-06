import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { runAgentTurn } from "../agent-runner/core.mjs";
import { createInteractionBroker } from "../agent-runner/interactions.mjs";
import { createProtocolEmitter } from "../agent-runner/protocol.mjs";
import { mapClaudeInitialPermission } from "../agent-runner/claude/permissions.mjs";
import { maybeHandleCodexApprovalRequest } from "../agent-runner/codex/permissions.mjs";
import {
  createCodexRunContext,
  mapCodexEventToNdjson,
  normalizeCodexAppServerEvent,
  normalizeCodexPlanSteps,
} from "../agent-runner/codex/timeline.mjs";
import {
  buildCodexCollaborationMode,
  buildCodexPlanRevisionPrompt,
  readCodexPlanModePreset,
  runCodexAppServer,
  maybeHandleCodexServerRequest,
  startCodexAppServerThread,
  updateCodexThreadSettings,
} from "../agent-runner/codex/runCodex.mjs";

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

async function waitUntil(predicate: () => boolean, timeoutMs = 1000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  throw new Error("timed out waiting for condition");
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
      type: "interaction_request",
      id: "consent-1",
      kind: "tool_consent",
      payload: {
        toolName: "Bash",
      },
    });
    broker.handleControlLine(JSON.stringify({
      type: "interaction_response",
      id: "consent-1",
      kind: "tool_consent",
      result: {
      decision: "allow",
      message: "ok",
      updatedInput: { command: "ls" },
      },
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
    broker.handleControlLine(JSON.stringify({ type: "interaction_response", id: "missing" }));
    broker.handleControlLine(JSON.stringify({
      type: "interaction_response",
      id: "ask-1",
      kind: "ask_user",
      result: { answers: { q1: { value: "yes" } } },
    }));
    expect(await ask).toMatchObject({
      cancelled: false,
      answers: { q1: { value: "yes" } },
    });
    expect(timelineCalls.map((call) => call[0])).toEqual(["consent", "ask", "ask"]);
  });

  it("计划确认通过统一 interaction_request 发出", async () => {
    const { protocol, json } = captureProtocol();
    const broker = createInteractionBroker({
      protocol,
      emitToolConsentTimeline: () => {},
      emitAskUserTimeline: () => {},
    });

    const ask = broker.requestAskUser({
      intent: "plan_approval",
      title: "确认 Codex 计划",
      questions: [{ id: "approve-plan", question: "", mode: "confirm" }],
    }, { backend: "codex", emitTimelineEvent: false });

    expect(json()[0]).toMatchObject({
      type: "interaction_request",
      id: "ask-1",
      kind: "plan_approval",
      backend: "codex",
      payload: {
        title: "确认 Codex 计划",
      },
    });

    broker.handleControlLine(JSON.stringify({
      type: "interaction_response",
      id: "ask-1",
      kind: "plan_approval",
      result: {
        cancelled: false,
        answers: {
          "approve-plan": { questionId: "approve-plan", value: "yes" },
        },
      },
    }));

    await expect(ask).resolves.toMatchObject({ cancelled: false });
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
        cwd: "C:/repo",
        stdout: "ok",
        stderr: "",
        aggregatedOutput: "ok",
        exitCode: 0,
        durationMs: 123,
        approvalId: "approval-1",
      },
    }, ctx);

    const lines = json();
    expect(lines).toEqual([
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
        input: expect.objectContaining({ id: "cmd-1", command: "yarn test" }),
      },
    ]);
    expect(lines[0].event.payload).toMatchObject({
      command: "yarn test",
      cwd: "C:/repo",
      stdout: "ok",
      aggregatedOutput: "ok",
      exitCode: 0,
      durationMs: 123,
      approvalId: "approval-1",
    });
  });

  it("maps Codex file and MCP item payload fields without dropping tool facts", () => {
    const { protocol, json } = captureProtocol();
    const ctx: any = createCodexRunContext({ permission: "ask" }, protocol, "thread-1");

    mapCodexEventToNdjson({
      type: "item.completed",
      item: {
        id: "file-1",
        type: "fileChange",
        path: "src/App.vue",
        changes: [{ kind: "update", path: "src/App.vue" }],
        grantRoot: "C:/repo",
        output: "patched",
        error: "warning",
      },
    }, ctx);
    mapCodexEventToNdjson({
      type: "item.completed",
      item: {
        id: "mcp-1",
        type: "mcpToolCall",
        server: "docs",
        tool: "search",
        arguments: { query: "timeline" },
        input: { query: "timeline" },
        result: { hits: 1 },
        error: null,
      },
    }, ctx);

    const events = json().filter((line) => line.type === "timeline").map((line) => line.event);
    expect(events[0]).toMatchObject({
      kind: "file_change",
      payload: {
        path: "src/App.vue",
        changes: [{ kind: "update", path: "src/App.vue" }],
        grantRoot: "C:/repo",
        output: "patched",
        error: "warning",
      },
    });
    expect(events[1]).toMatchObject({
      kind: "mcp",
      payload: {
        server: "docs",
        tool: "search",
        arguments: { query: "timeline" },
        input: { query: "timeline" },
        result: { hits: 1 },
        error: null,
      },
    });
  });

  it("ignores malformed Codex approval requests without throwing", async () => {
    const calls: any[] = [];
    const handled = await maybeHandleCodexApprovalRequest(
      {
        respond: (...args: any[]) => calls.push(args),
      },
      { id: "bad-request", params: {} },
      {
        interactions: {
          requestUserConsent: async () => {
            throw new Error("should not ask");
          },
        },
        emitToolConsentTimeline: () => {},
      },
    );

    expect(handled).toBe(false);
    expect(calls).toEqual([]);
  });

  it("Codex Agent 提问通过统一 interaction_request/response 往返", async () => {
    const { protocol, json } = captureProtocol();
    const broker = createInteractionBroker({
      protocol,
      emitToolConsentTimeline: () => {},
      emitAskUserTimeline: () => {},
    });
    const calls: any[] = [];

    const handled = maybeHandleCodexServerRequest(
      {
        respond: (...args: any[]) => calls.push(["respond", ...args]),
      } as any,
      {
        id: "request-user-input-1",
        method: "item/tool/requestUserInput",
        params: {
          questions: [{
            id: "choice",
            header: "方案",
            question: "选哪个方案？",
            options: [
              { label: "A", description: "先改合约" },
              { label: "B", description: "先改 UI" },
            ],
          }],
        },
      },
      { interactions: broker } as any,
    );

    await waitUntil(() => json().some((line) => line.type === "interaction_request"));
    expect(json()).toEqual([
      {
        type: "interaction_request",
        id: "ask-1",
        kind: "ask_user",
        backend: "codex",
        payload: expect.objectContaining({
          title: "Codex 想确认一下",
          source: "Codex",
          questions: [expect.objectContaining({ id: "choice" })],
        }),
      },
    ]);

    broker.handleControlLine(JSON.stringify({
      type: "interaction_response",
      id: "ask-1",
      kind: "ask_user",
      result: {
        cancelled: false,
        answers: {
          choice: { questionId: "choice", value: "B" },
        },
      },
    }));

    await expect(handled).resolves.toBe(true);
    expect(calls).toEqual([
      ["respond", "request-user-input-1", { answers: { choice: { answers: ["B"] } } }],
    ]);
  });

  it("Codex 工具确认通过统一 interaction_request/response 往返", async () => {
    const { protocol, json } = captureProtocol();
    const broker = createInteractionBroker({
      protocol,
      emitToolConsentTimeline: () => {},
      emitAskUserTimeline: () => {},
    });
    const calls: any[] = [];

    const handled = maybeHandleCodexApprovalRequest(
      {
        respond: (...args: any[]) => calls.push(["respond", ...args]),
      },
      {
        id: "approval-rpc-unified",
        method: "item/commandExecution/requestApproval",
        params: {
          approvalId: "approval-unified",
          command: "yarn test",
          cwd: "C:/repo",
          availableDecisions: ["accept", "decline", "cancel"],
        },
      },
      {
        protocol,
        interactions: broker,
        emitToolConsentTimeline: () => {},
      },
    );

    await waitUntil(() => json().some((line) => line.type === "interaction_request"));
    expect(json()).toEqual([
      {
        type: "interaction_request",
        id: "consent-1",
        kind: "tool_consent",
        backend: "codex",
        payload: expect.objectContaining({
          backend: "codex",
          toolName: "item/commandExecution/requestApproval",
          input: expect.objectContaining({ command: "yarn test" }),
          toolUseID: "approval-unified",
        }),
      },
    ]);

    broker.handleControlLine(JSON.stringify({
      type: "interaction_response",
      id: "consent-1",
      kind: "tool_consent",
      result: {
        decision: "allow",
        message: "",
      },
    }));

    await expect(handled).resolves.toBe(true);
    expect(calls).toEqual([
      ["respond", "approval-rpc-unified", { decision: "accept" }],
    ]);
  });

  it("passes Codex approval fields through and honors codexDecision first", async () => {
    const calls: any[] = [];
    let seenPayload: any = null;
    const handled = await maybeHandleCodexApprovalRequest(
      {
        respond: (...args: any[]) => calls.push(["respond", ...args]),
      },
      {
        id: "approval-rpc-1",
        method: "item/fileChange/requestApproval",
        params: {
          approvalId: "approval-1",
          grantRoot: "C:/repo",
          additionalPermissions: [{ permission: "filesystem" }],
          availableDecisions: ["accept", "decline", "cancel"],
          proposedExecpolicyAmendment: { sandbox: "workspace-write" },
          proposedNetworkPolicyAmendments: [{ host: "example.com" }],
          networkApprovalContext: { host: "example.com" },
          cwd: "C:/repo",
          reason: "needs write",
          commandActions: [{ text: "ignored" }],
        },
      },
      {
        interactions: {
          requestUserConsent: async (payload: any) => {
            seenPayload = payload;
            return {
              id: "consent-1",
              decision: "deny",
              message: "use explicit",
              codexDecision: "cancel",
            };
          },
        },
        emitToolConsentTimeline: (...args: any[]) => calls.push(["timeline", ...args]),
      },
    );

    expect(handled).toBe(true);
    expect(seenPayload).toMatchObject({
      backend: "codex",
      toolName: "item/fileChange/requestApproval",
      additionalPermissions: [{ permission: "filesystem" }],
      availableDecisions: ["accept", "decline", "cancel"],
      proposedExecpolicyAmendment: { sandbox: "workspace-write" },
      proposedNetworkPolicyAmendments: [{ host: "example.com" }],
      networkApprovalContext: { host: "example.com" },
      cwd: "C:/repo",
      reason: "needs write",
      commandActions: [{ text: "ignored" }],
    });
    expect(calls[0]).toEqual(["respond", "approval-rpc-1", { decision: "cancel" }]);
    expect(calls[1]).toMatchObject([
      "timeline",
      "consent-1",
      seenPayload,
      "cancelled",
      "use explicit",
    ]);
  });

  it("uses Codex cancel when a denied approval cannot decline", async () => {
    const calls: any[] = [];
    const seenTimeline: any[] = [];
    const handled = await maybeHandleCodexApprovalRequest(
      {
        respond: (...args: any[]) => calls.push(["respond", ...args]),
      },
      {
        id: "approval-rpc-cancel-only",
        method: "item/fileChange/requestApproval",
        params: {
          approvalId: "approval-cancel-only",
          grantRoot: "C:/repo",
          availableDecisions: ["accept", "cancel"],
        },
      },
      {
        interactions: {
          requestUserConsent: async () => ({
            id: "consent-cancel-only",
            decision: "deny",
            message: "暂不授权",
          }),
        },
        emitToolConsentTimeline: (...args: any[]) => seenTimeline.push(args),
      },
    );

    expect(handled).toBe(true);
    expect(calls).toEqual([
      ["respond", "approval-rpc-cancel-only", { decision: "cancel" }],
    ]);
    expect(seenTimeline[0]).toMatchObject([
      "consent-cancel-only",
      expect.objectContaining({ availableDecisions: ["accept", "cancel"] }),
      "cancelled",
      "暂不授权",
    ]);
  });

  it("runs edited Codex command approvals through Lilia and steers the result back", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push(["request", method, params]);
        if (method === "command/exec") {
          return { exitCode: 0, stdout: "tests passed\n", stderr: "" };
        }
        if (method === "turn/steer") return {};
        throw new Error(`unexpected request ${method}`);
      },
      respond: (...args: any[]) => calls.push(["respond", ...args]),
      drainNotifications: () => [],
    };
    const consentTimeline: any[] = [];

    const handled = await maybeHandleCodexApprovalRequest(
      server as any,
      {
        id: "approval-rpc-2",
        method: "item/commandExecution/requestApproval",
        params: {
          approvalId: "approval-2",
          command: "yarn test",
          cwd: "C:/repo",
          availableDecisions: ["accept", "decline", "cancel"],
        },
      },
      {
        protocol,
        threadId: "thread-1",
        currentTurnId: "turn-1",
        interactions: {
          requestUserConsent: async () => ({
            id: "consent-2",
            decision: "allow",
            message: "",
            updatedInput: { command: "yarn test --runInBand" },
          }),
        },
        emitToolConsentTimeline: (...args: any[]) => consentTimeline.push(args),
      },
    );

    expect(handled).toBe(true);
    expect(calls).toEqual([
      ["request", "command/exec", { command: "yarn test --runInBand", cwd: "C:/repo" }],
      ["respond", "approval-rpc-2", { decision: "cancel" }],
      ["request", "turn/steer", expect.objectContaining({
        threadId: "thread-1",
        turnId: "turn-1",
        additionalContext: expect.stringContaining("yarn test --runInBand"),
      })],
    ]);
    expect(calls[2][2].additionalContext).toContain("退出码：0");
    expect(calls[2][2].additionalContext).toContain("tests passed");
    expect(json()).toEqual([
      {
        type: "timeline",
        event: expect.objectContaining({
          kind: "command",
          status: "success",
          title: "yarn test --runInBand",
          payload: expect.objectContaining({
            executionOwner: "lilia",
            subkind: "lilia_edit_exec",
            originalCommand: "yarn test",
            modifiedCommand: "yarn test --runInBand",
            exitCode: 0,
          }),
        }),
      },
    ]);
    expect(consentTimeline[0]).toMatchObject([
      "consent-2",
      expect.objectContaining({
        commandEdited: true,
        originalCommand: "yarn test",
        modifiedCommand: "yarn test --runInBand",
      }),
      "success",
      "Lilia 已执行用户修改后的 Codex 命令",
    ]);
  });

  it("falls back to Codex process/spawn when command/exec is unavailable", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    let drained = false;
    const server = {
      request: async (method: string, params: any) => {
        calls.push(["request", method, params]);
        if (method === "command/exec") throw new Error("method not found");
        if (method === "process/spawn") return { processId: "proc-1" };
        if (method === "turn/steer") return {};
        throw new Error(`unexpected request ${method}`);
      },
      respond: (...args: any[]) => calls.push(["respond", ...args]),
      drainNotifications: () => {
        if (drained) return [];
        drained = true;
        return [
          {
            method: "process/outputDelta",
            params: { processId: "proc-1", stream: "stdout", delta: "ok\n" },
          },
          {
            method: "process/exited",
            params: { processId: "proc-1", exitCode: 0 },
          },
        ];
      },
    };

    await maybeHandleCodexApprovalRequest(
      server as any,
      {
        id: "approval-rpc-3",
        method: "item/commandExecution/requestApproval",
        params: {
          approvalId: "approval-3",
          command: "npm test",
          availableDecisions: ["accept", "decline"],
        },
      },
      {
        protocol,
        threadId: "thread-1",
        currentTurnId: "turn-1",
        interactions: {
          requestUserConsent: async () => ({
            id: "consent-3",
            decision: "allow",
            updatedInput: { command: "npm test -- --watch=false" },
          }),
        },
        emitToolConsentTimeline: () => {},
      },
    );

    expect(calls.map((call) => call[1])).toEqual([
      "command/exec",
      "process/spawn",
      "approval-rpc-3",
      "turn/steer",
    ]);
    expect(calls[2]).toEqual(["respond", "approval-rpc-3", { decision: "decline" }]);
    expect(json()[0].event.payload).toMatchObject({
      executionOwner: "lilia",
      modifiedCommand: "npm test -- --watch=false",
      output: "ok\n",
    });
  });

  it("emits an error timeline and declines the original Codex command when Lilia cannot execute the edit", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    const consentTimeline: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push(["request", method, params]);
        throw new Error(`${method} unavailable`);
      },
      respond: (...args: any[]) => calls.push(["respond", ...args]),
      drainNotifications: () => [],
    };

    await maybeHandleCodexApprovalRequest(
      server as any,
      {
        id: "approval-rpc-4",
        method: "item/commandExecution/requestApproval",
        params: {
          approvalId: "approval-4",
          command: "pnpm test",
          availableDecisions: ["accept", "decline"],
        },
      },
      {
        protocol,
        threadId: "thread-1",
        currentTurnId: "turn-1",
        interactions: {
          requestUserConsent: async () => ({
            id: "consent-4",
            decision: "allow",
            updatedInput: { command: "pnpm test --changed" },
          }),
        },
        emitToolConsentTimeline: (...args: any[]) => consentTimeline.push(args),
      },
    );

    expect(calls).toEqual([
      ["request", "command/exec", { command: "pnpm test --changed", cwd: undefined }],
      ["request", "process/spawn", { command: "pnpm test --changed", cwd: undefined }],
      ["respond", "approval-rpc-4", { decision: "decline" }],
    ]);
    expect(json()).toEqual([
      {
        type: "timeline",
        event: expect.objectContaining({
          kind: "command",
          status: "error",
          payload: expect.objectContaining({
            executionOwner: "lilia",
            subkind: "lilia_edit_exec",
            modifiedCommand: "pnpm test --changed",
          }),
        }),
      },
    ]);
    expect(consentTimeline[0]).toMatchObject([
      "consent-4",
      expect.any(Object),
      "cancelled",
      "Lilia 无法执行用户修改后的命令，已取消原始 Codex 命令",
    ]);
  });

  it("resume thread still registers Lilia AskUser dynamic tool without app-server plan-tool params", async () => {
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        return { thread: { id: "thread-1" }, model: "gpt-5.1" };
      },
    };

    await startCodexAppServerThread(server as any, {
      resumeSessionId: "thread-1",
      permission: "ask",
      planMode: true,
    }, () => "C:/repo");

    expect(calls[0]).toMatchObject({
      method: "thread/resume",
      params: {
        threadId: "thread-1",
        dynamicTools: [expect.objectContaining({ name: "AskUserQuestion" })],
      },
    });
    expect(calls[0].params.includePlanTool).toBeUndefined();
  });

  it("applies Codex sticky settings before the first turn", async () => {
    const { protocol } = captureProtocol();
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        if (method === "thread/start") return { thread: { id: "thread-1" }, model: "gpt-5.5" };
        if (method === "turn/start") return { turn: { id: "turn-1" } };
        return {};
      },
      notify: () => {},
      respond: () => {},
      drainNotifications: () => [{
        method: "turn/completed",
        params: { threadId: "thread-1", turn: { status: "completed" } },
      }],
      close: () => {},
    };

    await runCodexAppServer({
      backend: "codex",
      prompt: "hi",
      permission: "ask",
      planMode: false,
      codexSettings: {
        model: "gpt-5.4-mini",
        reasoningEffort: "high",
        runtimeWorkspaceRoots: ["C:/repo", "D:/shared"],
        permissions: { profile: "workspaceWrite" },
      },
    }, { mcpServers: [], warnings: [] }, {
      protocol,
      interactions: { requestAskUser: async () => ({ cancelled: true, answers: {} }) },
      emitToolConsentTimeline: () => {},
      createCodexAppServer: () => server,
      env: {},
      cwd: () => "C:/repo",
    });

    const updateIndex = calls.findIndex((call) => call.method === "thread/settings/update");
    const turnIndex = calls.findIndex((call) => call.method === "turn/start");
    expect(updateIndex).toBeGreaterThan(-1);
    expect(updateIndex).toBeLessThan(turnIndex);
    expect(calls[updateIndex].params).toMatchObject({
      threadId: "thread-1",
      model: "gpt-5.4-mini",
      reasoningEffort: "high",
      effort: "high",
      runtimeWorkspaceRoots: ["C:/repo", "D:/shared"],
      permissions: ":workspace",
    });
    expect(calls[updateIndex].params.collaborationMode).toBeUndefined();
  });

  it("falls back to turn/start settings and emits a diagnostic when thread settings update fails", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        if (method === "thread/settings/update") throw new Error("unsupported");
        return {};
      },
    };

    const result = await updateCodexThreadSettings(server as any, "thread-1", {
      permission: "ask",
      codexSettings: {
        runtimeWorkspaceRoots: ["C:/repo"],
        permissions: { profile: "readOnly" },
      },
    }, protocol);

    expect(result).toEqual({ ok: false, fallback: true });
    expect(calls[0]).toMatchObject({
      method: "thread/settings/update",
      params: {
        threadId: "thread-1",
        runtimeWorkspaceRoots: ["C:/repo"],
        permissions: ":read-only",
      },
    });
    expect(json()).toEqual([
      {
        type: "timeline",
        event: expect.objectContaining({
          kind: "diagnostic",
          status: "error",
          title: "Codex settings update failed",
        }),
      },
    ]);
  });

  it("builds Codex plan collaboration mode from preset or fallback", async () => {
    const server = {
      request: async () => ({
        data: [
          { name: "chat", mode: "default", reasoning_effort: null },
          { name: "plan", mode: "plan", reasoning_effort: "high" },
        ],
      }),
    };

    await expect(readCodexPlanModePreset(server as any)).resolves.toMatchObject({
      mode: "plan",
      reasoning_effort: "high",
    });
    expect(buildCodexCollaborationMode("plan", "gpt-5.1", { reasoning_effort: "high" })).toEqual({
      mode: "plan",
      settings: {
        model: "gpt-5.1",
        reasoning_effort: "high",
        developer_instructions: null,
      },
    });
    expect(buildCodexCollaborationMode("plan", null, null)).toEqual({
      mode: "plan",
      settings: {
        model: "gpt-5",
        reasoning_effort: "medium",
        developer_instructions: null,
      },
    });
  });

  it("Codex 计划确认通过统一 interaction_request/response 往返", async () => {
    const { protocol, json } = captureProtocol();
    const broker = createInteractionBroker({
      protocol,
      emitToolConsentTimeline: () => {},
      emitAskUserTimeline: () => {},
    });
    let turnStarts = 0;
    let planSent = false;
    let planCompletionSent = false;
    let executionCompletionSent = false;
    const server = {
      request: async (method: string) => {
        if (method === "thread/start") return { thread: { id: "thread-1" }, model: "gpt-5.1" };
        if (method === "collaborationMode/list") return { data: [] };
        if (method === "turn/start") {
          turnStarts += 1;
          return { turn: { id: `turn-${turnStarts}` } };
        }
        return {};
      },
      notify: () => {},
      respond: () => {},
      drainNotifications: () => {
        if (turnStarts === 1 && !planSent) {
          planSent = true;
          return [{
            method: "turn/plan/updated",
            params: {
              threadId: "thread-1",
              turnId: "turn-1",
              explanation: "计划草稿",
              plan: [{ step: "改代码", status: "pending" }],
            },
          }];
        }
        if (turnStarts === 1 && !planCompletionSent) {
          planCompletionSent = true;
          return [{
            method: "turn/completed",
            params: { threadId: "thread-1", turn: { status: "completed" } },
          }];
        }
        if (turnStarts === 2 && !executionCompletionSent) {
          executionCompletionSent = true;
          return [{
            method: "turn/completed",
            params: { threadId: "thread-1", turn: { status: "completed" } },
          }];
        }
        return [];
      },
      close: () => {},
    };

    const run = runCodexAppServer({
      backend: "codex",
      prompt: "请制定计划",
      permission: "ask",
      planMode: true,
    }, { mcpServers: [], warnings: [] }, {
      protocol,
      interactions: broker,
      emitToolConsentTimeline: () => {},
      createCodexAppServer: () => server,
      env: {},
      cwd: () => "C:/repo",
    });

    await waitUntil(() =>
      json().some((line) => line.type === "interaction_request" && line.kind === "plan_approval")
    );
    const requests = json().filter((line) => line.type === "interaction_request");
    expect(requests).toEqual([
      {
        type: "interaction_request",
        id: "ask-1",
        kind: "plan_approval",
        backend: "codex",
        payload: expect.objectContaining({
          title: "确认 Codex 计划",
          source: "Codex Plan",
          intent: "plan_approval",
        }),
      },
    ]);

    broker.handleControlLine(JSON.stringify({
      type: "interaction_response",
      id: "ask-1",
      kind: "plan_approval",
      result: {
        cancelled: false,
        answers: {
          "approve-plan": {
            questionId: "approve-plan",
            value: "yes",
          },
        },
      },
    }));

    await run;
    expect(json().some((line) =>
      line.type === "timeline" &&
      line.event.kind === "plan" &&
      line.event.status === "success" &&
      line.event.payload.approved === true
    )).toBe(true);
  });

  it("Codex plan mode waits for the plan turn to complete before asking Lilia", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    let turnStarts = 0;
    let planSent = false;
    let planCompletionSent = false;
    let executionCompletionSent = false;
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ type: "server", method, params });
        if (method === "thread/start") return { thread: { id: "thread-1" }, model: "gpt-5.1" };
        if (method === "collaborationMode/list") {
          return { data: [{ name: "plan", mode: "plan", reasoning_effort: "high" }] };
        }
        if (method === "turn/start") {
          turnStarts += 1;
          return { turn: { id: `turn-${turnStarts}` } };
        }
        return {};
      },
      notify: (method: string, params: any) => {
        calls.push({ type: "notify", method, params });
      },
      respond: () => {},
      drainNotifications: () => {
        if (turnStarts === 1 && !planSent) {
          planSent = true;
          return [{
            method: "turn/plan/updated",
            params: {
              threadId: "thread-1",
              turnId: "turn-1",
              explanation: "计划草稿",
              plan: [{ step: "改代码", status: "pending" }],
            },
          }];
        }
        if (turnStarts === 1 && !planCompletionSent) {
          planCompletionSent = true;
          return [{
            method: "turn/completed",
            params: { threadId: "thread-1", turn: { status: "completed" } },
          }];
        }
        if (turnStarts === 2 && !executionCompletionSent) {
          executionCompletionSent = true;
          return [{
            method: "turn/completed",
            params: { threadId: "thread-1", turn: { status: "completed" } },
          }];
        }
        return [];
      },
      close: () => {
        calls.push({ type: "close" });
      },
    };
    let seenSpec: any = null;
    let seenOptions: any = null;
    const interactions = {
      requestAskUser: async (spec: any, options: any) => {
        calls.push({ type: "ask", spec, options });
        seenSpec = spec;
        seenOptions = options;
        return {
          cancelled: false,
          answers: {
            "approve-plan": {
              questionId: "approve-plan",
              value: "yes",
            },
          },
        };
      },
    };

    await runCodexAppServer({
      backend: "codex",
      prompt: "请制定计划",
      permission: "ask",
      planMode: true,
    }, { mcpServers: [], warnings: [] }, {
      protocol,
      interactions,
      emitToolConsentTimeline: () => {},
      createCodexAppServer: () => server,
      env: {},
      cwd: () => "C:/repo",
    });

    const startCalls = calls.filter((call) => call.type === "server" && call.method === "turn/start");
    const askIndex = calls.findIndex((call) => call.type === "ask");
    const executionStartIndex = calls.findIndex((call) =>
      call.type === "server" &&
      call.method === "turn/start" &&
      call.params.input[0].text.includes("用户已确认上一版计划")
    );
    expect(calls.some((call) => call.type === "server" && call.method === "collaborationMode/list")).toBe(true);
    expect(calls.some((call) => call.type === "server" && call.method === "turn/interrupt")).toBe(false);
    expect(askIndex).toBeGreaterThan(-1);
    expect(askIndex).toBeLessThan(executionStartIndex);
    expect(startCalls).toHaveLength(2);
    expect(startCalls[0].params.collaborationMode).toEqual({
      mode: "plan",
      settings: {
        model: "gpt-5.1",
        reasoning_effort: "high",
        developer_instructions: null,
      },
    });
    expect(startCalls[1].params.collaborationMode).toMatchObject({
      mode: "default",
      settings: { model: "gpt-5.1" },
    });
    expect(seenSpec).toMatchObject({
      title: "确认 Codex 计划",
      source: "Codex Plan",
      intent: "plan_approval",
    });
    expect(seenOptions).toMatchObject({
      backend: "codex",
      emitTimelineEvent: false,
    });
    expect(startCalls[1].params.input[0].text).toContain("用户已确认上一版计划");
    expect(startCalls[1].params.input[0].text).toContain("[ ] 改代码");
    expect(json().some((line) =>
      line.type === "timeline" &&
      line.event.kind === "todo_list" &&
      line.event.payload.items?.[0]?.text === "改代码"
    )).toBe(true);
    expect(json().some((line) =>
      line.type === "timeline" &&
      line.event.kind === "plan" &&
      line.event.status === "requires_action"
    )).toBe(true);
    expect(json().some((line) =>
      line.type === "timeline" &&
      line.event.kind === "plan" &&
      line.event.status === "success" &&
      line.event.payload.approved === true
    )).toBe(true);
  });

  it("Codex plan revision starts another plan-mode turn without creating a normal message", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    let turnStarts = 0;
    const completedTurns = new Set<number>();
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ type: "server", method, params });
        if (method === "thread/start") return { thread: { id: "thread-1" }, model: "gpt-5.1" };
        if (method === "collaborationMode/list") return { data: [] };
        if (method === "turn/start") {
          turnStarts += 1;
          return { turn: { id: `turn-${turnStarts}` } };
        }
        return {};
      },
      notify: (method: string, params: any) => {
        calls.push({ type: "notify", method, params });
      },
      respond: () => {},
      drainNotifications: () => {
        if (turnStarts > 0 && !completedTurns.has(turnStarts)) {
          completedTurns.add(turnStarts);
          return [{
            method: "item/agentMessage/delta",
            params: { delta: "计划：先改代码，再补测试。" },
          }, {
            method: "turn/completed",
            params: { threadId: "thread-1", turn: { status: "completed" } },
          }];
        }
        return [];
      },
      close: () => {},
    };
    let askCount = 0;
    const interactions = {
      requestAskUser: async () => {
        askCount += 1;
        if (askCount === 1) {
          return {
            cancelled: false,
            answers: {
              "approve-plan": {
                questionId: "approve-plan",
                value: "revision_request",
                notes: "先补充回滚方案",
              },
            },
          };
        }
        return { cancelled: true, answers: {} };
      },
    };

    await runCodexAppServer({
      backend: "codex",
      prompt: "请制定计划",
      permission: "ask",
      planMode: true,
    }, { mcpServers: [], warnings: [] }, {
      protocol,
      interactions,
      emitToolConsentTimeline: () => {},
      createCodexAppServer: () => server,
      env: {},
      cwd: () => "C:/repo",
    });

    const startCalls = calls.filter((call) => call.type === "server" && call.method === "turn/start");
    expect(startCalls).toHaveLength(2);
    expect(startCalls[1].params.input[0].text).toContain(buildCodexPlanRevisionPrompt("先补充回滚方案"));
    expect(startCalls[0].params.collaborationMode).toMatchObject({
      mode: "plan",
      settings: { model: "gpt-5.1", reasoning_effort: "medium" },
    });
    expect(startCalls[1].params.collaborationMode).toMatchObject({
      mode: "plan",
      settings: { model: "gpt-5.1", reasoning_effort: "medium" },
    });
    expect(calls.some((call) => call.type === "server" && call.method === "turn/interrupt")).toBe(false);
    expect(json().some((line) =>
      line.type === "timeline" &&
      line.event.kind === "plan" &&
      line.event.status === "cancelled" &&
      line.event.payload.revisionRequest === "先补充回滚方案"
    )).toBe(true);
    expect(json().some((line) => line.type === "user_message")).toBe(false);
  });
});
