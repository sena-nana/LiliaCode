import { render, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SessionsView from "../src/pages/project/SessionsView.vue";
import * as tasksStore from "../src/services/tasksStore";

vi.mock("../src/services/tasksStore", async () => {
  const { shallowRef } = await import("vue");
  const loadedProjects = shallowRef<Record<string, boolean>>({});
  const tasksByProject = shallowRef<Record<string, Array<{ id: string; title: string; status: string }>>>({
    lilia: [
      { id: "t-001", title: "接入 Claude Code 会话发现", status: "done" },
    ],
    tools: [
      { id: "t-002", title: "整理窗口快捷键", status: "running" },
    ],
  });
  const ensureProjectTasksLoaded = vi.fn(async (projectId: string) => {
    loadedProjects.value = {
      ...loadedProjects.value,
      [projectId]: true,
    };
  });
  const resetSessionsViewMock = () => {
    loadedProjects.value = {};
    ensureProjectTasksLoaded.mockClear();
  };
  return {
    ensureProjectTasksLoaded,
    isProjectTasksLoaded: (projectId: string) => loadedProjects.value[projectId] === true,
    listProjectConversations: (projectId: string) => tasksByProject.value[projectId] ?? [],
    __resetSessionsViewMock: resetSessionsViewMock,
  };
});

type SessionsTasksStoreMock = typeof tasksStore & {
  __resetSessionsViewMock: () => void;
};

const mockedTasksStore = tasksStore as SessionsTasksStoreMock;

async function renderSessions(projectId = "lilia") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/projects/:projectId/tasks/:taskId", component: { template: "<div />" } },
    ],
  });
  await router.push("/");
  await router.isReady();

  return render(SessionsView, {
    props: { projectId },
    global: { plugins: [router] },
  });
}

async function flushDeferredProjectLoad() {
  await vi.advanceTimersByTimeAsync(20);
  await Promise.resolve();
  await Promise.resolve();
}

describe("SessionsView", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockedTasksStore.__resetSessionsViewMock();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("先渲染加载占位，再在空闲阶段拉取项目对话", async () => {
    const view = await renderSessions();

    expect(tasksStore.ensureProjectTasksLoaded).not.toHaveBeenCalled();
    expect(view.getByText("正在加载对话…")).toBeInTheDocument();

    await flushDeferredProjectLoad();

    expect(tasksStore.ensureProjectTasksLoaded).toHaveBeenCalledTimes(1);
    expect(tasksStore.ensureProjectTasksLoaded).toHaveBeenCalledWith("lilia");
    await waitFor(() => {
      expect(view.getByText("接入 Claude Code 会话发现")).toBeInTheDocument();
    });
  });

  it("快速切换项目时只为最新项目执行延后加载", async () => {
    const view = await renderSessions();

    await view.rerender({ projectId: "tools" });
    expect(tasksStore.ensureProjectTasksLoaded).not.toHaveBeenCalled();

    await flushDeferredProjectLoad();

    expect(tasksStore.ensureProjectTasksLoaded).toHaveBeenCalledTimes(1);
    expect(tasksStore.ensureProjectTasksLoaded).toHaveBeenCalledWith("tools");
    expect(tasksStore.ensureProjectTasksLoaded).not.toHaveBeenCalledWith("lilia");
    await waitFor(() => {
      expect(view.getByText("整理窗口快捷键")).toBeInTheDocument();
    });
  });
});
