import { fireEvent, render } from "@testing-library/vue";
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
  type ChatSidebarPanel,
} from "../src/composables/useChatSidebar";

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

function titlebarSidebarButton(container: HTMLElement): HTMLButtonElement {
  const button = container.querySelector(".titlebar__chat-sidebar-btn");
  if (!(button instanceof HTMLButtonElement)) {
    throw new Error("未找到标题栏侧栏按钮");
  }
  return button;
}

beforeEach(() => {
  localStorage.clear();
  closeChatSidebar();
});

afterEach(() => {
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
    expect(toggle).toHaveAttribute("aria-label", "关闭对话侧栏");
    expect(localStorage.getItem(STORAGE_KEY)).toBe("1");

    await fireEvent.click(toggle);

    expect(sidebar).not.toHaveClass("is-open");
    expect(toggle).toHaveAttribute("aria-label", "打开对话侧栏");
    expect(localStorage.getItem(STORAGE_KEY)).toBe("0");
  });
});
