import { render } from "@testing-library/vue";
import { defineComponent, nextTick, reactive } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useTaskConversationContext } from "../src/pages/taskDetail/useTaskConversationContext";
import { invalidateConversationContextSnapshot } from "../src/services/conversationContextInvalidation";
import { homeDir } from "@tauri-apps/api/path";

const mocks = vi.hoisted(() => ({
  ensureProjectLoaded: vi.fn(),
  ensureTaskLoaded: vi.fn(),
  getProject: vi.fn(),
  getTask: vi.fn(),
  getOrphanConversation: vi.fn(),
  promoteDraftOrphan: vi.fn(),
  promoteDraftTask: vi.fn(),
  resolveConversationRouteState: vi.fn(),
}));

vi.mock("../src/services/projectsStore", () => ({
  ensureProjectLoaded: mocks.ensureProjectLoaded,
  getProject: mocks.getProject,
}));

vi.mock("../src/services/tasksStore", () => ({
  createDraftOrphan: vi.fn(() => ({ id: "draft-orphan" })),
  createDraftTask: vi.fn(() => ({ id: "draft-task" })),
  ensureTaskLoaded: mocks.ensureTaskLoaded,
  getOrphanConversation: mocks.getOrphanConversation,
  getTask: mocks.getTask,
  promoteDraftOrphan: mocks.promoteDraftOrphan,
  promoteDraftTask: mocks.promoteDraftTask,
  resolveConversationRouteState: mocks.resolveConversationRouteState,
}));

vi.mock("../src/services/popupWindows", () => ({
  rememberPopupLastProject: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: vi.fn(async () => "C:\\Users\\Administrator"),
}));

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

async function createRouterPlugin() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: { template: "<div />" } }],
  });
  await router.push("/");
  await router.isReady();
  return router;
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.ensureProjectLoaded.mockResolvedValue(null);
  mocks.ensureTaskLoaded.mockResolvedValue(null);
  mocks.getProject.mockReturnValue(undefined);
  mocks.getTask.mockReturnValue(undefined);
  mocks.getOrphanConversation.mockReturnValue(undefined);
  mocks.resolveConversationRouteState.mockReturnValue({
    isDraftRoute: false,
    isLiveDraft: false,
    isLostDraft: false,
  });
});

describe("useTaskConversationContext", () => {
  it("项目草稿不等待项目缓存即可渲染聊天界面", async () => {
    mocks.resolveConversationRouteState.mockReturnValue({
      isDraftRoute: true,
      isLiveDraft: true,
      isLostDraft: false,
    });
    mocks.getTask.mockReturnValue({
      id: "t-draft-1",
      projectId: "lilia",
      sessionId: "t-draft-1",
      title: "新对话",
      status: "draft",
      createdAt: 1,
      pinned: false,
      parentId: null,
      dependsOn: [],
    });

    let context: ReturnType<typeof useTaskConversationContext> | null = null;
    const Harness = defineComponent({
      setup() {
        context = useTaskConversationContext({
          variant: "main",
          projectId: "lilia",
          taskId: "t-draft-1",
        });
        return () => null;
      },
    });

    render(Harness, {
      global: {
        plugins: [await createRouterPlugin()],
      },
    });
    await nextTick();

    expect(context?.hasContext.value).toBe(true);
    expect(context?.shouldRenderChat.value).toBe(true);
    expect(context?.isContextLoading.value).toBe(false);
  });

  it("发送项目草稿前先加载项目上下文", async () => {
    mocks.resolveConversationRouteState.mockReturnValue({
      isDraftRoute: true,
      isLiveDraft: true,
      isLostDraft: false,
    });
    mocks.ensureProjectLoaded.mockResolvedValue({
      id: "lilia",
      name: "Lilia",
      cwd: "C:\\Files\\workspace\\Lilia",
      sessionCount: 0,
      pinned: false,
    });

    let context: ReturnType<typeof useTaskConversationContext> | null = null;
    const Harness = defineComponent({
      setup() {
        context = useTaskConversationContext({
          variant: "main",
          projectId: "lilia",
          taskId: "t-draft-1",
        });
        return () => null;
      },
    });

    render(Harness, {
      global: {
        plugins: [await createRouterPlugin()],
      },
    });
    await nextTick();

    await context!.ensureTaskReadyForMessage("开始实现", []);

    expect(mocks.ensureProjectLoaded).toHaveBeenCalledWith("lilia");
    expect(mocks.promoteDraftTask).toHaveBeenCalledWith("t-draft-1", "开始实现");
    expect(mocks.ensureProjectLoaded.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.promoteDraftTask.mock.invocationCallOrder[0],
    );
  });

  it("卸载后忽略仍在完成的弹窗上下文加载结果", async () => {
    const projectLoad = deferred();
    const taskLoad = deferred();
    mocks.ensureProjectLoaded.mockReturnValueOnce(projectLoad.promise);
    mocks.ensureTaskLoaded.mockReturnValueOnce(taskLoad.promise);

    let context: ReturnType<typeof useTaskConversationContext> | null = null;
    const Harness = defineComponent({
      setup() {
        context = useTaskConversationContext({
          variant: "popup",
          projectId: "lilia",
          taskId: "task-1",
        });
        return () => null;
      },
    });

    const view = render(Harness, {
      global: {
        plugins: [await createRouterPlugin()],
      },
    });
    await nextTick();

    expect(context?.popupContextHydrating.value).toBe(true);

    view.unmount();
    projectLoad.resolve();
    taskLoad.resolve();
    await nextTick();
    await nextTick();

    expect(context?.popupContextHydrating.value).toBe(true);
    expect(context?.popupContextHydrated.value).toBe(false);
  });

  it("卸载后忽略仍在返回的收集箱 cwd 读取结果", async () => {
    let resolveHomeDir: (value: string) => void = () => {};
    vi.mocked(homeDir).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveHomeDir = resolve;
      }),
    );

    let context: ReturnType<typeof useTaskConversationContext> | null = null;
    const Harness = defineComponent({
      setup() {
        context = useTaskConversationContext({
          variant: "main",
          taskId: "orphan-1",
        });
        void context.ensureOrphanCwd();
        return () => null;
      },
    });

    const view = render(Harness, {
      global: {
        plugins: [await createRouterPlugin()],
      },
    });

    view.unmount();
    resolveHomeDir("C:\\Users\\Late");
    await nextTick();
    await nextTick();

    expect(context?.orphanCwd.value).toBeNull();
  });

  it("路由切换会让已捕获的会话上下文快照失效", async () => {
    const props = reactive({
      variant: "main" as const,
      projectId: "lilia",
      taskId: "task-1",
    });
    let context: ReturnType<typeof useTaskConversationContext> | null = null;
    const Harness = defineComponent({
      setup() {
        context = useTaskConversationContext(props);
        return () => null;
      },
    });

    render(Harness, {
      global: {
        plugins: [await createRouterPlugin()],
      },
    });
    await nextTick();

    const snapshot = context!.captureContextSnapshot();
    context!.prepareForRouteChange();
    props.taskId = "task-2";
    await nextTick();

    expect(context!.isCurrentContextSnapshot(snapshot, "lilia", "task-1")).toBe(false);
  });

  it("弹窗关闭失效事件会让当前会话上下文快照失效", async () => {
    let context: ReturnType<typeof useTaskConversationContext> | null = null;
    const Harness = defineComponent({
      setup() {
        context = useTaskConversationContext({
          variant: "popup",
          projectId: "lilia",
          taskId: "task-1",
        });
        return () => null;
      },
    });

    render(Harness, {
      global: {
        plugins: [await createRouterPlugin()],
      },
    });
    await nextTick();

    const snapshot = context!.captureContextSnapshot();
    invalidateConversationContextSnapshot("popup-close");

    expect(context!.isCurrentContextSnapshot(snapshot, "lilia", "task-1")).toBe(false);
  });
});
