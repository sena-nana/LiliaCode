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
