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
  startCodexAppServerThread,
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
