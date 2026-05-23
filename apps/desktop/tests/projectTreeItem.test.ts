import { render, fireEvent, waitFor } from "@testing-library/vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it } from "vitest";
import { defineComponent } from "vue";
import ProjectTreeItem from "../src/components/sidebar/ProjectTreeItem.vue";
import ContextMenuHost from "../src/components/ContextMenuHost.vue";
import { mockInvoke } from "./tauriMock";

async function renderProjectTreeItem() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: { template: "<div />" } },
      { path: "/projects/:projectId", component: { template: "<div />" } },
      {
        path: "/projects/:projectId/tasks/:taskId",
        component: { template: "<div />" },
      },
    ],
  });
  await router.push("/projects/lilia");
  await router.isReady();

  const Wrapper = defineComponent({
    components: { ProjectTreeItem, ContextMenuHost },
    template: `
      <ProjectTreeItem
        :project="project"
        :is-expanded="true"
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

  return render(Wrapper, {
    global: {
      plugins: [router],
      directives: {
        contextMenu: {},
      },
    },
  });
}

describe("ProjectTreeItem", () => {
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

  it("归档成功后才发出 archived", async () => {
    const view = await renderProjectTreeItem();

    await fireEvent.click(view.getByLabelText("更多"));
    await fireEvent.click(await view.findByText("归档所有对话"));
    await fireEvent.click(await view.findByText("确认归档？再点一次"));

    await waitFor(() => {
      expect(view.emitted("archived")).toHaveLength(1);
    });
    expect(view.emitted("error")).toBeUndefined();
  });
});
