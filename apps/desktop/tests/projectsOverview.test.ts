import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it, beforeEach } from "vitest";
import ProjectsOverview from "../src/pages/project/ProjectsOverview.vue";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";

async function renderProjectsOverview() {
  await Promise.all([projectsReady, allTasksReady]);
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

  it("展示所有项目并能进入单个项目页", async () => {
    const view = await renderProjectsOverview();

    expect(view.getByRole("heading", { name: "项目" })).toBeInTheDocument();
    expect(view.getByText("Lilia")).toBeInTheDocument();
    expect(view.getByText("D:\\PROJECT\\workspace\\Lilia")).toBeInTheDocument();
    expect(view.getByText("2 个对话")).toBeInTheDocument();

    await fireEvent.click(view.getByRole("link", { name: /Lilia/ }));

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe("/projects/lilia");
    });
  });
});
