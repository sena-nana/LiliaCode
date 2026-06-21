import { cleanup, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent } from "vue";
import TitleBar from "../src/components/TitleBar.vue";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { createLiliaRouter } from "../src/router";
import { closeChatSidebar, openChatSidebar } from "../src/composables/useChatSidebar";
import { resolveAskUserById, useAskUser } from "../src/composables/useAskUser";
import { setAgentInteractionSettings } from "../src/services/chat";

const sidebarPanelRegistrations = vi.hoisted(() => ({
  debug: vi.fn(() => () => {}),
  architecture: vi.fn(() => () => {}),
  iab: vi.fn(() => () => {}),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: vi.fn(async () => false),
    onResized: vi.fn(async () => vi.fn()),
    minimize: vi.fn(async () => undefined),
    toggleMaximize: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  }),
}));

vi.mock("../src/pages/taskDetail/taskDetailSidebarPanels", () => ({
  registerTaskDetailDebugSidebarPanel: sidebarPanelRegistrations.debug,
  registerTaskDetailArchitectureSidebarPanel: sidebarPanelRegistrations.architecture,
  registerTaskDetailIabSidebarPanel: sidebarPanelRegistrations.iab,
}));

function sidebarElement(container: HTMLElement): HTMLElement {
  const sidebar = container.querySelector(".chat-sidebar");
  if (!(sidebar instanceof HTMLElement)) {
    throw new Error("未找到对话侧栏");
  }
  return sidebar;
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
    expect(sidebarElement(view.container)).toBeInstanceOf(HTMLElement);
  }, { timeout: 4000 });

  return view;
}

beforeEach(() => {
  closeChatSidebar();
  localStorage.clear();
  sidebarPanelRegistrations.debug.mockClear();
  sidebarPanelRegistrations.architecture.mockClear();
  sidebarPanelRegistrations.iab.mockClear();
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
  vi.unstubAllGlobals();
  closeChatSidebar();
  localStorage.clear();
});

describe("TaskDetail sidebar panel activation", () => {
  it("卸载时取消任务详情入口的 paint 调度", async () => {
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 77));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);
    const router = createLiliaRouter(createMemoryHistory());
    await router.push("/projects/lilia/tasks/t-002");
    await router.isReady();

    const view = render(TaskDetail, {
      props: {
        projectId: "lilia",
        taskId: "t-002",
      },
      global: {
        plugins: [router],
      },
    });

    if (typeof vi.dynamicImportSettled === "function") {
      await vi.dynamicImportSettled();
    }
    await Promise.resolve();
    view.unmount();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(77);
  });

  it("关闭状态下不会预注册侧栏 panel，首次打开后才激活注册", async () => {
    await setAgentInteractionSettings({ debug: true });
    const view = await renderTaskDetail();

    if (typeof vi.dynamicImportSettled === "function") {
      await vi.dynamicImportSettled();
    }
    await new Promise((resolve) => globalThis.setTimeout(resolve, 30));

    expect(view.container.querySelector(".titlebar__chat-sidebar-btn")).toBeNull();
    expect(sidebarPanelRegistrations.debug).not.toHaveBeenCalled();
    expect(sidebarPanelRegistrations.architecture).not.toHaveBeenCalled();
    expect(sidebarPanelRegistrations.iab).not.toHaveBeenCalled();

    openChatSidebar("debug");

    await waitFor(() => {
      expect(sidebarPanelRegistrations.debug).toHaveBeenCalledTimes(1);
      expect(sidebarPanelRegistrations.architecture).toHaveBeenCalledTimes(1);
      expect(sidebarPanelRegistrations.iab).toHaveBeenCalledTimes(1);
    });

    await waitFor(() => {
      expect(sidebarElement(view.container)).toHaveClass("is-open");
    });
  });
});
