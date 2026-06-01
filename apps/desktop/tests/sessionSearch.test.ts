import { describe, expect, it } from "vitest";
import { searchSessions } from "../src/services/sessionSearch";
import { allTasksReady } from "../src/data/tasks";
import { projectsReady } from "../src/data/projects";

describe("sessionSearch", () => {
  it("按子串命中标题时返回高亮区间", async () => {
    await Promise.all([projectsReady, allTasksReady]);
    const res = searchSessions("Claude");
    // IPC fixtures 里至少有「接入 Claude Code 会话发现」和「随手问问 Claude：tsconfig paths」。
    expect(res.length).toBeGreaterThanOrEqual(2);
    for (const r of res.filter((result) =>
      result.title.toLowerCase().includes("claude")
    )) {
      expect(r.title.toLowerCase()).toContain("claude");
      expect(r.highlights.length).toBeGreaterThan(0);
    }
  });

  it("查询为空返回空数组", () => {
    expect(searchSessions("")).toEqual([]);
    expect(searchSessions("   ")).toEqual([]);
  });

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
