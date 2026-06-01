import { describe, expect, it } from "vitest";
import { searchSessions } from "../src/services/sessionSearch";
import { allTasksReady } from "../src/data/tasks";
import { projectsReady } from "../src/data/projects";

describe("sessionSearch", () => {

  it("合并子串与相似度结果并按分值从高到低排序", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const res = searchSessions("tsconfig");
    expect(res.length).toBeGreaterThan(0);
    for (const r of res) {
      expect(r.score).toBeGreaterThan(0);
      expect(r.score).toBeLessThanOrEqual(1);
    }

    const routes = res.map((r) => r.route);
    expect(new Set(routes).size).toBe(routes.length);

    const scores = res.map((r) => r.score);
    const sorted = [...scores].sort((a, b) => b - a);
    expect(scores).toEqual(sorted);
  });

  it("project-task 走 /projects 路由，orphan 走 /chats 路由", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const res = searchSessions("Claude");
    const task = res.find((r) => r.kind === "project-task");
    const orphan = res.find((r) => r.kind === "orphan");
    expect(task?.route.startsWith("/projects/")).toBe(true);
    expect(orphan?.route.startsWith("/chats/")).toBe(true);
  });
});
