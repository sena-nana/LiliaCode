import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it, beforeEach } from "vitest";
import SecondaryPanel from "../src/layouts/SecondaryPanel.vue";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";

function seedTreeExpansionState(state: unknown) {
  localStorage.setItem("lilia.projectTree.expansion", JSON.stringify(state));
}

async function renderSecondaryPanel() {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push("/");
  await router.isReady();

  return render(SecondaryPanel, {
    global: {
      plugins: [router],
    },
  });
}

function getProjectToggle(view: ReturnType<typeof render>, projectName: string): HTMLElement {
  const row = view.getByText(projectName).closest(".sb-tree__row--project");
  if (!(row instanceof HTMLElement)) {
    throw new Error(`未找到项目行：${projectName}`);
  }
  return row;
}

describe("SecondaryPanel project tree expansion", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
    localStorage.clear();
  });

  it("会恢复上次关闭时的项目展开状态和收集箱状态", async () => {
    seedTreeExpansionState({
      projects: {
        lilia: false,
        tools: true,
      },
      orphansExpanded: false,
    });

    const view = await renderSecondaryPanel();

    const liliaToggle = getProjectToggle(view, "Lilia");
    const toolsToggle = getProjectToggle(view, "工具箱");
    const orphansToggle = view.getByRole("button", {
      name: "展开收集箱",
    });

    expect(liliaToggle).toHaveAttribute("aria-expanded", "false");
    expect(toolsToggle).toHaveAttribute("aria-expanded", "true");
    expect(orphansToggle).toBeInTheDocument();
  });

  it("用户切换后会写回本地存储", async () => {
    seedTreeExpansionState({
      projects: {
        lilia: true,
        tools: true,
      },
      orphansExpanded: true,
    });

    const view = await renderSecondaryPanel();
    await fireEvent.click(getProjectToggle(view, "Lilia"));
    await fireEvent.click(view.getByRole("button", { name: "折叠收集箱" }));

    await waitFor(() => {
      const raw = localStorage.getItem("lilia.projectTree.expansion");
      expect(raw).toBeTruthy();
      const parsed = JSON.parse(raw ?? "{}") as {
        projects?: Record<string, boolean>;
        orphansExpanded?: boolean;
      };
      expect(parsed.projects?.lilia).toBe(false);
      expect(parsed.projects?.tools).toBe(true);
      expect(parsed.orphansExpanded).toBe(false);
    });
  });

  it("新增项目会默认展开并同步保存", async () => {
    seedTreeExpansionState({
      projects: {
        lilia: false,
        tools: false,
      },
      orphansExpanded: true,
    });

    const view = await renderSecondaryPanel();
    await fireEvent.click(view.getByRole("button", { name: "添加项目" }));
    await fireEvent.click(await view.findByRole("menuitem", { name: "创建空分类" }));
    await fireEvent.update(
      await view.findByPlaceholderText("例如：实验、归档…"),
      "临时分类",
    );
    await fireEvent.click(view.getByRole("button", { name: "创建" }));

    await waitFor(() => {
      expect(getProjectToggle(view, "临时分类")).toHaveAttribute(
        "aria-expanded",
        "true",
      );
    });

    const raw = localStorage.getItem("lilia.projectTree.expansion");
    expect(raw).toBeTruthy();
    const parsed = JSON.parse(raw ?? "{}") as {
      projects?: Record<string, boolean>;
    };
    expect(parsed.projects?.lilia).toBe(false);
    expect(parsed.projects?.tools).toBe(false);
    expect(Object.values(parsed.projects ?? {}).some(Boolean)).toBe(true);
  });
});
