import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentTimelineEvent } from "@lilia/contracts";

type DeferredEntry = {
  snapshotResolve: (value: { phase: string }) => void;
  timelineResolve: (value: AgentTimelineEvent[]) => void;
  snapshotPromise: Promise<{ phase: string }>;
  timelinePromise: Promise<AgentTimelineEvent[]>;
};

const startOrder: string[] = [];
const deferredByTaskId = new Map<string, DeferredEntry>();

function ensureDeferred(taskId: string): DeferredEntry {
  const existing = deferredByTaskId.get(taskId);
  if (existing) return existing;
  let snapshotResolve!: (value: { phase: string }) => void;
  let timelineResolve!: (value: AgentTimelineEvent[]) => void;
  const entry: DeferredEntry = {
    snapshotResolve,
    timelineResolve,
    snapshotPromise: new Promise((resolve) => {
      snapshotResolve = resolve;
    }),
    timelinePromise: new Promise((resolve) => {
      timelineResolve = resolve;
    }),
  };
  entry.snapshotResolve = snapshotResolve;
  entry.timelineResolve = timelineResolve;
  deferredByTaskId.set(taskId, entry);
  return entry;
}

function resolveTask(
  taskId: string,
  snapshot: { phase: string } = { phase: "idle" },
  timeline: AgentTimelineEvent[] = [],
) {
  const entry = ensureDeferred(taskId);
  entry.snapshotResolve(snapshot);
  entry.timelineResolve(timeline);
}

vi.mock("../src/services/chat", () => ({
  getRuntimeSnapshot: vi.fn((taskId: string) => {
    startOrder.push(taskId);
    return ensureDeferred(taskId).snapshotPromise;
  }),
  listAgentTimeline: vi.fn((taskId: string) => ensureDeferred(taskId).timelinePromise),
  onAgentTimeline: vi.fn(async () => () => {}),
  onDone: vi.fn(async () => () => {}),
  onTurnStarted: vi.fn(async () => () => {}),
}));

describe("hydrateConversationActivities", () => {
  beforeEach(() => {
    startOrder.length = 0;
    deferredByTaskId.clear();
    vi.clearAllMocks();
  });

  afterEach(async () => {
    const mod = await import("../src/composables/useConversationActivity");
    mod.resetConversationActivity();
  });

  it("按优先级入队，并将并发限制在 3 个任务", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const pending = mod.hydrateConversationActivities(
      ["t-3", "t-4", "t-1", "t-2", "t-5"],
      { priorityTaskIds: ["t-1", "t-2"] },
    );

    await Promise.resolve();
    await Promise.resolve();

    expect(startOrder).toEqual(["t-1", "t-2", "t-3"]);

    resolveTask("t-1");
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(startOrder).toEqual(["t-1", "t-2", "t-3", "t-4"]);

    resolveTask("t-2");
    resolveTask("t-3");
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(startOrder).toEqual(["t-1", "t-2", "t-3", "t-4", "t-5"]);

    resolveTask("t-4");
    resolveTask("t-5");
    await pending;
  });

  it("会去重已 hydrate 的任务，并保留等待交互状态", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const askEvent: AgentTimelineEvent = {
      id: "tl-ask-1",
      taskId: "t-ask",
      turnId: "turn-1",
      backend: "codex",
      kind: "ask_user",
      status: "requires_action",
      title: "需要确认",
      summary: "继续？",
      payload: { requestId: "ask-1" },
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    };

    const pending = mod.hydrateConversationActivities(["t-ask"], {
      priorityTaskIds: ["t-ask"],
    });
    await Promise.resolve();
    resolveTask("t-ask", { phase: "idle" }, [askEvent]);
    await pending;

    expect(mod.conversationActivityForTask("t-ask")).toBe("requires_action");
    const callCount = startOrder.length;

    await mod.hydrateConversationActivities(["t-ask"], {
      priorityTaskIds: ["t-ask"],
    });

    expect(startOrder).toHaveLength(callCount);
  });
});
