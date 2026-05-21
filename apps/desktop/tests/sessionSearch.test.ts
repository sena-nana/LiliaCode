import { describe, expect, it } from "vitest";
import { searchSessions } from "../src/services/sessionSearch";

describe("sessionSearch", () => {
  it("text 模式按子串命中标题，返回高亮区间", () => {
    const res = searchSessions("Claude", "text");
    // stub 数据里至少有「接入 Claude Code 会话发现」和「随手问问 Claude：tsconfig paths」。
    expect(res.length).toBeGreaterThanOrEqual(2);
    for (const r of res) {
      expect(r.title.toLowerCase()).toContain("claude");
      expect(r.highlights.length).toBeGreaterThan(0);
    }
  });

  it("text 模式查询为空返回空数组", () => {
    expect(searchSessions("", "text")).toEqual([]);
    expect(searchSessions("   ", "text")).toEqual([]);
  });

  it("vector 模式按相似度排序、分值 0..1", () => {
    const res = searchSessions("tsconfig", "vector");
    expect(res.length).toBeGreaterThan(0);
    for (const r of res) {
      expect(r.score).toBeGreaterThan(0);
      expect(r.score).toBeLessThanOrEqual(1);
    }
    // 排序应当从高到低
    const scores = res.map((r) => r.score);
    const sorted = [...scores].sort((a, b) => b - a);
    expect(scores).toEqual(sorted);
  });

  it("project-task 走 /projects 路由，orphan 走 /chats 路由", () => {
    const res = searchSessions("Claude", "text");
    const task = res.find((r) => r.kind === "project-task");
    const orphan = res.find((r) => r.kind === "orphan");
    expect(task?.route.startsWith("/projects/")).toBe(true);
    expect(orphan?.route.startsWith("/chats/")).toBe(true);
  });
});
