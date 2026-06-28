import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  AUTOMATION_CHANGED_EVENT_NAME,
  AUTOMATION_GET_RUN_COMMAND,
  AUTOMATION_RUN_FINISHED_EVENT_NAME,
  AUTOMATION_RUN_STARTED_EVENT_NAME,
  AUTOMATION_RUN_UPDATED_EVENT_NAME,
  AGENT_TIMELINE_LIST_COMMAND,
  CHAT_AGENT_INTERACTION_REQUEST_EVENT_NAME,
  CHAT_GET_RUNTIME_SNAPSHOT_COMMAND,
  CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND,
  CLI_PROJECT_OPEN_EVENT_NAME,
  MAIN_NAVIGATE_EVENT_NAME,
  POPUP_NAVIGATE_EVENT_NAME,
  PROJECT_LIST_COMMAND,
  createAppNavigateEvent,
  createCliProjectOpenEvent,
} from "@lilia/contracts";
import App from "../src/App.vue";
import { createDraftOrphan } from "../src/data/tasks";
import { createLiliaRouter } from "../src/router";
import {
  emitTauriEvent,
  failNextMockListen,
  mockInvoke,
  mockListenerCount,
  seedMockAutomationRun,
  setMockAgentTimelineDelay,
  setMockCurrentWindowLabel,
  setMockPendingCliProjectOpen,
} from "./tauriMock";

const vueFlowImportState = vi.hoisted(() => ({
  loaded: false,
}));

vi.mock("@vue-flow/core", async () => {
  vueFlowImportState.loaded = true;
  const { defineComponent, h } = await import("vue");
  return {
    Handle: defineComponent({ name: "Handle", setup: () => () => h("span") }),
    Position: { Left: "left", Right: "right" },
    VueFlow: defineComponent({
      name: "VueFlow",
      setup: (_, { slots }) => () =>
        h("div", { "data-testid": "automation-flow" }, slots.default?.()),
    }),
    useVueFlow: () => ({
      fitView: vi.fn(),
      zoomIn: vi.fn(),
      zoomOut: vi.fn(),
      dimensions: { value: { width: 740, height: 470 } },
      viewport: { value: { x: 0, y: 0, zoom: 1 } },
    }),
  };
});

async function renderApp(windowLabel: string, initialRoute = "/") {
  setMockCurrentWindowLabel(windowLabel);
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();

  const view = render(App, {
    global: {
      plugins: [router],
    },
  });

  await waitFor(() => {
    expect(mockListenerCount(CHAT_AGENT_INTERACTION_REQUEST_EVENT_NAME)).toBe(1);
  });
  await waitFor(() => {
    expect(mockListenerCount(
      windowLabel === "main" ? MAIN_NAVIGATE_EVENT_NAME : POPUP_NAVIGATE_EVENT_NAME,
    )).toBe(1);
  });
  await Promise.resolve();

  return {
    ...view,
    router,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("App main navigation events", () => {
  it("卸载时取消 deferred bridge paint 安装调度", async () => {
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 61));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);
    setMockCurrentWindowLabel("main");
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/:pathMatch(.*)*", component: { template: "<div />" } }],
    });
    await router.push("/");
    await router.isReady();

    const view = render(App, {
      global: {
        plugins: [router],
      },
    });
    await Promise.resolve();
    view.unmount();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(61);
  });

  it("主窗口导航 listener 注册失败时会回滚已注册的监听", async () => {
    failNextMockListen(CLI_PROJECT_OPEN_EVENT_NAME, "cli listener failed");
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    setMockCurrentWindowLabel("main");
    const router = createLiliaRouter(createMemoryHistory());
    await router.push("/");
    await router.isReady();

    const view = render(App, {
      global: {
        plugins: [router],
      },
    });

    try {
      await waitFor(() => {
        expect(errorSpy).toHaveBeenCalledWith(
          "[app] install window navigation listeners failed",
          expect.any(Error),
        );
      });
      expect(mockListenerCount(MAIN_NAVIGATE_EVENT_NAME)).toBe(0);
      expect(mockListenerCount(CLI_PROJECT_OPEN_EVENT_NAME)).toBe(0);
    } finally {
      view.unmount();
      errorSpy.mockRestore();
    }
  });

  it("自动化页面只在进入路由后加载重 UI 并注册事件监听", async () => {
    const view = await renderApp("main");

    expect(view.router.currentRoute.value.fullPath).toBe("/");
    expect(vueFlowImportState.loaded).toBe(false);
    expect(mockListenerCount(AUTOMATION_CHANGED_EVENT_NAME)).toBe(0);
    expect(mockListenerCount(AUTOMATION_RUN_STARTED_EVENT_NAME)).toBe(0);
    expect(mockListenerCount(AUTOMATION_RUN_UPDATED_EVENT_NAME)).toBe(0);
    expect(mockListenerCount(AUTOMATION_RUN_FINISHED_EVENT_NAME)).toBe(0);

    await view.router.push("/automations");

    await waitFor(() => {
      expect(vueFlowImportState.loaded).toBe(true);
      expect(mockListenerCount(AUTOMATION_CHANGED_EVENT_NAME)).toBe(1);
      expect(mockListenerCount(AUTOMATION_RUN_STARTED_EVENT_NAME)).toBe(1);
      expect(mockListenerCount(AUTOMATION_RUN_UPDATED_EVENT_NAME)).toBe(1);
      expect(mockListenerCount(AUTOMATION_RUN_FINISHED_EVENT_NAME)).toBe(1);
    }, { timeout: 3000 });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === AUTOMATION_GET_RUN_COMMAND)).toBe(false);
  });

  it("自动化 listener 注册失败时会回滚已注册的 changed listener", async () => {
    failNextMockListen(AUTOMATION_RUN_STARTED_EVENT_NAME, "automation run listener failed");
    const view = await renderApp("main");

    await view.router.push("/automations");

    await waitFor(() => {
      expect(mockListenerCount(AUTOMATION_CHANGED_EVENT_NAME)).toBe(0);
      expect(mockListenerCount(AUTOMATION_RUN_STARTED_EVENT_NAME)).toBe(0);
      expect(mockListenerCount(AUTOMATION_RUN_UPDATED_EVENT_NAME)).toBe(0);
      expect(mockListenerCount(AUTOMATION_RUN_FINISHED_EVENT_NAME)).toBe(0);
    });
  });

  it("自动化页已有运行记录时首屏只拉摘要，点击运行后才加载详情", async () => {
    seedMockAutomationRun();
    const view = await renderApp("main", "/automations");

    await waitFor(() => {
      expect(view.getByRole("button", { name: /manual/ })).toBeInTheDocument();
    }, { timeout: 10_000 });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === AUTOMATION_GET_RUN_COMMAND)).toBe(false);

    await fireEvent.click(view.getByRole("button", { name: /manual/ }));

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === AUTOMATION_GET_RUN_COMMAND)).toBe(true);
      expect(view.getByText("trigger-1")).toBeInTheDocument();
    }, { timeout: 10_000 });
  });

  it("自动化页切到新对话会先渲染草稿聊天，不等待 timeline 和 runtime", async () => {
    const view = await renderApp("main", "/automations");

    await waitFor(() => {
      expect(view.getByTestId("automation-flow")).toBeInTheDocument();
    });
    mockInvoke.mockClear();
    setMockAgentTimelineDelay(1_000);

    const draft = createDraftOrphan();
    await view.router.push(`/chats/${draft.id}`);

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toMatch(/^\/chats\/o-draft-/);
      expect(view.getByText("今天想做什么？")).toBeInTheDocument();
      expect(view.container.querySelector(".chat-composer [role='textbox']")).toBeInTheDocument();
    }, { timeout: 3000 });

    expect(mockInvoke.mock.calls.some(([cmd, args]) =>
      cmd === AGENT_TIMELINE_LIST_COMMAND && args?.taskId === draft.id
    )).toBe(false);
    expect(mockInvoke.mock.calls.some(([cmd, args]) =>
      cmd === CHAT_GET_RUNTIME_SNAPSHOT_COMMAND && args?.taskId === draft.id
    )).toBe(false);
  });

  it("主窗口收到主导航事件时会打开目标路由", async () => {
    const view = await renderApp("main");

    await waitFor(() => {
      expect(mockListenerCount(MAIN_NAVIGATE_EVENT_NAME)).toBe(1);
    });

    emitTauriEvent(MAIN_NAVIGATE_EVENT_NAME, createAppNavigateEvent("/projects/lilia/tasks/t-001"));

    await waitFor(() => {
      expect(view.router.currentRoute.value.fullPath).toBe(
        "/projects/lilia/tasks/t-001",
      );
    });
  });

  it("主窗口收到 CLI 项目打开事件时刷新项目并打开项目页", async () => {
    const view = await renderApp("main");

    await waitFor(() => {
      expect(mockListenerCount(CLI_PROJECT_OPEN_EVENT_NAME)).toBe(1);
    });
    mockInvoke.mockClear();

    emitTauriEvent(
      CLI_PROJECT_OPEN_EVENT_NAME,
      createCliProjectOpenEvent("lilia", "D:\\PROJECT\\workspace\\Lilia"),
    );

    await waitFor(() => {
      expect(view.router.currentRoute.value.fullPath).toBe("/projects/lilia");
    });
    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === PROJECT_LIST_COMMAND)).toBe(false);
  });

  it("主窗口挂载后会消费首启 CLI 项目打开请求", async () => {
    setMockPendingCliProjectOpen({
      projectId: "tools",
      cwd: "D:\\PROJECT\\workspace\\tools",
    });

    const view = await renderApp("main");

    await waitFor(() => {
      expect(view.router.currentRoute.value.fullPath).toBe("/projects/tools");
    });
    expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND))
      .toHaveLength(1);
  });

  it("主窗口不会订阅弹窗导航事件", async () => {
    const view = await renderApp("main");

    expect(mockListenerCount(POPUP_NAVIGATE_EVENT_NAME)).toBe(0);

    emitTauriEvent(
      POPUP_NAVIGATE_EVENT_NAME,
      createAppNavigateEvent("/popup/projects/lilia/tasks/t-001"),
    );

    await Promise.resolve();
    expect(view.router.currentRoute.value.fullPath).toBe("/");
  });

  it("弹窗收到弹窗导航事件时会切换目标对话", async () => {
    const view = await renderApp("popup-task-t-001", "/popup/chats/o-001");

    await waitFor(() => {
      expect(mockListenerCount(POPUP_NAVIGATE_EVENT_NAME)).toBe(1);
    });

    emitTauriEvent(
      POPUP_NAVIGATE_EVENT_NAME,
      createAppNavigateEvent("/popup/projects/lilia/tasks/t-001"),
    );

    await waitFor(() => {
      expect(view.router.currentRoute.value.fullPath).toBe(
        "/popup/projects/lilia/tasks/t-001",
      );
    });
  });

  it("弹窗不会订阅主窗口导航事件", async () => {
    const view = await renderApp(
      "popup-task-t-001",
      "/popup/projects/lilia/tasks/t-001",
    );

    expect(mockListenerCount(MAIN_NAVIGATE_EVENT_NAME)).toBe(0);
    expect(mockListenerCount(CLI_PROJECT_OPEN_EVENT_NAME)).toBe(0);
    expect(mockListenerCount(POPUP_NAVIGATE_EVENT_NAME)).toBe(1);
    expect(view.router.currentRoute.value.fullPath).toBe(
      "/popup/projects/lilia/tasks/t-001",
    );
  });
});

