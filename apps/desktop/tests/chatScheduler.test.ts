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
  emitTauriEvent,
  emitWebviewDragDropEvent,
  mockInvoke,
  seedMockChatMessages,
} from "./tauriMock";

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

async function sendText(view: ReturnType<typeof render>, text: string) {
  const input = await view.findByPlaceholderText("可向 agent 询问任何事，输入 @ 使用插件或提及文件");
  await fireEvent.update(input, text);
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

async function expectInitialReasoning(view: ReturnType<typeof render>) {
  await waitFor(() => {
    const node = view.getByText("从持久化时间线恢复的公开摘要。");
    expect(node.closest(".agent-timeline__item--reasoning-inline"))
      .not.toBeNull();
  });
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

    const send = mockInvoke.mock.calls.find(([cmd]) => cmd === "chat_send_message");
    expect(send?.[1]).toMatchObject({
      content: "参考附件总结项目",
      attachments: [
        {
          name: "README.md",
          path: "D:\\PROJECT\\workspace\\Lilia\\README.md",
          kind: "file",
        },
      ],
    });
  });

  it("会把 Agent 运行中追加的用户消息进入调度队列", async () => {
    const view = await renderTaskDetail();

    await sendText(view, "先检查当前实现");
    await waitFor(() => {
      expect(view.getByText("先检查当前实现")).toBeInTheDocument();
    });

    await sendText(view, "补充：优先看调度器");
    await waitFor(() => {
      expect(view.getByText("补充：优先看调度器")).toBeInTheDocument();
      expect(view.getByText("补充：优先看调度器").closest(".chat-bubble"))
        .toHaveClass("is-queued");
    });

    const sends = mockInvoke.mock.calls.filter(([cmd]) => cmd === "chat_send_message");
    expect(sends).toHaveLength(2);
    expect(sends[1][1]).toMatchObject({ content: "补充：优先看调度器" });

    completeMockAgentTurn("t-002");

    await waitFor(() => {
      expect(view.getByText("补充：优先看调度器").closest(".chat-bubble"))
        .not.toHaveClass("is-queued");
    });
  });

  it("初始加载完成后仍保留刚发送的用户气泡", async () => {
    const view = await renderTaskDetail();

    await sendText(view, "页面刚打开时发送的用户消息");

    await expectInitialReasoning(view);
    await waitFor(() => {
      expect(view.getByText("页面刚打开时发送的用户消息")).toBeInTheDocument();
    });

    expect(view.getByText("页面刚打开时发送的用户消息").closest(".chat-bubble"))
      .toHaveAttribute("data-role", "user");
  });

  it("会显示持久化和实时 Agent 工作过程", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoning(view);

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

  it("kind=turn 的 Claude session/status/turn completed 事件不在 timeline 中渲染", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoning(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-hidden-session",
      kind: "turn",
      status: "started",
      title: "Claude session",
      summary: "claude-sonnet-4-6",
      payload: { backend: "claude", model: "claude-sonnet-4-6" },
      order: 1,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-hidden-status",
      kind: "turn",
      status: "info",
      title: "Claude status",
      summary: "requesting",
      payload: { backend: "claude", status: "requesting" },
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-hidden-turn-done",
      kind: "turn",
      status: "success",
      title: "Claude turn completed",
      summary: "",
      payload: { backend: "claude", sessionId: "session-x" },
      order: 3,
    });

    await waitFor(() => {
      expect(view.queryByText("Claude session")).toBeNull();
      expect(view.queryByText("Claude status")).toBeNull();
      expect(view.queryByText("Claude turn completed")).toBeNull();
    });
  });

  it("turn 在跑且尚未流式回复时，timeline 末尾显示「思考中…」指示器", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoning(view);

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

    await expectInitialReasoning(view);

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

  it("过程事件默认显示为单行，点击后展开详情", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoning(view);

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

    await expectInitialReasoning(view);

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
  });

  it("同 turn 内工具、最终回复按 order 内联渲染，最终回复不再吞掉前置工具", async () => {
    // runner 现在按 text block 拆 inline 文本片段：工具事件不会被折叠到 final 卡
    // 下面，而是按 order 各自占一行。这条 case 防止「final 卡跑到工具上方 / 吃掉
    // 同 turn 工具」的回归。
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

    await expectInitialReasoning(view);
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
      updatedAt: 2100,
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
      createdAt: 2200,
      updatedAt: 2200,
      order: 2,
    });

    await waitFor(() => {
      expect(view.getByText("最终结果完整展示。")).toBeInTheDocument();
      // 工具事件仍可见、独立成行——没被折叠到 final 下。
      expect(view.getByRole("button", { name: /yarn verify/ })).toBeInTheDocument();
    });

    const commandItem = view.getByRole("button", { name: /yarn verify/ })
      .closest(".agent-timeline__item");
    const finalItem = view.getByText("最终结果完整展示。")
      .closest(".agent-timeline__item");
    expect(commandItem).not.toBe(finalItem);
    expect(commandItem).not.toHaveClass("is-final-reply");
    expect(finalItem).toHaveClass("is-final-reply");

    // 按 order 排序：command(1) 在 final(2) 上方。
    const timelineText = view.getByLabelText("Agent 工作过程").textContent ?? "";
    expect(timelineText.indexOf("yarn verify"))
      .toBeLessThan(timelineText.indexOf("最终结果完整展示。"));

    await fireEvent.click(view.getByRole("button", { name: /yarn verify/ }));
    await waitFor(() => {
      expect(view.getByText("验证输出详情")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "true");
    });
  });

  it("已有历史最终回复时，新一轮工具事件仍按 order 内联显示，不被新一轮 final 卡吞掉", async () => {
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
      updatedAt: 2100,
      order: 2,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /cargo check/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: /cargo check/ }));
    await waitFor(() => {
      expect(view.getByText("Rust 验证输出详情")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /cargo check/ }))
        .toHaveAttribute("aria-expanded", "true");
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
      createdAt: 2200,
      updatedAt: 2200,
      order: 3,
    });

    await waitFor(() => {
      expect(view.getByText("新一轮最终回复。")).toBeInTheDocument();
      // 工具事件不再折叠到 final 下面——保持 inline 显示，展开态也不被重置。
      expect(view.getByRole("button", { name: /cargo check/ }))
        .toHaveAttribute("aria-expanded", "true");
      expect(view.getByText("Rust 验证输出详情")).toBeInTheDocument();
    });
  });

  it("最终回复之前的工具与计划事件按 order 内联显示，不被 final 卡折叠", async () => {
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
      status: "success",
      title: "yarn test",
      summary: "命令折叠态预览",
      payload: {
        command: "yarn test",
        stdout: "折叠后的命令详情",
      },
      turnId: "turn-fold",
      createdAt: 2100,
      updatedAt: 2100,
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
      createdAt: 2200,
      updatedAt: 2200,
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
      createdAt: 2300,
      updatedAt: 2300,
      order: 4,
    });

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("请实现时间线折叠")).toBeInTheDocument();
      expect(view.getByText("最终回复应该直接可见。")).toBeInTheDocument();
      // 工具/计划事件按 order 内联展示，默认折叠（点开才看到详情）。
      expect(view.getByRole("button", { name: /yarn test/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByRole("button", { name: /更新计划/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    const processItem = view.getByRole("button", { name: /yarn test/ })
      .closest(".agent-timeline__item");
    const finalItem = view.getByText("最终回复应该直接可见。")
      .closest(".agent-timeline__item");
    expect(processItem).not.toBe(finalItem);
    expect(processItem).not.toHaveClass("is-final-reply");

    // 按 order 排：工具/计划（2、3）在 final（4）上方。
    const timelineText = view.getByLabelText("Agent 工作过程").textContent ?? "";
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

  it("多个思考事件之间的过程事件仍按时间线顺序显示", async () => {
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
      summary: "第一段思考：先定位渲染归并规则。",
      payload: {
        text: "第一段思考：先定位渲染归并规则。",
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
      order: 5,
    });

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("第一段思考：先定位渲染归并规则。")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /yarn inspect/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText("第二段思考：再确认命令事件没有被吞掉。")).toBeInTheDocument();
      expect(view.getByText("已按真实顺序展示时间线。")).toBeInTheDocument();
    });

    const firstReasoningItem = view.getByText("第一段思考：先定位渲染归并规则。")
      .closest(".agent-timeline__item");
    const commandItem = view.getByRole("button", { name: /yarn inspect/ })
      .closest(".agent-timeline__item");
    const secondReasoningItem = view.getByText("第二段思考：再确认命令事件没有被吞掉。")
      .closest(".agent-timeline__item");
    const finalItem = view.getByText("已按真实顺序展示时间线。")
      .closest(".agent-timeline__item");

    expect(firstReasoningItem).not.toBeNull();
    expect(commandItem).not.toBeNull();
    expect(secondReasoningItem).not.toBeNull();
    expect(finalItem).not.toBeNull();
    expect(firstReasoningItem!.compareDocumentPosition(commandItem!) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();
    expect(commandItem!.compareDocumentPosition(secondReasoningItem!) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();
    expect(secondReasoningItem!.compareDocumentPosition(finalItem!) & Node.DOCUMENT_POSITION_FOLLOWING)
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

    await expectInitialReasoning(view);
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

    await expectInitialReasoning(view);

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

    await expectInitialReasoning(view);

    // 持久层不再有 display 字段；事件生产方只塞 payload，前端 deriveTimelineDisplay
    // 命中 default (tool) 分支：kind 不是 "tool" 时动词降级为「处理」，
    // payload.toolName 进入标题，input/output 自动渲染为 INPUT / OUTPUT 代码块。
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
      expect(view.getByText("工具")).toBeInTheDocument();
      expect(view.getByText("Index")).toBeInTheDocument();
      expect(view.getByText("indexed 42 files")).toBeInTheDocument();
    });
  });

  it("相邻事件分组展开后保留原始事件详情", async () => {
    const view = await renderTaskDetail();
    await expectInitialReasoning(view);

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
    // runner 现在按 text block 拆 inline 文本片段，原有的「展开过程」聚合按钮
    // 已经删除——这里只验证文本片段更新（running → success）不影响同 turn 内
    // 工具事件本身的 expand 状态。
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
