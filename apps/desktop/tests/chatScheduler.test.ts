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

    await waitFor(() => {
      expect(view.getByText("历史思考摘要")).toBeInTheDocument();
      expect(view.getByText("页面刚打开时发送的用户消息")).toBeInTheDocument();
    });

    expect(view.getByText("页面刚打开时发送的用户消息").closest(".chat-bubble"))
      .toHaveAttribute("data-role", "user");
  });

  it("会显示持久化和实时 Agent 工作过程", async () => {
    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("历史思考摘要")).toBeInTheDocument();
      expect(view.getByText("从持久化时间线恢复的公开摘要。")).toBeInTheDocument();
    });

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
      expect(view.getAllByText("yarn verify").length).toBeGreaterThan(0);
      expect(view.getByText("正在运行完整验证")).toBeInTheDocument();
    });
  });

  it("过程事件默认显示为单行，点击后展开详情", async () => {
    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("历史思考摘要")).toBeInTheDocument();
    });

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

    await waitFor(() => {
      expect(view.getByText("历史思考摘要")).toBeInTheDocument();
    });

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
      expect(view.getByText("最终回复")).toBeInTheDocument();
      expect(view.getByText(/这是 Claude turn 完成后返回给用户的完整结果/))
        .toBeInTheDocument();
    });

    expect(view.queryByText(/这是 Claude turn 完成后返回给用户的完整结果/)
      ?.closest(".chat-bubble")).toBeNull();
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

    await waitFor(() => {
      expect(view.getByText("历史思考摘要")).toBeInTheDocument();
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
      expect(view.getByText("最终回复")).toBeInTheDocument();
      expect(view.getByText("最终结果完整展示。")).toBeInTheDocument();
      expect(view.queryByRole("button", { name: /yarn verify/ })).toBeNull();
      expect(view.queryByText("验证输出详情")).toBeNull();
      expect(view.getByRole("button", { name: "展开过程 1 项" }))
        .toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: "展开过程 1 项" }));
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
      expect(view.getByRole("button", { name: "展开过程 1 项" }))
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
      expect(view.getByRole("button", { name: "展开过程 2 项" }))
        .toHaveAttribute("aria-expanded", "false");
    });

    await fireEvent.click(view.getByRole("button", { name: "展开过程 2 项" }));
    await waitFor(() => {
      expect(view.getByRole("button", { name: /yarn test/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText(/这条过程默认不应显示/)).toBeInTheDocument();
      expect(view.getByRole("button", { name: /更新计划/ }))
        .toHaveAttribute("aria-expanded", "false");
      expect(view.getByText(/这条计划默认不应显示/)).toBeInTheDocument();
      expect(view.getByRole("button", { name: "收起过程 2 项" }))
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

    await waitFor(() => {
      expect(view.getByText("历史思考摘要")).toBeInTheDocument();
      expect(view.getByText("插在过程中的用户消息")).toBeInTheDocument();
      expect(view.getByText("用户消息之后的最终回复")).toBeInTheDocument();
    });

    const timelineText = view.getByLabelText("Agent 工作过程").textContent ?? "";
    expect(timelineText.indexOf("历史思考摘要"))
      .toBeLessThan(timelineText.indexOf("插在过程中的用户消息"));
    expect(timelineText.indexOf("插在过程中的用户消息"))
      .toBeLessThan(timelineText.indexOf("用户消息之后的最终回复"));
  });

  it("agent 错误走 timeline 错误事件，不再创建 system 普通气泡", async () => {
    const view = await renderTaskDetail();

    await waitFor(() => {
      expect(view.getByText("历史思考摘要")).toBeInTheDocument();
    });

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
