import { afterEach, describe, expect, it } from "vitest";
import {
  ASK_USER_INTERACTION_KIND,
  MCP_ELICITATION_INTERACTION_KIND,
  PERMISSION_APPROVAL_INTERACTION_KIND,
  PLAN_APPROVAL_INTERACTION_KIND,
  TITLE_UPDATE_ACTION_KIND,
  TOOL_CONSENT_INTERACTION_KIND,
} from "@lilia/contracts";
import {
  pendingInteractionRequestIdsForSources,
  pendingInteractionRequestIdsToKeepAfterLoad,
  pendingInteractionHydrationForTimelineEvent,
  hydratePendingInteractions,
  syncPendingInteractionsForTimelineEvents,
  shouldClearConversationRequiresAction,
  clearPendingInteractionsForTask,
} from "../src/pages/taskDetail/usePendingInteractionActions";
import { shouldClearPendingInteraction } from "../src/composables/pendingInteractionClearOptions";
import { usePendingAsksForTask } from "../src/composables/useAskUser";
import { conversationActivityForTask } from "../src/composables/useConversationActivity";
import { timelineEventFixture } from "./timelineTestHelpers";

function timelineEvent(overrides: Parameters<typeof timelineEventFixture>[0]) {
  return timelineEventFixture(overrides, {
    kind: "diagnostic",
    status: "requires_action",
    title: "待处理事件",
    payload: {},
  });
}

describe("pending interaction hydration", () => {
  afterEach(() => {
    clearPendingInteractionsForTask("task-1");
    clearPendingInteractionsForTask("task-2");
  });

  it("derives ask-user hydration from timeline ask events", () => {
    const hydration = pendingInteractionHydrationForTimelineEvent(
      timelineEvent({
        kind: ASK_USER_INTERACTION_KIND,
        payload: {
          requestId: "ask-1",
          spec: {
            title: "确认范围",
            questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
          },
        },
      }),
      "task-1",
    );

    expect(hydration).toMatchObject({
      target: "ask_user",
      activeRequestId: "ask-1",
      interactionKind: ASK_USER_INTERACTION_KIND,
      request: {
        taskId: "task-1",
        turnId: "turn-1",
        requestId: "ask-1",
        spec: { title: "确认范围" },
      },
    });
  });

  it("derives plan approval hydration from actionable plan events", () => {
    const hydration = pendingInteractionHydrationForTimelineEvent(
      timelineEvent({
        kind: "plan",
        title: "确认 Codex 计划",
        payload: { requestId: "plan-1" },
      }),
      "task-1",
    );

    expect(hydration).toMatchObject({
      target: "ask_user",
      activeRequestId: "plan-1",
      interactionKind: PLAN_APPROVAL_INTERACTION_KIND,
      request: {
        requestId: "plan-1",
        spec: {
          title: "确认 Codex 计划",
          intent: "plan_approval",
        },
      },
    });
  });

  it("does not hydrate plan approval without a request id", () => {
    expect(pendingInteractionHydrationForTimelineEvent(
      timelineEvent({
        kind: "plan",
        title: "确认 Codex 计划",
        payload: {},
      }),
      "task-1",
    )).toBeNull();
  });

  it("normalizes tool consent hydration from pending timeline interaction payloads", () => {
    const hydration = pendingInteractionHydrationForTimelineEvent(
      timelineEvent({
        kind: "command",
        payload: {
          interaction: TOOL_CONSENT_INTERACTION_KIND,
          requestId: "tool-1",
          toolName: "Bash",
          input: { command: "pwd" },
        },
      }),
      "task-1",
    );

    expect(hydration).toMatchObject({
      target: "tool_consent",
      activeRequestId: "tool-1",
      request: {
        taskId: "task-1",
        requestId: "tool-1",
        toolName: "Bash",
        input: { command: "pwd" },
      },
    });
  });

  it("normalizes runtime pending interactions from timeline payloads", () => {
    const mcp = pendingInteractionHydrationForTimelineEvent(
      timelineEvent({
        kind: "mcp",
        payload: {
          interaction: MCP_ELICITATION_INTERACTION_KIND,
          requestId: "mcp-1",
          threadId: "thread-1",
          turnId: "turn-1",
          serverName: "linear",
          mode: "form",
          message: "选择项目",
        },
      }),
      "task-1",
    );
    const permission = pendingInteractionHydrationForTimelineEvent(
      timelineEvent({
        payload: {
          interaction: PERMISSION_APPROVAL_INTERACTION_KIND,
          requestId: "permission-1",
          reason: "需要访问网络",
          requestedAccess: { network: true },
        },
      }),
      "task-1",
    );

    expect(mcp).toMatchObject({
      target: "agent_interaction",
      activeRequestId: "mcp-1",
      request: {
        kind: MCP_ELICITATION_INTERACTION_KIND,
        requestId: "mcp-1",
        payload: {
          threadId: "thread-1",
          turnId: "turn-1",
          serverName: "linear",
          mode: "form",
          message: "选择项目",
        },
      },
    });
    expect(permission).toMatchObject({
      target: "agent_interaction",
      activeRequestId: "permission-1",
      request: {
        kind: PERMISSION_APPROVAL_INTERACTION_KIND,
        requestId: "permission-1",
        payload: { reason: "需要访问网络", requestedAccess: { network: true } },
      },
    });
  });

  it("ignores non-actionable or unrelated timeline events", () => {
    expect(pendingInteractionHydrationForTimelineEvent(
      timelineEvent({ status: "success", payload: { requestId: "done-1" } }),
      "task-1",
    )).toBeNull();
    expect(pendingInteractionHydrationForTimelineEvent(
      timelineEvent({ taskId: "other-task", payload: { requestId: "other-1" } }),
      "task-1",
    )).toBeNull();
  });

  it("does not hydrate malformed title-update action events", () => {
    expect(pendingInteractionHydrationForTimelineEvent(
      timelineEvent({
        kind: TITLE_UPDATE_ACTION_KIND,
        payload: {
          requestId: "title-1",
        },
      }),
      "task-1",
    )).toBeNull();
  });

  it("keeps hydrated and newly arrived pending request ids after timeline load", () => {
    const keep = pendingInteractionRequestIdsToKeepAfterLoad({
      hydratedRequestIds: new Set(["ask-1", "tool-1"]),
      currentRequestIds: ["stale-before-load", "new-during-load"],
      pendingBeforeLoadRequestIds: new Set(["stale-before-load"]),
    });

    expect([...keep]).toEqual(["ask-1", "tool-1", "new-during-load"]);
  });

  it("hydrates full timeline events without applying realtime invalidation clears", () => {
    const pendingAsks = usePendingAsksForTask("task-1");
    const activeEvent = timelineEvent({
      id: "plan-load",
      kind: "plan",
      title: "确认 Codex 计划",
      payload: { requestId: "plan-load-1" },
    });

    expect([...hydratePendingInteractions([
      activeEvent,
      timelineEvent({
        id: "plan-load-terminal-copy",
        status: "success",
        payload: { requestId: "plan-load-1", approved: true },
      }),
    ], "task-1")]).toEqual(["plan-load-1"]);

    expect(pendingAsks.value.map((ask) => ask.requestId)).toContain("plan-load-1");
  });

  it("clears live hydrated pending asks when the same timeline event is no longer actionable", () => {
    const pendingAsks = usePendingAsksForTask("task-1");
    const activeEvent = timelineEvent({
      id: "plan-live",
      kind: "plan",
      title: "确认 Codex 计划",
      payload: { requestId: "plan-live-1" },
    });

    expect([...hydratePendingInteractions([activeEvent], "task-1")]).toEqual([
      "plan-live-1",
    ]);
    expect(pendingAsks.value.map((ask) => ask.requestId)).toContain("plan-live-1");

    syncPendingInteractionsForTimelineEvents([
      timelineEvent({
        ...activeEvent,
        status: "success",
        payload: { requestId: "plan-live-1", approved: true },
      }),
    ], "task-1");

    expect(pendingAsks.value.map((ask) => ask.requestId)).not.toContain("plan-live-1");
  });

  it("clears live hydrated pending asks when the same action event loses requestId", () => {
    const pendingAsks = usePendingAsksForTask("task-1");
    const activeEvent = timelineEvent({
      id: "plan-malformed-live",
      kind: "plan",
      title: "确认 Codex 计划",
      payload: { requestId: "plan-malformed-1" },
    });

    syncPendingInteractionsForTimelineEvents([activeEvent], "task-1");
    expect(pendingAsks.value.map((ask) => ask.requestId)).toContain("plan-malformed-1");

    syncPendingInteractionsForTimelineEvents([
      timelineEvent({
        ...activeEvent,
        payload: { approved: null },
      }),
    ], "task-1");

    expect(pendingAsks.value.map((ask) => ask.requestId)).not.toContain("plan-malformed-1");
  });

  it("clears the replaced request when a live timeline event changes requestId", () => {
    const pendingAsks = usePendingAsksForTask("task-1");
    const activeEvent = timelineEvent({
      id: "plan-replaced-live",
      kind: "plan",
      title: "确认 Codex 计划",
      payload: { requestId: "plan-replaced-1" },
    });

    syncPendingInteractionsForTimelineEvents([activeEvent], "task-1");
    expect(pendingAsks.value.map((ask) => ask.requestId)).toContain("plan-replaced-1");

    syncPendingInteractionsForTimelineEvents([
      timelineEvent({
        ...activeEvent,
        payload: { requestId: "plan-replaced-2" },
      }),
    ], "task-1");

    expect(pendingAsks.value.map((ask) => ask.requestId)).not.toContain("plan-replaced-1");
    expect(pendingAsks.value.map((ask) => ask.requestId)).toContain("plan-replaced-2");
  });

  it("keeps remembered live request ids isolated by task and event id", () => {
    const taskOnePendingAsks = usePendingAsksForTask("task-1");
    const taskTwoPendingAsks = usePendingAsksForTask("task-2");
    const taskOneEvent = timelineEvent({
      id: "shared-event-id",
      taskId: "task-1",
      kind: "plan",
      title: "确认 Codex 计划",
      payload: { requestId: "task-1-plan" },
    });
    const taskTwoEvent = timelineEvent({
      ...taskOneEvent,
      taskId: "task-2",
      payload: { requestId: "task-2-plan" },
    });

    syncPendingInteractionsForTimelineEvents([taskOneEvent], "task-1");
    syncPendingInteractionsForTimelineEvents([taskTwoEvent], "task-2");

    syncPendingInteractionsForTimelineEvents([
      timelineEvent({
        ...taskOneEvent,
        status: "success",
        payload: {},
      }),
    ], "task-1");

    expect(taskOnePendingAsks.value.map((ask) => ask.requestId)).not.toContain("task-1-plan");
    expect(taskTwoPendingAsks.value.map((ask) => ask.requestId)).toContain("task-2-plan");
  });

  it("clears replaced tool-consent activity when a single live event changes requestId", () => {
    const activeEvent = timelineEvent({
      id: "tool-replaced-live",
      kind: "command",
      payload: {
        interaction: TOOL_CONSENT_INTERACTION_KIND,
        requestId: "tool-replaced-1",
        toolName: "Bash",
        input: { command: "pwd" },
      },
    });

    syncPendingInteractionsForTimelineEvents([activeEvent], "task-1");
    expect(conversationActivityForTask("task-1")).toBe("requires_action");

    syncPendingInteractionsForTimelineEvents([
      timelineEvent({
        ...activeEvent,
        payload: {
          interaction: TOOL_CONSENT_INTERACTION_KIND,
          requestId: "tool-replaced-2",
          toolName: "Bash",
          input: { command: "ls" },
        },
      }),
    ], "task-1");

    syncPendingInteractionsForTimelineEvents([
      timelineEvent({
        ...activeEvent,
        status: "success",
        payload: { requestId: "tool-replaced-2" },
      }),
    ], "task-1");

    expect(conversationActivityForTask("task-1")).toBeNull();
  });

  it("collects request ids from all pending interaction sources", () => {
    const ids = pendingInteractionRequestIdsForSources({
      asks: [{ requestId: "ask-1" }, { requestId: null }],
      toolConsents: [{ requestId: "tool-1" }],
      agentInteractions: [{ requestId: "mcp-1" }, { requestId: "tool-1" }],
      architectureChanges: [{ requestId: "architecture-1" }],
    });

    expect([...ids]).toEqual(["ask-1", "tool-1", "mcp-1", "architecture-1"]);
  });

  it("only clears the task-level requires-action state for full clears", () => {
    expect(shouldClearConversationRequiresAction({})).toBe(true);
    expect(shouldClearConversationRequiresAction({ turnId: "turn-1" })).toBe(false);
    expect(shouldClearConversationRequiresAction({ turnId: null })).toBe(false);
    expect(shouldClearConversationRequiresAction({ requestId: "ask-1" })).toBe(false);
    expect(shouldClearConversationRequiresAction({
      keepRequestIds: new Set(["ask-1"]),
    })).toBe(false);
  });

  it("matches pending clear candidates by task, turn, and retained request ids", () => {
    const candidate = {
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
    };

    expect(shouldClearPendingInteraction(candidate, "task-1")).toBe(true);
    expect(shouldClearPendingInteraction(candidate, "other-task")).toBe(false);
    expect(shouldClearPendingInteraction(candidate, "task-1", { turnId: "turn-2" })).toBe(false);
    expect(shouldClearPendingInteraction(candidate, "task-1", { requestId: "ask-2" })).toBe(false);
    expect(shouldClearPendingInteraction(candidate, "task-1", { requestId: "ask-1" })).toBe(true);
    expect(shouldClearPendingInteraction(candidate, "task-1", {
      keepRequestIds: new Set(["ask-1"]),
    })).toBe(false);
  });
});

