import { describe, expect, it } from "vitest";
import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  hasTimelinePendingActionState,
  timelinePendingActionState,
} from "../src/components/chat/timelinePendingActions";
import type { PendingAgentAction } from "../src/composables/pendingAgentActions";

function timelineEvent(
  overrides: Partial<AgentTimelineEvent>,
): AgentTimelineEvent {
  return {
    id: "event-1",
    taskId: "task-1",
    turnId: "turn-1",
    backend: "codex",
    kind: "plan",
    status: "requires_action",
    title: "Codex plan",
    summary: "改代码",
    payload: { backend: "codex", approved: null },
    createdAt: 1,
    updatedAt: 1,
    turnSeq: 0,
    intraTurnOrder: 0,
    ...overrides,
  };
}

function planApprovalAction(): PendingAgentAction {
  return {
    kind: "plan_approval",
    taskId: "task-1",
    turnId: "turn-1",
    requestId: "ask-1",
    ask: {
      id: 1,
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
      spec: {
        title: "确认 Codex 计划",
        intent: "plan_approval",
        questions: [{ id: "approve-plan", question: "", mode: "confirm" }],
      },
      resolve: () => {},
    },
  };
}

describe("timeline pending action state", () => {
  it("returns the matching live pending action without marking it expired", () => {
    const action = planApprovalAction();
    const state = timelinePendingActionState(
      timelineEvent({ status: "requires_action" }),
      [action],
      true,
    );

    expect(state).toEqual({ action, expired: false });
    expect(hasTimelinePendingActionState(state)).toBe(true);
  });

  it("marks an actionable timeline event as expired only when expired display is enabled", () => {
    const event = timelineEvent({
      status: "requires_action",
      payload: { backend: "codex", approved: null, requestId: "ask-1" },
    });

    expect(timelinePendingActionState(event, [], true)).toEqual({
      action: null,
      expired: true,
    });
    expect(hasTimelinePendingActionState(timelinePendingActionState(event, [], true))).toBe(true);
    expect(timelinePendingActionState(event, [], false)).toEqual({
      action: null,
      expired: false,
    });
    expect(hasTimelinePendingActionState(timelinePendingActionState(event, [], false))).toBe(false);
  });

  it("does not mark malformed action events without requestId as expired", () => {
    const event = timelineEvent({
      status: "requires_action",
      payload: { backend: "codex", approved: null },
    });

    const state = timelinePendingActionState(event, [], true);

    expect(state).toEqual({ action: null, expired: false });
    expect(hasTimelinePendingActionState(state)).toBe(false);
  });

  it("does not create state for completed timeline events", () => {
    const state = timelinePendingActionState(
      timelineEvent({
        status: "success",
        payload: { backend: "codex", approved: true },
      }),
      [],
      true,
    );

    expect(state).toEqual({ action: null, expired: false });
    expect(hasTimelinePendingActionState(state)).toBe(false);
  });
});
