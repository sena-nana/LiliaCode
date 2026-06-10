import { describe, expect, it } from "vitest";
import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  pendingActionForTimelineEvent,
  timelineEventRequiresAgentAction,
  usePendingAgentActionsForTask,
  type PendingAgentAction,
} from "../src/composables/usePendingAgentActions";
import { computed, ref } from "vue";

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

  it("derives nonblocking title update actions from actionable timeline events", () => {
    const event = timelineEvent({
      id: "title-update:task-1:req-1",
      kind: "title_update",
      status: "requires_action",
      title: "标题已更新",
      summary: "标题事件化",
      payload: {
        requestId: "req-1",
        proposedTitle: "标题事件化",
        previousTitle: "旧标题",
      },
    });
    const actions = usePendingAgentActionsForTask(
      computed(() => []),
      computed(() => []),
      computed(() => []),
      ref([event]),
    );

    expect(actions.value).toHaveLength(1);
    expect(actions.value[0]).toMatchObject({
      kind: "title_update",
      requestId: "req-1",
      proposedTitle: "标题事件化",
      previousTitle: "旧标题",
    });
    expect(timelineEventRequiresAgentAction(event)).toBe(true);
    expect(pendingActionForTimelineEvent(event, actions.value)).toBe(actions.value[0]);
  });

  it("attaches Codex MCP and permission actions to matching timeline events", () => {
    const actions = usePendingAgentActionsForTask(
      computed(() => []),
      computed(() => []),
      ref([
        {
          kind: "mcp_elicitation",
          taskId: "task-1",
          turnId: "turn-1",
          requestId: "mcp-1",
          payload: {
            threadId: "thread-1",
            turnId: "turn-1",
            serverName: "linear",
            mode: "form",
            message: "选择项目",
          },
        },
        {
          kind: "permission_approval",
          taskId: "task-1",
          turnId: "turn-1",
          requestId: "permission-1",
          payload: {
            threadId: "thread-1",
            turnId: "turn-1",
            itemId: "item-1",
            startedAtMs: 1,
            cwd: "C:/repo",
            reason: "need network",
            permissions: { network: { domains: [{ domain: "example.com" }] } },
          },
        },
      ]),
      ref([]),
    );
    const mcpEvent = timelineEvent({
      kind: "mcp",
      status: "requires_action",
      payload: { interaction: "mcp_elicitation", requestId: "mcp-1" },
    });
    const permissionEvent = timelineEvent({
      kind: "diagnostic",
      status: "requires_action",
      payload: { interaction: "permission_approval", requestId: "permission-1" },
    });

    expect(actions.value).toHaveLength(2);
    expect(timelineEventRequiresAgentAction(mcpEvent)).toBe(true);
    expect(timelineEventRequiresAgentAction(permissionEvent)).toBe(true);
    expect(pendingActionForTimelineEvent(mcpEvent, actions.value)).toBe(actions.value[0]);
    expect(pendingActionForTimelineEvent(permissionEvent, actions.value)).toBe(actions.value[1]);
  });
});
