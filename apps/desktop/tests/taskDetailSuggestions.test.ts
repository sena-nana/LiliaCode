import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { defineComponent } from "vue";
import { describe, expect, it } from "vitest";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { useSidebarDisplayMode } from "../src/composables/useSidebarDisplayMode";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import { createDraftOrphan, createDraftTask } from "../src/services/tasksStore";
import {
  failNextMockConversationSuggestions,
  mockInvoke,
  setMockConversationSuggestions,
} from "./tauriMock";

async function renderProjectDraftTaskDetail(taskId: string) {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(`/projects/lilia/tasks/${taskId}`);
  await router.isReady();

  return render(TaskDetail, {
    props: {
      projectId: "lilia",
      taskId,
    },
    global: {
      plugins: [router],
    },
  });
}

async function renderOrphanDraftTaskDetail(taskId: string) {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(`/chats/${taskId}`);
  await router.isReady();

  return render(TaskDetail, {
    props: {
      taskId,
    },
    global: {
      plugins: [router],
    },
  });
}

async function renderRouterView(initialRoute: string) {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();

  const Wrapper = defineComponent({
    template: "<RouterView />",
  });

  const view = render(Wrapper, {
    global: {
      plugins: [router],
    },
  });
  return { ...view, router };
}

function expectComposerFocused(view: ReturnType<typeof render>) {
  const input = view.getByRole("textbox");
  expect(document.activeElement).toBe(input);
}

describe("TaskDetail conversation suggestions", () => {
  it("项目空白草稿会在输入框卡片内加载并展示新对话建议", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const draft = createDraftTask("lilia");
    const view = await renderProjectDraftTaskDetail(draft.id);

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "conversation_suggestions_get"))
        .toBe(true);
      expect(view.getByRole("button", { name: "补齐建议缓存测试" })).toBeInTheDocument();
    });
    const suggestions = view.getByLabelText("新对话建议");
    expect(suggestions.closest(".chat-composer")).not.toBeNull();
    expect(view.getByRole("button", { name: "刷新新对话建议" })).toBeInTheDocument();
  });

  it("GitHub 建议会展示可扫描来源且点击仍填入 prompt", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    setMockConversationSuggestions([
      {
        id: "sg-github-pr",
        projectId: "lilia",
        taskIds: [],
        source: "github",
        githubActivities: [
          {
            id: "gh-pr-12",
            repoFullName: "sena-nana/LiliaCode",
            kind: "pull_request",
            title: "PR #12: 补齐 GitHub 建议来源",
            url: "https://github.com/sena-nana/LiliaCode/pull/12",
          },
        ],
        summary: "跟进 GitHub 建议来源",
        reason: "近期 PR 活动触发了新对话建议。",
        prompt: "请继续跟进 PR #12 的建议来源展示。",
        generatedAt: Date.now(),
      },
    ]);
    const draft = createDraftTask("lilia");
    const view = await renderProjectDraftTaskDetail(draft.id);

    await waitFor(() => {
      expect(view.getByText("跟进 GitHub 建议来源")).toBeInTheDocument();
      expect(view.getByText("sena-nana/LiliaCode · PR #12")).toBeInTheDocument();
    });

    await fireEvent.click(view.getByText("跟进 GitHub 建议来源"));

    expect(view.getByRole("textbox")).toHaveTextContent("请继续跟进 PR #12 的建议来源展示。");
  });

  it("点击刷新入口会强制刷新新对话建议", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const draft = createDraftTask("lilia");
    const view = await renderProjectDraftTaskDetail(draft.id);

    await waitFor(() => {
      expect(view.getByRole("button", { name: "补齐建议缓存测试" })).toBeInTheDocument();
    });
    mockInvoke.mockClear();

    await fireEvent.click(view.getByRole("button", { name: "刷新新对话建议" }));

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd, args]) =>
        cmd === "conversation_suggestions_get" &&
        args?.projectId === "lilia" &&
        args?.forceRefresh === true
      )).toBe(true);
    });
  });

  it("没有可用建议时显示轻量状态且不影响输入", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    setMockConversationSuggestions([]);
    const draft = createDraftTask("lilia");
    const view = await renderProjectDraftTaskDetail(draft.id);

    await waitFor(() => {
      expect(view.getByText("暂无可用建议")).toBeInTheDocument();
    });

    const input = view.getByRole("textbox");
    await fireEvent.input(input, { target: { textContent: "继续写草稿" } });

    expect(input).toHaveTextContent("继续写草稿");
    expect(view.queryByText("暂无可用建议")).toBeNull();
  });

  it("建议加载失败时显示轻量状态且不影响输入", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    failNextMockConversationSuggestions("suggestions unavailable");
    const draft = createDraftTask("lilia");
    const view = await renderProjectDraftTaskDetail(draft.id);

    await waitFor(() => {
      expect(view.getByText("建议暂时不可用")).toBeInTheDocument();
    });

    const input = view.getByRole("textbox");
    await fireEvent.input(input, { target: { textContent: "先自己输入" } });

    expect(input).toHaveTextContent("先自己输入");
    expect(view.queryByText("建议暂时不可用")).toBeNull();
  });

  it("主窗口项目草稿进入对话后自动聚焦输入框", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const draft = createDraftTask("lilia");
    const view = await renderProjectDraftTaskDetail(draft.id);

    await waitFor(() => {
      expectComposerFocused(view);
    });
  });

  it("收集箱空白草稿不加载也不展示建议", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const draft = createDraftOrphan();
    const view = await renderOrphanDraftTaskDetail(draft.id);

    await waitFor(() => {
      expect(view.getByText("今天想做什么？")).toBeInTheDocument();
    });

    expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "conversation_suggestions_get"))
      .toBe(false);
    expect(view.queryByLabelText("新对话建议")).toBeNull();
    expect(view.queryByRole("button", { name: "补齐建议缓存测试" })).toBeNull();
    expect(view.queryByRole("button", { name: "刷新新对话建议" })).toBeNull();
  });

  it("主窗口收集箱草稿进入对话后自动聚焦输入框", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const draft = createDraftOrphan();
    const view = await renderOrphanDraftTaskDetail(draft.id);

    await waitFor(() => {
      expectComposerFocused(view);
    });
  });

  it("统一侧栏模式下收集箱草稿可在输入框下方切换到项目草稿并保留输入", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    useSidebarDisplayMode().setSidebarDisplayMode("unified");
    const draft = createDraftOrphan();
    const view = await renderRouterView(`/chats/${draft.id}`);

    const input = await view.findByRole("textbox");
    await fireEvent.input(input, { target: { textContent: "把这个想法放进项目" } });
    await fireEvent.change(view.getByLabelText("选择项目"), {
      target: { value: "lilia" },
    });

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toMatch(
        /^\/projects\/lilia\/tasks\/t-draft-/,
      );
    });
    await waitFor(() => {
      expect(view.getByRole("textbox")).toHaveTextContent("把这个想法放进项目");
    });
  });
});
