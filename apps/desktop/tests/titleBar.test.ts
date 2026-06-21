import { render, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent } from "vue";
import TitleBar from "../src/components/TitleBar.vue";
import {
  createDraftOrphan,
  createDraftTask,
  promoteDraftOrphan,
  promoteDraftTask,
} from "../src/data/tasks";

const titleBarWindowMock = vi.hoisted(() => ({
  isMaximized: vi.fn(async () => false),
  onResized: vi.fn(async () => vi.fn()),
  minimize: vi.fn(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
  close: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => titleBarWindowMock,
}));

async function flushAsyncLifecycle() {
  await Promise.resolve();
  await Promise.resolve();
}

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
  beforeEach(() => {
    titleBarWindowMock.isMaximized.mockReset();
    titleBarWindowMock.onResized.mockReset();
    titleBarWindowMock.minimize.mockReset();
    titleBarWindowMock.toggleMaximize.mockReset();
    titleBarWindowMock.close.mockReset();
    titleBarWindowMock.isMaximized.mockResolvedValue(false);
    titleBarWindowMock.onResized.mockResolvedValue(vi.fn());
    titleBarWindowMock.minimize.mockResolvedValue(undefined);
    titleBarWindowMock.toggleMaximize.mockResolvedValue(undefined);
    titleBarWindowMock.close.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("项目草稿首条消息入库后会跟随正式标题更新", async () => {
    const draft = createDraftTask("lilia");
    const view = await renderTitleBar(`/projects/lilia/tasks/${draft.id}`);

    expect(view.getByText("新对话")).toBeInTheDocument();

    await promoteDraftTask(draft.id, "用首条消息生成标题");

    await waitFor(() => {
      expect(view.getByText("用首条消息生成标题")).toBeInTheDocument();
    });
  });

  it("已有会话优先通过轻量 summary 解析标题", async () => {
    const view = await renderTitleBar("/projects/lilia/tasks/t-001");

    await waitFor(() => {
      expect(view.getByText("接入 Claude Code 会话发现")).toBeInTheDocument();
    });
  });

  it("卸载期间完成 resize listener 注册时会立即反注册", async () => {
    const unlisten = vi.fn();
    let resolveResizeListener: ((unlisten: () => void) => void) | null = null;
    titleBarWindowMock.onResized.mockImplementationOnce(
      () => new Promise((resolve) => {
        resolveResizeListener = resolve;
      }),
    );

    const view = await renderTitleBar("/projects/lilia/tasks/t-001");
    await waitFor(() => {
      expect(titleBarWindowMock.onResized).toHaveBeenCalledTimes(1);
    });

    view.unmount();
    expect(unlisten).not.toHaveBeenCalled();
    expect(resolveResizeListener).not.toBeNull();
    resolveResizeListener?.(unlisten);
    await flushAsyncLifecycle();

    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("并发窗口状态同步时保留最后一次最大化结果", async () => {
    let resizeHandler: (() => void) | null = null;
    titleBarWindowMock.onResized.mockImplementationOnce(async (handler: () => void) => {
      resizeHandler = handler;
      return vi.fn();
    });

    const view = await renderTitleBar("/projects/lilia/tasks/t-001");
    await waitFor(() => {
      expect(resizeHandler).not.toBeNull();
    });

    let resolveOlderSync: (value: boolean) => void = () => {};
    titleBarWindowMock.isMaximized.mockReset();
    titleBarWindowMock.isMaximized
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveOlderSync = resolve;
        }),
      )
      .mockResolvedValueOnce(true);

    resizeHandler?.();
    resizeHandler?.();

    await waitFor(() => {
      expect(view.getByLabelText("还原")).toBeInTheDocument();
    });

    resolveOlderSync(false);
    await flushAsyncLifecycle();

    expect(view.getByLabelText("还原")).toBeInTheDocument();
  });

  it("卸载时取消面包屑 fallback 的 paint 调度", async () => {
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 23));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);

    const view = await renderTitleBar("/projects/lilia/tasks/t-titlebar-missing");
    view.unmount();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(23);
  });
});
