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

  it("最终回复出现后默认折叠中间过程，只展开最终结果", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-collapse-start",
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

    await fireEvent.click(view.getByRole("button", { name: /yarn verify/ }));
    await waitFor(() => {
      expect(view.getByText("验证输出详情")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "true");
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
      expect(view.queryByRole("button", { name: /yarn verify/ })).toBeNull();
      expect(view.queryByText("验证输出详情")).toBeNull();
      expect(view.getByRole("button", { name: /展开过程 1 项.*1 条命令.*运行中/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: /展开过程 1 项.*1 条命令.*运行中/ }));
    await waitFor(() => {
      expect(view.getByRole("button", { name: /yarn verify/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText(/正在运行完整验证/)).toBeInTheDocument();
    });

    const commandItem = view.getByRole("button", { name: /yarn verify/ })
      .closest(".agent-timeline__item");
    const finalItem = view.getByText("最终结果完整展示。")
      .closest(".agent-timeline__item");
    expect(commandItem).not.toBe(finalItem);
    expect(commandItem).not.toHaveClass("is-final-reply");

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

  it("已有历史最终回复时，新一轮最终回复仍会折叠中间过程", async () => {
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
      expect(view.queryByRole("button", { name: /cargo check/ })).toBeNull();
      expect(view.queryByText("Rust 验证输出详情")).toBeNull();
      expect(view.getByRole("button", { name: /展开过程 1 项.*1 条命令.*运行中/ }))
        .toHaveAttribute("aria-expanded", "false");
    });
  });

  it("最终回复后默认折叠用户消息到最终回复之间的过程事件", async () => {
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
      summary: "这条过程默认不应显示",
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
      summary: "这条计划默认不应显示",
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
      expect(view.queryByText("这条过程默认不应显示")).toBeNull();
      expect(view.queryByText("这条计划默认不应显示")).toBeNull();
      expect(view.getByRole("button", { name: /展开过程 2 项.*1 条命令.*1 项计划/ }))
        .toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: /展开过程 2 项.*1 条命令.*1 项计划/ }));
    await waitFor(() => {
      expect(view.getByRole("button", { name: /yarn test/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText(/这条过程默认不应显示/)).toBeInTheDocument();
      expect(view.getByRole("button", { name: /更新计划/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText(/这条计划默认不应显示/)).toBeInTheDocument();
      expect(view.getByRole("button", { name: /收起过程 2 项.*1 条命令.*1 项计划/ }))
        .toHaveAttribute("aria-expanded", "true");
    });

    const processItem = view.getByRole("button", { name: /yarn test/ })
      .closest(".agent-timeline__item");
    const finalItem = view.getByText("最终回复应该直接可见。")
      .closest(".agent-timeline__item");
    expect(processItem).not.toBe(finalItem);
    expect(processItem).not.toHaveClass("is-final-reply");

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

  it("未知节点优先使用 display 声明渲染标题、预览和详情", async () => {
    const view = await renderTaskDetail();

    await expectInitialReasoning(view);

    emitMockTimelineEvent("t-002", {
      id: "tl-declared-custom",
      kind: "extension_index",
      status: "success",
      title: "title should not win",
      summary: "summary should not win",
      payload: {
        raw: "payload should not be required for display",
      },
      display: {
        icon: "wrench",
        action: "同步",
        object: "索引",
        preview: "索引完成",
        details: [
          {
            type: "fields",
            fields: [
              { label: "范围", value: "workspace" },
            ],
          },
          {
            type: "code",
            label: "OUTPUT",
            content: "indexed 42 files",
          },
        ],
        group: {
          key: "extension:index",
          bucket: "index",
          unit: "次索引",
          count: 2,
        },
      },
      order: 5,
    });

    await waitFor(() => {
      expect(view.getByRole("button", { name: /已同步 索引/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText("索引完成")).toBeInTheDocument();
    });

    await fireEvent.click(view.getByRole("button", { name: /已同步 索引/ }));
    await waitFor(() => {
      expect(view.getByText("范围")).toBeInTheDocument();
      expect(view.getByText("workspace")).toBeInTheDocument();
      expect(view.getByText("indexed 42 files")).toBeInTheDocument();
    });
  });

  it("折叠后的过程摘要会显示事件类型和异常状态", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-process-summary",
        taskId: "t-002",
        role: "user",
        content: "请修复失败验证",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-summary-command",
      kind: "command",
      status: "failed",
      title: "yarn test",
      summary: "测试失败",
      payload: {
        command: "yarn test",
        stderr: "Expected true to be false",
      },
      turnId: "turn-summary",
      createdAt: 2100,
      updatedAt: 2100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-summary-file",
      kind: "file_change",
      status: "success",
      title: "更新测试",
      summary: "修改测试文件",
      payload: {
        changes: [{ kind: "update", path: "apps/desktop/tests/chatScheduler.test.ts" }],
      },
      turnId: "turn-summary",
      createdAt: 2200,
      updatedAt: 2200,
      order: 3,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-summary-final",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "已经定位失败原因。",
      },
      turnId: "turn-summary",
      createdAt: 2300,
      updatedAt: 2300,
      order: 4,
    });

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("已经定位失败原因。")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /展开过程 2 项.*1 条命令.*1 个文件.*有失败/ }))
        .toHaveAttribute("aria-expanded", "false");
    });
  });

  it("折叠后的过程摘要优先使用 display group 声明", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-declared-process-summary",
        taskId: "t-002",
        role: "user",
        content: "请同步索引",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-declared-process-a",
      kind: "extension_index",
      status: "success",
      title: "Index A",
      summary: "索引 A",
      payload: {},
      display: {
        icon: "wrench",
        action: "同步",
        object: "索引 A",
        preview: "索引 A",
        group: {
          key: "extension:index",
          bucket: "index",
          unit: "次索引",
          count: 2,
        },
      },
      turnId: "turn-declared-process",
      createdAt: 2100,
      updatedAt: 2100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-declared-process-b",
      kind: "extension_index",
      status: "success",
      title: "Index B",
      summary: "索引 B",
      payload: {},
      display: {
        icon: "wrench",
        action: "同步",
        object: "索引 B",
        preview: "索引 B",
        group: {
          key: "extension:index",
          bucket: "index",
          unit: "次索引",
          count: 1,
        },
      },
      turnId: "turn-declared-process",
      createdAt: 2200,
      updatedAt: 2200,
      order: 3,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-declared-process-final",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "索引同步完成。",
      },
      turnId: "turn-declared-process",
      createdAt: 2300,
      updatedAt: 2300,
      order: 4,
    });

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("索引同步完成。")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /展开过程 2 项.*3 次索引/ }))
        .toHaveAttribute("aria-expanded", "false");
    });
  });

  it("折叠后的过程摘要会把 Bash 和文件工具归入命令与文件", async () => {
    seedMockChatMessages("t-002", [
      {
        id: "u-tool-process-summary",
        taskId: "t-002",
        role: "user",
        content: "请检查文件并验证",
        createdAt: 2000,
      },
    ]);
    emitMockTimelineEvent("t-002", {
      id: "tl-tool-summary-bash",
      kind: "tool",
      status: "success",
      title: "Bash",
      summary: "运行验证",
      payload: {
        toolName: "Bash",
        input: { command: "yarn verify" },
      },
      turnId: "turn-tool-summary",
      createdAt: 2100,
      updatedAt: 2100,
      order: 2,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-tool-summary-read",
      kind: "tool",
      status: "success",
      title: "Read",
      summary: "读取组件",
      payload: {
        toolName: "Read",
        input: { path: "apps/desktop/src/components/chat/AgentTimeline.vue" },
      },
      turnId: "turn-tool-summary",
      createdAt: 2200,
      updatedAt: 2200,
      order: 3,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-tool-summary-edit",
      kind: "tool",
      status: "success",
      title: "Edit",
      summary: "修改样式",
      payload: {
        toolName: "Edit",
        input: { path: "apps/desktop/src/styles.css" },
      },
      turnId: "turn-tool-summary",
      createdAt: 2300,
      updatedAt: 2300,
      order: 4,
    });
    emitMockTimelineEvent("t-002", {
      id: "tl-tool-summary-final",
      kind: "message",
      status: "success",
      title: "Assistant",
      payload: {
        backend: "claude",
        role: "assistant",
        content: "文件检查完成。",
      },
      turnId: "turn-tool-summary",
      createdAt: 2400,
      updatedAt: 2400,
      order: 5,
    });

    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("文件检查完成。")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /展开过程 3 项.*1 条命令.*2 个文件/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.queryByRole("button", { name: /3 个工具/ })).toBeNull();
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

  it("最终回复状态更新不会重置当前 turn 的过程展开状态", async () => {
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
    await fireEvent.click(view.getByRole("button", { name: /展开过程 1 项.*1 条命令/ }));
    await fireEvent.click(view.getByRole("button", { name: /yarn verify/ }));
    await waitFor(() => {
      expect(view.getByText("验证输出保持展开")).toBeInTheDocument();
      expect(view.getByRole("button", { name: /收起过程 1 项.*1 条命令/ }))
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
      expect(view.getByRole("button", { name: /收起过程 1 项.*1 条命令/ }))
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
