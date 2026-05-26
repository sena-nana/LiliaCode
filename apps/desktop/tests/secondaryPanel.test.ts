import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it, beforeEach } from "vitest";
import SecondaryPanel from "../src/layouts/SecondaryPanel.vue";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import { mockInvoke, setMockProjectPinned } from "./tauriMock";

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

function getConversationRow(view: ReturnType<typeof render>, title: string): HTMLElement {
  const row = view.getByText(title).closest(".sb-tree__row");
  if (!(row instanceof HTMLElement)) {
    throw new Error(`未找到对话行：${title}`);
  }
  return row;
}

function box(top: number, bottom: number): DOMRect {
  return {
    x: 0,
    y: top,
    left: 0,
    top,
    right: 220,
    bottom,
    width: 220,
    height: bottom - top,
    toJSON: () => ({}),
  } as DOMRect;
}

async function dragFromTo(source: HTMLElement, target: HTMLElement, targetY: number) {
  await fireEvent.pointerDown(source, {
    button: 0,
    pointerId: 1,
    clientX: 20,
    clientY: 10,
  });
  await fireEvent.pointerMove(window, {
    pointerId: 1,
    clientX: 20,
    clientY: targetY,
  });
  await fireEvent.pointerUp(target, {
    pointerId: 1,
    clientX: 20,
    clientY: targetY,
  });
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

describe("SecondaryPanel project tree drag", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
    localStorage.clear();
  });

  it("项目拖动时显示落点指示，松手后更新同置顶分组顺序", async () => {
    const view = await renderSecondaryPanel();
    const lilia = getProjectToggle(view, "Lilia");
    const tools = getProjectToggle(view, "工具箱");
    lilia.getBoundingClientRect = () => box(0, 28);
    tools.getBoundingClientRect = () => box(40, 68);

    await fireEvent.pointerDown(lilia, {
      button: 0,
      pointerId: 1,
      clientX: 20,
      clientY: 10,
    });
    await fireEvent.pointerMove(tools, {
      pointerId: 1,
      clientX: 20,
      clientY: 60,
    });

    expect(lilia).toHaveClass("is-tree-drag-source");
    expect(tools).toHaveClass("is-tree-drop-target");
    expect(tools).toHaveClass("is-tree-drop-after");
    expect(tools).not.toHaveClass("is-tree-drop-invalid");

    await fireEvent.pointerUp(tools, {
      pointerId: 1,
      clientX: 20,
      clientY: 60,
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("project_reorder", {
        orderedIds: ["tools", "lilia"],
      }, undefined);
    });
  });

  it("对话行禁用原生链接拖拽，并能从标题拖到另一个项目", async () => {
    const view = await renderSecondaryPanel();
    const source = getConversationRow(view, "打通 tsconfig paths 搜索");
    const sourceTitle = view.getByText("打通 tsconfig paths 搜索");
    const targetProject = getProjectToggle(view, "工具箱");
    source.getBoundingClientRect = () => box(30, 58);
    targetProject.getBoundingClientRect = () => box(90, 118);

    expect(source).toHaveAttribute("draggable", "false");

    await dragFromTo(sourceTitle, targetProject, 104);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("task_reparent", {
        taskId: "t-002",
        newProjectId: "tools",
      }, undefined);
      expect(mockInvoke).toHaveBeenCalledWith("task_reorder", {
        projectId: "tools",
        orderedIds: ["t-003", "t-002"],
      }, undefined);
    });
  });

  it("对话拖到收集箱时 newProjectId 为 null", async () => {
    const view = await renderSecondaryPanel();
    const source = getConversationRow(view, "打通 tsconfig paths 搜索");
    const orphan = getConversationRow(view, "随手问问 Claude：tsconfig paths");
    source.getBoundingClientRect = () => box(30, 58);
    orphan.getBoundingClientRect = () => box(150, 178);

    await dragFromTo(source, orphan, 166);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("task_reparent", {
        taskId: "t-002",
        newProjectId: null,
      }, undefined);
    });
  });

  it("跨置顶分组拖动项目时显示不可投放，并且不会发排序命令", async () => {
    setMockProjectPinned("tools", true);
    const view = await renderSecondaryPanel();
    const lilia = getProjectToggle(view, "Lilia");
    const tools = getProjectToggle(view, "工具箱");
    lilia.getBoundingClientRect = () => box(40, 68);
    tools.getBoundingClientRect = () => box(0, 28);

    await fireEvent.pointerDown(lilia, {
      button: 0,
      pointerId: 1,
      clientX: 20,
      clientY: 54,
    });
    await fireEvent.pointerMove(tools, {
      pointerId: 1,
      clientX: 20,
      clientY: 14,
    });

    expect(tools).toHaveClass("is-tree-drop-target");
    expect(tools).toHaveClass("is-tree-drop-invalid");

    await fireEvent.pointerUp(tools, {
      pointerId: 1,
      clientX: 20,
      clientY: 14,
    });

    expect(
      mockInvoke.mock.calls.some(([cmd]) => cmd === "project_reorder"),
    ).toBe(false);
  });
});
