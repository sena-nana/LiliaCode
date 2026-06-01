import { cleanup, fireEvent, render, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, h, nextTick } from "vue";
import ChatSidebarHost from "../src/components/chat/ChatSidebarHost.vue";
import TitleBar from "../src/components/TitleBar.vue";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import {
  closeChatSidebar,
  openChatSidebar,
  registerChatSidebarPanel,
  setChatSidebarWidth,
  type ChatSidebarPanel,
} from "../src/composables/useChatSidebar";
import { setAgentInteractionSettings } from "../src/services/chat";
import { createTodo } from "../src/services/todos";
import { mockInvoke } from "./tauriMock";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: vi.fn(async () => false),
    onResized: vi.fn(async () => vi.fn()),
    minimize: vi.fn(async () => undefined),
    toggleMaximize: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  }),
}));

const STORAGE_KEY = "lilia.chatSidebar.open";
const WIDTH_STORAGE_KEY = "lilia.chatSidebar.width";
const PROJECT_CWD = "D:\\PROJECT\\workspace\\Lilia";

const cleanups: Array<() => void> = [];

const DummyPanel = defineComponent({
  name: "DummyPanel",
  props: {
    taskId: { type: String, required: true },
    projectId: String,
    projectCwd: String,
  },
  setup(props) {
    return () =>
      h(
        "div",
        { "data-testid": "dummy-panel" },
        `${props.taskId}|${props.projectId ?? ""}|${props.projectCwd ?? ""}`,
      );
  },
});

const FirstPanel = defineComponent({
  name: "FirstPanel",
  setup() {
    return () => h("div", { "data-testid": "first-panel" }, "first");
  },
});

const SecondPanel = defineComponent({
  name: "SecondPanel",
  setup() {
    return () => h("div", { "data-testid": "second-panel" }, "second");
  },
});

function trackPanel(panel: ChatSidebarPanel) {
  const cleanup = registerChatSidebarPanel(panel);
  cleanups.push(cleanup);
  return cleanup;
}

function renderHost() {
  return render(ChatSidebarHost, {
    props: {
      taskId: "t-002",
      projectId: "lilia",
      projectCwd: PROJECT_CWD,
    },
  });
}

function sidebarElement(container: HTMLElement): HTMLElement {
  const sidebar = container.querySelector(".chat-sidebar");
  if (!(sidebar instanceof HTMLElement)) {
    throw new Error("未找到对话侧栏");
  }
  return sidebar;
}

function sidebarResizer(container: HTMLElement): HTMLElement {
  const resizer = container.querySelector(".chat-sidebar__resizer");
  if (!(resizer instanceof HTMLElement)) {
    throw new Error("未找到对话侧栏拖拽线");
  }
  return resizer;
}

async function renderTaskDetail() {
  await Promise.all([projectsReady, allTasksReady]);
  const router = createLiliaRouter(createMemoryHistory());
  await router.push("/projects/lilia/tasks/t-002");
  await router.isReady();

  const Wrapper = defineComponent({
    components: { TaskDetail, TitleBar },
    template: `
      <TitleBar />
      <TaskDetail project-id="lilia" task-id="t-002" />
    `,
  });

  return render(Wrapper, {
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
    return;
  }
  input.textContent = text;
  placeEditableCaret(input, text.length);
  await fireEvent.input(input);
}

function titlebarSidebarButton(container: HTMLElement): HTMLButtonElement {
  const button = container.querySelector(".titlebar__chat-sidebar-btn");
  if (!(button instanceof HTMLButtonElement)) {
    throw new Error("未找到标题栏侧栏按钮");
  }
  return button;
}

function debugSidebar(container: HTMLElement) {
  return within(sidebarElement(container));
}

beforeEach(() => {
  localStorage.clear();
  closeChatSidebar();
  setChatSidebarWidth(340);
});

afterEach(() => {
  cleanup();
  for (const cleanup of cleanups.splice(0)) cleanup();
  closeChatSidebar();
  localStorage.clear();
});

describe("chat sidebar host", () => {
  it("默认关闭并保留空态 host", () => {
    const view = renderHost();
    const sidebar = sidebarElement(view.container);

    expect(sidebar).not.toHaveClass("is-open");
    expect(sidebar).toHaveAttribute("aria-hidden", "true");
    expect(view.getByText("暂无内容")).toBeInTheDocument();
    expect(view.queryByText("侧栏")).not.toBeInTheDocument();
  });

  it("会从本地存储恢复打开状态", () => {
    localStorage.setItem(STORAGE_KEY, "1");

    const view = renderHost();
    const sidebar = sidebarElement(view.container);

    expect(sidebar).toHaveClass("is-open");
    expect(sidebar).not.toHaveAttribute("aria-hidden");
  });

  it("渲染注册内容并传入当前对话上下文", () => {
    trackPanel({
      id: "dummy",
      title: "上下文",
      component: DummyPanel,
    });
    openChatSidebar("dummy");

    const view = renderHost();

    expect(view.getByTestId("dummy-panel")).toHaveTextContent(
      `t-002|lilia|${PROJECT_CWD}`,
    );
  });

  it("active panel 注销后切到下一个注册内容", async () => {
    const unregisterFirst = trackPanel({
      id: "first",
      title: "First",
      order: 1,
      component: FirstPanel,
    });
    trackPanel({
      id: "second",
      title: "Second",
      order: 2,
      component: SecondPanel,
    });
    openChatSidebar("first");

    const view = renderHost();
    expect(view.getByTestId("first-panel")).toBeInTheDocument();

    unregisterFirst();
    await nextTick();

    expect(view.getByTestId("second-panel")).toBeInTheDocument();
  });

  it("可从左边缘拖动调整宽度并在松手后写回本地存储", async () => {
    openChatSidebar();
    const view = renderHost();
    const sidebar = sidebarElement(view.container);
    const resizer = sidebarResizer(view.container);

    expect(sidebar.style.getPropertyValue("--chat-sidebar-width")).toBe("340px");
    expect(resizer).toHaveAttribute("aria-valuemin", "180");
    expect(resizer).toHaveAttribute("aria-valuenow", "340");

    await fireEvent.pointerDown(resizer, {
      button: 0,
      clientX: 600,
      pointerId: 1,
    });
    await fireEvent.pointerMove(window, {
      clientX: 500,
      pointerId: 1,
    });

    expect(sidebar.style.getPropertyValue("--chat-sidebar-width")).toBe("440px");
    expect(resizer).toHaveAttribute("aria-valuenow", "440");

    await fireEvent.pointerMove(window, {
      clientX: 200,
      pointerId: 1,
    });

    expect(sidebar.style.getPropertyValue("--chat-sidebar-width")).toBe("520px");
    expect(resizer).toHaveAttribute("aria-valuenow", "520");

    await fireEvent.pointerMove(window, {
      clientX: 900,
      pointerId: 1,
    });

    expect(sidebar.style.getPropertyValue("--chat-sidebar-width")).toBe("180px");
    expect(resizer).toHaveAttribute("aria-valuenow", "180");

    await fireEvent.pointerUp(window, {
      clientX: 900,
      pointerId: 1,
    });

    expect(localStorage.getItem(WIDTH_STORAGE_KEY)).toBe("180");
  });

  it("对话侧栏宽度可双击恢复默认值", async () => {
    localStorage.setItem(WIDTH_STORAGE_KEY, "480");
    openChatSidebar();

    const view = renderHost();
    const sidebar = sidebarElement(view.container);
    const resizer = sidebarResizer(view.container);

    expect(sidebar.style.getPropertyValue("--chat-sidebar-width")).toBe("480px");

    await fireEvent.dblClick(resizer);

    expect(sidebar.style.getPropertyValue("--chat-sidebar-width")).toBe("340px");
    expect(localStorage.getItem(WIDTH_STORAGE_KEY)).toBe("340");
  });
});

describe("TaskDetail chat sidebar toggle", () => {
  it("通过标题栏右侧按钮打开和关闭侧栏，并写回本地存储", async () => {
    const view = await renderTaskDetail();
    const sidebar = sidebarElement(view.container);
    const toggle = titlebarSidebarButton(view.container);

    expect(sidebar).not.toHaveClass("is-open");
    expect(toggle).toHaveAttribute("aria-label", "打开对话侧栏");

    await fireEvent.click(toggle);

    expect(sidebar).toHaveClass("is-open");
    expect(toggle).not.toHaveClass("is-active");
    expect(toggle).toHaveAttribute("aria-label", "关闭对话侧栏");
    expect(localStorage.getItem(STORAGE_KEY)).toBe("1");

    await fireEvent.click(toggle);

    expect(sidebar).not.toHaveClass("is-open");
    expect(toggle).toHaveAttribute("aria-label", "打开对话侧栏");
    expect(localStorage.getItem(STORAGE_KEY)).toBe("0");
  });

  it("开启 debug 后注册调试面板，并模拟计划确认完整流程", async () => {
    await setAgentInteractionSettings({ debug: true });
    mockInvoke.mockClear();
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "计划" }));

    await waitFor(() => {
      expect(view.container.querySelector(".timeline-card--plan")).toBeInTheDocument();
    });
    expect(view.getByRole("button", { name: /等待确认计划/ })).toBeInTheDocument();
    expect(view.queryByText("已失效")).toBeNull();
    expect(view.getByRole("region", { name: "确认 Debug 计划" })).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(view.getByText("已同意")).toBeInTheDocument();
    });
    expect(mockInvoke.mock.calls).not.toContainEqual([
      "agent_timeline_insert",
      expect.anything(),
      undefined,
    ]);
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_respond_ask_user")).toBe(false);
  });

  it("debug 提问使用真实 pending ask 流程并在回答后回写时间线", async () => {
    await setAgentInteractionSettings({ debug: true });
    mockInvoke.mockClear();
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "单选提问" }));

    expect(await view.findByRole("region", { name: "Debug 提问" })).toBeInTheDocument();
    expect(view.queryByText("已失效")).toBeNull();

    await fireEvent.click(view.getByRole("radio", { name: "待办" }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await waitFor(() => {
      expect(view.getByText("Debug 提问已回答")).toBeInTheDocument();
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_respond_ask_user")).toBe(false);
  });

  it("debug 多选提问覆盖多选交互并本地完成", async () => {
    await setAgentInteractionSettings({ debug: true });
    mockInvoke.mockClear();
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "多选提问" }));

    expect(await view.findByRole("region", { name: "Debug 多选提问" })).toBeInTheDocument();
    expect(view.queryByText("已失效")).toBeNull();

    await fireEvent.click(view.getByRole("checkbox", { name: /权限申请/ }));
    await fireEvent.click(view.getByRole("checkbox", { name: /普通卡片/ }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await waitFor(() => {
      expect(view.getByText("Debug 多选提问已回答")).toBeInTheDocument();
    });

    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_respond_ask_user")).toBe(false);
  });

  it("debug 示例提问显示选项预览并本地完成", async () => {
    await setAgentInteractionSettings({ debug: true });
    mockInvoke.mockClear();
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "示例提问" }));
    expect(await view.findByRole("region", { name: "Debug 示例提问" })).toBeInTheDocument();

    await fireEvent.mouseEnter(view.getByRole("radio", { name: /详细/ }));
    expect(view.getByLabelText("选项预览")).toHaveTextContent("本地 resolve 后回写卡片状态");

    await fireEvent.click(view.getByRole("radio", { name: /详细/ }));
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await waitFor(() => {
      expect(view.getByText("Debug 示例提问已回答")).toBeInTheDocument();
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_respond_ask_user")).toBe(false);
  });

  it("debug 多题提问保留前后题选择并本地完成", async () => {
    await setAgentInteractionSettings({ debug: true });
    mockInvoke.mockClear();
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "多题提问" }));

    expect(await view.findByRole("region", { name: "Debug 多题提问" })).toBeInTheDocument();
    await fireEvent.click(view.getByRole("radio", { name: /时间线/ }));
    await fireEvent.click(view.getByRole("button", { name: "继续" }));
    await fireEvent.click(view.getByRole("checkbox", { name: "完成态" }));
    await fireEvent.click(view.getByRole("button", { name: "上一题" }));

    expect(view.getByRole("radio", { name: /时间线/ })).toHaveAttribute("aria-checked", "true");

    await fireEvent.click(view.getByRole("button", { name: "继续" }));
    expect(view.getByRole("checkbox", { name: "完成态" })).toHaveAttribute("aria-checked", "true");
    await fireEvent.click(view.getByRole("button", { name: "完成" }));

    await waitFor(() => {
      expect(view.getByText("Debug 多题提问已回答")).toBeInTheDocument();
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_respond_ask_user")).toBe(false);
  });

  it("debug 权限申请走本地 pending tool consent，不触发 runner IPC", async () => {
    await setAgentInteractionSettings({ debug: true });
    mockInvoke.mockClear();
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "权限申请" }));

    expect(await view.findByRole("alert")).toBeInTheDocument();
    expect(view.queryByText("已失效")).toBeNull();
    expect(view.getByText("写入调试夹具")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(view.getByText("Debug 权限已同意")).toBeInTheDocument();
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_respond_tool_consent")).toBe(false);
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "agent_timeline_insert")).toBe(false);
  });

  it("debug 待办卡片可插入并展开查看详情", async () => {
    await setAgentInteractionSettings({ debug: true });
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "待办卡片" }));

    const todoButton = await view.findByRole("button", { name: /更新待办/ });
    await fireEvent.click(todoButton);

    expect(view.getByText("点击预制事件按钮")).toBeInTheDocument();
  });

  it("debug Todo工具刷新原生 Todo，并把用户输入先进 Lilia 引导", async () => {
    await setAgentInteractionSettings({ debug: true });
    mockInvoke.mockClear();
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    await fireEvent.click(await debugSidebar(view.container).findByRole("button", { name: "Todo工具" }));

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "todo_apply_agent_event")).toBe(true);
      expect(view.getByText("确认 TodoFloat 自动刷新")).toBeInTheDocument();
    });
    expect(view.getByText("验证手动 Todo 不被 agent 覆盖")).toBeInTheDocument();
    expect(view.queryByText("完成 Claude TodoWrite 调试接线")).toBeNull();
    const todoFloat = view.getByLabelText("Todo 与引导");
    expect(within(todoFloat).queryByText("Todo")).toBeNull();
    expect(within(todoFloat).queryByText("引导")).toBeNull();
    expect(todoFloat.querySelectorAll(".todo-float__section")).toHaveLength(1);
    expect(view.getByText("模拟 Claude TodoWrite 并刷新 Lilia Todo")).toBeInTheDocument();

    await setComposerText(view, "手动补充调试项");
    await fireEvent.click(view.getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "todo_create")).toBe(true);
      expect(view.getByText("手动补充调试项")).toBeInTheDocument();
      expect(todoFloat.querySelectorAll(".todo-float__section")).toHaveLength(2);
    });

    await waitFor(() => {
      const createIndex = mockInvoke.mock.calls.findIndex(([cmd]) => cmd === "todo_create");
      const sendIndex = mockInvoke.mock.calls.findIndex(([cmd]) => cmd === "chat_send_message");
      expect(createIndex).toBeGreaterThanOrEqual(0);
      expect(sendIndex).toBeGreaterThan(createIndex);
    });
  });

  it("Lilia 引导可以编辑优先级、正文并手动插入", async () => {
    mockInvoke.mockClear();
    const view = await renderTaskDetail();

    await createTodo("t-002", "需要补充上下文");
    mockInvoke.mockClear();

    const todoFloat = await view.findByLabelText("Todo 与引导");
    expect(within(todoFloat).queryByText("Todo")).toBeNull();
    expect(within(todoFloat).queryByText("引导")).toBeNull();
    expect(todoFloat.querySelectorAll(".todo-float__section")).toHaveLength(1);

    const guide = await view.findByRole("button", { name: "需要补充上下文" });
    await fireEvent.click(guide);
    const editInput = await view.findByLabelText("编辑引导");
    await fireEvent.update(editInput, "需要补充关键上下文");
    await fireEvent.blur(editInput);
    await waitFor(() => {
      expect(view.getByText("需要补充关键上下文")).toBeInTheDocument();
    });
    await fireEvent.click(view.getAllByTitle("设为高优先级").at(-1)!);

    await waitFor(() => {
      expect(view.getByText("需要补充关键上下文")).toBeInTheDocument();
    });

    const insert = view.getByRole("button", { name: /立即插入引导：需要补充关键上下文/ });
    await fireEvent.click(insert);

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "chat_send_message")).toBe(true);
      expect(view.getByText("已插入")).toBeInTheDocument();
    });
  });

  it("debug 普通卡片事件覆盖命令、读文件和改文件", async () => {
    await setAgentInteractionSettings({ debug: true });
    openChatSidebar("debug");

    const view = await renderTaskDetail();

    const debug = debugSidebar(view.container);
    await fireEvent.click(await debug.findByRole("button", { name: "命令" }));
    await fireEvent.click(debug.getByRole("button", { name: "读文件" }));
    await fireEvent.click(debug.getByRole("button", { name: "改文件" }));

    await waitFor(() => {
      expect(view.getByText("yarn verify:contracts")).toBeInTheDocument();
      expect(view.getByText("packages/contracts/src/index.ts")).toBeInTheDocument();
      expect(view.getByText("apps/desktop/src/pages/TaskDetail.vue")).toBeInTheDocument();
    });
  });
});
