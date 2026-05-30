import { render, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it, vi } from "vitest";
import { defineComponent } from "vue";
import TitleBar from "../src/components/TitleBar.vue";
import {
  createDraftOrphan,
  createDraftTask,
  promoteDraftOrphan,
  promoteDraftTask,
} from "../src/data/tasks";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isMaximized: vi.fn(async () => false),
    onResized: vi.fn(async () => vi.fn()),
    minimize: vi.fn(async () => undefined),
    toggleMaximize: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined),
  }),
}));

async function renderTitleBar(initialRoute: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/projects/:projectId/tasks/:taskId", component: { template: "<div />" } },
      { path: "/chats/:taskId", component: { template: "<div />" } },
    ],
  });
  await router.push(initialRoute);
  await router.isReady();

  const Wrapper = defineComponent({
    components: { TitleBar },
    template: "<TitleBar />",
  });

  return render(Wrapper, {
    global: {
      plugins: [router],
    },
  });
}

describe("TitleBar breadcrumbs", () => {
  it("项目草稿首条消息入库后会跟随正式标题更新", async () => {
    const draft = createDraftTask("lilia");
    const view = await renderTitleBar(`/projects/lilia/tasks/${draft.id}`);

    expect(view.getByText("新对话")).toBeInTheDocument();

    await promoteDraftTask(draft.id, "用首条消息生成标题");

    await waitFor(() => {
      expect(view.getByText("用首条消息生成标题")).toBeInTheDocument();
    });
  });

  it("收集箱草稿首条消息入库后会跟随正式标题更新", async () => {
    const draft = createDraftOrphan();
    const view = await renderTitleBar(`/chats/${draft.id}`);

    expect(view.getByText("新对话")).toBeInTheDocument();

    await promoteDraftOrphan(draft.id, "收集箱里的第一条消息");

    await waitFor(() => {
      expect(view.getByText("收集箱里的第一条消息")).toBeInTheDocument();
    });
  });
});
