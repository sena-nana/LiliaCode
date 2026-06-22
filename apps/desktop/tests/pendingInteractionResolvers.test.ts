import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ref } from "vue";
import {
  ASK_USER_INTERACTION_KIND,
  ARCHITECTURE_INTERACTION_KIND,
  CHAT_RESPOND_AGENT_INTERACTION_COMMAND,
  MCP_ELICITATION_INTERACTION_KIND,
  PERMISSION_APPROVAL_INTERACTION_KIND,
  TOOL_CONSENT_INTERACTION_KIND,
  type AskUserResult,
} from "@lilia/contracts";
import {
  clearAgentPendingInteractionsForTask,
  type PendingAgentInteraction,
} from "../src/composables/useAgentPendingInteractions";
import {
  clearAskUsersForTask,
  hydrateAskUserForTask,
} from "../src/composables/useAskUser";
import { clearToolConsentForTask } from "../src/composables/useToolConsentBridge";
import type { PendingArchitectureChange } from "../src/composables/useProjectArchitectureInteractions";
import {
  pendingToolConsentsForResolution,
  pendingResolutionPlan,
  usePendingInteractionResolvers,
} from "../src/pages/taskDetail/usePendingInteractionResolvers";
import type { ToolConsentRequest } from "../src/services/chat";
import {
  mockInvoke,
  resetTauriMockData,
} from "./tauriMock";

const taskId = "task-1";

function toolConsentRequest(): ToolConsentRequest {
  return {
    taskId,
    turnId: "turn-1",
    backend: "claude",
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
}

function permissionApprovalInteraction(): PendingAgentInteraction {
  return {
    kind: PERMISSION_APPROVAL_INTERACTION_KIND,
    taskId,
    turnId: "turn-1",
    requestId: "permission-1",
    payload: {
      reason: "需要访问网络",
      requestedAccess: { network: { domains: [{ domain: "example.com" }] } },
      scopeSuggestion: "turn",
      providerContext: {
        codex: {
          threadId: "thread-1",
          turnId: "turn-1",
          itemId: "item-1",
          startedAtMs: 1,
          cwd: "D:/PROJECT/workspace/Lilia",
          permissions: { network: true },
        },
      },
    },
  };
}

function mcpElicitationInteraction(): PendingAgentInteraction {
  return {
    kind: MCP_ELICITATION_INTERACTION_KIND,
    taskId,
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
}

function architectureChange(): PendingArchitectureChange {
  return {
    kind: ARCHITECTURE_INTERACTION_KIND,
    taskId,
    turnId: "turn-1",
    requestId: "architecture-1",
    id: "architecture-1",
    title: "更新架构",
    summary: "改架构文档",
    draft: "draft",
    response: { status: "pending" },
  };
}

function setupResolvers(input: {
  pendingAskUser?: ReturnType<typeof hydrateAskUserForTask> | null;
  pendingAskUsers?: ReturnType<typeof hydrateAskUserForTask>[];
  pendingTitleUpdateRequestIds?: Set<string>;
  pendingToolConsent?: ToolConsentRequest | null;
  pendingToolConsents?: ToolConsentRequest[];
  pendingAgentInteractions?: PendingAgentInteraction[];
} = {}) {
  const pendingAskUsers = input.pendingAskUsers ??
    (input.pendingAskUser ? [input.pendingAskUser] : []);
  return usePendingInteractionResolvers({
    taskId: () => taskId,
    pendingAskUser: ref(input.pendingAskUser ?? null),
    pendingAskUsers: ref(pendingAskUsers),
    pendingTitleUpdateRequestIds: ref(input.pendingTitleUpdateRequestIds ?? new Set()),
    pendingToolConsent: ref(input.pendingToolConsent ?? input.pendingToolConsents?.[0] ?? null),
    pendingToolConsents: ref(input.pendingToolConsents ?? []),
    pendingAgentInteractions: ref(input.pendingAgentInteractions ?? []),
    pendingArchitectureChanges: ref([]),
  });
}

beforeEach(() => {
  resetTauriMockData();
});

afterEach(() => {
  clearAskUsersForTask(taskId);
  clearToolConsentForTask(taskId);
  clearAgentPendingInteractionsForTask(taskId);
  vi.restoreAllMocks();
});

describe("pending interaction resolvers", () => {
  it("merges the active composer tool consent into the resolution list", () => {
    const active = toolConsentRequest();
    const other = { ...toolConsentRequest(), requestId: "tool-2" };

    expect(pendingToolConsentsForResolution(active, [other])).toEqual([active, other]);
    expect(pendingToolConsentsForResolution(active, [active, other])).toEqual([active, other]);
    expect(pendingToolConsentsForResolution(null, [other])).toEqual([other]);
  });

  it("plans pending resolutions from current pending state", () => {
    const tool = toolConsentRequest();
    const permission = permissionApprovalInteraction();
    const mcp = mcpElicitationInteraction();
    const architecture = architectureChange();
    const context = {
      taskId,
      pendingAsks: [],
      pendingAgentInteractions: [permission, mcp],
      pendingArchitectureChanges: [architecture],
      pendingTitleUpdateRequestIds: new Set(["title-1"]),
      pendingToolConsents: [tool],
    };

    expect(pendingResolutionPlan(context, {
      kind: "title_update",
      requestId: "title-1",
      decision: "accept",
    })).toMatchObject({
      label: "title-update",
      target: "title_update",
      requestId: "title-1",
      decision: "accept",
    });
    expect(pendingResolutionPlan(context, {
      kind: "title_update",
      requestId: "missing-title",
      decision: "accept",
    })).toBeNull();
    expect(pendingResolutionPlan(context, {
      kind: TOOL_CONSENT_INTERACTION_KIND,
      requestId: "tool-1",
      decision: "allow",
      updatedInput: { command: "pwd" },
    })).toMatchObject({
      label: "tool-consent",
      target: "tool_consent",
      request: tool,
      decision: "allow",
      updatedInput: { command: "pwd" },
    });
    expect(pendingResolutionPlan(context, {
      kind: TOOL_CONSENT_INTERACTION_KIND,
      requestId: "missing-tool",
      decision: "allow",
    })).toBeNull();
    expect(pendingResolutionPlan(context, {
      kind: MCP_ELICITATION_INTERACTION_KIND,
      requestId: "mcp-1",
      action: "accept",
      content: { project: "lilia" },
    })).toMatchObject({
      label: "mcp-elicitation",
      target: "mcp_elicitation",
      result: {
        action: "accept",
        content: { project: "lilia" },
      },
    });
    expect(pendingResolutionPlan(context, {
      kind: MCP_ELICITATION_INTERACTION_KIND,
      requestId: "missing-mcp",
      action: "accept",
      content: { project: "lilia" },
    })).toBeNull();
    expect(pendingResolutionPlan(context, {
      kind: PERMISSION_APPROVAL_INTERACTION_KIND,
      requestId: "permission-1",
      decision: "allow",
    })).toMatchObject({
      label: "permission-approval",
      target: "permission_approval",
      result: {
        action: "approve",
        grantedAccess: { network: { domains: [{ domain: "example.com" }] } },
      },
    });
    expect(pendingResolutionPlan(context, {
      kind: ARCHITECTURE_INTERACTION_KIND,
      requestId: "architecture-1",
      decision: "deny",
    })).toMatchObject({
      label: "project-architecture",
      target: "architecture_change",
      request: architecture,
      decision: "deny",
    });
  });

  it("writes tool consent resolutions back through the agent interaction command", async () => {
    const request = toolConsentRequest();
    const resolvers = setupResolvers({ pendingToolConsents: [request] });

    await resolvers.onResolvePendingAgentAction({
      kind: TOOL_CONSENT_INTERACTION_KIND,
      requestId: request.requestId,
      decision: "deny",
      message: "先不执行",
      updatedInput: { command: "echo skipped" },
    });

    expect(mockInvoke).toHaveBeenCalledWith(CHAT_RESPOND_AGENT_INTERACTION_COMMAND, {
      taskId,
      requestId: "tool-1",
      kind: TOOL_CONSENT_INTERACTION_KIND,
      result: {
        taskId,
        requestId: "tool-1",
        decision: "deny",
        message: "先不执行",
        updatedInput: { command: "echo skipped" },
      },
    }, undefined);
  });

  it("resolves the active composer tool consent through the same pending action path", async () => {
    const request = toolConsentRequest();
    const resolvers = setupResolvers({ pendingToolConsent: request });

    await resolvers.onResolveToolConsent("allow", undefined, { command: "pwd" });

    expect(mockInvoke).toHaveBeenCalledWith(CHAT_RESPOND_AGENT_INTERACTION_COMMAND, {
      taskId,
      requestId: "tool-1",
      kind: TOOL_CONSENT_INTERACTION_KIND,
      result: {
        taskId,
        requestId: "tool-1",
        decision: "allow",
        message: null,
        updatedInput: { command: "pwd" },
      },
    }, undefined);
  });

  it("uses the pending permission payload when approving permission requests", async () => {
    const resolvers = setupResolvers({
      pendingAgentInteractions: [permissionApprovalInteraction()],
    });

    await resolvers.onResolvePendingAgentAction({
      kind: PERMISSION_APPROVAL_INTERACTION_KIND,
      requestId: "permission-1",
      decision: "allow",
    });

    expect(mockInvoke).toHaveBeenCalledWith(CHAT_RESPOND_AGENT_INTERACTION_COMMAND, {
      taskId,
      requestId: "permission-1",
      kind: PERMISSION_APPROVAL_INTERACTION_KIND,
      result: expect.objectContaining({
        action: "approve",
        grantedAccess: { network: { domains: [{ domain: "example.com" }] } },
        scope: "turn",
      }),
    }, undefined);
  });

  it("responds to current MCP elicitation resolutions", async () => {
    const resolvers = setupResolvers({
      pendingAgentInteractions: [mcpElicitationInteraction()],
    });

    await resolvers.onResolvePendingAgentAction({
      kind: MCP_ELICITATION_INTERACTION_KIND,
      requestId: "mcp-1",
      action: "accept",
      content: { project: "lilia" },
    });

    expect(mockInvoke).toHaveBeenCalledWith(CHAT_RESPOND_AGENT_INTERACTION_COMMAND, {
      taskId,
      requestId: "mcp-1",
      kind: MCP_ELICITATION_INTERACTION_KIND,
      result: {
        action: "accept",
        content: { project: "lilia" },
      },
    }, undefined);
  });

  it("does not respond to stale MCP elicitation resolutions", async () => {
    const resolvers = setupResolvers({ pendingAgentInteractions: [] });

    await resolvers.onResolvePendingAgentAction({
      kind: MCP_ELICITATION_INTERACTION_KIND,
      requestId: "missing-mcp",
      action: "accept",
      content: { project: "lilia" },
    });

    expect(mockInvoke).not.toHaveBeenCalledWith(
      CHAT_RESPOND_AGENT_INTERACTION_COMMAND,
      expect.anything(),
      undefined,
    );
  });

  it("resolves ask-user actions by ask id", async () => {
    let resolved: AskUserResult | null = null;
    const ask = hydrateAskUserForTask(
      taskId,
      {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      "ask-1",
      (result) => {
        resolved = result;
      },
    );
    const resolvers = setupResolvers({ pendingAskUser: ask });
    const result: AskUserResult = {
      cancelled: false,
      answers: { "q-1": { questionId: "q-1", value: "yes" } },
    };

    await resolvers.onResolvePendingAgentAction({
      kind: ASK_USER_INTERACTION_KIND,
      requestId: "ask-1",
      askId: ask.id,
      result,
    });

    expect(resolved).toEqual(result);
  });

  it("resolves ask-user actions by request id when local hydration ids change", async () => {
    let resolved: AskUserResult | null = null;
    const staleAsk = hydrateAskUserForTask(
      taskId,
      {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      "ask-1",
    );
    const rehydratedAsk = hydrateAskUserForTask(
      taskId,
      {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      "ask-1",
      (result) => {
        resolved = result;
      },
    );
    const resolvers = setupResolvers({
      pendingAskUser: rehydratedAsk,
      pendingAskUsers: [rehydratedAsk],
    });
    const result: AskUserResult = {
      cancelled: false,
      answers: { "q-1": { questionId: "q-1", value: "yes" } },
    };

    await resolvers.onResolvePendingAgentAction({
      kind: ASK_USER_INTERACTION_KIND,
      requestId: "ask-1",
      askId: staleAsk.id,
      result,
    });

    expect(resolved).toEqual(result);
  });

  it("does not resolve stale ask-user actions with mismatched request ids", async () => {
    let resolved: AskUserResult | null = null;
    const ask = hydrateAskUserForTask(
      taskId,
      {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      "ask-current",
      (result) => {
        resolved = result;
      },
    );
    const resolvers = setupResolvers({ pendingAskUser: ask });
    const result: AskUserResult = {
      cancelled: false,
      answers: { "q-1": { questionId: "q-1", value: "yes" } },
    };

    await resolvers.onResolvePendingAgentAction({
      kind: ASK_USER_INTERACTION_KIND,
      requestId: "ask-stale",
      askId: ask.id,
      result,
    });

    expect(resolved).toBeNull();
  });

  it("resolves non-active pending ask actions from the full pending ask list", async () => {
    let resolved: AskUserResult | null = null;
    const activeAsk = hydrateAskUserForTask(
      taskId,
      {
        title: "当前确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      "ask-active",
    );
    const queuedAsk = hydrateAskUserForTask(
      taskId,
      {
        title: "计划确认",
        intent: "plan_approval",
        questions: [{ id: "approve-plan", question: "", mode: "confirm" }],
      },
      "turn-2",
      "plan-queued",
      (result) => {
        resolved = result;
      },
    );
    const resolvers = setupResolvers({
      pendingAskUser: activeAsk,
      pendingAskUsers: [activeAsk, queuedAsk],
    });
    const result: AskUserResult = {
      cancelled: false,
      answers: { "approve-plan": { questionId: "approve-plan", value: "confirm" } },
    };

    await resolvers.onResolvePendingAgentAction({
      kind: "plan_approval",
      requestId: "plan-queued",
      askId: queuedAsk.id,
      result,
    });

    expect(resolved).toEqual(result);
  });

  it("resolves the active composer ask-user through the same pending action path", async () => {
    let resolved: AskUserResult | null = null;
    const ask = hydrateAskUserForTask(
      taskId,
      {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      "ask-1",
      (result) => {
        resolved = result;
      },
    );
    const resolvers = setupResolvers({ pendingAskUser: ask });
    const result: AskUserResult = {
      cancelled: false,
      answers: { "q-1": { questionId: "q-1", value: "yes" } },
    };

    await resolvers.onResolveAskUser(result);

    expect(resolved).toEqual(result);
  });
});
