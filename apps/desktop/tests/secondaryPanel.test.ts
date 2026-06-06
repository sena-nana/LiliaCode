import { render, fireEvent, waitFor, within } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { describe, expect, it, beforeEach, vi } from "vitest";
import { defineComponent, nextTick } from "vue";
import type { Task } from "@lilia/contracts";
import SecondaryPanel from "../src/layouts/SecondaryPanel.vue";
import ContextMenuHost from "../src/components/ContextMenuHost.vue";
import { useConnectionStatus } from "../src/composables/useConnectionStatus";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady, TASKS } from "../src/data/tasks";
import {
  mockInvoke,
  setMockActiveBackend,
  setMockCodexAppServerStatus,
  setMockProjectPinned,
} from "./tauriMock";

function seedTreeExpansionState(state: unknown) {
  localStorage.setItem("lilia.projectTree.expansion", JSON.stringify(state));
}

async function renderSecondaryPanel(initialRoute = "/") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(initialRoute);
  await router.isReady();

  const Wrapper = defineComponent({
    components: { SecondaryPanel, ContextMenuHost },
    template: `
      <SecondaryPanel />
      <ContextMenuHost />
    `,
  });

  const view = render(Wrapper, {
    global: {
      plugins: [router],
    },
  });
  return { ...view, router };
}

function getProjectRow(view: ReturnType<typeof render>, projectName: string): HTMLElement {
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

function projectConversation(id: string, title: string, index: number): Task {
  return {
    id,
    projectId: "lilia",
    sessionId: `session-${id}`,
    title,
    status: "done",
    createdAt: 1000 + index,
    pinned: false,
    parentId: null,
    dependsOn: [],
  };
}

function seedSecondaryPanelOverflowConversations() {
  TASKS.value = {
    ...TASKS.value,
    lilia: Array.from({ length: 6 }, (_, index) =>
      projectConversation(`t-overflow-${index + 1}`, `溢出对话 ${index + 1}`, index)
    ),
  };
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

  it("左下角连接徽章显示全局 active provider", async () => {
    setMockActiveBackend("codex");
    const view = await renderSecondaryPanel();

    await waitFor(() => {
      const label = view.container.querySelector(".sb-conn__label");
      expect(label).toHaveTextContent("Codex");
    });
  });

  it("Codex app-server 环境不满足时左下角 provider 卡片标红", async () => {
    setMockActiveBackend("codex");
    setMockCodexAppServerStatus({
      supportsRequiredProtocol: false,
      failureKind: "experimentalApiUnsupported",
      issues: ["Codex app-server 环境不满足。"],
    });
    await useConnectionStatus().refresh();
    const view = await renderSecondaryPanel();

    await waitFor(() => {
      const card = view.container.querySelector(".sb-conn");
      expect(card).toHaveClass("sb-conn--error");
      expect(card).toHaveTextContent("异常");
      expect(card).toHaveAttribute("title", expect.stringContaining("Codex app-server 环境不满足"));
      expect(card).toHaveAttribute("title", expect.not.stringContaining("OpenAI Responses API"));
    });
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

    const liliaRow = getProjectRow(view, "Lilia");
    const toolsRow = getProjectRow(view, "工具箱");
    const orphansToggle = view.getByRole("button", {
      name: "展开收集箱",
    });

    expect(liliaRow).toHaveAttribute("aria-expanded", "false");
    expect(toolsRow).toHaveAttribute("aria-expanded", "true");
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
    await fireEvent.click(getProjectRow(view, "Lilia"));
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
});

describe("SecondaryPanel project chat navigation", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
    localStorage.clear();
  });


  it("创建空分类后会自动进入该项目的新对话", async () => {
    const view = await renderSecondaryPanel();

    await fireEvent.click(view.getByRole("button", { name: "添加项目" }));
    await fireEvent.click(await view.findByRole("menuitem", { name: "创建空分类" }));
    await fireEvent.update(
      await view.findByPlaceholderText("例如：实验、归档…"),
      "临时分类",
    );
    await fireEvent.click(view.getByRole("button", { name: "创建" }));

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toMatch(
        /^\/projects\/p-3\/tasks\/t-draft-/,
      );
    });
  });


  it("顶部搜索会话后点击结果进入对应对话并关闭搜索", async () => {
    const view = await renderSecondaryPanel();

    await fireEvent.click(view.getByRole("button", { name: "搜索会话" }));
    await fireEvent.update(
      view.getByPlaceholderText("搜索会话…"),
      "tsconfig",
    );

    const listbox = await view.findByRole("listbox");
    await fireEvent.click(
      within(listbox).getByRole("option", {
        name: /打通 tsconfig paths 搜索/,
      }),
    );

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe(
        "/projects/lilia/tasks/t-002",
      );
    });
    expect(view.queryByPlaceholderText("搜索会话…")).not.toBeInTheDocument();
    expect(view.getByRole("button", { name: "搜索会话" })).toBeInTheDocument();
  });


  it("归档当前打开的单条项目对话后会进入该项目的新对话", async () => {
    const view = await renderSecondaryPanel("/projects/lilia/tasks/t-001");
    const row = getConversationRow(view, "接入 Claude Code 会话发现");

    await fireEvent.click(within(row).getByRole("button", { name: "归档" }));
    await fireEvent.click(
      within(row).getByRole("button", { name: "确认归档，再点一次" }),
    );

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toMatch(
        /^\/projects\/lilia\/tasks\/t-draft-/,
      );
    });
  });

  it("中键点击收集箱对话会在弹出窗口中打开", async () => {
    const view = await renderSecondaryPanel();
    const row = getConversationRow(view, "随手问问 Claude：tsconfig paths");

    await fireEvent(
      row,
      new MouseEvent("auxclick", { bubbles: true, button: 1 }),
    );

    expect(mockInvoke).toHaveBeenCalledWith("popup_open_task", {
      projectId: null,
      taskId: "o-001",
    }, undefined);
  });
});

describe("SecondaryPanel project tree drag", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
    localStorage.clear();
  });


  it("项目拖动时显示落点指示，松手后更新同置顶分组顺序", async () => {
    const view = await renderSecondaryPanel();
    const lilia = getProjectRow(view, "Lilia");
    const tools = getProjectRow(view, "工具箱");
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
});
