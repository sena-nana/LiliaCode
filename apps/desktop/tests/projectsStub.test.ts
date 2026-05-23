import { describe, it, expect } from "vitest";
import { listProjects, getProject, projectsReady, removeProject } from "../src/data/projects";
import {
  allTasksReady,
  getTask,
  listOrphanConversations,
  listTasks,
  toggleTaskPin,
} from "../src/data/tasks";

describe("projects (IPC data layer)", () => {
  it("通过 mocked Tauri IPC 初始化项目缓存", async () => {
    await projectsReady;
    expect(listProjects().length).toBeGreaterThan(0);
  });

  it("通过 id 找到项目", async () => {
    await projectsReady;
    expect(getProject("lilia")?.name).toBe("Lilia");
    expect(getProject("missing")).toBeUndefined();
  });

  it("通过 projectId 列出任务", async () => {
    await allTasksReady;
    expect(listTasks("lilia").length).toBeGreaterThan(0);
    expect(listTasks("missing")).toEqual([]);
  });

  it("任务缓存按后端返回的显示顺序排列", async () => {
    await allTasksReady;
    expect(listTasks("lilia").map((t) => t.id)).toEqual(["t-001", "t-002"]);
  });

  it("置顶 session 后在项目列表内优先显示", async () => {
    await allTasksReady;

    const pinned = await toggleTaskPin("t-002");

    expect(pinned).toBe(true);
    expect(listTasks("lilia").map((t) => [t.id, t.pinned])).toEqual([
      ["t-002", true],
      ["t-001", false],
    ]);
  });

  it("通过 (projectId, taskId) 取出任务", async () => {
    await allTasksReady;
    const t = getTask("lilia", "t-002");
    expect(t?.dependsOn).toContain("t-001");
  });

  it("移除项目后把原项目任务刷新到收集箱缓存", async () => {
    await allTasksReady;
    expect(listTasks("tools").map((t) => t.id)).toEqual(["t-003"]);

    await removeProject("tools");

    expect(getProject("tools")).toBeUndefined();
    expect(listTasks("tools")).toEqual([]);
    expect(listOrphanConversations().map((o) => o.id)).toContain("t-003");
  });
});
