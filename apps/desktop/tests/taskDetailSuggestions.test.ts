import { render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it } from "vitest";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import { createDraftTask } from "../src/services/tasksStore";
import { mockInvoke } from "./tauriMock";

async function renderDraftTaskDetail(taskId: string) {
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

describe("TaskDetail conversation suggestions", () => {
  it("空白草稿会加载并展示新对话建议", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const draft = createDraftTask("lilia");
    const view = await renderDraftTaskDetail(draft.id);

    await waitFor(() => {
      expect(mockInvoke.mock.calls.some(([cmd]) => cmd === "conversation_suggestions_get"))
        .toBe(true);
      expect(view.getByRole("button", { name: "补齐建议缓存测试" })).toBeInTheDocument();
    });
  });
});
