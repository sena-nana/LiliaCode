import { waitFor } from "@testing-library/vue";
import { describe, expect, it } from "vitest";
import { listProjectConversations } from "../src/data/tasks";
import { mockInvoke } from "./tauriMock";

describe("tasks:changed sync", () => {
  it("收到 task 变更事件后刷新对应项目对话缓存", async () => {
    expect(
      listProjectConversations("lilia").some((task) => task.title === "弹窗首条消息"),
    ).toBe(false);

    await mockInvoke("task_create", {
      projectId: "lilia",
      title: "弹窗首条消息",
      status: "running",
      parentId: null,
      dependsOn: [],
    });

    await waitFor(() => {
      expect(
        listProjectConversations("lilia").some((task) => task.title === "弹窗首条消息"),
      ).toBe(true);
    });
  });
});
