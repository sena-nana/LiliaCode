import { describe, expect, it } from "vitest";
import { getCurrentWindow, invoke, listen } from "../src/tauri/devMock";

describe("dev Tauri mock", () => {
  it("provides the minimum browser-preview shell data", async () => {
    await expect(invoke("project_list")).resolves.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: "lilia" })]),
    );
    await expect(invoke("project_dashboard_list")).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: "lilia",
          statusCounts: expect.objectContaining({ running: 1 }),
          totalTokens: expect.any(Number),
        }),
      ]),
    );
    await expect(invoke("task_list", { projectId: null })).resolves.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: "o-001" })]),
    );
    await expect(listen("mock", () => undefined)).resolves.toEqual(expect.any(Function));
    expect(getCurrentWindow().label).toBe("main");
  });
});
