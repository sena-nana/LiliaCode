import { fireEvent, render, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { AskUserSpec } from "@lilia/contracts";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { installAgentAskUserBridge } from "../src/composables/useAgentAskUserBridge";
import { resolveAskUser, useAskUser } from "../src/composables/useAskUser";
import { installToolConsentBridge } from "../src/composables/useToolConsentBridge";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import { setAgentInteractionSettings } from "../src/services/chat";
import { createTodo } from "../src/services/todos";
import {
  emitTauriEvent,
  emitMockTimelineEvent,
  mockInvoke,
  replaceMockTimelineEvents,
  setMockChatRunning,
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

function emitAskUserRequest(
  taskId: string,
  spec: AskUserSpec = askUserSpec,
  turnId = "turn-ask",
) {
  emitTauriEvent("chat:ask-user-request", {
    taskId,
    turnId,
    backend: "claude",
    requestId: `ask-${taskId}`,
    spec,
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
  emitTauriEvent("chat:ask-user-request", {
    taskId,
    turnId: "turn-plan",
    backend: "claude",
    requestId: `ask-${taskId}`,
    spec: {
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

function emitPlanTimelineEvent(taskId: string) {
  emitMockTimelineEvent(taskId, {
    id: `plan-card-${taskId}`,
    kind: "plan",
    status: "requires_action",
    title: "ExitPlanMode",
    summary: "当前计划等待确认",
    turnId: "turn-plan",
    payload: {
      plan: "## 当前计划\n- 先改 runner\n- 再补测试",
      approved: null,
      executionPermission: "ask",
    },
  });
}

function emitSnakeCaseAskUserRequest(taskId: string) {
  emitTauriEvent("chat:ask-user-request", {
    task_id: taskId,
    turn_id: "turn-ask",
    backend: "claude",
    request_id: `ask-${taskId}`,
    spec: askUserSpec,
  });
}

function emitToolConsentRequest(taskId: string) {
  emitTauriEvent("chat:tool-consent-request", {
    taskId,
    turnId: "turn-tool",
    backend: "claude",
    requestId: `tool-${taskId}`,
    toolName: "Write",
    input: { file_path: "src/main.ts" },
    title: null,
    displayName: null,
    description: null,
    blockedPath: null,
    decisionReason: null,
    toolUseId: null,
  });
}

function emitToolConsentTimelineEvent(taskId: string) {
  emitMockTimelineEvent(taskId, {
    id: `tool-card-${taskId}`,
    kind: "file_change",
    status: "requires_action",
    title: "Write",
    summary: "src/main.ts",
    turnId: "turn-tool",
    payload: {
      backend: "claude",
      interaction: "tool_consent",
      requestId: `tool-${taskId}`,
      toolName: "Write",
      input: { file_path: "src/main.ts" },
      path: "src/main.ts",
      permissionRequest: true,
    },
  });
}

function emitBashToolConsentRequest(taskId: string) {
  emitTauriEvent("chat:tool-consent-request", {
    taskId,
    turnId: "turn-bash",
    backend: "claude",
    requestId: `bash-tool-${taskId}`,
    toolName: "Bash",
    input: { command: "pwd" },
    title: null,
    displayName: null,
    description: null,
    blockedPath: null,
    decisionReason: null,
    toolUseId: "bash-tool-use",
  });
}

function emitBashToolConsentTimelineEvent(taskId: string) {
  emitMockTimelineEvent(taskId, {
    id: `bash-tool-card-${taskId}`,
    kind: "command",
    status: "requires_action",
    title: "Bash",
    summary: "pwd",
    turnId: "turn-bash",
    payload: {
      backend: "claude",
      interaction: "tool_consent",
      requestId: `bash-tool-${taskId}`,
      toolName: "Bash",
      input: { command: "pwd" },
      command: "pwd",
      permissionRequest: true,
    },
  });
}

async function expectAskUserResponse(taskId: string) {
  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_ask_user", {
      taskId,
      requestId: `ask-${taskId}`,
      result: {
        cancelled: false,
        answers: {
          "q-1": { questionId: "q-1", value: "o-2" },
        },
      },
    }, undefined);
  });
}

let unlistenAskUser: (() => void) | null = null;
let unlistenToolConsent: (() => void) | null = null;

describe("chat AskUser prompt", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
    unlistenAskUser = await installAgentAskUserBridge();
    unlistenToolConsent = await installToolConsentBridge();
  });

  afterEach(async () => {
    const { state } = useAskUser();
    while (state.current || state.queue.length) {
      resolveAskUser({ answers: {}, cancelled: true });
    }
    await Promise.resolve();

    unlistenAskUser?.();
    unlistenToolConsent?.();
    unlistenAskUser = null;
    unlistenToolConsent = null;
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
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_ask_user", {
        taskId: "t-002",
        requestId: "ask-t-002",
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
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_tool_consent", {
        taskId: "t-002",
        requestId: "tool-t-002",
        decision: "deny",
        message: "先不要写这个文件",
        updatedInput: null,
      }, undefined);
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(false);
  });

  it("非打断模式把工具授权留在时间线卡片并从卡片回写拒绝理由", async () => {
    await enableNonInterruptMode();
    const view = await renderTaskDetail();

    emitToolConsentRequest("t-002");
    emitToolConsentTimelineEvent("t-002");

    await waitFor(() => {
      expect(view.container.querySelector(".chat-composer .composer-inline--tool")).toBeNull();
    });
    const prompt = view.container.querySelector(".timeline-pending-action.composer-inline--tool");
    expect(prompt).not.toBeNull();
    expect(prompt).toHaveClass("timeline-pending-action");
    expect(view.getByRole("button", { name: "添加附件" })).toBeInTheDocument();

    expect(view.getByRole("button", { name: "忽略" })).toBeDisabled();
    await fireEvent.update(view.getByPlaceholderText("拒绝理由"), "先不要写这个文件");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_tool_consent", {
        taskId: "t-002",
        requestId: "tool-t-002",
        decision: "deny",
        message: "先不要写这个文件",
        updatedInput: null,
      }, undefined);
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
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_tool_consent", {
        taskId: "t-002",
        requestId: "bash-tool-t-002",
        decision: "allow",
        message: null,
        updatedInput: { command: "pwd && echo ok" },
      }, undefined);
    });
  });

  it("非打断模式的 Bash 时间线待授权卡片复用可编辑命令入口", async () => {
    await enableNonInterruptMode();
    const view = await renderTaskDetail();

    emitBashToolConsentRequest("t-002");
    emitBashToolConsentTimelineEvent("t-002");

    await waitFor(() => {
      expect(view.container.querySelector(".chat-composer .composer-inline--tool")).toBeNull();
    });
    const prompt = view.container.querySelector(".timeline-pending-action.composer-inline--tool");
    expect(prompt).not.toBeNull();
    const promptView = within(prompt as HTMLElement);

    expect(promptView.getByText("COMMAND")).toBeInTheDocument();
    expect(promptView.getByRole("button", { name: "编辑完整命令" })).toHaveTextContent("pwd");

    await fireEvent.click(promptView.getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_tool_consent", {
        taskId: "t-002",
        requestId: "bash-tool-t-002",
        decision: "allow",
        message: null,
        updatedInput: null,
      }, undefined);
    });
  });

  it("非打断模式下没有运行时 pending 的待处理卡片显示已失效", async () => {
    await enableNonInterruptMode();
    emitToolConsentTimelineEvent("t-002");
    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("已失效")).toBeInTheDocument();
    });
    expect(view.queryByPlaceholderText("拒绝理由")).toBeNull();
    expect(view.container.querySelector(".chat-composer .composer-inline--tool")).toBeNull();
  });

  it("加载历史对话时不会展开待确认计划卡片", async () => {
    emitMockTimelineEvent("t-002", {
      id: "plan-loaded",
      kind: "plan",
      status: "requires_action",
      title: "ExitPlanMode",
      turnId: "turn-loaded",
      payload: {
        plan: "## 已加载计划\n- 等待确认上下文",
        approved: null,
        executionPermission: "ask",
      },
    });

    const view = await renderTaskDetail();
    const toggle = await view.findByRole("button", { name: /等待确认计划/ });

    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(view.container.querySelector(".timeline-card--plan"))
      .toHaveClass("is-collapsed");
  });

  it("当前计划确认请求匹配时才展开计划卡片", async () => {
    emitMockTimelineEvent("t-002", {
      id: "plan-active",
      kind: "plan",
      status: "requires_action",
      title: "ExitPlanMode",
      turnId: "turn-plan",
      payload: {
        plan: "## 当前计划\n- 等用户确认",
        approved: null,
        executionPermission: "ask",
      },
    });
    const view = await renderTaskDetail();

    const toggle = await view.findByRole("button", { name: /等待确认计划/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");

    emitPlanApprovalRequest("t-002");

    await waitFor(() => {
      expect(view.getByRole("button", { name: /等待确认计划/ }))
        .toHaveAttribute("aria-expanded", "true");
      expect(view.container.querySelector(".timeline-card--plan"))
        .toHaveClass("is-expanded");
    });
  });

  it("非打断模式把计划确认留在计划卡片并可同意", async () => {
    await enableNonInterruptMode();
    emitPlanTimelineEvent("t-002");
    const view = await renderTaskDetail();

    emitPlanApprovalRequest("t-002");

    await waitFor(() => {
      expect(view.container.querySelector(".chat-composer .composer-inline--plan")).toBeNull();
      expect(view.container.querySelector(".timeline-pending-action--plan")).not.toBeNull();
    });
    const action = view.container.querySelector(".timeline-pending-action--plan");
    expect(action).not.toBeNull();
    expect(action).toHaveClass("timeline-pending-action");
    expect(view.getByRole("region", { name: "确认 Claude 计划" })).toBe(action);
    expect(view.container.querySelector(".chat-composer__rich-input"))
      .toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_ask_user", {
        taskId: "t-002",
        requestId: "ask-t-002",
        result: {
          cancelled: false,
          answers: {
            "approve-plan": { questionId: "approve-plan", value: "yes" },
          },
        },
      }, undefined);
    });
  });

  it("非打断模式的计划卡片可提交修改要求", async () => {
    await enableNonInterruptMode();
    emitPlanTimelineEvent("t-002");
    const view = await renderTaskDetail();

    emitPlanApprovalRequest("t-002");

    await waitFor(() => {
      expect(view.container.querySelector(".timeline-pending-action--plan")).not.toBeNull();
    });
    expect(view.getByRole("button", { name: "忽略" })).toBeDisabled();
    await fireEvent.update(view.getByPlaceholderText("修改要求"), "请把测试计划拆细");
    await fireEvent.click(view.getByRole("button", { name: "修改" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_ask_user", {
        taskId: "t-002",
        requestId: "ask-t-002",
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
  });

  it("不会在当前对话显示其他 task 的 Agent 提问", async () => {
    const view = await renderTaskDetail("t-002");

    emitAskUserRequest("t-003");

    await waitFor(() => {
      expect(useAskUser().state.current?.taskId).toBe("t-003");
    });
    expect(view.queryByRole("region", { name: "Claude 想确认一下" })).toBeNull();
    expect(view.container.querySelector(".chat-composer .composer-inline--ask")).toBeNull();
  });

  it("兼容 Tauri 事件里 snake_case 的 task 和 request 字段", async () => {
    const view = await renderTaskDetail();

    emitSnakeCaseAskUserRequest("t-002");

    await view.findByRole("region", { name: "Claude 想确认一下" });
    await fireEvent.click(view.getByRole("radio", { name: "B" }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await expectAskUserResponse("t-002");
  });

  it("快速连续发送的本地 pending 消息不会互相覆盖", async () => {
    const view = await renderTaskDetail();
    setMockChatRunning("t-002", true);
    emitTauriEvent("chat:turn-started", { taskId: "t-002", queuedCount: 0 });

    await createTodo("t-002", "第一条补充");
    await createTodo("t-002", "第二条补充");
    await fireEvent.click(await view.findByRole("button", { name: /立即插入引导：第一条补充/ }));
    await fireEvent.click(await view.findByRole("button", { name: /立即插入引导：第二条补充/ }));

    await waitFor(() => {
      expect(view.getByText("第一条补充")).toBeInTheDocument();
      expect(view.getByText("第二条补充")).toBeInTheDocument();
      expect(view.container.querySelectorAll(".chat-bubble.is-queued")).toHaveLength(2);
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

  it("重新加载历史时间线时不保留后端已删除的旧本地事件", async () => {
    replaceMockTimelineEvents("t-002", [
      {
        id: "stale-command",
        kind: "command",
        status: "success",
        title: "stale command",
        summary: "旧事件",
        payload: { command: "stale command" },
      },
    ]);
    const view = await renderTaskDetail();
    await view.findByText("旧事件");

    replaceMockTimelineEvents("t-002", [
      {
        id: "fresh-command",
        kind: "command",
        status: "success",
        title: "fresh command",
        summary: "新事件",
        payload: { command: "fresh command" },
      },
    ]);
    await view.router.push("/projects/lilia/tasks/t-001");
    await view.rerender({ projectId: "lilia", taskId: "t-001" });
    await view.router.push("/projects/lilia/tasks/t-002");
    await view.rerender({ projectId: "lilia", taskId: "t-002" });

    await waitFor(() => {
      expect(view.getByText("新事件")).toBeInTheDocument();
    });
    expect(view.queryByText("旧事件")).toBeNull();
  });
});
