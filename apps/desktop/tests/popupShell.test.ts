import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { defineComponent } from "vue";
import { describe, expect, it } from "vitest";
import {
  createLiliaRouter,
  shouldUsePopupHashHistory,
} from "../src/router";
import { mockInvoke } from "./tauriMock";

async function renderPopup(initialRoute = "/popup/projects/lilia/tasks/t-001") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();

  const Root = defineComponent({
    template: "<RouterView />",
  });

  return {
    ...render(Root, {
      global: {
        plugins: [router],
      },
    }),
    router,
  };
}

describe("Popup shell", () => {
  it("弹窗 index hash 入口使用 hash history", () => {
    expect(shouldUsePopupHashHistory("#/popup/chats/new")).toBe(true);
    expect(shouldUsePopupHashHistory("#/projects/lilia")).toBe(false);
    expect(shouldUsePopupHashHistory("")).toBe(false);
  });

  it("弹出窗口路由只渲染对话主体，不渲染左右侧栏", async () => {
    const view = await renderPopup();

    expect(view.container.querySelector(".popup-shell")).toBeInTheDocument();
    expect(view.container.querySelector(".secondary-panel")).not.toBeInTheDocument();
    expect(view.container.querySelector(".chat-sidebar")).not.toBeInTheDocument();
    expect(view.getByRole("button", { name: "关闭弹出窗口" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "新对话" })).toBeInTheDocument();
    expect(view.getByRole("button", { name: "回到主窗口" })).toBeInTheDocument();
  });

  it("回到主窗口按钮会同步当前对话路由", async () => {
    const view = await renderPopup();

    await fireEvent.click(view.getByRole("button", { name: "回到主窗口" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("popup_focus_main", {
        route: "/projects/lilia/tasks/t-001",
      }, undefined);
    });
  });

  it("刷新到已丢失的弹窗草稿时会重新创建窗口内草稿", async () => {
    const lostDraftRoute = "/popup/projects/lilia/tasks/t-draft-missing-refresh";
    const view = await renderPopup(lostDraftRoute);

    await waitFor(() => {
      expect(view.router.currentRoute.value.fullPath).not.toBe(lostDraftRoute);
      expect(view.router.currentRoute.value.fullPath).toMatch(
        /^\/popup\/projects\/lilia\/tasks\/t-draft-/,
      );
    });
  });
});
