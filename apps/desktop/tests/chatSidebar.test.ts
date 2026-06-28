import { cleanup, fireEvent, render, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, h, nextTick } from "vue";
import ChatSidebarHost from "../src/components/chat/ChatSidebarHost.vue";
import TitleBar from "../src/components/TitleBar.vue";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { createLiliaRouter } from "../src/router";
import {
  closeChatSidebar,
  openChatSidebar,
  registerChatSidebarPanel,
  type ChatSidebarPanel,
} from "../src/composables/useChatSidebar";
import { setAgentInteractionSettings } from "../src/services/chat";
import { createTodo } from "../src/services/todos";
import { resolveAskUserById, useAskUser } from "../src/composables/useAskUser";
import { mockInvoke } from "./tauriMock";
import { placeEditableCaret } from "./domTestHelpers";

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

  const view = render(Wrapper, {
    global: {
      plugins: [router],
    },
  });
  await waitFor(() => {
    expect(titlebarSidebarButton(view.container)).toBeInstanceOf(HTMLButtonElement);
    expect(sidebarElement(view.container)).toBeInstanceOf(HTMLElement);
  }, { timeout: 4000 });
  return view;
}

async function setComposerText(view: ReturnType<typeof render>, text: string) {
  const input = await waitFor(() => {
    const element = view.container.querySelector(".chat-composer [role='textbox']");
    expect(element).toBeInstanceOf(HTMLElement);
    return element as HTMLElement;
  });
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

async function enableDebugSidebar() {
  await setAgentInteractionSettings({ debug: true });
  mockInvoke.mockClear();
  openChatSidebar("debug");
  return renderTaskDetail();
}

beforeEach(() => {
  localStorage.clear();
  closeChatSidebar();
  localStorage.setItem(WIDTH_STORAGE_KEY, "340");
});

afterEach(async () => {
  const { state } = useAskUser();
  while (state.current || state.queue.length || state.pending.length) {
    const id = state.current?.id ?? state.queue[0]?.id ?? state.pending[0]?.id;
    if (typeof id !== "number") break;
    resolveAskUserById(id, { answers: {}, cancelled: true });
  }
  if (typeof vi.dynamicImportSettled === "function") {
    await vi.dynamicImportSettled();
  }
  cleanup();
  if (typeof vi.dynamicImportSettled === "function") {
    await vi.dynamicImportSettled();
  }
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

    return waitFor(() => {
      expect(view.getByTestId("dummy-panel")).toHaveTextContent(
        `t-002|lilia|${PROJECT_CWD}`,
      );
    });
  });

  it("异步 panel 仅在首次打开时加载一次，并在切换后复用缓存", async () => {
    const loadDebug = vi.fn(async () => FirstPanel);
    const loadContext = vi.fn(async () => SecondPanel);
    trackPanel({
      id: "debug",
      title: "Debug",
      order: 10,
      loader: loadDebug,
    });
    trackPanel({
      id: "context",
      title: "上下文",
      order: 20,
      loader: loadContext,
    });
    openChatSidebar("debug");

    const view = renderHost();

    await waitFor(() => {
      expect(view.getByTestId("first-panel")).toBeInTheDocument();
    });
    expect(loadDebug).toHaveBeenCalledTimes(1);
    expect(loadContext).toHaveBeenCalledTimes(0);

    await fireEvent.click(view.getByRole("tab", { name: "上下文" }));
    await waitFor(() => {
      expect(view.getByTestId("second-panel")).toBeInTheDocument();
    });
    expect(loadContext).toHaveBeenCalledTimes(1);

    await fireEvent.click(view.getByRole("tab", { name: "Debug" }));
    await waitFor(() => {
      expect(view.getByTestId("first-panel")).toBeInTheDocument();
    });
    expect(loadDebug).toHaveBeenCalledTimes(1);
  });

  it("关闭侧栏后缓存仍在返回的异步 panel 加载", async () => {
    let resolveStaleLoad: (component: typeof FirstPanel) => void = () => {};
    const loadDebug = vi.fn()
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveStaleLoad = resolve;
        }),
      )
      .mockResolvedValueOnce(FirstPanel);
    trackPanel({
      id: "debug",
      title: "Debug",
      order: 10,
      loader: loadDebug,
    });
    openChatSidebar("debug");

    const view = renderHost();

    await waitFor(() => {
      expect(view.getByText("正在加载...")).toBeInTheDocument();
    });

    closeChatSidebar();
    resolveStaleLoad(FirstPanel);
    await nextTick();

    openChatSidebar("debug");

    await waitFor(() => {
      expect(loadDebug).toHaveBeenCalledTimes(1);
      expect(view.getByTestId("first-panel")).toBeInTheDocument();
    });
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
});

describe("TaskDetail chat sidebar toggle", () => {
  it("通过标题栏右侧按钮打开和关闭侧栏，并写回本地存储", async () => {
    const view = await enableDebugSidebar();
    const sidebar = await waitFor(() => sidebarElement(view.container));
    const toggle = await waitFor(() => titlebarSidebarButton(view.container));

    closeChatSidebar();

    await waitFor(() => {
      expect(sidebar).not.toHaveClass("is-open");
      expect(toggle).toHaveAttribute("aria-label", "打开对话侧栏");
    });

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
});

