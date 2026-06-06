import { describe, expect, it } from "vitest";
import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  pendingActionForTimelineEvent,
  timelineEventRequiresAgentAction,
  type PendingAgentAction,
} from "../src/composables/usePendingAgentActions";

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

describe("pending agent actions", () => {
  it("only attaches plan approval controls to actionable plan events", () => {
    const action: PendingAgentAction = {
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

    expect(pendingActionForTimelineEvent(
      timelineEvent({ status: "requires_action" }),
      [action],
    )).toBe(action);
    expect(pendingActionForTimelineEvent(
      timelineEvent({ status: "success", payload: { backend: "codex", approved: true } }),
      [action],
    )).toBeNull();
    expect(pendingActionForTimelineEvent(
      timelineEvent({ status: "cancelled", payload: { backend: "codex", approved: false } }),
      [action],
    )).toBeNull();
  });

  it("does not mark completed tool consent timeline events as pending", () => {
    const action: PendingAgentAction = {
      kind: "tool_consent",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "consent-1",
      request: {
        taskId: "task-1",
        turnId: "turn-1",
        backend: "claude",
        requestId: "consent-1",
        toolName: "Bash",
        input: { command: "pwd" },
        title: null,
        displayName: null,
        description: null,
        blockedPath: null,
        decisionReason: null,
        toolUseId: null,
      },
    };
    const completed = timelineEvent({
      kind: "command",
      status: "success",
      payload: { interaction: "tool_consent", requestId: "consent-1" },
    });

    expect(timelineEventRequiresAgentAction(completed)).toBe(false);
    expect(pendingActionForTimelineEvent(completed, [action])).toBeNull();
  });
});
