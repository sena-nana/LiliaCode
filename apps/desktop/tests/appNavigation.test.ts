import { render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it } from "vitest";
import App from "../src/App.vue";
import { createLiliaRouter } from "../src/router";
import {
  emitTauriEvent,
  mockListenerCount,
  setMockCurrentWindowLabel,
} from "./tauriMock";

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
    expect(mockListenerCount("chat:tool-consent-request")).toBe(1);
    expect(mockListenerCount("chat:ask-user-request")).toBe(1);
  });
  await Promise.resolve();

  return {
    ...view,
    router,
  };
}

describe("App main navigation events", () => {
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

  it("弹窗不会订阅主窗口导航事件", async () => {
    const view = await renderApp(
      "popup-task-t-001",
      "/popup/projects/lilia/tasks/t-001",
    );

    expect(mockListenerCount("lilia:main:navigate")).toBe(0);
    expect(view.router.currentRoute.value.fullPath).toBe(
      "/popup/projects/lilia/tasks/t-001",
    );
  });
});
