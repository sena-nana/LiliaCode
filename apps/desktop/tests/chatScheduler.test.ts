import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it, beforeEach, afterEach } from "vitest";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import {
  completeMockAgentTurn,
  emitMockTimelineEvent,
  emitMockTurnCompleted,
  emitTauriEvent,
  emitWebviewDragDropEvent,
  mockInvoke,
  seedMockChatMessages,
  setMockComposerStateHandler,
} from "./tauriMock";
import { createTodo, updateTodo } from "../src/services/todos";
import {
  respondConsent,
  useToolConsentForTask,
} from "../src/composables/useToolConsentBridge";

async function renderTaskDetail() {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push("/projects/lilia/tasks/t-002");
  await router.isReady();

  return render(TaskDetail, {
    props: {
      projectId: "lilia",
      taskId: "t-002",
    },
    global: {
      plugins: [router],
    },
  });
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
  const hasInlineReference = input.querySelector(".chat-file-reference") !== null;
  if (hasInlineReference) {
    input.append(document.createTextNode(` ${text}`));
    placeEditableCaret(input, input.textContent?.length ?? text.length);
  } else {
    input.textContent = text;
    placeEditableCaret(input, text.length);
  }
  await fireEvent.input(input);
  return input;
}

async function sendText(view: ReturnType<typeof render>, text: string) {
  await setComposerText(view, text);
  await fireEvent.click(view.getByRole("button", { name: /发送|加入调度队列/ }));
}

function setChatDropBounds(view: ReturnType<typeof render>) {
  const page = view.container.querySelector(".chat-page") as HTMLElement | null;
  if (!page) throw new Error("未找到聊天页面");
  page.getBoundingClientRect = () => ({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 800,
    bottom: 800,
    width: 800,
    height: 800,
    toJSON: () => ({}),
  });
}

async function expectInitialReasoningHidden(view: ReturnType<typeof render>) {
  await waitFor(() => {
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_timeline_list")).toBe(true);
  });
  expect(view.queryByText("从持久化时间线恢复的公开摘要。")).toBeNull();
}

describe("chat scheduler", () => {
  let unlistenToolConsent: (() => void) | null = null;

  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
  });

  afterEach(async () => {
    const pendingConsent = useToolConsentForTask("t-002").value;
    if (pendingConsent) {
      await respondConsent("t-002", pendingConsent.requestId, "deny", "测试清理");
    }
    unlistenToolConsent?.();
    unlistenToolConsent = null;
  });


  it("只处理落在当前聊天区域内的文件 drop，并随消息发送", async () => {
    const view = await renderTaskDetail();
    setChatDropBounds(view);

    emitWebviewDragDropEvent({
      type: "drop",
      paths: ["D:\\PROJECT\\workspace\\Lilia\\IGNORED.md"],
      position: { x: 900, y: 900 },
    });

    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_describe_attachments"),
    ).toBe(false);
    expect(view.queryByText("IGNORED.md")).not.toBeInTheDocument();

    emitWebviewDragDropEvent({
      type: "drop",
      paths: ["D:\\PROJECT\\workspace\\Lilia\\README.md"],
      position: { x: 120, y: 160 },
    });

    await waitFor(() => {
      expect(view.getByText("README.md")).toBeInTheDocument();
    });

    await sendText(view, "参考附件总结项目");

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message"))
        .toBe(true);
    });
    const send = mockInvoke.mock.calls.find(([cmd]) => cmd === "chat_send_message");
    expect(send?.[1]).toMatchObject({
      attachments: [expect.objectContaining({
        path: "D:\\PROJECT\\workspace\\Lilia\\README.md",
      })],
    });
    expect(send?.[1].content).toContain("[Lilia 引导]");
    expect(send?.[1].content).toContain("参考附件总结项目");
    expect(send?.[1].content).toContain("[文件引用: README.md | D:\\PROJECT\\workspace\\Lilia\\README.md]");
  });


  it("composer 尚未加载完成时发送会等待后端状态再发给 agent", async () => {
    let resolveComposer!: (value: unknown) => void;
    setMockComposerStateHandler((taskId) =>
      new Promise((resolve) => {
        resolveComposer = resolve;
      }).then(() => ({
        taskId,
        backend: "claude",
        model: "claude-sonnet-4-6",
        planMode: false,
        permission: "ask",
      }))
    );

    const view = await renderTaskDetail();
    await sendText(view, "加载期间发送");

    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(false);

    resolveComposer(null);

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message"))
        .toBe(true);
    });
    const send = mockInvoke.mock.calls.find(([cmd]) => cmd === "chat_send_message");
    expect(send?.[1].content).toContain("加载期间发送");
  });


  it("composer 加载失败时不会发送或清空附件", async () => {
    setMockComposerStateHandler(() => Promise.reject(new Error("composer failed")));
    const view = await renderTaskDetail();
    setChatDropBounds(view);

    emitWebviewDragDropEvent({
      type: "drop",
      paths: ["D:\\PROJECT\\workspace\\Lilia\\README.md"],
      position: { x: 120, y: 160 },
    });
    await waitFor(() => {
      expect(view.getByText("README.md")).toBeInTheDocument();
    });

    await sendText(view, "失败时保留附件");

    await waitFor(() => {
      expect(view.getByText(/创建引导失败/)).toBeInTheDocument();
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(false);
    expect(view.getByText("README.md")).toBeInTheDocument();
  });


  it("会把 Agent 运行中追加的用户消息先放入 Lilia 引导，并在 turn 结束后插入", async () => {
    const view = await renderTaskDetail();

    await sendText(view, "先检查当前实现");
    await waitFor(() => {
      expect(view.getByText("先检查当前实现")).toBeInTheDocument();
    });

    await sendText(view, "补充：优先看调度器");
    await waitFor(() => {
      expect(view.getByText("补充：优先看调度器")).toBeInTheDocument();
      expect(view.getByText("补充：优先看调度器").closest(".todo-float"))
        .not.toBeNull();
    });

    const sends = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
    expect(sends).toHaveLength(1);

    completeMockAgentTurn("t-002");

    await waitFor(() => {
      const nextSends = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
      expect(nextSends).toHaveLength(2);
      expect(nextSends[1][1].content).toContain("补充：优先看调度器");
    });
  });


  it("工具类 timeline 窗口会触发高优先级引导，但不会触发普通优先级引导", async () => {
    const view = await renderTaskDetail();

    const high = await createTodo("t-002", "高优先级工具窗口引导");
    await updateTodo(high.id, { priority: "high" });
    await createTodo("t-002", "普通优先级保留");
    mockInvoke.mockClear();

    emitMockTimelineEvent("t-002", {
      id: "tl-command-window",
      kind: "command",
      status: "started",
      title: "Bash",
      summary: "yarn test",
      payload: {
        backend: "claude",
        command: "yarn test",
      },
      turnId: "turn-tool-window",
    });

    await waitFor(() => {
      const sends = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
      expect(sends).toHaveLength(1);
      expect(sends[0][1].content).toContain("高优先级工具窗口引导");
      expect(sends[0][1].content).not.toContain("普通优先级保留");
    });

    await waitFor(() => {
      expect(view.getByText("普通优先级保留")).toBeInTheDocument();
    });
  });


  it("会显示持久化和实时 Agent 工作过程", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-live-command",
      kind: "command",
      status: "running",
      title: "yarn verify",
      summary: "正在运行完整验证",
      payload: { command: "yarn verify" },
      order: 1,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /yarn verify/ })).toBeInTheDocument();
      expect(view.getByText("正在运行完整验证")).toBeInTheDocument();
    });
  });

  it("图片查看器遮罩挂在主内容页层级，覆盖 shell 主区留白", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-image",
        taskId: "t-002",
        role: "user",
        content: "看图",
        attachments: [
          {
            id: "image-1",
            name: "shot.png",
            path: "D:\\PROJECT\\workspace\\Lilia\\shot.png",
            kind: "file",
            size: 42,
            exists: true,
            mime: "image/png",
            directory: null,
          },
        ],
        createdAt: 2000,
      },
    ]);

    const view = await renderTaskDetail();
    const imageButton = await waitFor(() =>
      view.getByRole("button", { name: "查看图片 shot.png" })
    );
    await fireEvent.click(imageButton);

    const viewer = view.getByRole("dialog", { name: "图片查看器" });
    const page = view.container.querySelector(".chat-page");
    const main = view.container.querySelector(".chat-layout__main");

    expect(viewer.parentElement).toBe(page);
    expect(viewer.parentElement).not.toBe(main);
    expect(viewer).toHaveClass("chat-file-drop-overlay");
  });


  it("turn 完成后，最终回复之前的多个工具/计划事件折叠到 final 卡下，点开按钮才显出", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-turn-start",
        taskId: "t-002",
        role: "user",
        content: "请实现时间线折叠",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-hidden-process-command",
      kind: "command",
      status: "error",
      title: "yarn test",
      summary: "命令折叠态预览",
      payload: {
        command: "yarn test",
        stdout: "折叠后的命令详情",
      },
      turnId: "turn-fold",
      createdAt: 2100,
      updatedAt: 3100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-hidden-process-plan",
      kind: "plan",
      status: "completed",
      title: "更新计划",
      summary: "计划折叠态预览",
      payload: {
        plan: "折叠后的计划详情",
      },
      turnId: "turn-fold",
      createdAt: 6200,
      updatedAt: 8100,
      order: 3,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-final-with-hidden-process",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "最终回复应该直接可见。",
      },
      turnId: "turn-fold",
      createdAt: 8200,
      updatedAt: 8200,
      order: 4,
    });
    emitMockTurnCompleted("t-002", "turn-fold", "success", 8300);

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("请实现时间线折叠")).toBeInTheDocument();
      expect(view.getByText("最终回复应该直接可见。")).toBeInTheDocument();
      expect(view.queryByRole("button", { name: /yarn test/ })).toBeNull();
      expect(view.queryByRole("button", { name: /更新计划/ })).toBeNull();
      const toggle = view.getByRole("button", { name: "命令执行、计划更新 · 6 秒" });
      expect(toggle).not.toHaveTextContent("有失败");
      expect(toggle).not.toHaveTextContent(/展开过程|查看过程|2 项/);
      expect(toggle).not.toHaveClass("agent-timeline__process-toggle--failed");
      expect(toggle).toHaveAttribute("aria-expanded", "false");
    });

    const finalItem = view.getByText("最终回复应该直接可见。")
      .closest(".agent-timeline__item");
    const processCollapse = finalItem?.querySelector(".agent-timeline__process-collapse");
    expect(processCollapse).not.toBeNull();
    expect(processCollapse).toHaveAttribute("aria-hidden", "true");
    expect(processCollapse).toHaveAttribute("inert");
    expect(processCollapse).not.toHaveClass("is-open");
    expect(processCollapse?.querySelectorAll(".agent-timeline__item.is-process-child"))
      .toHaveLength(2);

    await fireEvent.click(view.getByRole("button", { name: "命令执行、计划更新 · 6 秒" }));

    await waitFor(() => {
      expect(processCollapse).toHaveAttribute("aria-hidden", "false");
      expect(processCollapse).not.toHaveAttribute("inert");
      expect(processCollapse).toHaveClass("is-open");
      expect(view.getByRole("button", { name: /yarn test/ })).toBeInTheDocument();
      expect(view.getByRole("button", { name: /更新计划/ })).toBeInTheDocument();
    });

    // 展开态下按 order 排：工具/计划（2、3）在 final（4）上方。
    const timelineText = finalItem?.textContent ?? "";
    expect(timelineText.indexOf("yarn test"))
      .toBeLessThan(timelineText.indexOf("最终回复应该直接可见。"));
    expect(timelineText.indexOf("更新计划"))
      .toBeLessThan(timelineText.indexOf("最终回复应该直接可见。"));

    await fireEvent.click(view.getByRole("button", { name: /yarn test/ }));
    await fireEvent.click(view.getByRole("button", { name: /更新计划/ }));
    await waitFor(() => {
      expect(view.getByText("折叠后的命令详情")).toBeInTheDocument();
      expect(view.getByText("折叠后的计划详情")).toBeInTheDocument();
    });
  });


  it("agent 错误走 timeline 错误事件，不再创建 system 普通气泡", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-agent-error",
      kind: "error",
      status: "error",
      title: "错误",
      summary: "agent 报错：连接失败",
      payload: {
        message: "agent 报错：连接失败",
      },
      order: 1,
    });
    emitTauriEvent("chat:error", {
      taskId: "t-002",
      message: "旧错误通道不应生成气泡",
    });

    await waitFor(() => {
      expect(view.getByText("agent 报错：连接失败")).toBeInTheDocument();
    });
    expect(view.getByText("agent 报错：连接失败").closest(".chat-bubble")).toBeNull();
    expect(view.queryByText("旧错误通道不应生成气泡")).toBeNull();
  });
});
