import { render, fireEvent, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { defineComponent } from "vue";
import type { Task } from "@lilia/contracts";
import SecondaryPanel from "../src/layouts/SecondaryPanel.vue";
import ContextMenuHost from "../src/components/ContextMenuHost.vue";
import { useConnectionStatus } from "../src/composables/useConnectionStatus";
import { useSidebarDisplayMode } from "../src/composables/useSidebarDisplayMode";
import { createLiliaRouter } from "../src/router";
import { TASKS } from "../src/data/tasks";
import {
  emitMockTimelineEvent,
  emitTauriEvent,
  mockInvoke,
  replaceMockTimelineEvents,
  setMockActiveBackend,
  setMockCodexAppServerStatus,
  setMockChatRunning,
  setMockRuntimeSnapshot,
} from "./tauriMock";
import {
  installConversationActivityBridge,
  resetConversationActivity,
} from "../src/composables/useConversationActivity";
import { installAgentInteractionBridge } from "../src/composables/useAgentInteractionBridge";

function seedTreeExpansionState(state: unknown) {
  localStorage.setItem("lilia.projectTree.expansion", JSON.stringify(state));
}

async function renderSecondaryPanel(initialRoute = "/") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();

  const Wrapper = defineComponent({
    components: { SecondaryPanel, ContextMenuHost },
    template: `
      <SecondaryPanel />
      <ContextMenuHost />
    `,
  });

  const view = render(Wrapper, {
    global: {
      plugins: [router],
    },
  });
  return { ...view, router };
}

function getProjectRow(view: ReturnType<typeof render>, projectName: string): HTMLElement {
  const row = view.getByText(projectName).closest(".sb-tree__row--project");
  if (!(row instanceof HTMLElement)) {
    throw new Error(`未找到项目行：${projectName}`);
  }
  return row;
}

function getConversationRow(view: ReturnType<typeof render>, title: string): HTMLElement {
  const row = view.getByText(title).closest(".sb-tree__row");
  if (!(row instanceof HTMLElement)) {
    throw new Error(`未找到对话行：${title}`);
  }
  return row;
}

function box(top: number, bottom: number): DOMRect {
  return {
    x: 0,
    y: top,
    left: 0,
    top,
    right: 220,
    bottom,
    width: 220,
    height: bottom - top,
    toJSON: () => ({}),
  } as DOMRect;
}

function projectConversation(id: string, title: string, index: number): Task {
  return {
    id,
    projectId: "lilia",
    sessionId: `session-${id}`,
    title,
    status: "done",
    createdAt: 1000 + index,
    pinned: false,
    parentId: null,
    dependsOn: [],
  };
}

function pendingAskTimelineEvent(taskId: string, requestId: string) {
  return {
    id: `tl-pending-${requestId}`,
    taskId,
    turnId: "turn-ask",
    backend: "codex" as const,
    kind: "ask_user",
    status: "requires_action",
    title: "需要确认",
    summary: "继续？",
    payload: { requestId },
    createdAt: Date.now(),
    updatedAt: Date.now(),
    turnSeq: 1,
    intraTurnOrder: 0,
  };
}

function pendingToolTimelineEvent(taskId: string, requestId: string) {
  return {
    id: `tl-pending-${requestId}`,
    taskId,
    turnId: "turn-tool",
    backend: "codex" as const,
    kind: "command",
    status: "requires_action",
    title: "需要授权",
    summary: "运行命令？",
    payload: {
      interaction: "tool_consent",
      requestId,
    },
    createdAt: Date.now(),
    updatedAt: Date.now(),
    turnSeq: 1,
    intraTurnOrder: 0,
  };
}

function seedSecondaryPanelOverflowConversations() {
  TASKS.value = {
    ...TASKS.value,
    lilia: Array.from({ length: 6 }, (_, index) =>
      projectConversation(`t-overflow-${index + 1}`, `溢出对话 ${index + 1}`, index)
    ),
  };
}

async function dragFromTo(source: HTMLElement, target: HTMLElement, targetY: number) {
  await fireEvent.pointerDown(source, {
    button: 0,
    pointerId: 1,
    clientX: 20,
    clientY: 10,
  });
  await fireEvent.pointerMove(window, {
    pointerId: 1,
    clientX: 20,
    clientY: targetY,
  });
  await fireEvent.pointerUp(target, {
    pointerId: 1,
    clientX: 20,
    clientY: targetY,
  });
}

describe("SecondaryPanel project tree expansion", () => {
  beforeEach(() => {
    localStorage.clear();
    useSidebarDisplayMode().setSidebarDisplayMode("grouped");
    resetConversationActivity();
  });

  it("左下角连接徽章显示全局 active provider", async () => {
    setMockActiveBackend("codex");
    const view = await renderSecondaryPanel();

    await waitFor(() => {
      const label = view.container.querySelector(".sb-conn__label");
      expect(label).toHaveTextContent("Codex");
    });
  });

  it("Codex app-server 环境不满足时左下角 provider 卡片标红", async () => {
    setMockActiveBackend("codex");
    setMockCodexAppServerStatus({
      supportsRequiredProtocol: false,
      failureKind: "experimentalApiUnsupported",
      issues: ["Codex app-server 环境不满足。"],
    });
    await useConnectionStatus().refresh();
    const view = await renderSecondaryPanel();

    await waitFor(() => {
      const card = view.container.querySelector(".sb-conn");
      expect(card).toHaveClass("sb-conn--error");
      expect(card).toHaveTextContent("异常");
      expect(card).toHaveAttribute("title", expect.stringContaining("Codex app-server 环境不满足"));
      expect(card).toHaveAttribute("title", expect.not.stringContaining("OpenAI Responses API"));
    });
  });

  it("会恢复上次关闭时的项目展开状态和收集箱状态", async () => {
    seedTreeExpansionState({
      projects: {
        lilia: false,
        tools: true,
      },
      orphansExpanded: false,
    });

    const view = await renderSecondaryPanel();

    const liliaRow = getProjectRow(view, "Lilia");
    const toolsRow = getProjectRow(view, "工具箱");
    const orphansToggle = view.getByRole("button", {
      name: "展开收集箱",
    });

    expect(liliaRow).toHaveAttribute("aria-expanded", "false");
    expect(toolsRow).toHaveAttribute("aria-expanded", "true");
    expect(orphansToggle).toBeInTheDocument();
  });


  it("用户切换后会写回本地存储", async () => {
    seedTreeExpansionState({
      projects: {
        lilia: true,
        tools: true,
      },
      orphansExpanded: true,
    });

    const view = await renderSecondaryPanel();
    await fireEvent.click(getProjectRow(view, "Lilia"));
    await fireEvent.click(view.getByRole("button", { name: "折叠收集箱" }));

    await waitFor(() => {
      const raw = localStorage.getItem("lilia.projectTree.expansion");
      expect(raw).toBeTruthy();
      const parsed = JSON.parse(raw ?? "{}") as {
        projects?: Record<string, boolean>;
        orphansExpanded?: boolean;
      };
      expect(parsed.projects?.lilia).toBe(false);
      expect(parsed.projects?.tools).toBe(true);
      expect(parsed.orphansExpanded).toBe(false);
    });
  });
});

describe("SecondaryPanel project chat navigation", () => {
  let unlistenConversationActivity: (() => void) | null = null;
  let unlistenAgentInteraction: (() => void) | null = null;

  beforeEach(async () => {
    localStorage.clear();
    useSidebarDisplayMode().setSidebarDisplayMode("grouped");
    resetConversationActivity();
    unlistenAgentInteraction = await installAgentInteractionBridge();
    unlistenConversationActivity = await installConversationActivityBridge();
  });

  afterEach(() => {
    unlistenAgentInteraction?.();
    unlistenConversationActivity?.();
    unlistenAgentInteraction = null;
    unlistenConversationActivity = null;
  });


  it("创建空分类后会自动进入该项目的新对话", async () => {
    const view = await renderSecondaryPanel();

    await fireEvent.click(view.getByRole("button", { name: "添加项目" }));
    await fireEvent.click(await view.findByRole("menuitem", { name: "创建空分类" }));
    await fireEvent.update(
      await view.findByPlaceholderText("例如：实验、归档…"),
      "临时分类",
    );
    await fireEvent.click(view.getByRole("button", { name: "创建" }));

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toMatch(
        /^\/projects\/p-3\/tasks\/t-draft-/,
      );
    });
  });


  it("顶部搜索会话后点击结果进入对应对话并关闭搜索", async () => {
    const view = await renderSecondaryPanel();

    await fireEvent.click(view.getByRole("button", { name: "搜索会话" }));
    await fireEvent.update(
      view.getByPlaceholderText("搜索会话…"),
      "tsconfig",
    );

    const listbox = await view.findByRole("listbox");
    await fireEvent.click(
      within(listbox).getByRole("option", {
        name: /打通 tsconfig paths 搜索/,
      }),
    );

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe(
        "/projects/lilia/tasks/t-002",
      );
    });
    expect(view.queryByPlaceholderText("搜索会话…")).not.toBeInTheDocument();
    expect(view.getByRole("button", { name: "搜索会话" })).toBeInTheDocument();
  });


  it("归档当前打开的单条项目对话后会进入该项目的新对话", async () => {
    const view = await renderSecondaryPanel("/projects/lilia/tasks/t-001");
    const row = getConversationRow(view, "接入 Claude Code 会话发现");

    await fireEvent.click(within(row).getByRole("button", { name: "归档" }));
    await fireEvent.click(
      within(row).getByRole("button", { name: "确认归档，再点一次" }),
    );

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toMatch(
        /^\/projects\/lilia\/tasks\/t-draft-/,
      );
    });
  });

  it("中键点击收集箱对话会在弹出窗口中打开", async () => {
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "随手问问 Claude：tsconfig paths");

    await fireEvent(
      row,
      new MouseEvent("auxclick", { bubbles: true, button: 1 }),
    );

    expect(mockInvoke).toHaveBeenCalledWith("popup_open_task", {
      projectId: null,
      taskId: "o-001",
    }, undefined);
  });

  it("统一列表模式隐藏项目分组和收集箱，并在项目会话右侧显示项目名", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    const view = await renderSecondaryPanel();

    expect(view.getByText("会话")).toBeInTheDocument();
    expect(view.queryByText("收集箱")).not.toBeInTheDocument();
    expect(view.queryByRole("button", { name: "添加项目" })).not.toBeInTheDocument();

    const projectRow = await view.findByText("打通 tsconfig paths 搜索");
    const projectConversation = projectRow.closest(".sb-tree__row--unified");
    expect(projectConversation).toBeInstanceOf(HTMLElement);
    expect(within(projectConversation as HTMLElement).getByText("Lilia")).toHaveClass(
      "sb-tree__project-label",
    );

    const orphan = getConversationRow(view, "随手问问 Claude：tsconfig paths");
    expect(within(orphan).queryByText("Lilia")).not.toBeInTheDocument();
  });

  it("统一列表项目会话仍能把项目归属传给弹出窗口", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    await fireEvent(
      row,
      new MouseEvent("auxclick", { bubbles: true, button: 1 }),
    );

    expect(mockInvoke).toHaveBeenCalledWith("popup_open_task", {
      projectId: "tools",
      taskId: "t-003",
    }, undefined);
  });

  it("非选中统一列表对话会在行尾显示运行中、等待交互和完成状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    emitTauriEvent("chat:turn-started", { taskId: "t-003", queuedCount: 0 });
    await waitFor(() => {
      expect(within(row).getByLabelText("对话中")).toHaveClass(
        "sb-tree__activity--running",
      );
    });

    emitTauriEvent("chat:agent-interaction-request", {
      taskId: "t-003",
      turnId: "turn-1",
      backend: "codex",
      requestId: "ask-1",
      kind: "ask_user",
      payload: {
        title: "需要确认",
        intent: "ask_user",
        questions: [{ id: "q1", question: "继续？", mode: "confirm" }],
      },
    });
    await waitFor(() => {
      expect(within(row).getByLabelText("等待交互")).toHaveClass(
        "sb-tree__activity--requires_action",
      );
    });

    emitTauriEvent("chat:done", { taskId: "t-003", sessionId: null, subtype: null });
    await waitFor(() => {
      expect(within(row).getByLabelText("对话完成")).toHaveClass(
        "sb-tree__activity--completed",
      );
    });
  });

  it("统一列表冷启动时会从 runtime snapshot 恢复运行中状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    setMockChatRunning("t-003", true);
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_get_runtime_snapshot"))
        .toBe(true);
      expect(within(row).getByLabelText("对话中")).toHaveClass(
        "sb-tree__activity--running",
      );
    });
  });

  it("统一列表冷启动时会把 reset_pending_finish runtime snapshot 显示为运行中", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    setMockRuntimeSnapshot("t-003", {
      phase: "reset_pending_finish",
      runtimeChannel: "mutsuki_core",
      backend: "codex",
      turnId: "turn-reset",
      pendingResetCleanup: true,
    });

    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    await waitFor(() => {
      expect(within(row).getByLabelText("对话中")).toHaveClass(
        "sb-tree__activity--running",
      );
    });
  });

  it("统一列表冷启动时会把 abandoned runtime snapshot 显示为错误状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    setMockRuntimeSnapshot("t-003", {
      phase: "abandoned",
      runtimeChannel: "mutsuki_core",
      backend: "codex",
      turnId: "turn-old",
    });

    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    await waitFor(() => {
      expect(within(row).getByLabelText("发生错误")).toHaveClass(
        "sb-tree__activity--error",
      );
    });
  });

  it("统一列表冷启动时 abandoned runtime snapshot 会覆盖持久化等待交互", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    setMockRuntimeSnapshot("t-003", {
      phase: "abandoned",
      runtimeChannel: "mutsuki_core",
      backend: "codex",
      turnId: "turn-old",
    });
    replaceMockTimelineEvents("t-003", [pendingAskTimelineEvent("t-003", "ask-overridden")]);

    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    await waitFor(() => {
      expect(within(row).getByLabelText("发生错误")).toHaveClass(
        "sb-tree__activity--error",
      );
      expect(within(row).queryByLabelText("等待交互")).toBeNull();
    });
  });

  it("统一列表冷启动时会从持久化 timeline 恢复等待交互状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    replaceMockTimelineEvents("t-003", [pendingAskTimelineEvent("t-003", "ask-1")]);

    const askView = await renderSecondaryPanel();
    const askRow = getConversationRow(askView, "整理窗口快捷键");
    await waitFor(() => {
      expect(within(askRow).getByLabelText("等待交互")).toHaveClass(
        "sb-tree__activity--requires_action",
      );
    });
  });

  it("统一列表冷启动时会从持久化 timeline 恢复错误状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    resetConversationActivity();
    replaceMockTimelineEvents("t-003", [{
      id: "tl-error-hydrated",
      taskId: "t-003",
      turnId: "turn-error",
      backend: "codex",
      kind: "error",
      status: "error",
      title: "错误",
      summary: "Agent 发生错误",
      payload: {},
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    }]);

    const errorView = await renderSecondaryPanel();
    const errorRow = getConversationRow(errorView, "整理窗口快捷键");
    await waitFor(() => {
      expect(within(errorRow).getByLabelText("发生错误")).toHaveClass(
        "sb-tree__activity--error",
      );
    });
  });

  it("项目列表冷启动时会从 runtime snapshot 恢复项目会话运行中状态", async () => {
    seedTreeExpansionState({
      projects: {
        lilia: false,
        tools: true,
      },
      orphansExpanded: true,
    });
    setMockRuntimeSnapshot("t-003", {
      phase: "running",
      runtimeChannel: "mutsuki_core",
      backend: "codex",
      turnId: "turn-project",
    });

    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    await waitFor(() => {
      expect(within(row).getByLabelText("对话中")).toHaveClass(
        "sb-tree__activity--running",
      );
    });
  });

  it("项目列表冷启动时会从持久化 timeline 恢复收集箱等待交互状态", async () => {
    replaceMockTimelineEvents("o-001", [pendingToolTimelineEvent("o-001", "tool-orphan")]);

    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "随手问问 Claude：tsconfig paths");

    await waitFor(() => {
      expect(within(row).getByLabelText("等待交互")).toHaveClass(
        "sb-tree__activity--requires_action",
      );
    });
  });

  it("结束事件会清理等待交互并显示完成状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    emitMockTimelineEvent("t-003", {
      ...pendingToolTimelineEvent("t-003", "tool-live"),
      id: "tl-live-pending-tool",
    });
    await waitFor(() => {
      expect(within(row).getByLabelText("等待交互")).toHaveClass(
        "sb-tree__activity--requires_action",
      );
    });

    emitTauriEvent("chat:done", { taskId: "t-003", sessionId: null, subtype: null });
    await waitFor(() => {
      expect(within(row).getByLabelText("对话完成")).toHaveClass(
        "sb-tree__activity--completed",
      );
      expect(within(row).queryByLabelText("等待交互")).toBeNull();
    });
  });

  it("发生错误时显示错误图标，并且结束事件不会覆盖错误状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    emitTauriEvent("chat:turn-started", { taskId: "t-003", queuedCount: 0 });
    emitMockTimelineEvent("t-003", {
      id: "tl-error-activity",
      kind: "error",
      status: "error",
      title: "错误",
      summary: "Agent 发生错误",
      turnId: "turn-1",
    });

    await waitFor(() => {
      expect(within(row).getByLabelText("发生错误")).toHaveClass(
        "sb-tree__activity--error",
      );
    });

    emitTauriEvent("chat:done", { taskId: "t-003", sessionId: null, subtype: null });
    await waitFor(() => {
      expect(within(row).getByLabelText("发生错误")).toBeInTheDocument();
      expect(within(row).queryByLabelText("对话完成")).toBeNull();
    });
  });

  it("完成图标会保留到用户进入该对话，选中对话隐藏状态", async () => {
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "整理窗口快捷键");

    emitTauriEvent("chat:done", { taskId: "t-003", sessionId: null, subtype: null });
    await waitFor(() => {
      expect(within(row).getByLabelText("对话完成")).toBeInTheDocument();
    });

    await fireEvent.click(row);

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe("/projects/tools/tasks/t-003");
      expect(within(row).queryByLabelText("对话完成")).toBeNull();
    });

    emitTauriEvent("chat:turn-started", { taskId: "t-003", queuedCount: 0 });
    await waitFor(() => {
      expect(within(row).queryByLabelText("对话中")).toBeNull();
    });
  });

  it("切换侧边栏对话不会调用打断接口", async () => {
    const view = await renderSecondaryPanel("/projects/lilia/tasks/t-001");
    const row = getConversationRow(view, "打通 tsconfig paths 搜索");
    mockInvoke.mockClear();

    await fireEvent.click(row);

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe("/projects/lilia/tasks/t-002");
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_interrupt_turn"))
      .toBe(false);
  });
});

describe("SecondaryPanel project tree drag", () => {
  beforeEach(() => {
    localStorage.clear();
    useSidebarDisplayMode().setSidebarDisplayMode("grouped");
    resetConversationActivity();
  });


  it("项目拖动时显示落点指示，松手后更新同置顶分组顺序", async () => {
    const view = await renderSecondaryPanel();
    const lilia = getProjectRow(view, "Lilia");
    const tools = getProjectRow(view, "工具箱");
    lilia.getBoundingClientRect = () => box(0, 28);
    tools.getBoundingClientRect = () => box(40, 68);

    await fireEvent.pointerDown(lilia, {
      button: 0,
      pointerId: 1,
      clientX: 20,
      clientY: 10,
    });
    await fireEvent.pointerMove(tools, {
      pointerId: 1,
      clientX: 20,
      clientY: 60,
    });

    expect(lilia).toHaveClass("is-tree-drag-source");
    expect(tools).toHaveClass("is-tree-drop-target");
    expect(tools).toHaveClass("is-tree-drop-after");
    expect(tools).not.toHaveClass("is-tree-drop-invalid");

    await fireEvent.pointerUp(tools, {
      pointerId: 1,
      clientX: 20,
      clientY: 60,
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("project_reorder", {
        orderedIds: ["tools", "lilia"],
      }, undefined);
    });
  });
});
