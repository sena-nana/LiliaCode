import { describe, it, expect } from "vitest";
import { listProjects, getProject } from "../src/data/projects";
import { listTasks, getTask } from "../src/data/tasks";

describe("projects (data layer)", () => {
  it("列出所有项目", () => {
    expect(listProjects().length).toBeGreaterThan(0);
  });

  it("通过 id 找到项目", () => {
    expect(getProject("lilia")?.name).toBe("Lilia");
    expect(getProject("missing")).toBeUndefined();
  });

  it("通过 projectId 列出任务", () => {
    expect(listTasks("lilia").length).toBeGreaterThan(0);
    expect(listTasks("missing")).toEqual([]);
  });

  it("通过 (projectId, taskId) 取出任务", () => {
    const t = getTask("lilia", "t-002");
    expect(t?.dependsOn).toContain("t-001");
  });
});
