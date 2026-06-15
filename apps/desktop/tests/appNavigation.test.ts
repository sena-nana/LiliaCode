import { render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it, vi } from "vitest";
import App from "../src/App.vue";
import { createLiliaRouter } from "../src/router";
import {
  emitTauriEvent,
  mockListenerCount,
  setMockCurrentWindowLabel,
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
    useVueFlow: () => ({ fitView: vi.fn(), zoomIn: vi.fn(), zoomOut: vi.fn() }),
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
    expect(mockListenerCount("chat:agent-interaction-request")).toBe(1);
  });
  await waitFor(() => {
    expect(mockListenerCount(
      windowLabel === "main" ? "lilia:main:navigate" : "lilia:popup:navigate",
    )).toBe(1);
  });
  await Promise.resolve();

  return {
    ...view,
    router,
  };
}

describe("App main navigation events", () => {
  it("自动化页面只在进入路由后加载重 UI 并注册事件监听", async () => {
    const view = await renderApp("main");

    expect(view.router.currentRoute.value.fullPath).toBe("/");
    expect(vueFlowImportState.loaded).toBe(false);
    expect(mockListenerCount("automation:changed")).toBe(0);
    expect(mockListenerCount("automation:run-started")).toBe(0);
    expect(mockListenerCount("automation:run-updated")).toBe(0);
    expect(mockListenerCount("automation:run-finished")).toBe(0);

    await view.router.push("/automations");

    await waitFor(() => {
      expect(vueFlowImportState.loaded).toBe(true);
      expect(mockListenerCount("automation:changed")).toBe(1);
      expect(mockListenerCount("automation:run-started")).toBe(1);
      expect(mockListenerCount("automation:run-updated")).toBe(1);
      expect(mockListenerCount("automation:run-finished")).toBe(1);
    });
  });

  it("主窗口收到主导航事件时会打开目标路由", async () => {
    const view = await renderApp("main");

    await waitFor(() => {
      expect(mockListenerCount("lilia:main:navigate")).toBe(1);
    });

    emitTauriEvent("lilia:main:navigate", {
      route: "/projects/lilia/tasks/t-001",
    });

    await waitFor(() => {
      expect(view.router.currentRoute.value.fullPath).toBe(
        "/projects/lilia/tasks/t-001",
      );
    });
  });

  it("主窗口不会订阅弹窗导航事件", async () => {
    const view = await renderApp("main");

    expect(mockListenerCount("lilia:popup:navigate")).toBe(0);

    emitTauriEvent("lilia:popup:navigate", {
      route: "/popup/projects/lilia/tasks/t-001",
    });

    await Promise.resolve();
    expect(view.router.currentRoute.value.fullPath).toBe("/");
  });

  it("弹窗收到弹窗导航事件时会切换目标对话", async () => {
    const view = await renderApp("popup-task-t-001", "/popup/chats/o-001");

    await waitFor(() => {
      expect(mockListenerCount("lilia:popup:navigate")).toBe(1);
    });

    emitTauriEvent("lilia:popup:navigate", {
      route: "/popup/projects/lilia/tasks/t-001",
    });

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

    expect(mockListenerCount("lilia:main:navigate")).toBe(0);
    expect(mockListenerCount("lilia:popup:navigate")).toBe(1);
    expect(view.router.currentRoute.value.fullPath).toBe(
      "/popup/projects/lilia/tasks/t-001",
    );
  });
});
