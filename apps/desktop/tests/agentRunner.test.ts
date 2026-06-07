import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { runAgentTurn } from "../agent-runner/core.mjs";
import { createInteractionBroker } from "../agent-runner/interactions.mjs";
import { createProtocolEmitter } from "../agent-runner/protocol.mjs";
import {
  applyClaudeRuntimePermission,
  mapClaudeInitialPermission,
} from "../agent-runner/claude/permissions.mjs";
import { createLiliaAskUserServer } from "../agent-runner/claude/runClaude.mjs";
import {
  createConversationContextHandler,
} from "../agent-runner/conversationContext.mjs";
import { maybeHandleCodexApprovalRequest } from "../agent-runner/codex/permissions.mjs";
import {
  codexHistoryTimelineEvents,
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
  syncCodexThreadHistory,
  updateCodexThreadSettings,
  applyCodexRuntimePermission,
  flushCodexRuntimeSettings,
  startCodexAppServerTurn,
} from "../agent-runner/codex/runCodex.mjs";
import {
  codexHistoryTimelineInputs,
  readCodexThreadTurns,
  searchCodexThreads,
  syncCodexThreadHistoryForTask,
} from "../agent-runner/codex/history.mjs";

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

  it("settings_update control lines are dispatched without disturbing pending interactions", async () => {
    const { protocol } = captureProtocol();
    const updates: any[] = [];
    const broker = createInteractionBroker({
      protocol,
      emitToolConsentTimeline: () => {},
      emitAskUserTimeline: () => {},
    });
    broker.handleSettingsUpdate((msg: any) => updates.push(msg));
    const consent = broker.requestUserConsent({ toolName: "Bash", input: { command: "pwd" } });

    broker.handleControlLine(JSON.stringify({ type: "settings_update", permission: "readonly" }));
    expect(updates).toEqual([{ type: "settings_update", permission: "readonly" }]);
    expect(broker.pendingCounts().consent).toBe(1);

    broker.handleControlLine(JSON.stringify({
      type: "interaction_response",
      id: "consent-1",
      kind: "tool_consent",
      result: { decision: "allow" },
    }));
    await expect(consent).resolves.toMatchObject({ decision: "allow" });
  });
});

describe("Claude helpers", () => {
  it("子对话才会注册 Claude 查询对话上下文工具", () => {
    const createdTools: any[] = [];
    const createTool = (...args: any[]) => {
      createdTools.push(args);
      return { name: args[0] };
    };
    const createServer = (config: any) => config;

    const normal = createLiliaAskUserServer({
      createServer,
      createTool,
      requestAskUser: async () => ({ cancelled: true, answers: {} }),
    });
    expect(normal.tools.map((tool: any) => tool.name)).toEqual(["ask_user_question"]);

    const child = createLiliaAskUserServer({
      createServer,
      createTool,
      requestAskUser: async () => ({ cancelled: true, answers: {} }),
      conversationContext: {
        currentTaskId: "child-1",
        parentTaskId: "parent-1",
        tasks: [],
      },
    });

    expect(child.tools.map((tool: any) => tool.name)).toEqual([
      "ask_user_question",
      "query_conversation_context",
    ]);
    expect(createdTools.some((args) => args[0] === "query_conversation_context")).toBe(true);
  });

  it("查询对话上下文工具默认返回父对话消息摘要", async () => {
    const handler = createConversationContextHandler({
      currentTaskId: "child-1",
      parentTaskId: "parent-1",
      tasks: [{
        taskId: "parent-1",
        projectId: "lilia",
        title: "父对话",
        status: "done",
        parentId: null,
        createdAt: 1000,
        messages: [
          { role: "user", content: "父对话里的问题", createdAt: 1001 },
          { role: "assistant", content: "父对话里的回答", createdAt: 1002 },
        ],
      }],
    });

    await expect(handler({})).resolves.toMatchObject({
      ok: true,
      currentTaskId: "child-1",
      parentTaskId: "parent-1",
      requestedTaskId: "parent-1",
      task: {
        taskId: "parent-1",
        title: "父对话",
        messages: [
          { role: "user", content: "父对话里的问题" },
          { role: "assistant", content: "父对话里的回答" },
        ],
      },
    });
  });

  it("plan mode 初始进入 Claude plan，确认后恢复原执行权限映射", () => {
    expect(mapClaudeInitialPermission("ask", true).permissionMode).toBe("plan");
    expect(mapClaudeInitialPermission("readonly", false).permissionMode).toBe("default");
    expect(mapClaudeInitialPermission("full", false)).toMatchObject({
      permissionMode: "bypassPermissions",
      allowDangerouslySkipPermissions: true,
    });
  });

  it("runtime permission update changes Claude SDK mode and Lilia gate", async () => {
    const { protocol, json } = captureProtocol();
    const modeCalls: string[] = [];
    const ctx: any = {
      protocol,
      executionPermission: "ask",
      query: {
        setPermissionMode: async (mode: string) => {
          modeCalls.push(mode);
        },
      },
    };

    expect(applyClaudeRuntimePermission(ctx, "readonly")).toBe(true);
    expect(ctx.executionPermission).toBe("readonly");
    expect(modeCalls).toEqual(["default"]);

    expect(applyClaudeRuntimePermission(ctx, "full")).toBe(true);
    expect(ctx.executionPermission).toBe("full");
    expect(modeCalls).toEqual(["default", "bypassPermissions"]);
    expect(json().filter((line) => line.type === "timeline")).toHaveLength(2);
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

  it("Codex 子对话工具调用可查询父对话上下文", async () => {
    const calls: any[] = [];
    const handled = await maybeHandleCodexServerRequest(
      {
        respond: (...args: any[]) => calls.push(["respond", ...args]),
      } as any,
      {
        id: "query-context-1",
        method: "item/tool/call",
        params: {
          tool: "QueryConversationContext",
          arguments: {},
        },
      },
      {
        cmd: {
          conversationContext: {
            currentTaskId: "child-1",
            parentTaskId: "parent-1",
            tasks: [{
              taskId: "parent-1",
              title: "父对话",
              messages: [{ role: "user", content: "父问题", createdAt: 1 }],
            }],
          },
        },
        interactions: {
          requestAskUser: async () => {
            throw new Error("should not ask");
          },
        },
      } as any,
    );

    expect(handled).toBe(true);
    expect(calls).toHaveLength(1);
    expect(calls[0][1]).toBe("query-context-1");
    expect(calls[0][2]).toMatchObject({
      success: true,
      contentItems: [expect.objectContaining({ type: "inputText" })],
    });
    const output = JSON.parse(calls[0][2].contentItems[0].text);
    expect(output).toMatchObject({
      ok: true,
      currentTaskId: "child-1",
      parentTaskId: "parent-1",
      task: {
        taskId: "parent-1",
        messages: [{ role: "user", content: "父问题" }],
      },
    });
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

  it("syncs Codex thread history only when resuming a thread", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        if (method === "thread/turns/list") {
          return {
            data: [{
              id: "turn-old",
              status: "completed",
              startedAt: 10,
              completedAt: 11,
              items: [{ type: "agentMessage", id: "msg-1", text: "旧回复" }],
            }],
            nextCursor: null,
            backwardsCursor: null,
          };
        }
        return {};
      },
    };

    await expect(syncCodexThreadHistory(
      server as any,
      "thread-1",
      { resumeSessionId: "thread-1" },
      protocol,
    )).resolves.toEqual({ ok: true, skipped: false, count: 1 });
    await expect(syncCodexThreadHistory(
      server as any,
      "thread-2",
      {},
      protocol,
    )).resolves.toEqual({ ok: true, skipped: true, count: 0 });

    expect(calls).toEqual([{
      method: "thread/turns/list",
      params: {
        threadId: "thread-1",
        limit: 50,
        sortDirection: "asc",
        itemsView: "full",
      },
    }]);
    expect(json()).toEqual([
      {
        type: "timeline",
        event: expect.objectContaining({
          kind: "message",
          sourceId: "codex-history:thread-1:turn-old:msg-1",
          turnIdOverride: "turn-old",
          createdAt: 10_000,
          updatedAt: 11_000,
          payload: expect.objectContaining({
            history: true,
            threadId: "thread-1",
            turnId: "turn-old",
            itemId: "msg-1",
          }),
        }),
      },
      {
        type: "timeline",
        event: expect.objectContaining({
          kind: "diagnostic",
          status: "info",
          title: "Codex history synced",
        }),
      },
    ]);
  });

  it("continues Codex run when history sync fails", async () => {
    const { protocol, json } = captureProtocol();
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        if (method === "thread/resume") return { thread: { id: "thread-1" }, model: "gpt-5.1" };
        if (method === "thread/turns/list") throw new Error("history unavailable");
        if (method === "turn/start") return { turn: { id: "turn-new" } };
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
      resumeSessionId: "thread-1",
      permission: "ask",
      planMode: false,
    }, { mcpServers: [], warnings: [] }, {
      protocol,
      interactions: { requestAskUser: async () => ({ cancelled: true, answers: {} }) },
      emitToolConsentTimeline: () => {},
      createCodexAppServer: () => server,
      env: {},
      cwd: () => "C:/repo",
    });

    const historyIndex = calls.findIndex((call) => call.method === "thread/turns/list");
    const turnIndex = calls.findIndex((call) => call.method === "turn/start");
    expect(historyIndex).toBeGreaterThan(-1);
    expect(historyIndex).toBeLessThan(turnIndex);
    expect(json().some((line) =>
      line.type === "timeline" &&
      line.event.kind === "diagnostic" &&
      line.event.status === "error" &&
      line.event.title === "Codex history sync failed"
    )).toBe(true);
    expect(json().some((line) => line.type === "done" && line.sessionId === "thread-1")).toBe(true);
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

  it("runtime permission update refreshes Codex thread settings and later turn params", async () => {
    const { protocol } = captureProtocol();
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        if (method === "turn/start") return { turn: { id: "turn-1" } };
        return {};
      },
    };
    const cmd: any = {
      permission: "ask",
      cwd: "C:/repo",
      codexSettings: {},
    };
    const ctx: any = createCodexRunContext(cmd, protocol, "thread-1");
    ctx.settingsUpdatePromises = [];

    applyCodexRuntimePermission(server as any, "thread-1", cmd, ctx, "readonly", protocol);
    await flushCodexRuntimeSettings(ctx);
    await startCodexAppServerTurn(server as any, "thread-1", "after update", cmd, () => "C:/repo");

    expect(cmd.permission).toBe("readonly");
    expect(ctx.executionPermission).toBe("readonly");
    expect(calls[0]).toMatchObject({
      method: "thread/settings/update",
      params: {
        threadId: "thread-1",
        sandboxPolicy: { type: "readOnly" },
      },
    });
    expect(calls[1]).toMatchObject({
      method: "turn/start",
      params: {
        approvalPolicy: "never",
        sandboxPolicy: { type: "readOnly" },
      },
    });
  });

  it("maps Codex thread history turns into stable timeline events", () => {
    const events = codexHistoryTimelineEvents("thread-1", [{
      id: "turn-old",
      status: "completed",
      startedAt: 10,
      completedAt: 12,
      items: [{
        type: "userMessage",
        id: "user-1",
        clientId: "client-1",
        content: [{ type: "text", text: "继续修复历史恢复" }],
      }, {
        type: "commandExecution",
        id: "cmd-1",
        command: "yarn test",
        cwd: "C:/repo",
        status: "completed",
        aggregatedOutput: "ok",
        exitCode: 0,
        durationMs: 120,
      }, {
        type: "mcpToolCall",
        id: "mcp-1",
        server: "docs",
        tool: "search",
        status: "completed",
        arguments: { query: "thread/turns/list" },
        result: { hits: 1 },
        error: null,
      }, {
        type: "webSearch",
        id: "search-1",
        query: "codex history",
      }, {
        type: "agentMessage",
        id: "assistant-1",
        text: "历史已恢复",
      }],
    }]);

    expect(events).toHaveLength(5);
    expect(events[0]).toMatchObject({
      kind: "message",
      status: "success",
      title: "用户输入",
      summary: "继续修复历史恢复",
      sourceId: "codex-history:thread-1:turn-old:user-1",
      turnIdOverride: "turn-old",
      createdAt: 10_000,
      updatedAt: 12_000,
      payload: {
        history: true,
        threadId: "thread-1",
        turnId: "turn-old",
        itemId: "user-1",
        role: "user",
        content: "继续修复历史恢复",
      },
    });
    expect(events[1]).toMatchObject({
      kind: "command",
      status: "success",
      payload: {
        command: "yarn test",
        aggregatedOutput: "ok",
        exitCode: 0,
      },
    });
    expect(events[2]).toMatchObject({
      kind: "mcp",
      payload: {
        server: "docs",
        tool: "search",
        arguments: { query: "thread/turns/list" },
      },
    });
    expect(events[3]).toMatchObject({
      kind: "search",
      payload: { query: "codex history" },
    });
    expect(events[4]).toMatchObject({
      kind: "message",
      title: "Assistant",
      summary: "历史已恢复",
      payload: {
        role: "assistant",
        content: "历史已恢复",
      },
    });
    expect(events.map((event) => event.createdAt)).toEqual([
      10_000,
      10_001,
      10_002,
      10_003,
      10_004,
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

describe("Codex history utility", () => {
  it("searches Codex threads with cursor and archive params", async () => {
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        if (method === "initialize") return {};
        if (method === "thread/search") {
          return {
            data: [{
              id: "thread-1",
              title: "修复构建",
              model: "gpt-5.1",
              updatedAt: 10,
              preview: "最后一条回复",
            }],
            nextCursor: "cursor-2",
          };
        }
        return {};
      },
      notify: () => {},
      close: () => {},
    };

    await expect(searchCodexThreads({
      searchTerm: "build",
      cursor: "cursor-1",
      archived: true,
      limit: 30,
    }, { createServer: () => server as any })).resolves.toEqual({
      threads: [expect.objectContaining({
        id: "thread-1",
        title: "修复构建",
        model: "gpt-5.1",
        updatedAt: 10_000,
        preview: "最后一条回复",
      })],
      nextCursor: "cursor-2",
    });

    expect(calls.find((call) => call.method === "thread/search")).toEqual({
      method: "thread/search",
      params: {
        limit: 30,
        sortDirection: "desc",
        archived: true,
        searchTerm: "build",
        cursor: "cursor-1",
      },
    });
  });

  it("lists turns and backfills truncated turn items", async () => {
    const calls: any[] = [];
    const server = {
      request: async (method: string, params: any) => {
        calls.push({ method, params });
        if (method === "thread/turns/list") {
          return {
            data: [{
              id: "turn-1",
              status: "completed",
              startedAt: 1,
              completedAt: 2,
              itemsTruncated: true,
              items: [],
            }],
          };
        }
        if (method === "thread/turns/items/list") {
          return {
            data: [{ type: "agentMessage", id: "msg-1", text: "历史回复" }],
          };
        }
        return {};
      },
    };

    const result = await readCodexThreadTurns(server as any, "thread-1", { limit: 10 });

    expect(calls).toEqual([
      {
        method: "thread/turns/list",
        params: {
          threadId: "thread-1",
          limit: 10,
          sortDirection: "asc",
          itemsView: "full",
        },
      },
      {
        method: "thread/turns/items/list",
        params: {
          threadId: "thread-1",
          turnId: "turn-1",
          limit: 200,
          sortDirection: "asc",
        },
      },
    ]);
    expect(result.turns[0].items).toEqual([
      { type: "agentMessage", id: "msg-1", text: "历史回复" },
    ]);
  });

  it("syncs history into Lilia timeline inputs", async () => {
    const server = {
      request: async (method: string) => {
        if (method === "initialize") return {};
        if (method === "thread/turns/list") {
          return {
            data: [{
              id: "turn-1",
              status: "completed",
              startedAt: 1,
              completedAt: 2,
              items: [
                { type: "userMessage", id: "user-1", text: "旧问题" },
                { type: "agentMessage", id: "msg-1", text: "旧回复" },
              ],
            }],
          };
        }
        return {};
      },
      notify: () => {},
      close: () => {},
    };

    const result = await syncCodexThreadHistoryForTask({
      taskId: "task-1",
      threadId: "thread-1",
    }, { createServer: () => server as any });

    expect(result.eventCount).toBe(2);
    expect(result.events).toEqual([
      expect.objectContaining({
        taskId: "task-1",
        turnId: "turn-1",
        backend: "codex",
        kind: "message",
        payload: expect.objectContaining({
          history: true,
          threadId: "thread-1",
          itemId: "user-1",
        }),
      }),
      expect.objectContaining({
        taskId: "task-1",
        turnId: "turn-1",
        kind: "message",
        summary: "旧回复",
      }),
    ]);
    expect(codexHistoryTimelineInputs("task-2", "thread-1", [])).toEqual([]);
  });
});
