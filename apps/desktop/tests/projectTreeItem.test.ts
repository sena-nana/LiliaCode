import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { defineComponent, nextTick } from "vue";
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
      treeActivityToken: {
        type: Number,
        default: 0,
      },
    },
    template: `
      <ProjectTreeItem
        :project="project"
        :is-expanded="isExpanded"
        :tree-activity-token="treeActivityToken"
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
      treeActivityToken: 0,
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

afterEach(() => {
  vi.useRealTimers();
});

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

  it("展开剩余对话后 30 秒无树内活动会恢复折叠", async () => {
    seedProjectConversations(5);
    const view = await renderProjectTreeItem();
    vi.useFakeTimers();

    await fireEvent.click(view.getByRole("button", { name: "显示剩余对话" }));
    expect(view.getByText("对话 5")).toBeInTheDocument();

    await vi.advanceTimersByTimeAsync(29_999);
    expect(view.getByText("对话 5")).toBeInTheDocument();

    await vi.advanceTimersByTimeAsync(1);
    await nextTick();
    expect(view.queryByText("对话 5")).not.toBeInTheDocument();
    expect(view.getByRole("button", { name: "显示剩余对话" })).toBeInTheDocument();
  });

  it("树内活动会刷新展开剩余对话后的自动折叠计时", async () => {
    seedProjectConversations(5);
    const view = await renderProjectTreeItem();
    vi.useFakeTimers();

    await fireEvent.click(view.getByRole("button", { name: "显示剩余对话" }));
    await vi.advanceTimersByTimeAsync(20_000);
    await view.rerender({ isExpanded: true, treeActivityToken: 1 });

    await vi.advanceTimersByTimeAsync(20_000);
    expect(view.getByText("对话 5")).toBeInTheDocument();

    await vi.advanceTimersByTimeAsync(10_000);
    await nextTick();
    expect(view.queryByText("对话 5")).not.toBeInTheDocument();
  });

  it("折叠项目后会重置项目内对话折叠状态", async () => {
    seedProjectConversations(5);
    const view = await renderProjectTreeItem();

    await fireEvent.click(view.getByRole("button", { name: "显示剩余对话" }));
    expect(view.getByText("对话 5")).toBeInTheDocument();

    await view.rerender({ isExpanded: false, treeActivityToken: 0 });
    await view.rerender({ isExpanded: true, treeActivityToken: 0 });

    expect(view.queryByText("对话 5")).not.toBeInTheDocument();
    expect(view.getByRole("button", { name: "显示剩余对话" })).toBeInTheDocument();
  });

  it("当前打开的隐藏对话会在折叠态占用第四个可见位置", async () => {
    seedProjectConversations(6);

    const view = await renderProjectTreeItem("/projects/lilia/tasks/t-overflow-6");

    expect(view.getByText("对话 1")).toBeInTheDocument();
    expect(view.getByText("对话 2")).toBeInTheDocument();
    expect(view.getByText("对话 3")).toBeInTheDocument();
    expect(view.queryByText("对话 4")).not.toBeInTheDocument();
    expect(view.queryByText("对话 5")).not.toBeInTheDocument();
    expect(view.getByText("对话 6")).toBeInTheDocument();
    expect(view.getByRole("button", { name: "显示剩余对话" })).toBeInTheDocument();
  });
});
