import { describe, expect, it } from "vitest";
import {
  buildCodexCollaborationMode,
  buildCodexPlanExecutionPrompt,
  buildCodexPlanRevisionPrompt,
  readCodexPlanModePreset,
} from "../agent-runner/codex/runCodex.mjs";

describe("Codex plan helpers", () => {
  it("reads plan collaboration preset and builds fallback settings", async () => {
    const server = {
      request: async () => ({
        data: [
          { name: "chat", mode: "default", reasoning_effort: null },
          { name: "plan", mode: "plan", reasoning_effort: "high" },
        ],
      }),
    };

    await expect(readCodexPlanModePreset(server as any)).resolves.toMatchObject({
      mode: "plan",
      reasoning_effort: "high",
    });
    expect(buildCodexCollaborationMode("plan", "gpt-5.1", { reasoning_effort: "high" })).toEqual({
      mode: "plan",
      settings: {
        model: "gpt-5.1",
        reasoning_effort: "high",
        developer_instructions: null,
      },
    });
    expect(buildCodexCollaborationMode("plan", null, null)).toEqual({
      mode: "plan",
      settings: {
        model: "gpt-5",
        reasoning_effort: "medium",
        developer_instructions: null,
      },
    });
  });

  it("fails when plan collaboration preset is unavailable", async () => {
    await expect(readCodexPlanModePreset({
      request: async () => ({ data: [] }),
    } as any)).rejects.toThrow("plan collaboration preset is missing");
    await expect(readCodexPlanModePreset({
      request: async () => {
        throw new Error("unsupported");
      },
    } as any)).rejects.toThrow("collaborationMode/list failed");
  });

  it("builds revision and execution prompts", () => {
    expect(buildCodexPlanRevisionPrompt("先补充回滚方案")).toContain("先补充回滚方案");
    expect(buildCodexPlanRevisionPrompt("先补充回滚方案")).toContain("不要执行文件修改或命令");

    const executionPrompt = buildCodexPlanExecutionPrompt("- [ ] 改代码");
    expect(executionPrompt).toContain("用户已确认上一版计划");
    expect(executionPrompt).toContain("- [ ] 改代码");
  });
});

