import type { AgentTimelineEvent } from "@lilia/contracts";

export function timelineEventFixture(
  overrides: Partial<AgentTimelineEvent>,
  defaults: Partial<AgentTimelineEvent> = {},
): AgentTimelineEvent {
  return {
    id: "event-1",
    taskId: "task-1",
    turnId: "turn-1",
    backend: "codex",
    kind: "command",
    status: "success",
    title: "事件",
    summary: null,
    payload: null,
    createdAt: 1,
    updatedAt: 1,
    turnSeq: 1,
    intraTurnOrder: 1,
    ...defaults,
    ...overrides,
  };
}

export function pendingPlanTimelineEvent(overrides: Partial<AgentTimelineEvent>) {
  return timelineEventFixture(overrides, {
    kind: "plan",
    status: "requires_action",
    title: "Codex plan",
    summary: "改代码",
    payload: { backend: "codex", approved: null },
    turnSeq: 0,
    intraTurnOrder: 0,
  });
}

