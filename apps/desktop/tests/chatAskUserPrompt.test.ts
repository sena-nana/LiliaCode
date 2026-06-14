import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AskUserSpec } from "@lilia/contracts";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { installAgentInteractionBridge } from "../src/composables/useAgentInteractionBridge";
import { resolveAskUser, useAskUser } from "../src/composables/useAskUser";
import { useConnectionStatus } from "../src/composables/useConnectionStatus";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import { setAgentInteractionSettings } from "../src/services/chat";
import {
  emitTauriEvent,
  emitMockTimelineEvent,
  failNextMockAgentInteractionResponse,
  mockInvoke,
  replaceMockTimelineEvents,
  setMockActiveBackend,
  setMockChatRunning,
  setMockComposerStateHandler,
  setMockRuntimeSnapshot,
} from "./tauriMock";

const askUserSpec: AskUserSpec = {
  title: "Claude 想确认一下",
  source: "Claude",
  questions: [
    {
      id: "q-1",
      header: "方案",
      question: "选哪个方案？",
      mode: "single",
      options: [
        { id: "o-1", label: "A" },
        { id: "o-2", label: "B" },
      ],
    },
  ],
};

async function renderTaskDetail(taskId = "t-002") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(`/projects/lilia/tasks/${taskId}`);
  await router.isReady();

  const view = render(TaskDetail, {
    props: {
      projectId: "lilia",
      taskId,
    },
    global: {
      plugins: [router],
    },
  });
  await waitFor(() => {
    expect(mockInvoke.mock.calls.some(([cmd]) =>
      cmd === "agent_interaction_get_settings"
    )).toBe(true);
  });
  await Promise.resolve();
  return Object.assign(view, { router });
}

function placeEditableCaret(element: HTMLElement, offset: number) {
  const selection = window.getSelection();
  const range = document.createRange();
  const textNode = element.firstChild;
  if (textNode?.nodeType === Node.TEXT_NODE) {
    range.setStart(textNode, Math.min(offset, textNode.textContent?.length ?? 0));
  } else {
    range.selectNodeContents(element);
    range.collapse(false);
  }
  selection?.removeAllRanges();
  selection?.addRange(range);
}

async function setComposerText(view: ReturnType<typeof render>, text: string) {
  const input = view.getByRole("textbox") as HTMLElement;
  if (input instanceof HTMLTextAreaElement) {
    await fireEvent.update(input, text);
    return input;
  }
  input.textContent = text;
  placeEditableCaret(input, text.length);
  await fireEvent.input(input);
  return input;
}

async function enableNonInterruptMode() {
  await setAgentInteractionSettings({ nonInterruptMode: true });
  mockInvoke.mockClear();
}

async function renderCodexTaskDetail(taskId = "t-002", options: { clearMockCalls?: boolean } = {}) {
  const clearMockCalls = options.clearMockCalls ?? true;
  setMockActiveBackend("codex");
  await useConnectionStatus({ probe: false }).setActiveBackend("codex");
  setMockComposerStateHandler((id) => ({
    taskId: id,
    backend: "codex",
    model: "gpt-5.5",
    planMode: false,
    permission: "ask",
  }));
  const view = await renderTaskDetail(taskId);
  await waitFor(() => {
    expect(view.getByRole("button", { name: "代码审查" })).not.toBeDisabled();
  });
  if (clearMockCalls) mockInvoke.mockClear();
  return view;
}

function latestChatSendArgs() {
  const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
  return calls.at(-1)?.[1] as Record<string, unknown> | undefined;
}

async function expectLatestChatSend(partial: Record<string, unknown>) {
  await waitFor(() => {
    expect(latestChatSendArgs()).toMatchObject({
      taskId: "t-002",
      ...partial,
    });
  });
}

function finishCodexWorkflow(sessionId: string) {
  emitTauriEvent("chat:done", { taskId: "t-002", sessionId, subtype: null });
  mockInvoke.mockClear();
}

function emitAskUserRequest(
  taskId: string,
  spec: AskUserSpec = askUserSpec,
  turnId = "turn-ask",
) {
  emitTauriEvent("chat:agent-interaction-request", {
    taskId,
    turnId,
    backend: "claude",
    requestId: `ask-${taskId}`,
    kind: "ask_user",
    payload: spec,
  });
}

function emitUnifiedAskUserRequest(
  taskId: string,
  spec: AskUserSpec,
  kind: "ask_user" | "plan_approval" = "ask_user",
  backend = "codex",
  turnId = "turn-unified-ask",
) {
  emitTauriEvent("chat:agent-interaction-request", {
    taskId,
    turnId,
    backend,
    requestId: `unified-${taskId}`,
    kind,
    payload: spec,
  });
}

function emitAskUserTimelineEvent(
  taskId: string,
  spec: AskUserSpec = askUserSpec,
  turnId = "turn-ask",
) {
  emitMockTimelineEvent(taskId, {
    id: `ask-card-${taskId}`,
    kind: "ask_user",
    status: "requires_action",
    title: spec.title ?? "AskUserQuestion",
    summary: spec.questions[0]?.question ?? "",
    turnId,
    payload: {
      backend: "claude",
      interaction: "ask_user",
      requestId: `ask-${taskId}`,
      questions: spec.questions,
      spec,
    },
  });
}

function emitPlanApprovalRequest(taskId: string) {
  emitTauriEvent("chat:agent-interaction-request", {
    taskId,
    turnId: "turn-plan",
    backend: "claude",
    requestId: `ask-${taskId}`,
    kind: "plan_approval",
    payload: {
      title: "确认 Claude 计划",
      source: "Claude Plan",
      intent: "plan_approval",
      dismissable: true,
      questions: [
        {
          id: "approve-plan",
          header: "计划确认",
          question: "",
          mode: "confirm",
          confirmLabel: "按计划执行",
          cancelLabel: "先不执行",
        },
      ],
    },
  });
}

function emitToolConsentRequest(taskId: string) {
  emitTauriEvent("chat:agent-interaction-request", {
    taskId,
    turnId: "turn-tool",
    backend: "claude",
    requestId: `tool-${taskId}`,
    kind: "tool_consent",
    payload: {
      toolName: "Write",
      input: { file_path: "src/main.ts" },
      title: null,
      displayName: null,
      description: null,
      blockedPath: null,
      decisionReason: null,
      toolUseId: null,
    },
  });
}

function persistedToolConsentEvent() {
  return {
    id: "tool-card-t-002",
    taskId: "t-002",
    turnId: "turn-tool",
    backend: "claude",
    kind: "file_change",
    status: "requires_action",
    title: "Write",
    summary: "src/main.ts",
    payload: {
      interaction: "tool_consent",
      requestId: "tool-t-002",
      toolName: "Write",
      input: { file_path: "src/main.ts" },
    },
    createdAt: 10_000,
    updatedAt: 10_000,
    turnSeq: 1,
    intraTurnOrder: 0,
  } as const;
}

function emitUnifiedToolConsentRequest(taskId: string) {
  emitTauriEvent("chat:agent-interaction-request", {
    taskId,
    turnId: "turn-unified-tool",
    backend: "codex",
    requestId: `unified-tool-${taskId}`,
    kind: "tool_consent",
    payload: {
      toolName: "item/commandExecution/requestApproval",
      input: { command: "yarn test" },
      title: "Codex command approval",
      displayName: "item/commandExecution/requestApproval",
      description: "yarn test",
      blockedPath: null,
      decisionReason: null,
      toolUseId: "codex-tool-use",
    },
  });
}

function emitBashToolConsentRequest(taskId: string) {
  emitTauriEvent("chat:agent-interaction-request", {
    taskId,
    turnId: "turn-bash",
    backend: "claude",
    requestId: `bash-tool-${taskId}`,
    kind: "tool_consent",
    payload: {
      toolName: "Bash",
      input: { command: "pwd" },
      title: null,
      displayName: null,
      description: null,
      blockedPath: null,
      decisionReason: null,
      toolUseId: "bash-tool-use",
    },
  });
}

async function expectAskUserResponse(taskId: string) {
  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
      taskId,
      requestId: `ask-${taskId}`,
      kind: "ask_user",
      result: {
        cancelled: false,
        answers: {
          "q-1": { questionId: "q-1", value: "o-2" },
        },
      },
    }, undefined);
  });
}

async function expectUnifiedAskUserResponse(taskId: string, kind: "ask_user" | "plan_approval" = "ask_user") {
  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
      taskId,
      requestId: `unified-${taskId}`,
      kind,
      result: {
        cancelled: false,
        answers: {
          "q-1": { questionId: "q-1", value: "o-2" },
        },
      },
    }, undefined);
  });
}

let unlistenInteraction: (() => void) | null = null;

describe("chat AskUser prompt", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
    unlistenInteraction = await installAgentInteractionBridge();
  });


  afterEach(async () => {
    const { state } = useAskUser();
    while (state.current || state.queue.length) {
      resolveAskUser({ answers: {}, cancelled: true });
    }
    await Promise.resolve();
    await useConnectionStatus({ probe: false }).setActiveBackend("claude");
    setMockComposerStateHandler(null);

    unlistenInteraction?.();
    unlistenInteraction = null;
  });


  it("把当前 task 的 Agent 提问显示在 composer 内部，并保留回答回写", async () => {
    const view = await renderTaskDetail();

    emitAskUserRequest("t-002");

    const prompt = await view.findByRole("region", { name: "Claude 想确认一下" });
    expect(prompt).toHaveClass("composer-inline");
    expect(document.body.querySelector(".search-palette.ask-user")).toBeNull();

    const controls = view.container.querySelector(".chat-controls");
    const composer = controls?.querySelector(".chat-composer");
    expect(composer).not.toBeNull();
    expect(composer).toContainElement(prompt);
    expect(controls?.querySelector(":scope > .ask-user")).toBeNull();

    await fireEvent.click(view.getByRole("radio", { name: "B" }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await expectAskUserResponse("t-002");
  });

  it("统一 Codex Agent 提问显示在 composer 内部，并用统一命令回写", async () => {
    const view = await renderTaskDetail();

    emitUnifiedAskUserRequest("t-002", {
      ...askUserSpec,
      title: "Codex 想确认一下",
      source: "Codex",
    });

    const prompt = await view.findByRole("region", { name: "Codex 想确认一下" });
    expect(prompt).toHaveClass("composer-inline");

    await fireEvent.click(view.getByRole("radio", { name: "B" }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await expectUnifiedAskUserResponse("t-002");
  });


  it("非打断模式把 Agent 提问留在时间线卡片，composer 仍可加入调度队列", async () => {
    await enableNonInterruptMode();
    const view = await renderTaskDetail();

    emitAskUserRequest("t-002");
    emitAskUserTimelineEvent("t-002");

    await waitFor(() => {
      expect(view.container.querySelector(".chat-composer .composer-inline--ask")).toBeNull();
    });
    const prompt = view.container.querySelector(".timeline-pending-action.composer-inline--ask");
    expect(prompt).not.toBeNull();
    expect(prompt).toHaveClass("timeline-pending-action");
    expect(view.getByRole("region", { name: "Claude 想确认一下" })).toBe(prompt);
    expect(view.getByRole("button", { name: "添加附件" })).toBeInTheDocument();

    emitTauriEvent("chat:turn-started", { taskId: "t-002", queuedCount: 0 });
    await setComposerText(view, "补充上下文");
    await fireEvent.click(view.getByRole("button", { name: "加入调度队列" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_send_message", expect.objectContaining({
        taskId: "t-002",
        content: expect.stringContaining("补充上下文"),
      }), undefined);
    });

    await fireEvent.click(view.getByRole("radio", { name: "B" }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await expectAskUserResponse("t-002");
  });


  it("计划确认挂起时，输入框发送文本会回写为计划修改要求而不是新消息", async () => {
    const view = await renderTaskDetail();

    emitPlanApprovalRequest("t-002");

    const prompt = await view.findByRole("region", { name: "确认 Claude 计划" });
    expect(prompt).toHaveClass("composer-inline", "composer-inline--plan");
    expect(view.queryByRole("button", { name: "先不执行" })).toBeNull();
    expect(view.getByRole("button", { name: "忽略" })).toBeDisabled();
    expect(view.getByRole("button", { name: "同意" })).toBeInTheDocument();

    const input = await view.findByPlaceholderText("输入修改要求，Enter 退回计划");
    await fireEvent.update(input, "请把测试计划拆细");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "ask-t-002",
        kind: "plan_approval",
        result: {
          cancelled: false,
          answers: {
            "approve-plan": {
              questionId: "approve-plan",
              value: "revision_request",
              notes: "请把测试计划拆细",
            },
          },
        },
      }, undefined);
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(false);
    expect(view.queryByText("请把测试计划拆细")).toBeNull();
  });


  it("工具授权挂起时，输入框发送文本会作为拒绝备注而不是新消息", async () => {
    const view = await renderTaskDetail();

    emitToolConsentRequest("t-002");

    await view.findByRole("alert");
    const composer = view.container.querySelector(".chat-composer");
    expect(composer?.querySelector(".composer-inline--tool")).toBeInTheDocument();
    expect(view.queryByRole("button", { name: "添加附件" })).toBeNull();
    expect(view.queryByRole("button", { name: "拒绝" })).toBeNull();
    expect(view.getByRole("button", { name: "忽略" })).toBeDisabled();
    expect(view.getByRole("button", { name: "同意" })).toBeInTheDocument();

    const input = await view.findByPlaceholderText("输入拒绝理由，Enter 拒绝此次调用");
    await fireEvent.update(input, "先不要写这个文件");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "tool-t-002",
        kind: "tool_consent",
        result: {
          taskId: "t-002",
          requestId: "tool-t-002",
          decision: "deny",
          message: "先不要写这个文件",
        },
      }, undefined);
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(false);
  });

  it("统一 Codex 工具确认显示在 composer 内部，并用统一命令回写", async () => {
    const view = await renderTaskDetail();

    emitUnifiedToolConsentRequest("t-002");

    await view.findByRole("alert");
    expect(view.container.querySelector(".chat-composer .composer-inline--tool"))
      .toBeInTheDocument();

    const input = await view.findByPlaceholderText("输入拒绝理由，Enter 拒绝此次调用");
    await fireEvent.update(input, "先不跑测试");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "unified-tool-t-002",
        kind: "tool_consent",
        result: {
          taskId: "t-002",
          requestId: "unified-tool-t-002",
          decision: "deny",
          message: "先不跑测试",
        },
      }, undefined);
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(false);
  });

  it("重进任务页后会从持久化 timeline 恢复 ask_user 卡片", async () => {
    replaceMockTimelineEvents("t-002", [{
      id: "ask-card-t-002",
      taskId: "t-002",
      turnId: "turn-ask",
      backend: "claude",
      kind: "ask_user",
      status: "requires_action",
      title: "Claude 想确认一下",
      summary: askUserSpec.questions[0]?.question ?? "",
      payload: {
        backend: "claude",
        interaction: "ask_user",
        requestId: "ask-t-002",
        questions: askUserSpec.questions,
        spec: askUserSpec,
      },
      createdAt: 10_000,
      updatedAt: 10_000,
      turnSeq: 1,
      intraTurnOrder: 0,
    }]);

    const view = await renderTaskDetail();

    const prompt = await view.findByRole("region", { name: "Claude 想确认一下" });
    expect(prompt).toHaveClass("composer-inline");

    await fireEvent.click(view.getByRole("radio", { name: "B" }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await expectAskUserResponse("t-002");
  });

  it("重进任务页后会从持久化 timeline 恢复工具授权卡片", async () => {
    replaceMockTimelineEvents("t-002", [persistedToolConsentEvent()]);

    const view = await renderTaskDetail();

    await view.findByRole("alert");
    expect(view.container.querySelector(".chat-composer .composer-inline--tool"))
      .toBeInTheDocument();

    const input = await view.findByPlaceholderText("输入拒绝理由，Enter 拒绝此次调用");
    await fireEvent.update(input, "先不要写这个文件");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "tool-t-002",
        kind: "tool_consent",
        result: {
          taskId: "t-002",
          requestId: "tool-t-002",
          decision: "deny",
          message: "先不要写这个文件",
        },
      }, undefined);
    });
  });

  it("恢复出的工具授权响应失败后保留卡片可再次操作", async () => {
    replaceMockTimelineEvents("t-002", [persistedToolConsentEvent()]);
    failNextMockAgentInteractionResponse("runtime unavailable");

    const view = await renderTaskDetail();

    await view.findByRole("alert");
    const input = await view.findByPlaceholderText("输入拒绝理由，Enter 拒绝此次调用");
    await fireEvent.update(input, "先不要写这个文件");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", expect.objectContaining({
        taskId: "t-002",
        requestId: "tool-t-002",
        kind: "tool_consent",
      }), undefined);
    });
    expect(view.container.querySelector(".chat-composer .composer-inline--tool"))
      .toBeInTheDocument();
  });

  it("重进任务页后会从持久化 timeline 恢复 Codex MCP 确认卡片", async () => {
    replaceMockTimelineEvents("t-002", [{
      id: "mcp-card-t-002",
      taskId: "t-002",
      turnId: "turn-mcp",
      backend: "codex",
      kind: "mcp",
      status: "requires_action",
      title: "Linear MCP",
      summary: "选择项目",
      payload: {
        interaction: "mcp_elicitation",
        requestId: "mcp-t-002",
        threadId: "thread-1",
        turnId: "turn-mcp",
        serverName: "linear",
        mode: "form",
        message: "选择项目",
        requestedSchema: {
          type: "object",
          properties: {
            project: {
              type: "string",
              title: "项目",
              enum: ["A", "B"],
            },
          },
          required: ["project"],
        },
      },
      createdAt: 10_000,
      updatedAt: 10_000,
      turnSeq: 1,
      intraTurnOrder: 0,
    }]);

    const view = await renderCodexTaskDetail();

    const prompt = await view.findByRole("region", { name: "MCP 确认" });
    const promptView = within(prompt);
    await fireEvent.update(promptView.getByRole("combobox"), "B");
    await fireEvent.click(promptView.getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "mcp-t-002",
        kind: "mcp_elicitation",
        result: {
          action: "accept",
          content: { project: "B" },
        },
      }, undefined);
    });
  });

  it("重进任务页后会从持久化 timeline 恢复 Codex 权限确认卡片", async () => {
    replaceMockTimelineEvents("t-002", [{
      id: "permission-card-t-002",
      taskId: "t-002",
      turnId: "turn-permission",
      backend: "codex",
      kind: "diagnostic",
      status: "requires_action",
      title: "权限确认",
      summary: "need network",
      payload: {
        interaction: "permission_approval",
        requestId: "permission-t-002",
        threadId: "thread-1",
        turnId: "turn-permission",
        itemId: "item-1",
        startedAtMs: 123,
        cwd: "C:/repo",
        reason: "need network",
        permissions: {
          network: { domains: [{ domain: "example.com" }] },
        },
      },
      createdAt: 10_000,
      updatedAt: 10_000,
      turnSeq: 1,
      intraTurnOrder: 0,
    }]);

    const view = await renderCodexTaskDetail();

    const prompt = await view.findByRole("region", { name: "权限确认" });
    await fireEvent.click(within(prompt).getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "permission-t-002",
        kind: "permission_approval",
        result: {
          action: "approve",
          permissions: {
            network: { domains: [{ domain: "example.com" }] },
          },
          scope: "turn",
        },
      }, undefined);
    });
  });

  it("runtime abandoned 时清理恢复出的 pending 交互并允许重新发送", async () => {
    replaceMockTimelineEvents("t-002", [persistedToolConsentEvent()]);
    setMockRuntimeSnapshot("t-002", {
      phase: "abandoned",
      runtimeChannel: "mutsuki_core",
      backend: "codex",
      turnId: "turn-tool",
    });

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.container.querySelector(".chat-composer .composer-inline--tool"))
        .not.toBeInTheDocument();
    });
    await setComposerText(view, "继续");
    await fireEvent.click(view.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(true);
    });
  });

  it("统一 Codex 命令确认可编辑命令并回写 updatedInput", async () => {
    const view = await renderTaskDetail();

    emitUnifiedToolConsentRequest("t-002");

    const prompt = await view.findByRole("alert");
    const promptView = within(prompt);
    expect(promptView.getByText("COMMAND")).toBeInTheDocument();

    await fireEvent.click(promptView.getByRole("button", { name: "编辑完整命令" }));
    await fireEvent.update(promptView.getByRole("textbox", { name: "编辑命令" }), "yarn test --runInBand");
    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "unified-tool-t-002",
        kind: "tool_consent",
        result: {
          taskId: "t-002",
          requestId: "unified-tool-t-002",
          decision: "allow",
          message: null,
          updatedInput: { command: "yarn test --runInBand" },
        },
      }, undefined);
    });
  });

  it("Codex review 和 fix suggestion 从用户入口透传完整 workflow 到发送命令", async () => {
    const view = await renderCodexTaskDetail();

    await setComposerText(view, "重点看权限边界");
    await fireEvent.click(view.getByRole("button", { name: "代码审查" }));
    await fireEvent.click(view.getByRole("menuitem", { name: /未提交改动/ }));

    await expectLatestChatSend({
      content: "重点看权限边界",
      workflow: {
        type: "lilia_review",
        target: { type: "uncommittedChanges" },
        instructions: "重点看权限边界",
        delivery: "inline",
      },
    });

    finishCodexWorkflow("thread-review");

    await setComposerText(view, "优先给最小修复");
    await fireEvent.click(view.getByRole("button", { name: "修复建议" }));
    await fireEvent.click(view.getByRole("menuitem", { name: /未提交改动/ }));

    await expectLatestChatSend({
      content: "优先给最小修复",
      workflow: {
        type: "lilia_fix_suggestion",
        target: { type: "uncommittedChanges" },
        instructions: "优先给最小修复",
        mode: "suggest",
      },
    });
  });

  it("Codex timeline batch apply 入口透传来源 turn、类型和摘要", async () => {
    const view = await renderCodexTaskDetail();

    emitMockTimelineEvent("t-002", {
      id: "reply-apply-1",
      taskId: "t-002",
      turnId: "turn-source",
      backend: "codex",
      kind: "message",
      status: "success",
      title: "Assistant",
      summary: "建议修复权限边界",
      payload: {
        role: "assistant",
        backend: "codex",
        content: "建议修复权限边界",
        workflowSource: {
          sourceKind: "fix_suggestion",
          codexTurnId: "codex-turn-1",
        },
      },
      createdAt: 10_000,
      updatedAt: 10_000,
      turnSeq: 1,
      intraTurnOrder: 0,
    });

    await fireEvent.click(await view.findByRole("button", { name: "应用建议" }));

    await expectLatestChatSend({
      content: "",
      workflow: {
        type: "lilia_batch_apply",
        sourceTurnId: "turn-source",
        sourceKind: "fix_suggestion",
        sourceSummary: "建议修复权限边界",
      },
    });
  });

  it("Codex compact 入口以 workflow 进入发送命令", async () => {
    const view = await renderCodexTaskDetail();

    await fireEvent.click(view.getByRole("button", { name: "压缩上下文" }));
    await expectLatestChatSend({
      content: "",
      workflow: { type: "lilia_compact" },
    });
  });


  it("Bash 工具授权在 composer 中可编辑命令并同意回写 updatedInput", async () => {
    const view = await renderTaskDetail();

    emitBashToolConsentRequest("t-002");

    const prompt = await view.findByRole("alert");
    const promptView = within(prompt);
    expect(promptView.getByText("COMMAND")).toBeInTheDocument();
    expect(promptView.getByRole("button", { name: "编辑完整命令" })).toHaveTextContent("pwd");

    await fireEvent.click(promptView.getByRole("button", { name: "编辑完整命令" }));
    await fireEvent.update(promptView.getByRole("textbox", { name: "编辑命令" }), "pwd && echo ok");
    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
        taskId: "t-002",
        requestId: "bash-tool-t-002",
        kind: "tool_consent",
        result: {
          taskId: "t-002",
          requestId: "bash-tool-t-002",
          decision: "allow",
          message: null,
          updatedInput: { command: "pwd && echo ok" },
        },
      }, undefined);
    });
  });


  it("后端真实消息到达后不残留同内容 queued 乐观消息", async () => {
    const view = await renderTaskDetail();
    setMockChatRunning("t-002", true);
    emitTauriEvent("chat:turn-started", { taskId: "t-002", queuedCount: 0 });

    const sentContent = [
      "[Lilia 引导]",
      "优先级：中",
      "",
      "需要后端确认的消息",
    ].join("\n");
    await setComposerText(view, "需要后端确认的消息");
    await fireEvent.click(view.getByRole("button", { name: "加入调度队列" }));

    await waitFor(() => {
      expect(view.getByText("需要后端确认的消息")).toBeInTheDocument();
    });

    replaceMockTimelineEvents("t-002", [
      {
        id: "u-confirmed",
        kind: "message",
        status: "success",
        title: "用户输入",
        summary: sentContent,
        payload: {
          role: "user",
          content: sentContent,
          attachments: [],
          queued: false,
        },
        createdAt: 11_000,
        updatedAt: 11_000,
      },
    ]);
    await view.router.push("/projects/lilia/tasks/t-001");
    await view.rerender({ projectId: "lilia", taskId: "t-001" });
    await view.router.push("/projects/lilia/tasks/t-002");
    await view.rerender({ projectId: "lilia", taskId: "t-002" });

    await waitFor(() => {
      expect(view.container.querySelectorAll(".chat-bubble__content"))
        .toHaveLength(1);
    });
    expect(view.container.querySelector(".chat-bubble.is-queued")).toBeNull();
    expect(view.getByText("需要后端确认的消息")).toBeInTheDocument();
  });

  describe("cold-start rollback recovery", () => {
    it("持久化 rollback 草稿在冷启动时恢复出草稿内容并 ack", async () => {
      setMockRuntimeSnapshot("t-002", {
        phase: "idle",
        runtimeChannel: null,
        backend: null,
        turnId: null,
        pendingRollback: true,
        rollback: {
          rolledBack: true,
          restoredContent: "恢复的草稿内容",
          restoredAttachments: [],
          removedEventIds: ["evt-original"],
        },
      });
      replaceMockTimelineEvents("t-002", []);

      const view = await renderCodexTaskDetail("t-002", { clearMockCalls: false });
      await waitFor(() => {
        expect(view.getByText("恢复的草稿内容")).toBeInTheDocument();
      });
      const ackCalls = mockInvoke.mock.calls.filter(
        ([cmd]) => cmd === "chat_ack_restored_rollback"
      );
      expect(ackCalls.length).toBeGreaterThanOrEqual(1);
    });

    it("无 rollback 的冷启动不清除预期 composer 内容", async () => {
      setMockRuntimeSnapshot("t-002", {
        phase: "idle",
        rollback: null,
      });
      replaceMockTimelineEvents("t-002", []);

      const view = await renderCodexTaskDetail("t-002", { clearMockCalls: false });
      await waitFor(() => {
        expect(view.getByRole("textbox")).toBeInTheDocument();
      });
      const ackCalls = mockInvoke.mock.calls.filter(
        ([cmd]) => cmd === "chat_ack_restored_rollback"
      );
      expect(ackCalls.length).toBe(0);
    });
  });
});
