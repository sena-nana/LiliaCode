import { render, fireEvent, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it, beforeEach } from "vitest";
import ProjectsOverview from "../src/pages/project/ProjectsOverview.vue";

async function renderProjectsOverview() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/projects", component: ProjectsOverview },
      { path: "/projects/:projectId", component: { template: "<div />" } },
    ],
  });
  await router.push("/projects");
  await router.isReady();

  const view = render(ProjectsOverview, {
    global: {
      plugins: [router],
    },
  });
  return { ...view, router };
}

describe("ProjectsOverview", () => {
  beforeEach(() => {
    localStorage.clear();
  });


  it("展示项目 dashboard 并能进入单个项目页", async () => {
    const view = await renderProjectsOverview();

    expect(view.getByRole("heading", { name: "项目" })).toBeInTheDocument();
    await waitFor(() => {
      expect(view.getByText("D:\\PROJECT\\workspace\\Lilia")).toBeInTheDocument();
    });

    const lilia = view.getByRole("link", { name: /Lilia/ });
    expect(within(lilia).getByText("运行 1")).toBeInTheDocument();
    expect(within(lilia).getByText("完成 1")).toBeInTheDocument();
    expect(within(lilia).getByText("阻塞 0")).toBeInTheDocument();
    expect(within(lilia).getByText("4.2K tokens")).toBeInTheDocument();
    expect(within(lilia).getByText("$0.1500")).toBeInTheDocument();
    expect(within(lilia).getByText("1/2 条成本")).toBeInTheDocument();
    expect(within(lilia).getByText(/最近活跃/)).toBeInTheDocument();

    const tools = view.getByRole("link", { name: /工具箱/ });
    expect(within(tools).getByText("等待 1")).toBeInTheDocument();
    expect(within(tools).getByText("0 tokens")).toBeInTheDocument();
    expect(within(tools).getByText("暂无成本")).toBeInTheDocument();
    expect(within(tools).getByText("无用量记录")).toBeInTheDocument();

    await fireEvent.click(lilia);

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe("/projects/lilia");
    });
  });
});
