import { computed, effectScope, nextTick, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PendingAsk } from "../src/composables/useAskUser";
import { useComposerPendingInteraction } from "../src/components/chat/useComposerPendingInteraction";
import { recommendedAskUserResult } from "../src/components/chat/freeImplementationMode";

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
