import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it } from "vitest";
import { defineComponent } from "vue";
import ProjectTreeItem from "../src/components/sidebar/ProjectTreeItem.vue";
import ContextMenuHost from "../src/components/ContextMenuHost.vue";
import { mockInvoke } from "./tauriMock";

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
    template: `
      <ProjectTreeItem
        :project="project"
        :is-expanded="true"
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

  it("session 置顶按钮显示在归档按钮左侧并触发切换", async () => {
    const view = await renderProjectTreeItem();
    const sessionRow = view.getByText("接入 Claude Code 会话发现")
      .closest("a");
    const buttons = Array.from(sessionRow?.querySelectorAll("button") ?? []);

    expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual([
      "置顶",
      "归档",
    ]);

    await fireEvent.click(buttons[0]);

    expect(mockInvoke.mock.calls.at(-1)?.slice(0, 2)).toEqual([
      "task_toggle_pin",
      { id: "t-001" },
    ]);

    await waitFor(() => {
      expect(view.getByLabelText("取消置顶")).toHaveClass("is-pinned");
    });
  });

  it("归档失败时发出错误事件而不是 archived", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("archive failed"));
    const view = await renderProjectTreeItem();

    await fireEvent.click(view.getByLabelText("更多"));
    await fireEvent.click(await view.findByText("归档所有对话"));
    await fireEvent.click(await view.findByText("确认归档？再点一次"));

    await waitFor(() => {
      expect(view.emitted("error")?.[0]?.[0]).toContain("归档所有对话失败");
    });
    expect(view.emitted("archived")).toBeUndefined();
  });

  it("归档当前打开的项目对话成功后才发出 archived", async () => {
    const view = await renderProjectTreeItem("/projects/lilia/tasks/t-001");

    await fireEvent.click(view.getByLabelText("更多"));
    await fireEvent.click(await view.findByText("归档所有对话"));
    await fireEvent.click(await view.findByText("确认归档？再点一次"));

    await waitFor(() => {
      expect(view.emitted("archived")).toHaveLength(1);
    });
    expect(view.emitted("error")).toBeUndefined();
  });
});
