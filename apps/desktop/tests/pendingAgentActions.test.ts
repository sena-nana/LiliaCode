import { describe, expect, it } from "vitest";
import {
  ARCHITECTURE_INTERACTION_KIND,
  MCP_ELICITATION_INTERACTION_KIND,
  PLAN_APPROVAL_INTERACTION_KIND,
  TITLE_UPDATE_ACTION_KIND,
  TOOL_CONSENT_INTERACTION_KIND,
  agentTimelineEventRequiresAction,
  type AgentTimelineEvent,
} from "@lilia/contracts";
import {
  isPendingAskUserAgentAction,
  pendingAskAgentAction,
  pendingAgentActionBuckets,
  pendingAgentActionAutoDecisionLabel,
  pendingAgentActionAutoDecisionKey,
  pendingAgentActionAutoResolution,
  pendingAgentActionsForTask,
  pendingAgentActionKey,
  pendingAgentActionResolution,
  pendingAgentActionResolutionSubmittingTarget,
  pendingAgentActionTraits,
  pendingActionForTimelineEvent,
  toolConsentAgentAction,
  uniquePendingAgentActions,
  type PendingAgentAction,
} from "../src/composables/pendingAgentActions";
import { usePendingAgentActionsForTask } from "../src/composables/usePendingAgentActions";
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
  it("combines pending sources into the unified action model", () => {
    const ask = {
      id: 11,
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
      spec: {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" as const }],
      },
      resolve: () => {},
    };
    const planAsk = {
      id: 12,
      taskId: "task-1",
      turnId: "turn-2",
      requestId: "plan-1",
      spec: {
        title: "确认计划",
        intent: "plan_approval" as const,
        questions: [{ id: "approve-plan", question: "", mode: "confirm" as const }],
      },
      resolve: () => {},
    };
    const toolConsent = {
      taskId: "task-1",
      turnId: "turn-1",
      backend: "claude" as const,
      requestId: "tool-1",
      toolName: "Bash",
      input: { command: "pwd" },
      title: null,
      displayName: null,
      description: null,
      blockedPath: null,
      decisionReason: null,
      toolUseId: null,
    };
    const mcpAction: PendingAgentAction = {
      kind: MCP_ELICITATION_INTERACTION_KIND,
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
    };
    const architectureAction: PendingAgentAction = {
      kind: ARCHITECTURE_INTERACTION_KIND,
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "architecture-1",
      id: "architecture-1",
      title: "更新架构",
      summary: "改架构文档",
      draft: "draft",
      response: { status: "pending" },
    };
    const titleEvent = timelineEvent({
      id: "title-update:task-1:title-1",
      kind: TITLE_UPDATE_ACTION_KIND,
      status: "requires_action",
      payload: {
        requestId: "title-1",
        proposedTitle: "新标题",
        previousTitle: "旧标题",
      },
    });

    const actions = pendingAgentActionsForTask({
      asks: [ask, planAsk],
      toolConsents: [toolConsent],
      agentInteractions: [mcpAction],
      architectureChanges: [architectureAction],
      timelineEvents: [titleEvent],
    });

    expect(actions.map((action) => action.kind)).toEqual([
      "ask_user",
      PLAN_APPROVAL_INTERACTION_KIND,
      TOOL_CONSENT_INTERACTION_KIND,
      MCP_ELICITATION_INTERACTION_KIND,
      ARCHITECTURE_INTERACTION_KIND,
      TITLE_UPDATE_ACTION_KIND,
    ]);
    expect(actions[0]).toMatchObject({ requestId: "ask-1", ask });
    expect(actions[1]).toMatchObject({ requestId: "plan-1", ask: planAsk });
    expect(actions[2]).toMatchObject({ requestId: "tool-1", request: toolConsent });
    expect(actions[5]).toMatchObject({
      requestId: "title-1",
      proposedTitle: "新标题",
      previousTitle: "旧标题",
    });
  });

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
      timelineEvent({
        status: "requires_action",
        payload: { requestId: "other-plan", approved: null },
      }),
      [action],
    )).toBeNull();
    expect(pendingActionForTimelineEvent(
      timelineEvent({
        status: "requires_action",
        payload: { requestId: "ask-1", approved: null },
      }),
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

    expect(agentTimelineEventRequiresAction(completed)).toBe(false);
    expect(pendingActionForTimelineEvent(completed, [action])).toBeNull();
  });

  it("classifies visible and blocking pending actions", () => {
    const askAction: PendingAgentAction = {
      kind: "ask_user",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
      ask: {
        id: 11,
        taskId: "task-1",
        turnId: "turn-1",
        requestId: "ask-1",
        spec: {
          title: "确认",
          questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
        },
        resolve: () => {},
      },
    };
    const titleAction: PendingAgentAction = {
      kind: "title_update",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "title-1",
      proposedTitle: "新标题",
      previousTitle: "旧标题",
    };
    const toolAction: PendingAgentAction = {
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
    const mcpAction: PendingAgentAction = {
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
    };

    expect(isPendingAskUserAgentAction(askAction)).toBe(true);
    expect(isPendingAskUserAgentAction(toolAction)).toBe(false);
    expect(pendingAgentActionKey(askAction)).toBe("ask_user:ask-1");
    expect(pendingAgentActionKey(titleAction)).toBe("title:title-1");
    expect(pendingAgentActionKey(toolAction)).toBe("tool:consent-1");
    expect(pendingAgentActionKey(mcpAction)).toBe("mcp:mcp-1");
    expect(pendingAgentActionAutoDecisionLabel(askAction)).toBe("选择推荐项");
    expect(pendingAgentActionAutoDecisionLabel(titleAction)).toBe("同意标题");
    expect(pendingAgentActionAutoDecisionLabel(toolAction)).toBe("同意工具调用");
    expect(pendingAgentActionAutoDecisionLabel(mcpAction)).toBe("提交 MCP 表单");
    expect(pendingAgentActionTraits(titleAction)).toEqual({
      blocksComposer: false,
      visibleInTimelineWithoutInterrupt: true,
    });
    expect(pendingAgentActionTraits(mcpAction)).toEqual({
      blocksComposer: true,
      visibleInTimelineWithoutInterrupt: true,
    });
    expect(pendingAgentActionTraits(toolAction)).toEqual({
      blocksComposer: true,
      visibleInTimelineWithoutInterrupt: false,
    });
    expect(pendingAgentActionBuckets([askAction, titleAction, toolAction, mcpAction], {
      nonInterruptMode: false,
    })).toEqual({
      visible: [titleAction, mcpAction],
      blocking: [askAction, toolAction, mcpAction],
    });
    expect(pendingAgentActionBuckets([askAction, titleAction, toolAction, mcpAction], {
      nonInterruptMode: true,
    })).toEqual({
      visible: [askAction, titleAction, toolAction, mcpAction],
      blocking: [askAction, toolAction, mcpAction],
    });
  });

  it("derives free-mode auto-decision keys from eligible pending actions", () => {
    const askAction: PendingAgentAction = {
      kind: "ask_user",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
      ask: {
        id: 11,
        taskId: "task-1",
        turnId: "turn-1",
        requestId: "ask-1",
        spec: {
          title: "确认",
          questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
        },
        resolve: () => {},
      },
    };
    const toolAction: PendingAgentAction = {
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
    const mcpAction: PendingAgentAction = {
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
    };
    const titleAction: PendingAgentAction = {
      kind: "title_update",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "title-1",
      proposedTitle: "新标题",
      previousTitle: "旧标题",
    };
    const permissionAction: PendingAgentAction = {
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
    };
    const architectureAction: PendingAgentAction = {
      kind: "architecture_change",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "architecture-1",
      id: "architecture-1",
      title: "更新架构",
      summary: "改架构文档",
      draft: "draft",
      response: { status: "pending" },
    };

    expect(pendingAgentActionAutoDecisionKey(askAction, {
      askHasRecommendedResult: true,
      askQuestionId: "q-1",
    })).toBe("ask_user:ask-1:q-1");
    expect(pendingAgentActionAutoDecisionKey(askAction, {
      askHasRecommendedResult: false,
      askQuestionId: "q-1",
    })).toBe("");
    expect(pendingAgentActionAutoDecisionKey(toolAction, {
      toolCommandIsEmpty: false,
    })).toBe("tool:consent-1");
    expect(pendingAgentActionAutoDecisionKey(toolAction, {
      editingToolCommand: true,
      toolCommandIsEmpty: false,
    })).toBe("");
    expect(pendingAgentActionAutoDecisionKey(titleAction, {})).toBe("title:title-1");
    expect(pendingAgentActionAutoDecisionKey(mcpAction, { mcpCanSubmit: true })).toBe("mcp:mcp-1");
    expect(pendingAgentActionAutoDecisionKey(mcpAction, { mcpCanSubmit: false })).toBe("");
    expect(pendingAgentActionAutoDecisionKey(permissionAction, {})).toBe("permission:permission-1");
    expect(pendingAgentActionAutoDecisionKey(permissionAction, { submitting: true })).toBe("");
    expect(pendingAgentActionAutoDecisionKey(architectureAction, {})).toBe("architecture:architecture-1");
    expect(pendingAskAgentAction(askAction.ask)).toMatchObject({
      kind: "ask_user",
      requestId: "ask-1",
      ask: askAction.ask,
    });
    expect(pendingAgentActionAutoDecisionKey({
      ...askAction,
      requestId: null,
      ask: { ...askAction.ask, requestId: null },
    }, {
      askHasRecommendedResult: true,
      askQuestionId: "q-1",
    })).toBe("ask:11:q-1");
    expect(toolConsentAgentAction(toolAction.request)).toMatchObject({
      kind: "tool_consent",
      requestId: "consent-1",
      request: toolAction.request,
    });
  });

  it("builds typed resolutions from pending actions", () => {
    const askAction: PendingAgentAction = {
      kind: "ask_user",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
      ask: {
        id: 11,
        taskId: "task-1",
        turnId: "turn-1",
        requestId: "ask-1",
        spec: {
          title: "确认",
          questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
        },
        resolve: () => {},
      },
    };
    const toolAction: PendingAgentAction = {
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
    const mcpAction: PendingAgentAction = {
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
    };
    const titleAction: PendingAgentAction = {
      kind: "title_update",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "title-1",
      proposedTitle: "新标题",
      previousTitle: "旧标题",
    };
    const permissionAction: PendingAgentAction = {
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
    };
    const architectureAction: PendingAgentAction = {
      kind: "architecture_change",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "architecture-1",
      id: "architecture-1",
      title: "更新架构",
      summary: "改架构文档",
      draft: "draft",
      response: { status: "pending" },
    };

    expect(pendingAgentActionResolution(askAction, {
      askResult: { answers: {}, cancelled: false },
    })).toMatchObject({
      kind: "ask_user",
      requestId: "ask-1",
      askId: 11,
      result: { cancelled: false },
    });
    expect(pendingAgentActionResolution(toolAction, {
      toolDecision: "deny",
      toolMessage: "no",
    })).toMatchObject({
      kind: "tool_consent",
      requestId: "consent-1",
      decision: "deny",
      message: "no",
    });
    expect(pendingAgentActionResolution(mcpAction, {
      mcpDecision: "accept",
      mcpContent: { project: "lilia" },
    })).toMatchObject({
      kind: "mcp_elicitation",
      requestId: "mcp-1",
      action: "accept",
      content: { project: "lilia" },
    });
    expect(pendingAgentActionResolution(titleAction, {
      titleDecision: "accept",
    })).toEqual({
      kind: "title_update",
      requestId: "title-1",
      decision: "accept",
    });
    expect(pendingAgentActionResolution(permissionAction, {
      permissionDecision: "allow",
    })).toEqual({
      kind: "permission_approval",
      requestId: "permission-1",
      decision: "allow",
    });
    expect(pendingAgentActionResolution(architectureAction, {
      architectureDecision: "deny",
    })).toEqual({
      kind: "architecture_change",
      requestId: "architecture-1",
      decision: "deny",
    });
    expect(pendingAgentActionResolution(toolAction, {})).toBeNull();
  });

  it("classifies which resolutions should enter a submitting state", () => {
    expect(pendingAgentActionResolutionSubmittingTarget({
      kind: TOOL_CONSENT_INTERACTION_KIND,
      requestId: "tool-1",
      decision: "allow",
    })).toBe("tool");
    expect(pendingAgentActionResolutionSubmittingTarget({
      kind: MCP_ELICITATION_INTERACTION_KIND,
      requestId: "mcp-1",
      action: "accept",
      content: {},
    })).toBe("codex");
    expect(pendingAgentActionResolutionSubmittingTarget({
      kind: ARCHITECTURE_INTERACTION_KIND,
      requestId: "architecture-1",
      decision: "allow",
    })).toBe("codex");
    expect(pendingAgentActionResolutionSubmittingTarget({
      kind: TITLE_UPDATE_ACTION_KIND,
      requestId: "title-1",
      decision: "accept",
    })).toBeNull();
  });

  it("builds auto resolutions only when the action is eligible", () => {
    const askAction: PendingAgentAction = {
      kind: "ask_user",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
      ask: {
        id: 11,
        taskId: "task-1",
        turnId: "turn-1",
        requestId: "ask-1",
        spec: {
          title: "确认",
          questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
        },
        resolve: () => {},
      },
    };
    const toolAction: PendingAgentAction = {
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
    const mcpAction: PendingAgentAction = {
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
    };
    const titleAction: PendingAgentAction = {
      kind: "title_update",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "title-1",
      proposedTitle: "新标题",
      previousTitle: "旧标题",
    };
    const permissionAction: PendingAgentAction = {
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
    };
    const architectureAction: PendingAgentAction = {
      kind: "architecture_change",
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "architecture-1",
      id: "architecture-1",
      title: "更新架构",
      summary: "改架构文档",
      draft: "draft",
      response: { status: "pending" },
    };

    expect(pendingAgentActionAutoResolution(askAction, {
      askHasRecommendedResult: true,
      askQuestionId: "q-1",
      askResult: { answers: {}, cancelled: false },
    })).toMatchObject({
      kind: "ask_user",
      requestId: "ask-1",
      askId: 11,
    });
    expect(pendingAgentActionAutoResolution(askAction, {
      askHasRecommendedResult: false,
      askResult: { answers: {}, cancelled: false },
    })).toBeNull();
    expect(pendingAgentActionAutoResolution(toolAction, {
      toolCommandIsEmpty: false,
      toolUpdatedInput: { command: "pwd" },
    })).toMatchObject({
      kind: "tool_consent",
      requestId: "consent-1",
      decision: "allow",
      updatedInput: { command: "pwd" },
    });
    expect(pendingAgentActionAutoResolution(toolAction, {
      toolCommandIsEmpty: false,
      toolDanger: true,
    })).toBeNull();
    expect(pendingAgentActionAutoResolution(titleAction, {})).toEqual({
      kind: "title_update",
      requestId: "title-1",
      decision: "accept",
    });
    expect(pendingAgentActionAutoResolution(mcpAction, {
      mcpCanSubmit: true,
      mcpContent: { project: "lilia" },
    })).toEqual({
      kind: "mcp_elicitation",
      requestId: "mcp-1",
      action: "accept",
      content: { project: "lilia" },
    });
    expect(pendingAgentActionAutoResolution(permissionAction, {})).toEqual({
      kind: "permission_approval",
      requestId: "permission-1",
      decision: "allow",
    });
    expect(pendingAgentActionAutoResolution(architectureAction, {
      submitting: true,
    })).toBeNull();
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
    expect(agentTimelineEventRequiresAction(event)).toBe(true);
    expect(pendingActionForTimelineEvent(event, actions.value)).toBe(actions.value[0]);
  });

  it("deduplicates request-keyed pending actions and keeps the latest title payload", () => {
    const firstTitle = timelineEvent({
      id: "title-update:task-1:req-1:first",
      kind: TITLE_UPDATE_ACTION_KIND,
      status: "requires_action",
      payload: {
        requestId: "req-1",
        proposedTitle: "旧建议",
        previousTitle: "原标题",
      },
    });
    const latestTitle = timelineEvent({
      id: "title-update:task-1:req-1:latest",
      kind: TITLE_UPDATE_ACTION_KIND,
      status: "requires_action",
      payload: {
        requestId: "req-1",
        proposedTitle: "新建议",
        previousTitle: "原标题",
      },
    });

    const actions = pendingAgentActionsForTask({
      asks: [],
      toolConsents: [],
      timelineEvents: [firstTitle, latestTitle],
    });

    expect(actions).toHaveLength(1);
    expect(actions[0]).toMatchObject({
      kind: TITLE_UPDATE_ACTION_KIND,
      requestId: "req-1",
      proposedTitle: "新建议",
    });
    expect(uniquePendingAgentActions([actions[0]!, actions[0]!])).toHaveLength(1);
  });

  it("deduplicates ask actions by request id when a backend request is rehydrated", () => {
    const firstAsk = {
      id: 11,
      taskId: "task-1",
      turnId: "turn-1",
      requestId: "ask-1",
      spec: {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" as const }],
      },
      resolve: () => {},
    };
    const latestAsk = {
      ...firstAsk,
      id: 12,
      spec: {
        title: "确认更新",
        questions: [{ id: "q-1", question: "继续执行？", mode: "confirm" as const }],
      },
    };

    const actions = pendingAgentActionsForTask({
      asks: [firstAsk, latestAsk],
      toolConsents: [],
    });

    expect(actions).toHaveLength(1);
    expect(actions[0]).toMatchObject({
      kind: "ask_user",
      requestId: "ask-1",
      ask: latestAsk,
    });
  });

  it("attaches MCP and permission actions to matching timeline events", () => {
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
    expect(agentTimelineEventRequiresAction(mcpEvent)).toBe(true);
    expect(agentTimelineEventRequiresAction(permissionEvent)).toBe(true);
    expect(pendingActionForTimelineEvent(mcpEvent, actions.value)).toBe(actions.value[0]);
    expect(pendingActionForTimelineEvent(permissionEvent, actions.value)).toBe(actions.value[1]);
  });
});
