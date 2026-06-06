import { render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it } from "vitest";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import { createDraftOrphan, createDraftTask } from "../src/services/tasksStore";
import { mockInvoke } from "./tauriMock";

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
    expect(view.queryByRole("button", { name: "刷新" })).toBeNull();
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
  });
});
