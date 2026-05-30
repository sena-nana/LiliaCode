import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { installAgentAskUserBridge } from "../src/composables/useAgentAskUserBridge";
import { resolveAskUser, useAskUser } from "../src/composables/useAskUser";
import { installToolConsentBridge } from "../src/composables/useToolConsentBridge";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import {
  emitTauriEvent,
  emitMockTimelineEvent,
  mockInvoke,
} from "./tauriMock";

const askUserSpec = {
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

  return render(TaskDetail, {
    props: {
      projectId: "lilia",
      taskId,
    },
    global: {
      plugins: [router],
    },
  });
}

function emitAskUserRequest(taskId: string) {
  emitTauriEvent("chat:ask-user-request", {
    taskId,
    turnId: "turn-ask",
    backend: "claude",
    requestId: `ask-${taskId}`,
    spec: askUserSpec,
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

  it("计划确认挂起时，输入框发送文本会回写为计划修改要求而不是新消息", async () => {
    const view = await renderTaskDetail();

    emitPlanApprovalRequest("t-002");

    const prompt = await view.findByRole("region", { name: "确认 Claude 计划" });
    expect(prompt).toHaveClass("composer-inline", "composer-inline--plan");
    expect(view.queryByRole("button", { name: "先不执行" })).toBeNull();
    expect(view.getByRole("button", { name: "忽略" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "同意" })).toBeInTheDocument();

    const input = await view.findByPlaceholderText("输入修改要求，Enter 退回计划");
    await fireEvent.update(input, "请把测试计划拆细");
    await fireEvent.click(view.getByRole("button", { name: "发送计划修改要求" }));

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
    expect(view.getByRole("button", { name: "忽略" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "同意" })).toBeInTheDocument();

    const input = await view.findByPlaceholderText("输入拒绝理由，Enter 拒绝此次调用");
    await fireEvent.update(input, "先不要写这个文件");
    await fireEvent.click(view.getByRole("button", { name: "发送拒绝备注" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("chat_respond_tool_consent", {
        taskId: "t-002",
        requestId: "tool-t-002",
        decision: "deny",
        message: "先不要写这个文件",
      }, undefined);
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(false);
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
});
