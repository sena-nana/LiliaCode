import { computed, effectScope, nextTick, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PendingAsk } from "../src/composables/useAskUser";
import {
  composerPendingAutoResolution,
  composerPendingEntryActionsMode,
  composerPendingInteractionKey,
  useComposerPendingInteraction,
} from "../src/components/chat/useComposerPendingInteraction";
import {
  pendingAgentActionAutoDecisionKey,
  pendingAskAgentAction,
  toolConsentAgentAction,
} from "../src/composables/pendingAgentActions";
import type { ToolConsentRequest } from "../src/services/chat";
import { recommendedAskUserResult } from "../src/components/chat/freeImplementationMode";
import { pendingAutoDecisionLabel } from "@lilia/contracts";

afterEach(() => {
  vi.useRealTimers();
});

describe("free implementation mode ask-user recommendations", () => {
  it("approves confirm questions automatically", () => {
    expect(recommendedAskUserResult({
      questions: [{ id: "approve-plan", question: "approve?", mode: "confirm" }],
    })).toEqual({
      cancelled: false,
      answers: {
        "approve-plan": { questionId: "approve-plan", value: "yes" },
      },
    });
  });

  it("selects the recommended single option", () => {
    expect(recommendedAskUserResult({
      questions: [{
        id: "choice",
        question: "pick one",
        mode: "single",
        options: [
          { id: "manual", label: "Manual" },
          { id: "recommended", label: "Recommended", recommended: true },
        ],
      }],
    })).toMatchObject({
      answers: {
        choice: { questionId: "choice", value: "recommended" },
      },
    });
  });

  it("selects recommended multi options while respecting min and max", () => {
    expect(recommendedAskUserResult({
      questions: [{
        id: "multi",
        question: "pick many",
        mode: "multi",
        minSelections: 1,
        maxSelections: 2,
        options: [
          { id: "a", label: "A", recommended: true },
          { id: "b", label: "B", recommended: true },
          { id: "c", label: "C", recommended: true },
        ],
      }],
    })).toMatchObject({
      answers: {
        multi: { questionId: "multi", value: ["a", "b"] },
      },
    });
  });

  it("does not auto-submit non-confirm questions without recommendations", () => {
    expect(recommendedAskUserResult({
      questions: [{
        id: "choice",
        question: "pick one",
        mode: "single",
        options: [{ id: "manual", label: "Manual" }],
      }],
    })).toBeNull();
  });
});

describe("free implementation mode countdown", () => {
  function pendingAsk(spec: PendingAsk["spec"]): PendingAsk {
    return {
      id: 1,
      requestId: "ask-1",
      spec,
      taskId: "task-1",
      turnId: "turn-1",
      resolve: () => {},
    };
  }

  it("auto-approves plan approval after eight seconds", async () => {
    vi.useFakeTimers();
    const ask = ref<PendingAsk | null>(pendingAsk({
      intent: "plan_approval",
      questions: [{ id: "approve-plan", question: "", mode: "confirm" }],
    }));
    const resolved = vi.fn();
    const scope = effectScope();
    const controller = scope.run(() => useComposerPendingInteraction({
      clearContextState: () => {},
      freeImplementation: computed(() => true),
      pendingAsk: computed(() => ask.value),
      queueResize: () => {},
      resolveAsk: resolved,
      resolveToolConsent: () => {},
      toolConsent: computed(() => null),
    }))!;

    await nextTick();
    expect(controller.autoDecisionText.value).toContain("8 秒后同意计划");

    await vi.advanceTimersByTimeAsync(8000);

    expect(resolved).toHaveBeenCalledWith({
      cancelled: false,
      answers: {
        "approve-plan": { questionId: "approve-plan", value: "yes" },
      },
    });
    scope.stop();
  });

  it("does not start countdown when ask-user has no recommendation", async () => {
    vi.useFakeTimers();
    const ask = ref<PendingAsk | null>(pendingAsk({
      questions: [{
        id: "choice",
        question: "pick",
        mode: "single",
        options: [{ id: "manual", label: "Manual" }],
      }],
    }));
    const resolved = vi.fn();
    const scope = effectScope();
    const controller = scope.run(() => useComposerPendingInteraction({
      clearContextState: () => {},
      freeImplementation: computed(() => true),
      pendingAsk: computed(() => ask.value),
      queueResize: () => {},
      resolveAsk: resolved,
      resolveToolConsent: () => {},
      toolConsent: computed(() => null),
    }))!;

    await nextTick();
    expect(controller.autoDecisionText.value).toBe("");

    await vi.advanceTimersByTimeAsync(8100);

    expect(resolved).not.toHaveBeenCalled();
    scope.stop();
  });
});

describe("composer pending interaction keys", () => {
  function pendingAsk(spec: PendingAsk["spec"]): PendingAsk {
    return {
      id: 7,
      requestId: "ask-7",
      spec,
      taskId: "task-1",
      turnId: "turn-1",
      resolve: () => {},
    };
  }

  const toolConsent: ToolConsentRequest = {
    taskId: "task-1",
    turnId: "turn-1",
    backend: "codex",
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

  it("keeps composer pending keys centralized", () => {
    const ask = pendingAsk({
      questions: [{ id: "q-1", question: "continue?", mode: "confirm" }],
    });

    expect(composerPendingInteractionKey(ask, toolConsent)).toBe("ask:ask-7");
    expect(composerPendingInteractionKey(null, toolConsent)).toBe("tool:tool-1");
    expect(composerPendingInteractionKey(null, null)).toBe("none");
  });

  it("keeps ask keys stable across local hydration ids", () => {
    const ask = pendingAsk({
      questions: [{ id: "q-1", question: "continue?", mode: "confirm" }],
    });
    const rehydratedAsk: PendingAsk = {
      ...ask,
      id: 8,
    };
    const legacyAsk: PendingAsk = {
      ...ask,
      id: 9,
      requestId: null,
    };

    expect(composerPendingInteractionKey(rehydratedAsk, null)).toBe("ask:ask-7");
    expect(composerPendingInteractionKey(legacyAsk, null)).toBe("ask:9");
  });

  it("derives auto-decision keys from unified pending actions", () => {
    const ask = pendingAsk({
      questions: [{ id: "q-1", question: "continue?", mode: "confirm" }],
    });
    const baseToolState = {
      toolDanger: false,
      toolSubmitting: false,
      editingToolCommand: false,
      toolCommandIsEmpty: false,
    };

    expect(pendingAgentActionAutoDecisionKey(pendingAskAgentAction(ask), {
      askQuestionId: "q-1",
      askHasRecommendedResult: true,
    })).toBe("ask_user:ask-7:q-1");
    expect(pendingAgentActionAutoDecisionKey(pendingAskAgentAction(ask), {
      askQuestionId: "q-1",
      askHasRecommendedResult: false,
    })).toBe("");
    const toolAction = toolConsentAgentAction(toolConsent);
    expect(pendingAgentActionAutoDecisionKey(toolAction, baseToolState)).toBe("tool:tool-1");
    expect(pendingAgentActionAutoDecisionKey(toolAction, { ...baseToolState, toolDanger: true })).toBe("");
    expect(pendingAgentActionAutoDecisionKey(toolAction, { ...baseToolState, editingToolCommand: true })).toBe("");
    expect(pendingAgentActionAutoDecisionKey(toolAction, { ...baseToolState, toolCommandIsEmpty: true })).toBe("");
  });

  it("maps unified auto resolutions back to composer resolver calls", () => {
    const ask = pendingAsk({
      questions: [{ id: "q-1", question: "continue?", mode: "confirm" }],
    });
    const askResult = recommendedAskUserResult(ask.spec);

    expect(composerPendingAutoResolution(pendingAskAgentAction(ask), {
      askHasRecommendedResult: !!askResult,
      askQuestionId: "q-1",
      askResult,
    })).toEqual({
      target: "ask_user",
      submittingTarget: null,
      result: askResult,
    });
    expect(composerPendingAutoResolution(toolConsentAgentAction(toolConsent), {
      editingToolCommand: false,
      toolCommandIsEmpty: false,
      toolDanger: false,
      toolSubmitting: false,
      toolUpdatedInput: { command: "pwd" },
    })).toEqual({
      target: "tool_consent",
      submittingTarget: "tool",
      decision: "allow",
      updatedInput: { command: "pwd" },
    });
  });

  it("derives stable entry action mode from pending state", () => {
    expect(composerPendingEntryActionsMode({
      askUsesInputActions: true,
      askIsPlanApproval: false,
      hasAsk: true,
      hasToolConsent: false,
    })).toBe("ask-input");
    expect(composerPendingEntryActionsMode({
      askUsesInputActions: false,
      askIsPlanApproval: true,
      hasAsk: true,
      hasToolConsent: false,
    })).toBe("ask-plan");
    expect(composerPendingEntryActionsMode({
      askUsesInputActions: false,
      askIsPlanApproval: false,
      hasAsk: true,
      hasToolConsent: false,
    })).toBe("ask-confirm");
    expect(composerPendingEntryActionsMode({
      askUsesInputActions: false,
      askIsPlanApproval: false,
      hasAsk: false,
      hasToolConsent: true,
    })).toBe("tool");
    expect(composerPendingEntryActionsMode({
      askUsesInputActions: false,
      askIsPlanApproval: false,
      hasAsk: false,
      hasToolConsent: false,
    })).toBe("none");
  });
});

describe("pending action auto-decision labels", () => {
  it("keeps countdown labels centralized for composer and timeline pending actions", () => {
    expect(pendingAutoDecisionLabel("ask_user")).toBe("选择推荐项");
    expect(pendingAutoDecisionLabel("plan_approval")).toBe("同意计划");
    expect(pendingAutoDecisionLabel("tool_consent")).toBe("同意工具调用");
    expect(pendingAutoDecisionLabel("title_update")).toBe("同意标题");
    expect(pendingAutoDecisionLabel("mcp_elicitation")).toBe("提交 MCP 表单");
    expect(pendingAutoDecisionLabel("architecture_change")).toBe("应用架构变更");
    expect(pendingAutoDecisionLabel("permission_approval")).toBe("同意权限请求");
  });
});

