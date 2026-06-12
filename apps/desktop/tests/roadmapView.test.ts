import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it } from "vitest";
import RoadmapView from "../src/pages/project/RoadmapView.vue";
import { setMockRoadmap } from "./tauriMock";

async function renderRoadmap(projectId = "lilia") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/projects/:projectId/tasks/:taskId", component: { template: "<div />" } },
    ],
  });
  await router.push("/");
  await router.isReady();

  return render(RoadmapView, {
    props: { projectId },
    global: { plugins: [router] },
  });
}

describe("RoadmapView", () => {
  it("从真实 milestone/link 快照展示路线图进度", async () => {
    const view = await renderRoadmap();

    await waitFor(() => {
      expect(view.getByText("首发可用路线图")).toBeInTheDocument();
    });

    expect(view.queryByText(/Task 状态聚合/)).not.toBeInTheDocument();
    expect(view.getByLabelText("路线图完成度")).toHaveTextContent("50%");
    expect(view.getAllByText("1/2 done").length).toBeGreaterThan(0);
    expect(view.getAllByText("接入 Claude Code 会话发现").length).toBeGreaterThan(0);
    expect(view.getAllByText("打通 tsconfig paths 搜索").length).toBeGreaterThan(0);
  });

  it("没有 milestone 时显示空状态并能新建", async () => {
    setMockRoadmap([], []);
    const view = await renderRoadmap();

    await waitFor(() => {
      expect(view.getByText("暂无路线图")).toBeInTheDocument();
    });

    await fireEvent.update(view.getByLabelText("新 Milestone 标题"), "第二阶段");
    await fireEvent.click(view.getByRole("button", { name: "新增" }));

    await waitFor(() => {
      expect(view.getByText("第二阶段")).toBeInTheDocument();
    });
    expect(view.queryByText("暂无路线图")).not.toBeInTheDocument();
  });

  it("能切换 milestone 状态并勾选关联任务", async () => {
    const view = await renderRoadmap();

    const statusSelect = await view.findByLabelText("首发可用路线图 状态");
    await fireEvent.update(statusSelect, "done");

    await waitFor(() => {
      expect(statusSelect).toHaveValue("done");
    });

    const linkedTask = view.getByLabelText(/打通 tsconfig paths 搜索/);
    await fireEvent.click(linkedTask);

    await waitFor(() => {
      expect(view.getByLabelText("路线图完成度")).toHaveTextContent("100%");
    });
    expect(linkedTask).not.toBeChecked();
    expect(view.getAllByText("1/1 done").length).toBeGreaterThan(0);
  });
});
