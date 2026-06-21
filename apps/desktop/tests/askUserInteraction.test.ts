import { computed, effectScope, nextTick, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AskUserSpec } from "@lilia/contracts";
import {
  clearAskUsersForTask,
  hydrateAskUserForTask,
  pendingAskInteractionKey,
  type PendingAsk,
  usePendingAsksForTask,
} from "../src/composables/useAskUser";
import {
  useAskUserInteraction,
} from "../src/composables/useAskUserInteraction";

function pendingAsk(input: {
  id: number;
  requestId?: string | null;
  spec?: AskUserSpec;
}): PendingAsk {
  return {
    id: input.id,
    requestId: input.requestId ?? null,
    spec: input.spec ?? {
      questions: [
        {
          id: "q-1",
          question: "Pick one",
          mode: "single",
          options: [{ id: "a", label: "A" }],
        },
        {
          id: "q-2",
          question: "Continue?",
          mode: "confirm",
        },
      ],
    },
    taskId: "task-1",
    turnId: "turn-1",
    resolve: () => {},
  };
}

describe("useAskUserInteraction", () => {
  afterEach(() => {
    clearAskUsersForTask("task-1");
  });

  it("uses requestId as the stable ask interaction key", () => {
    expect(pendingAskInteractionKey(pendingAsk({ id: 1, requestId: "ask-1" }))).toBe("ask:ask-1");
    expect(pendingAskInteractionKey(pendingAsk({ id: 2, requestId: null }))).toBe("ask:2");
  });

  it("updates existing hydrated asks by requestId instead of duplicating them", () => {
    const pendingAsks = usePendingAsksForTask("task-1");
    const first = hydrateAskUserForTask(
      "task-1",
      {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      "ask-1",
    );
    const second = hydrateAskUserForTask(
      "task-1",
      {
        title: "更新确认",
        questions: [{ id: "q-1", question: "继续执行？", mode: "confirm" }],
      },
      "turn-2",
      "ask-1",
    );

    expect(second.id).toBe(first.id);
    expect(second.turnId).toBe("turn-2");
    expect(second.spec.title).toBe("更新确认");
    expect(pendingAsks.value.filter((ask) => ask.requestId === "ask-1")).toHaveLength(1);
  });

  it("deduplicates requestless hydrated asks with semantically identical specs", () => {
    const pendingAsks = usePendingAsksForTask("task-1");
    const first = hydrateAskUserForTask(
      "task-1",
      {
        title: "确认",
        questions: [{ id: "q-1", question: "继续？", mode: "confirm" }],
      },
      "turn-1",
      null,
    );
    const second = hydrateAskUserForTask(
      "task-1",
      {
        questions: [{ mode: "confirm", question: "继续？", id: "q-1" }],
        title: "确认",
      },
      "turn-1",
      null,
    );

    expect(second.id).toBe(first.id);
    expect(pendingAsks.value.filter((ask) => ask.turnId === "turn-1")).toHaveLength(1);
  });

  it("keeps navigable answers when the same request rehydrates with a new local id", async () => {
    const activeAsk = ref<PendingAsk | null>(pendingAsk({ id: 1, requestId: "ask-1" }));
    const freeformText = ref("");
    const resolved = vi.fn();
    const scope = effectScope();
    const controller = scope.run(() =>
      useAskUserInteraction(computed(() => activeAsk.value), freeformText, resolved)
    )!;

    await nextTick();
    controller.selectSingleOption("a");
    controller.submitAsk();
    expect(controller.askIndex.value).toBe(1);

    activeAsk.value = pendingAsk({ id: 2, requestId: "ask-1" });
    await nextTick();
    expect(controller.askIndex.value).toBe(1);

    activeAsk.value = pendingAsk({ id: 3, requestId: "ask-2" });
    await nextTick();
    expect(controller.askIndex.value).toBe(0);

    scope.stop();
  });
});
