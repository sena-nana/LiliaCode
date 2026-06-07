import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it, vi } from "vitest";
import { defineComponent } from "vue";
import type { Task } from "@lilia/contracts";
import ProjectTreeItem from "../src/components/sidebar/ProjectTreeItem.vue";
import ContextMenuHost from "../src/components/ContextMenuHost.vue";
import { TASKS } from "../src/data/tasks";
import { mockInvoke } from "./tauriMock";

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

function seedProjectConversations(count: number) {
  TASKS.value = {
    ...TASKS.value,
    lilia: Array.from({ length: count }, (_, index) =>
      projectConversation(`t-overflow-${index + 1}`, `对话 ${index + 1}`, index)
    ),
  };
}

async function renderProjectTreeItem(initialRoute = "/projects/lilia") {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/projects", component: { template: "<div />" } },
      { path: "/projects/:projectId", component: { template: "<div />" } },
      {
        path: "/projects/:projectId/tasks/:taskId",
        component: { template: "<div />" },
      },
    ],
  });
  await router.push(initialRoute);
  await router.isReady();

  const Wrapper = defineComponent({
    components: { ProjectTreeItem, ContextMenuHost },
    props: {
      isExpanded: {
        type: Boolean,
        default: true,
      },
    },
    template: `
      <ProjectTreeItem
        :project="project"
        :is-expanded="isExpanded"
        @toggle="$emit('toggle', $event)"
        @error="emitError"
        @archived="emitArchived"
      />
      <ContextMenuHost />
    `,
    setup() {
      return {
        project: {
          id: "lilia",
          name: "Lilia",
          cwd: "D:\\PROJECT\\workspace\\Lilia",
          sessionCount: 2,
          pinned: false,
        },
      };
    },
    methods: {
      emitError(message: string) {
        this.$emit("error", message);
      },
      emitArchived() {
        this.$emit("archived");
      },
    },
  });

  const view = render(Wrapper, {
    props: {
      isExpanded: true,
    },
    global: {
      plugins: [router],
      directives: {
        contextMenu: {},
      },
    },
  });
  return { ...view, router };
}

describe("ProjectTreeItem", () => {
  it("项目行主体点击会展开折叠，项目名称不再直接导航", async () => {
    const view = await renderProjectTreeItem("/projects/tools");
    const projectRow = view.getByText("Lilia").closest(".sb-tree__row--project");

    expect(view.queryByRole("link", { name: "Lilia" })).not.toBeInTheDocument();
    expect(view.queryByLabelText("折叠项目")).not.toBeInTheDocument();
    expect(projectRow).toHaveAttribute("aria-expanded", "true");

    await fireEvent.click(projectRow!);

    expect(view.emitted("toggle")).toEqual([["lilia"]]);
  });


  it("项目右键菜单第一项进入对应项目", async () => {
    const view = await renderProjectTreeItem("/projects/tools");

    await fireEvent.click(view.getByLabelText("更多"));

    const menuItems = await view.findAllByRole("menuitem");
    expect(menuItems[0]).toHaveTextContent("进入项目");

    await fireEvent.click(menuItems[0]);

    await waitFor(() => {
      expect(view.router.currentRoute.value.path).toBe("/projects/lilia");
    });
  });

  it("项目菜单可在弹出窗口中创建对话", async () => {
    const view = await renderProjectTreeItem("/projects/tools");

    await fireEvent.click(view.getByLabelText("更多"));
    await fireEvent.click(await view.findByRole("menuitem", {
      name: "在弹出窗口中创建对话",
    }));

    expect(mockInvoke).toHaveBeenCalledWith("popup_open_new_chat", {
      projectId: "lilia",
      initialDraftContent: null,
    }, undefined);
  });

  it("中键点击项目行会在弹出窗口中创建对话", async () => {
    const view = await renderProjectTreeItem("/projects/tools");
    const projectRow = view.getByText("Lilia").closest(".sb-tree__row--project");

    await fireEvent(
      projectRow!,
      new MouseEvent("auxclick", { bubbles: true, button: 1 }),
    );

    expect(mockInvoke).toHaveBeenCalledWith("popup_open_new_chat", {
      projectId: "lilia",
      initialDraftContent: null,
    }, undefined);
  });

  it("中键点击项目对话会在弹出窗口中打开", async () => {
    const view = await renderProjectTreeItem();
    const row = view.getByText("接入 Claude Code 会话发现").closest(".sb-tree__row");

    await fireEvent(
      row!,
      new MouseEvent("auxclick", { bubbles: true, button: 1 }),
    );

    expect(mockInvoke).toHaveBeenCalledWith("popup_open_task", {
      projectId: "lilia",
      taskId: "t-001",
    }, undefined);
  });

  it("项目对话超过四条时用省略行折叠剩余对话，并可点击展开", async () => {
    seedProjectConversations(6);

    const view = await renderProjectTreeItem();

    expect(view.getByText("对话 1")).toBeInTheDocument();
    expect(view.getByText("对话 4")).toBeInTheDocument();
    expect(view.queryByText("对话 5")).not.toBeInTheDocument();
    expect(view.queryByText("对话 6")).not.toBeInTheDocument();

    const more = view.getByRole("button", { name: "显示剩余对话" });
    expect(more).toHaveTextContent("...");
    expect(more).toHaveClass("sb-tree__row--child");

    await fireEvent.click(more);

    expect(view.getByText("对话 5")).toBeInTheDocument();
    expect(view.getByText("对话 6")).toBeInTheDocument();
    expect(view.queryByRole("button", { name: "显示剩余对话" })).not.toBeInTheDocument();
  });
});
