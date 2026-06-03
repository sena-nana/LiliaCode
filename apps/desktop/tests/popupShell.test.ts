import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { defineComponent } from "vue";
import { describe, expect, it } from "vitest";
import {
  createLiliaRouter,
  shouldUsePopupHashHistory,
} from "../src/router";
import {
  mockCurrentWindow,
  mockInvoke,
} from "./tauriMock";
import { TASKS } from "../src/data/tasks";

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

async function expectReturnToMainRoute(initialRoute: string, mainRoute: string) {
  const view = await renderPopup(initialRoute);

  await fireEvent.click(view.getByRole("button", { name: "回到主窗口" }));

  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("popup_focus_main", {
      route: mainRoute,
    }, undefined);
    expect(mockCurrentWindow.close).toHaveBeenCalledTimes(1);
  });
}

function seedPersistedDraftPrefixTask() {
  TASKS.value = {
    ...TASKS.value,
    lilia: [
      {
        id: "t-draft-persisted",
        projectId: "lilia",
        sessionId: "persisted-session",
        title: "已发送的草稿前缀对话",
        status: "done",
        createdAt: 5000,
        pinned: false,
        parentId: null,
        dependsOn: [],
      },
      ...(TASKS.value.lilia ?? []),
    ],
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

  it.each([
    ["/popup/projects/lilia/tasks/t-001", "/projects/lilia/tasks/t-001"],
    ["/popup/chats/o-001", "/chats/o-001"],
    ["/popup/projects/lilia/tasks/t-draft-temp", "/projects/lilia"],
    ["/popup/chats/o-draft-temp", "/"],
  ])("回到主窗口会把 %s 映射到 %s", async (popupRoute, mainRoute) => {
    await expectReturnToMainRoute(popupRoute, mainRoute);
  });

  it("弹窗被重新导航到对话后，回到主窗口会聚焦该对话", async () => {
    const view = await renderPopup("/popup/chats/o-001");

    await view.router.replace("/popup/projects/lilia/tasks/t-001");
    await fireEvent.click(view.getByRole("button", { name: "回到主窗口" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("popup_focus_main", {
        route: "/projects/lilia/tasks/t-001",
      }, undefined);
      expect(mockCurrentWindow.close).toHaveBeenCalledTimes(1);
    });
  });

  it("弹窗可打开已持久化的草稿前缀对话，不会误跳到新对话", async () => {
    const route = "/popup/projects/lilia/tasks/t-draft-persisted";
    seedPersistedDraftPrefixTask();

    const view = await renderPopup(route);

    await waitFor(() => {
      expect(view.router.currentRoute.value.fullPath).toBe(route);
      expect(view.queryByText("正在创建新对话…")).not.toBeInTheDocument();
    });
  });

  it("已持久化的草稿前缀对话回到主窗口时会聚焦对话而不是项目", async () => {
    seedPersistedDraftPrefixTask();

    await expectReturnToMainRoute(
      "/popup/projects/lilia/tasks/t-draft-persisted",
      "/projects/lilia/tasks/t-draft-persisted",
    );
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
