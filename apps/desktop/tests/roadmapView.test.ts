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

async function waitForTaskInsightsReady(view: Awaited<ReturnType<typeof renderRoadmap>>) {
  await waitFor(() => {
    expect(view.getByLabelText("路线图完成度")).not.toHaveTextContent("--");
    expect(view.getAllByText("接入 Claude Code 会话发现").length).toBeGreaterThan(0);
  });
}

describe("RoadmapView", () => {
  it("从真实 milestone/link 快照展示路线图进度", async () => {
    const view = await renderRoadmap();

    await waitFor(() => {
      expect(view.getByText("首发可用路线图")).toBeInTheDocument();
    });
    await waitForTaskInsightsReady(view);

    expect(view.queryByText(/Task 状态聚合/)).not.toBeInTheDocument();
    expect(view.getByLabelText("路线图完成度")).toHaveTextContent("50%");
    expect(view.getAllByText("1/2 done").length).toBeGreaterThan(0);
    expect(view.getAllByText("接入 Claude Code 会话发现").length).toBeGreaterThan(0);
    expect(view.getAllByText("打通 tsconfig paths 搜索").length).toBeGreaterThan(0);
    expect(view.getAllByText("1 子任务").length).toBeGreaterThan(0);
    expect(view.getAllByText(/1 依赖/).length).toBeGreaterThan(0);
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
    await waitForTaskInsightsReady(view);

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

  it("能编辑 milestone 描述和截止日期", async () => {
    const view = await renderRoadmap();

    const description = await view.findByLabelText("首发可用路线图 描述");
    await fireEvent.update(description, "先让路线图可以落地跟踪");
    await fireEvent.change(description);

    await waitFor(() => {
      expect(view.getByLabelText("首发可用路线图 描述")).toHaveValue("先让路线图可以落地跟踪");
    });

    const dueDate = view.getByLabelText("首发可用路线图 截止日期");
    await fireEvent.update(dueDate, "2026-06-30");
    await fireEvent.change(dueDate);

    await waitFor(() => {
      expect(view.getByLabelText("首发可用路线图 截止日期")).toHaveValue("2026-06-30");
    });

    await fireEvent.update(dueDate, "");
    await fireEvent.change(dueDate);

    await waitFor(() => {
      expect(view.getByLabelText("首发可用路线图 截止日期")).toHaveValue("");
    });
  });

  it("能删除 milestone 并清理本地任务关联", async () => {
    const view = await renderRoadmap();

    await view.findByText("首发可用路线图");
    await waitForTaskInsightsReady(view);
    await fireEvent.click(view.getByRole("button", { name: "删除 首发可用路线图" }));

    await waitFor(() => {
      expect(view.queryByText("首发可用路线图")).not.toBeInTheDocument();
    });
    expect(view.getByText("暂无路线图")).toBeInTheDocument();
    expect(view.getByLabelText("路线图完成度")).toHaveTextContent("0%");
    expect(view.queryByText("接入 Claude Code 会话发现")).not.toBeInTheDocument();
  });

  it("能通过上移下移重排 milestone", async () => {
    setMockRoadmap([
      {
        id: "m-001",
        projectId: "lilia",
        title: "第一阶段",
        description: "",
        status: "in-progress",
        dueDate: null,
        order: 0,
        createdAt: 5000,
      },
      {
        id: "m-002",
        projectId: "lilia",
        title: "第二阶段",
        description: "",
        status: "upcoming",
        dueDate: null,
        order: 1,
        createdAt: 6000,
      },
    ]);
    const view = await renderRoadmap();

    await view.findByText("第一阶段");
    await fireEvent.click(view.getByRole("button", { name: "第一阶段 下移" }));

    await waitFor(() => {
      const headings = view.getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent);
      expect(headings).toEqual(["第二阶段", "第一阶段"]);
    });

    await fireEvent.click(view.getByRole("button", { name: "第一阶段 上移" }));

    await waitFor(() => {
      const headings = view.getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent);
      expect(headings).toEqual(["第一阶段", "第二阶段"]);
    });
  });
});
