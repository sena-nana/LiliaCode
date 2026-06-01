import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it, beforeEach } from "vitest";
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

function chatTranscriptElement(view: ReturnType<typeof render>): HTMLElement {
  const transcript = view.container.querySelector(".chat-transcript");
  if (!(transcript instanceof HTMLElement)) {
    throw new Error("未找到聊天记录滚动容器");
  }
  return transcript;
}

function mockChatScrollGeometry(
  element: HTMLElement,
  values: {
    clientHeight: number;
    scrollHeight: number;
    scrollTop: number;
  },
) {
  Object.defineProperty(element, "clientHeight", {
    configurable: true,
    value: values.clientHeight,
  });
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    value: values.scrollHeight,
  });
  element.scrollTop = values.scrollTop;
}

async function expectInitialReasoningHidden(view: ReturnType<typeof render>) {
  await waitFor(() => {
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_timeline_list")).toBe(true);
  });
  expect(view.queryByText("从持久化时间线恢复的公开摘要。")).toBeNull();
}

describe("chat scheduler", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
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

  it("@ 搜索选中的文件作为路径上下文进入 Lilia 引导", async () => {
    const view = await renderTaskDetail();
    const input = await setComposerText(view, "总结这个文件 @read");

    await waitFor(() => {
      expect(view.getAllByText("README.md").length).toBeGreaterThan(0);
    });
    await fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(view.getByRole("textbox")).toHaveTextContent("README.md");
    });

    await fireEvent.click(view.getByRole("button", { name: /发送|加入调度队列/ }));

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
    expect(send?.[1].content).toContain("总结这个文件");
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

  it("运行中手动插入的引导会保持 queued，直到 queued turn 真正启动才变 sent", async () => {
    const view = await renderTaskDetail();

    await sendText(view, "先检查当前实现");
    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message"))
        .toBe(true);
    });

    const guide = await createTodo("t-002", "排队后再处理");
    const insert = await view.findByRole("button", { name: /立即插入引导：排队后再处理/ });
    await fireEvent.click(insert);

    await waitFor(() => {
      const row = view.getByText("排队后再处理").closest(".todo-float__row");
      expect(row).toHaveTextContent("已排队");
      expect(row).not.toHaveTextContent("已插入");
    });

    const sends = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
    expect(sends).toHaveLength(2);
    expect(sends[1][1]).toMatchObject({ guideId: guide.id });

    completeMockAgentTurn("t-002");

    await waitFor(() => {
      const row = view.getByText("排队后再处理").closest(".todo-float__row");
      expect(row).toHaveTextContent("已插入");
      expect(row).not.toHaveTextContent("已排队");
    });
  });

  it("打断会把已排队但未启动的引导恢复为 pending，可再次手动插入", async () => {
    const view = await renderTaskDetail();

    await sendText(view, "先检查当前实现");
    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message"))
        .toBe(true);
    });

    await createTodo("t-002", "打断后保留");
    await fireEvent.click(await view.findByRole("button", { name: /立即插入引导：打断后保留/ }));

    await waitFor(() => {
      expect(view.getByText("打断后保留").closest(".todo-float__row"))
        .toHaveTextContent("已排队");
    });

    await fireEvent.click(view.getByRole("button", { name: "打断 Agent" }));

    await waitFor(() => {
      expect(view.getByText("打断后保留").closest(".todo-float__row"))
        .not.toHaveTextContent("已排队");
    });

    await fireEvent.click(view.getByRole("button", { name: /立即插入引导：打断后保留/ }));

    await waitFor(() => {
      const sends = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
      expect(sends).toHaveLength(3);
    });
  });

  it("空输入打断当前 turn 后不自动执行 queued 消息", async () => {
    const view = await renderTaskDetail();

    await sendText(view, "先检查当前实现");
    await waitFor(() => {
      expect(view.getByText("先检查当前实现")).toBeInTheDocument();
    });

    await sendText(view, "补充：优先看调度器");
    await waitFor(() => {
      expect(view.getByText("补充：优先看调度器").closest(".todo-float"))
        .not.toBeNull();
    });

    await fireEvent.click(view.getByRole("button", { name: "打断 Agent" }));

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.some(([cmd, args]) =>
          cmd === "chat_interrupt_turn" && args.taskId === "t-002"
        ),
      ).toBe(true);
      expect(view.getByRole("button", { name: /Agent 已打断/ }))
        .toBeInTheDocument();
    });
    const sends = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
    expect(sends).toHaveLength(1);
    expect(view.getByText("补充：优先看调度器").closest(".todo-float"))
      .not.toBeNull();
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

  it("初始加载完成后仍保留刚发送的 Lilia 引导", async () => {
    const view = await renderTaskDetail();

    await sendText(view, "页面刚打开时发送的用户消息");

    await expectInitialReasoningHidden(view);
    await waitFor(() => {
      expect(view.getByText("页面刚打开时发送的用户消息")).toBeInTheDocument();
    });

    expect(view.getByText("页面刚打开时发送的用户消息").closest(".todo-float"))
      .not.toBeNull();
  });

  it("用户停在历史位置时，发送普通消息后自动滚动到底部", async () => {
    const view = await renderTaskDetail();
    await expectInitialReasoningHidden(view);
    const transcript = chatTranscriptElement(view);
    mockChatScrollGeometry(transcript, {
      clientHeight: 240,
      scrollHeight: 1000,
      scrollTop: 100,
    });

    await fireEvent.scroll(transcript);
    await sendText(view, "发送后应该贴到底部");

    await waitFor(() => {
      expect(view.getByText("发送后应该贴到底部")).toBeInTheDocument();
      expect(transcript.scrollTop).toBe(1000);
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

  it("kind=turn 和 reasoning 不在 timeline 中渲染", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-hidden-turn",
      kind: "turn",
      status: "success",
      title: "Claude turn completed",
      summary: "",
      payload: { backend: "claude", sessionId: "session-x" },
      order: 1,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-hidden-reasoning",
      kind: "reasoning",
      status: "running",
      title: "思考中",
      summary: "这段思考内容不应进入主时间线。",
      payload: { text: "这段思考内容不应进入主时间线。" },
      order: 2,
    });

    await waitFor(() => {
      expect(view.queryByText("Claude turn completed")).toBeNull();
      expect(view.queryByText("这段思考内容不应进入主时间线。")).toBeNull();
    });
  });

  it("Agent 打断以错误事件显示，并折叠同 turn 过程", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-interrupt",
        taskId: "t-002",
        role: "user",
        content: "请运行验证",
        createdAt: 2000,
      },
    ]);

    const view = await renderTaskDetail();
    await waitFor(() => {
      expect(view.getByText("请运行验证")).toBeInTheDocument();
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-interrupt-command",
      kind: "command",
      status: "running",
      title: "yarn verify",
      summary: "正在运行完整验证",
      payload: {
        command: "yarn verify",
        stdout: "验证输出详情",
      },
      turnId: "turn-interrupt",
      createdAt: 2100,
      updatedAt: 3100,
      order: 1,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "false");
    });
    await fireEvent.click(view.getByRole("button", { name: /yarn verify/ }));
    await waitFor(() => {
      expect(view.getByText("验证输出详情")).toBeInTheDocument();
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-interrupt-error",
      kind: "error",
      status: "error",
      title: "Agent 已打断",
      summary: "用户打断了当前 Agent 运行",
      payload: {
        backend: "claude",
        interrupted: true,
        message: "用户打断了当前 Agent 运行",
      },
      turnId: "turn-interrupt",
      createdAt: 3200,
      updatedAt: 3200,
      order: 2,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /Agent 已打断/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByRole("button", { name: "命令执行 · 1 秒" }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.queryByRole("button", { name: /yarn verify/ })).toBeNull();
      expect(view.queryByText("验证输出详情")).toBeNull();
    });

    await fireEvent.click(view.getByRole("button", { name: "命令执行 · 1 秒" }));
    await waitFor(() => {
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.queryByText("验证输出详情")).toBeNull();
    });
  });

  it("turn 在跑且尚未流式回复时，timeline 末尾显示「思考中…」指示器", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    await sendText(view, "去想一下怎么做");
    await waitFor(() => {
      expect(view.getByText("思考中…")).toBeInTheDocument();
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-stream-start",
      kind: "message",
      status: "running",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "开始回复",
      },
      turnId: "turn-thinking",
      order: 5,
    });

    await waitFor(() => {
      expect(view.getByText("开始回复")).toBeInTheDocument();
      expect(view.queryByText("思考中…")).toBeNull();
    });

    completeMockAgentTurn("t-002");

    await waitFor(() => {
      expect(view.queryByText("思考中…")).toBeNull();
    });
  });

  it("Claude tool result 事件（payload.toolName=Bash + 顶层 command + output）折叠态显示指令而不是输出", async () => {
    // 复现 runner emit 的 Bash 完成事件形态：summary="" 不再被 output 占用、payload
    // 同时带 toolName/command/output。派生器要能从顶层 command 拼出预览，而不是被
    // Claude normalizer 用空 payload.input 折出空 command 后回退到 output。
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-bash-result",
      kind: "command",
      status: "success",
      title: "Bash",
      summary: "",
      payload: {
        backend: "claude",
        toolName: "Bash",
        command: "yarn verify",
        isError: false,
        output: "verify 的完整输出文本在折叠态绝不应该露出来",
      },
      order: 1,
    });

    await waitFor(() => {
      const button = view.getByRole("button", { name: /yarn verify/ });
      expect(button).toHaveAttribute("aria-expanded", "false");
      expect(button.closest(".agent-timeline__item")).not.toBeNull();
      const preview = button
        .closest(".agent-timeline__head")
        ?.querySelector(".agent-timeline__preview");
      expect(preview?.textContent ?? "").toContain("yarn verify");
      expect(preview?.textContent ?? "").not.toContain("verify 的完整输出文本");
    });

    await fireEvent.click(view.getByRole("button", { name: /yarn verify/ }));
    await waitFor(() => {
      expect(view.getByText("verify 的完整输出文本在折叠态绝不应该露出来"))
        .toBeInTheDocument();
    });
  });

  it("AskUserQuestion 时间线折叠态显示提问标题，展开后显示用户选择", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    const askUserPayload = {
      toolName: "AskUserQuestion",
      questions: [
        {
          id: "q-1",
          header: "方案",
          question: "选哪个方案？",
          options: [{ label: "方案 A" }, { label: "方案 B" }],
        },
      ],
    };

    emitMockTimelineEvent("t-002", {
      id: "tl-ask-user-question",
      kind: "ask_user",
      status: "started",
      title: "AskUserQuestion",
      summary: "",
      payload: askUserPayload,
      order: 1,
    });

    await waitFor(() => {
      const button = view.getByRole("button", { name: /正在提问/ });
      expect(button).toHaveAttribute("aria-expanded", "false");
      const preview = button
        .closest(".agent-timeline__head")
        ?.querySelector(".agent-timeline__preview");
      expect(preview?.textContent ?? "").toContain("方案 · 选哪个方案？");
      expect(view.queryByText(/方案 B（备注：保留回滚入口）/)).toBeNull();
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-ask-user-question",
      kind: "ask_user",
      status: "success",
      title: "AskUserQuestion",
      summary: "",
      payload: {
        ...askUserPayload,
        output: JSON.stringify({
          answers: { "选哪个方案？": "方案 B" },
          annotations: { "选哪个方案？": { notes: "保留回滚入口" } },
          cancelled: false,
        }),
      },
      order: 1,
    });

    await waitFor(() => {
      const button = view.getByRole("button", { name: /已提问/ });
      expect(button).toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: /已提问/ }));
    await waitFor(() => {
      expect(view.getByText("方案 · 选哪个方案？：方案 B（备注：保留回滚入口）"))
        .toBeInTheDocument();
    });
  });

  it("过程事件默认显示为单行，点击后展开详情", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-single-line-command",
      kind: "command",
      status: "running",
      title: "pnpm test",
      summary: "正在运行单测",
      payload: {
        command: "pnpm test",
        stdout: "详细输出只在展开后出现",
      },
      order: 1,
    });

    await waitFor(() => {
      expect(view.getByText("正在运行单测")).toBeInTheDocument();
      expect(view.queryByText("详细输出只在展开后出现")).toBeNull();
      expect(view.getByRole("button", { name: /pnpm test/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: /pnpm test/ }));
    await waitFor(() => {
      expect(view.getByText("详细输出只在展开后出现")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /pnpm test/ }))
        .toHaveAttribute("aria-expanded", "true");
    });
  });

  it("最终回复显示在 timeline 中，不再创建 assistant 普通气泡", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    await sendText(view, "实现 timeline 最终回复");
    await waitFor(() => {
      expect(view.getByText("实现 timeline 最终回复")).toBeInTheDocument();
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-final-reply",
      kind: "message",
      status: "success",
      title: "Assistant",
      summary: "这是 Claude turn 完成后返回给用户的完整结果。",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "这是 Claude turn 完成后返回给用户的完整结果。\n包含第二行。",
      },
      order: 2,
    });

    completeMockAgentTurn("t-002");

    await waitFor(() => {
      expect(view.getByText(/这是 Claude turn 完成后返回给用户的完整结果/))
        .toBeInTheDocument();
    });

    const finalContent = view.getByText(/这是 Claude turn 完成后返回给用户的完整结果/);
    expect(finalContent.closest(".chat-bubble")).toBeNull();
    expect(finalContent.closest(".agent-timeline__item")).toHaveClass("is-final-reply");
    expect(finalContent.closest(".agent-timeline__event")?.querySelector(".agent-timeline__rail"))
      .not.toBeNull();
    expect(finalContent.closest(".agent-timeline__event")?.querySelector(".agent-timeline__node"))
      .not.toBeNull();
  });

  it("同 turn 内工具与最终回复：流式中按 order 内联，turn 完成后折叠到 final 下", async () => {
    // runner 现在按 text block 拆 inline 文本片段；turn 未结束时所有事件 inline
    // 显示——避免「最后一条」随新 text block 漂移导致折叠抖动。runner 在
    // `case "result"` emit kind:"turn" status:success 那一帧，UI 才把过程折叠到
    // 该 turn 的最后一条 assistant message 下面。
    seedMockChatMessages("t-002", [
      {
        id: "u-inline-start",
        taskId: "t-002",
        role: "user",
        content: "请执行完整验证",
        createdAt: 2000,
      },
    ]);

    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);
    await waitFor(() => {
      expect(view.getByText("请执行完整验证")).toBeInTheDocument();
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-running-before-final",
      kind: "command",
      status: "running",
      title: "yarn verify",
      summary: "正在运行完整验证",
      payload: {
        command: "yarn verify",
        stdout: "验证输出详情",
      },
      turnId: "turn-collapse",
      createdAt: 2100,
      updatedAt: 5100,
      order: 1,
    });

    await waitFor(() => {
      expect(view.queryByText("验证输出详情")).toBeNull();
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-final-collapse",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "## 完成\n\n最终结果完整展示。",
      },
      turnId: "turn-collapse",
      createdAt: 5200,
      updatedAt: 5200,
      order: 2,
    });

    // 流式中（尚未 emit kind:"turn"）：command 仍 inline 显示，不被吞掉。
    await waitFor(() => {
      expect(view.getByText("最终结果完整展示。")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /yarn verify/ })).toBeInTheDocument();
    });
    expect(view.queryByRole("button", { name: /展开过程/ })).toBeNull();

    // 按 order 排序：command(1) 在 final(2) 上方。
    const timelineText = view.getByLabelText("Agent 工作过程").textContent ?? "";
    expect(timelineText.indexOf("yarn verify"))
      .toBeLessThan(timelineText.indexOf("最终结果完整展示。"));
    expect(view.getByRole("button", { name: /yarn verify/ })
      .closest(".agent-timeline__event")
      ?.querySelector(".agent-timeline__rail"))
      .not.toBeNull();
    expect(view.getByText("最终结果完整展示。")
      .closest(".agent-timeline__event")
      ?.querySelector(".agent-timeline__rail"))
      .not.toBeNull();
    expect(view.getByText("最终结果完整展示。")
      .closest(".agent-timeline__event")
      ?.querySelector(".agent-timeline__node"))
      .not.toBeNull();

    // Runner 发出 turn 终结事件，UI 这才把命令折叠到 final 下、默认收起。
    emitMockTurnCompleted("t-002", "turn-collapse");

    await waitFor(() => {
      expect(view.queryByRole("button", { name: /yarn verify/ })).toBeNull();
      const toggle = view.getByRole("button", { name: "命令执行 · 3 秒" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(toggle).not.toHaveTextContent(/展开过程|查看过程|1 项/);
    });

    await fireEvent.click(view.getByRole("button", { name: "命令执行 · 3 秒" }));
    await waitFor(() => {
      expect(view.getByRole("button", { name: /yarn verify/ })).toBeInTheDocument();
    });
    await fireEvent.click(view.getByRole("button", { name: /yarn verify/ }));
    await waitFor(() => {
      expect(view.getByText("验证输出详情")).toBeInTheDocument();
    });
  });

  it("历史 turn 已折叠时，新一轮 inline 事件不会被旧的 final 卡吞掉，新一轮 final 待 turn 完成才折叠", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-after-history-final",
        taskId: "t-002",
        role: "user",
        content: "继续验证 Rust",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-history-final",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "上一轮已经完成的最终回复。",
      },
      turnId: "turn-history",
      createdAt: 1600,
      updatedAt: 1600,
      order: 1,
    });
    emitMockTurnCompleted("t-002", "turn-history", "success", 1700);

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("上一轮已经完成的最终回复。")).toBeInTheDocument();
      expect(view.getByText("继续验证 Rust")).toBeInTheDocument();
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-running-after-history-final",
      kind: "command",
      status: "running",
      title: "cargo check",
      summary: "正在验证 Rust",
      payload: {
        command: "cargo check",
        stdout: "Rust 验证输出详情",
      },
      turnId: "turn-new",
      createdAt: 2100,
      updatedAt: 4100,
      order: 2,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /cargo check/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-new-final-after-history",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "新一轮最终回复。",
      },
      turnId: "turn-new",
      createdAt: 4200,
      updatedAt: 4200,
      order: 3,
    });

    // 新一轮 turn 还没收到终结事件——cargo check 保持 inline，展开态不被重置。
    await waitFor(() => {
      expect(view.getByText("新一轮最终回复。")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /cargo check/ }))
        .toHaveAttribute("aria-expanded", "false");
    });
    expect(view.queryByRole("button", { name: /展开过程/ })).toBeNull();

    // Runner 发新一轮 turn 终结，cargo check 折叠到新 final 下面，默认收起。
    emitMockTurnCompleted("t-002", "turn-new", "success", 4300);

    await waitFor(() => {
      expect(view.queryByRole("button", { name: /cargo check/ })).toBeNull();
      const toggle = view.getByRole("button", { name: "命令执行 · 2 秒" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
    });
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

  it("无法归类的过程折叠后用已处理时长兜底", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-unknown-process",
        taskId: "t-002",
        role: "user",
        content: "请处理无法归类的过程",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-unknown-process",
      kind: "message",
      status: "success",
      title: "Assistant",
      summary: "扩展事件摘要",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "无法归类的过程详情",
      },
      turnId: "turn-unknown-process",
      createdAt: 2100,
      updatedAt: 2100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-unknown-process-final",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "未知过程也已完成。",
      },
      turnId: "turn-unknown-process",
      createdAt: 6100,
      updatedAt: 6100,
      order: 3,
    });
    emitMockTurnCompleted("t-002", "turn-unknown-process", "success", 6200);

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("未知过程也已完成。")).toBeInTheDocument();
      const toggle = view.getByRole("button", { name: "已处理 4 秒" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(toggle).not.toHaveTextContent(/展开过程|查看过程|1 项|执行过程/);
    });
  });

  it("turn 完成后，reasoning 不计入最终回复下的过程项", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-multi-reasoning",
        taskId: "t-002",
        role: "user",
        content: "请分段思考并检查实现",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-reasoning-first",
      kind: "reasoning",
      status: "success",
      title: "已思考",
      summary: "这段思考内容不应计入过程项。",
      payload: {
        text: "这段思考内容不应计入过程项。",
      },
      turnId: "turn-multi-reasoning",
      createdAt: 2100,
      updatedAt: 2100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-between-reasoning-command",
      kind: "command",
      status: "success",
      title: "yarn inspect",
      summary: "读取时间线事件",
      payload: {
        command: "yarn inspect",
        stdout: "中间事件详情",
      },
      turnId: "turn-multi-reasoning",
      createdAt: 2200,
      updatedAt: 2200,
      order: 3,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-reasoning-second",
      kind: "reasoning",
      status: "success",
      title: "已思考",
      summary: "第二段思考：再确认命令事件没有被吞掉。",
      payload: {
        text: "第二段思考：再确认命令事件没有被吞掉。",
      },
      turnId: "turn-multi-reasoning",
      createdAt: 2300,
      updatedAt: 2300,
      order: 4,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-middle-agent-reply",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "中间 Agent 回复片段。",
      },
      turnId: "turn-multi-reasoning",
      createdAt: 2350,
      updatedAt: 2350,
      order: 5,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-multi-reasoning-final",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "已按真实顺序展示时间线。",
      },
      turnId: "turn-multi-reasoning",
      createdAt: 2400,
      updatedAt: 2400,
      order: 6,
    });
    emitMockTurnCompleted("t-002", "turn-multi-reasoning", "success", 2500);

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("请分段思考并检查实现")).toBeInTheDocument();
      expect(view.getByText("已按真实顺序展示时间线。")).toBeInTheDocument();
      expect(view.queryByText("这段思考内容不应计入过程项。")).toBeNull();
      expect(view.queryByText("第二段思考：再确认命令事件没有被吞掉。")).toBeNull();
      const middleReply = view.getByText("中间 Agent 回复片段。");
      expect(middleReply.closest(".agent-timeline__process-collapse"))
        .toHaveAttribute("aria-hidden", "true");
      expect(middleReply.closest(".agent-timeline__process-collapse"))
        .toHaveAttribute("inert");
      expect(middleReply.closest(".agent-timeline__process-collapse"))
        .not.toHaveClass("is-open");
      expect(view.queryByRole("button", { name: /yarn inspect/ })).toBeNull();
      const toggle = view.getByRole("button", { name: "命令执行 · 1 秒" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: "命令执行 · 1 秒" }));
    await waitFor(() => {
      expect(view.queryByText("这段思考内容不应计入过程项。")).toBeNull();
      expect(view.queryByText("第二段思考：再确认命令事件没有被吞掉。")).toBeNull();
      expect(view.getByRole("button", { name: /yarn inspect/ })).toBeInTheDocument();
      expect(view.getByText("中间 Agent 回复片段。")).toBeInTheDocument();
    });

    const commandItem = view.getByRole("button", { name: /yarn inspect/ })
      .closest(".agent-timeline__item");
    const middleReplyItem = view.getByText("中间 Agent 回复片段。")
      .closest(".agent-timeline__item");
    const finalItem = view.getByText("已按真实顺序展示时间线。")
      .closest(".agent-timeline__item");
    expect(middleReplyItem).toHaveClass("is-process-child");
    expect(middleReplyItem?.querySelector(".agent-timeline__rail")).not.toBeNull();
    expect(middleReplyItem?.querySelector(".agent-timeline__node")).toBeNull();
    expect(finalItem?.querySelector(".agent-timeline__node")).not.toBeNull();
    expect(commandItem).toHaveClass("is-process-child");
    expect(finalItem).toContainElement(commandItem);
    expect(finalItem).toContainElement(middleReplyItem);
    expect(commandItem!.compareDocumentPosition(middleReplyItem!) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();
    expect(middleReplyItem!.compareDocumentPosition(view.getByText("已按真实顺序展示时间线。")) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();
  });

  it("用户消息按时间插入 Agent timeline，而不是固定显示在顶部", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-between",
        taskId: "t-002",
        role: "user",
        content: "插在过程中的用户消息",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-final-after-user",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "用户消息之后的最终回复",
      },
      turnId: "turn-after-user",
      createdAt: 3000,
      updatedAt: 3000,
      order: 2,
    });

    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);
    await waitFor(() => {
      expect(view.getByText("插在过程中的用户消息")).toBeInTheDocument();
      expect(view.getByText("用户消息之后的最终回复")).toBeInTheDocument();
    });

    const timelineText = view.getByLabelText("Agent 工作过程").textContent ?? "";
    expect(timelineText.indexOf("从持久化时间线恢复的公开摘要。"))
      .toBeLessThan(timelineText.indexOf("插在过程中的用户消息"));
    expect(timelineText.indexOf("插在过程中的用户消息"))
      .toBeLessThan(timelineText.indexOf("用户消息之后的最终回复"));
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

  it("未知 kind 仍能由 payload 推导出可用的标题、预览和详情", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoningHidden(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-derived-custom",
      kind: "extension_index",
      status: "success",
      title: "title fallback",
      summary: "索引完成",
      payload: {
        toolName: "Index",
        input: { scope: "workspace" },
        output: "indexed 42 files",
      },
      order: 5,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /已处理 Index/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText("索引完成")).toBeInTheDocument();
    });

    await fireEvent.click(view.getByRole("button", { name: /已处理 Index/ }));
    await waitFor(() => {
      expect(view.queryByText("工具")).not.toBeInTheDocument();
      expect(view.getByText("INPUT")).toBeInTheDocument();
      expect(view.getByText(/"scope": "workspace"/)).toBeInTheDocument();
      expect(view.getByText("OUTPUT")).toBeInTheDocument();
      expect(view.getByText("indexed 42 files")).toBeInTheDocument();
    });
  });

  it("相邻事件分组展开后保留原始事件详情", async () => {
    const view = await renderTaskDetail();
    await expectInitialReasoningHidden(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-group-command-1",
      kind: "command",
      status: "success",
      title: "pnpm test",
      summary: "运行前端测试",
      payload: {
        command: "pnpm test",
        stdout: "pnpm 测试输出",
      },
      turnId: null,
      createdAt: 2100,
      updatedAt: 2100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-group-command-2",
      kind: "command",
      status: "success",
      title: "yarn build",
      summary: "运行构建",
      payload: {
        command: "yarn build",
        stdout: "构建输出",
      },
      turnId: null,
      createdAt: 2200,
      updatedAt: 2200,
      order: 3,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /2 条命令/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: /2 条命令/ }));
    await waitFor(() => {
      expect(view.getByRole("button", { name: /pnpm test/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByRole("button", { name: /yarn build/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.queryByText("pnpm 测试输出")).toBeNull();
    });

    await fireEvent.click(view.getByRole("button", { name: /pnpm test/ }));
    await waitFor(() => {
      expect(view.getByText("pnpm 测试输出")).toBeInTheDocument();
    });
  });

  it("最终回复状态更新不会重置同 turn 内已展开的工具事件", async () => {
    // 流式中 turn 还没收到终结事件，工具事件保持 inline。验证 assistant message
    // 自身 running → success 的状态翻新不会顺手把已展开的工具事件 collapse 掉。
    seedMockChatMessages("t-002", [
      {
        id: "u-stream-final",
        taskId: "t-002",
        role: "user",
        content: "请边跑边回复",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-stream-command",
      kind: "command",
      status: "success",
      title: "yarn verify",
      summary: "验证完成",
      payload: {
        command: "yarn verify",
        stdout: "验证输出保持展开",
      },
      turnId: "turn-stream-final",
      createdAt: 2100,
      updatedAt: 2100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-stream-final",
      kind: "message",
      status: "running",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "正在整理结果",
      },
      turnId: "turn-stream-final",
      createdAt: 2200,
      updatedAt: 2200,
      order: 3,
    });

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("正在整理结果")).toBeInTheDocument();
    });
    await fireEvent.click(view.getByRole("button", { name: /yarn verify/ }));
    await waitFor(() => {
      expect(view.getByText("验证输出保持展开")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "true");
    });

    emitMockTimelineEvent("t-002", {
      id: "tl-stream-final",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "整理完成",
      },
      turnId: "turn-stream-final",
      createdAt: 2200,
      updatedAt: 2300,
      order: 3,
    });

    await waitFor(() => {
      expect(view.getByText("整理完成")).toBeInTheDocument();
      expect(view.getByText("验证输出保持展开")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "true");
    });
  });

  it("历史 assistant 消息不会作为普通气泡显示", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-history",
        taskId: "t-002",
        role: "user",
        content: "历史用户问题",
        createdAt: 1000,
      },
      {
        id: "a-history",
        taskId: "t-002",
        role: "assistant",
        content: "旧版 assistant 气泡内容",
        createdAt: 1001,
      },
    ]);

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("历史用户问题")).toBeInTheDocument();
    });
    expect(view.queryByText("旧版 assistant 气泡内容")).toBeNull();
  });
});
