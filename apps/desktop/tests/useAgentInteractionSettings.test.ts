import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockInvoke } from "./tauriMock";

async function loadStoreModule() {
  return import("../src/composables/useAgentInteractionSettings");
}

describe("useAgentInteractionSettings", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("loads normalized subagent defaults from persisted settings", async () => {
    const { useAgentInteractionSettings } = await loadStoreModule();
    const store = useAgentInteractionSettings();

    await store.load();

    expect(store.settings.value.subagentMode).toEqual({
      enabled: false,
      codex: { enabled: true },
      claude: {
        enabled: true,
        forwardSubagentText: true,
        agentProgressSummaries: true,
      },
    });
    expect(store.settings.value.permissionMode).toBe("ask");
    expect(store.settings.value.mainAgentPromptMode).toBe("conservative");
    expect(store.settings.value.autoTurnDecision).toEqual({
      enabled: true,
      allowModelTier: true,
      allowReasoningEffort: true,
      allowPlanMode: true,
      allowGoalMode: true,
      allowSessionFork: true,
    });
  });

  it("saves supported permission modes", async () => {
    const { useAgentInteractionSettings } = await loadStoreModule();
    const store = useAgentInteractionSettings();
    await store.load();

    await store.update({ permissionMode: "readonly" });
    expect(store.settings.value.permissionMode).toBe("readonly");

    await store.update({ permissionMode: "full" });
    expect(store.settings.value.permissionMode).toBe("full");

    await store.update({ permissionMode: "free" });
    expect(store.settings.value.permissionMode).toBe("free");
  });

  it("saves supported main agent prompt modes", async () => {
    const { useAgentInteractionSettings } = await loadStoreModule();
    const store = useAgentInteractionSettings();
    await store.load();

    await store.update({ mainAgentPromptMode: "aggressive" });
    expect(store.settings.value.mainAgentPromptMode).toBe("aggressive");

    await store.update({ mainAgentPromptMode: "conservative" });
    expect(store.settings.value.mainAgentPromptMode).toBe("conservative");
  });

  it("rolls back subagent settings when saving fails", async () => {
    const { useAgentInteractionSettings } = await loadStoreModule();
    const store = useAgentInteractionSettings();
    await store.load();

    const previous = JSON.parse(JSON.stringify(store.settings.value.subagentMode));
    const invokeImpl = mockInvoke.getMockImplementation();
    mockInvoke.mockImplementationOnce(async (cmd: string, args: Record<string, unknown> = {}) => {
      if (cmd === "agent_interaction_set_settings") throw new Error("save failed");
      return invokeImpl?.(cmd, args);
    });

    await expect(store.update({
      subagentMode: {
        enabled: true,
        codex: { enabled: false },
        claude: {
          enabled: true,
          forwardSubagentText: false,
          agentProgressSummaries: false,
        },
      },
    })).rejects.toThrow("save failed");

    expect(store.settings.value.subagentMode).toEqual(previous);
  });

  it("normalizes and saves auto turn decision settings", async () => {
    const { useAgentInteractionSettings } = await loadStoreModule();
    const store = useAgentInteractionSettings();
    await store.load();

    await store.update({
      autoTurnDecision: {
        enabled: true,
        allowModelTier: false,
        allowReasoningEffort: true,
        allowPlanMode: false,
        allowGoalMode: true,
        allowSessionFork: false,
      },
    });

    expect(store.settings.value.autoTurnDecision).toEqual({
      enabled: true,
      allowModelTier: false,
      allowReasoningEffort: true,
      allowPlanMode: false,
      allowGoalMode: true,
      allowSessionFork: false,
    });
  });

  it("loads, saves, updates, and deletes custom subagents through the shared store", async () => {
    const { useAgentInteractionSettings } = await loadStoreModule();
    const store = useAgentInteractionSettings();

    await store.loadSubagents();
    expect(store.subagents.value.map((item) => item.name)).toEqual(["Reviewer"]);

    const builder = await store.saveSubagent({
      id: null,
      name: "Builder",
      description: "处理实现细节",
      instruction: "Implement the requested code changes.",
      enabled: true,
    });

    expect(builder.name).toBe("Builder");
    expect(store.subagents.value.map((item) => item.name)).toEqual(["Builder", "Reviewer"]);

    await store.saveSubagent({
      id: builder.id,
      name: "Builder",
      description: "处理实现与重构",
      instruction: "Implement and refactor the requested code changes.",
      enabled: false,
    });

    expect(store.subagents.value.find((item) => item.id === builder.id)).toMatchObject({
      name: "Builder",
      description: "处理实现与重构",
      instruction: "Implement and refactor the requested code changes.",
      enabled: false,
    });

    await store.deleteSubagent(builder.id);
    expect(store.subagents.value.map((item) => item.name)).toEqual(["Reviewer"]);
  });
});
