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
const timelineListeners: Array<(event: AgentTimelineEvent) => void> = [];
const timelineBatchListeners: Array<(event: { taskId: string; events: AgentTimelineEvent[] }) => void> = [];

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
  onAgentTimeline: vi.fn(async (listener: (event: AgentTimelineEvent) => void) => {
    timelineListeners.push(listener);
    return () => {
      const index = timelineListeners.indexOf(listener);
      if (index >= 0) timelineListeners.splice(index, 1);
    };
  }),
  onAgentTimelineBatch: vi.fn(async (listener: (event: {
    taskId: string;
    events: AgentTimelineEvent[];
  }) => void) => {
    timelineBatchListeners.push(listener);
    return () => {
      const index = timelineBatchListeners.indexOf(listener);
      if (index >= 0) timelineBatchListeners.splice(index, 1);
    };
  }),
  onAgentTimelineEvents: vi.fn(async (listener: (
    events: AgentTimelineEvent[],
    source: "single" | "batch",
  ) => void) => {
    const singleListener = (event: AgentTimelineEvent) => listener([event], "single");
    const batchListener = (event: { taskId: string; events: AgentTimelineEvent[] }) =>
      listener(event.events, "batch");
    timelineListeners.push(singleListener);
    timelineBatchListeners.push(batchListener);
    return () => {
      const singleIndex = timelineListeners.indexOf(singleListener);
      if (singleIndex >= 0) timelineListeners.splice(singleIndex, 1);
      const batchIndex = timelineBatchListeners.indexOf(batchListener);
      if (batchIndex >= 0) timelineBatchListeners.splice(batchIndex, 1);
    };
  }),
  onDone: vi.fn(async () => () => {}),
  onTurnStarted: vi.fn(async () => () => {}),
}));

describe("hydrateConversationActivities", () => {
  beforeEach(() => {
    startOrder.length = 0;
    deferredByTaskId.clear();
    timelineListeners.length = 0;
    timelineBatchListeners.length = 0;
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

  it("不会把缺少真实 requestId 的 action timeline 标成等待交互", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const malformedPlanEvent: AgentTimelineEvent = {
      id: "tl-plan-1",
      taskId: "t-plan",
      turnId: "turn-1",
      backend: "codex",
      kind: "plan",
      status: "requires_action",
      title: "需要确认计划",
      summary: "计划内容",
      payload: { approved: null },
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    };

    const pending = mod.hydrateConversationActivities(["t-plan"], {
      priorityTaskIds: ["t-plan"],
    });
    await Promise.resolve();
    resolveTask("t-plan", { phase: "idle" }, [malformedPlanEvent]);
    await pending;

    expect(mod.conversationActivityForTask("t-plan")).toBeNull();
  });

  it("reset 后会忽略旧 hydration 的晚到结果", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const pending = mod.hydrateConversationActivities(["t-reset-race"], {
      priorityTaskIds: ["t-reset-race"],
    });
    await Promise.resolve();
    await Promise.resolve();

    mod.resetConversationActivity();
    resolveTask("t-reset-race", { phase: "idle" }, [{
      id: "tl-reset-race",
      taskId: "t-reset-race",
      turnId: "turn-1",
      backend: "codex",
      kind: "ask_user",
      status: "requires_action",
      title: "需要确认",
      summary: "继续？",
      payload: { requestId: "ask-reset-race" },
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    }]);

    await pending;
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(mod.conversationActivityForTask("t-reset-race")).toBeNull();
  });

  it("同一 timeline action 失去真实 requestId 时会清掉旧等待状态", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const unlisten = await mod.installConversationActivityBridge();
    const baseEvent = {
      id: "tl-plan-transition",
      taskId: "t-plan-transition",
      turnId: "turn-1",
      backend: "codex",
      kind: "plan",
      status: "requires_action",
      title: "需要确认计划",
      summary: "计划内容",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    } satisfies Omit<AgentTimelineEvent, "payload">;

    try {
      timelineListeners[0]?.({
        ...baseEvent,
        payload: { requestId: "plan-transition-1", approved: null },
      });
      expect(mod.conversationActivityForTask("t-plan-transition")).toBe(
        "requires_action",
      );

      timelineListeners[0]?.({
        ...baseEvent,
        payload: { approved: null },
        updatedAt: Date.now(),
      });

      expect(mod.conversationActivityForTask("t-plan-transition")).toBeNull();
    } finally {
      unlisten();
    }
  });

  it("跨任务相同 timeline event id 不会互相覆盖等待状态记忆", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const unlisten = await mod.installConversationActivityBridge();
    const baseEvent = {
      id: "shared-timeline-event",
      turnId: "turn-1",
      backend: "codex",
      kind: "plan",
      status: "requires_action",
      title: "需要确认计划",
      summary: "计划内容",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    } satisfies Omit<AgentTimelineEvent, "taskId" | "payload">;

    try {
      timelineListeners[0]?.({
        ...baseEvent,
        taskId: "t-shared-1",
        payload: { requestId: "task-1-plan", approved: null },
      });
      timelineListeners[0]?.({
        ...baseEvent,
        taskId: "t-shared-2",
        payload: { requestId: "task-2-plan", approved: null },
      });

      timelineListeners[0]?.({
        ...baseEvent,
        taskId: "t-shared-1",
        status: "success",
        payload: {},
      });

      expect(mod.conversationActivityForTask("t-shared-1")).toBeNull();
      expect(mod.conversationActivityForTask("t-shared-2")).toBe("requires_action");
    } finally {
      unlisten();
    }
  });

  it("同一 timeline action 更换 requestId 时会清掉旧等待状态", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const unlisten = await mod.installConversationActivityBridge();
    const baseEvent = {
      id: "tl-plan-replaced",
      taskId: "t-plan-replaced",
      turnId: "turn-1",
      backend: "codex",
      kind: "plan",
      status: "requires_action",
      title: "需要确认计划",
      summary: "计划内容",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    } satisfies Omit<AgentTimelineEvent, "payload">;

    try {
      timelineListeners[0]?.({
        ...baseEvent,
        payload: { requestId: "plan-replaced-1", approved: null },
      });
      timelineListeners[0]?.({
        ...baseEvent,
        payload: { requestId: "plan-replaced-2", approved: null },
        updatedAt: Date.now(),
      });

      timelineListeners[0]?.({
        ...baseEvent,
        status: "success",
        payload: { requestId: "plan-replaced-2", approved: true },
        updatedAt: Date.now(),
      });

      expect(mod.conversationActivityForTask("t-plan-replaced")).toBeNull();
    } finally {
      unlisten();
    }
  });

  it("批量 timeline action 会同步标记等待交互状态", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const unlisten = await mod.installConversationActivityBridge();

    try {
      timelineBatchListeners[0]?.({
        taskId: "t-batch-pending",
        events: [{
          id: "tl-batch-plan",
          taskId: "t-batch-pending",
          turnId: "turn-1",
          backend: "codex",
          kind: "plan",
          status: "requires_action",
          title: "需要确认计划",
          summary: "计划内容",
          payload: { requestId: "batch-plan-1", approved: null },
          createdAt: Date.now(),
          updatedAt: Date.now(),
          turnSeq: 1,
          intraTurnOrder: 0,
        }],
      });

      expect(mod.conversationActivityForTask("t-batch-pending")).toBe("requires_action");
    } finally {
      unlisten();
    }
  });

  it("批量 timeline action 完成时会清理旧等待交互状态", async () => {
    const mod = await import("../src/composables/useConversationActivity");
    const unlisten = await mod.installConversationActivityBridge();
    const baseEvent = {
      id: "tl-batch-plan-complete",
      taskId: "t-batch-complete",
      turnId: "turn-1",
      backend: "codex",
      kind: "plan",
      title: "需要确认计划",
      summary: "计划内容",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      turnSeq: 1,
      intraTurnOrder: 0,
    } satisfies Omit<AgentTimelineEvent, "payload" | "status">;

    try {
      timelineBatchListeners[0]?.({
        taskId: "t-batch-complete",
        events: [{
          ...baseEvent,
          status: "requires_action",
          payload: { requestId: "batch-plan-complete", approved: null },
        }],
      });
      timelineBatchListeners[0]?.({
        taskId: "t-batch-complete",
        events: [{
          ...baseEvent,
          status: "success",
          payload: { requestId: "batch-plan-complete", approved: true },
          updatedAt: Date.now(),
        }],
      });

      expect(mod.conversationActivityForTask("t-batch-complete")).toBeNull();
    } finally {
      unlisten();
    }
  });
});

