import { describe, expect, it } from "vitest";
import { createConversationActivityStagePlan } from "../src/composables/conversationActivityStages";

describe("createConversationActivityStagePlan", () => {
  it("会将首批任务固定在当前会话和首屏行，其余延后", () => {
    const plan = createConversationActivityStagePlan({
      taskIds: ["t-1", "t-2", "t-3", "t-4", "t-5"],
      initialTaskIds: ["t-3", "t-1", "t-3"],
      priorityTaskIds: ["t-1", "t-3", "t-4"],
    });

    expect(plan.initialTaskIds).toEqual(["t-1", "t-3"]);
    expect(plan.initialPriorityTaskIds).toEqual(["t-1", "t-3"]);
    expect(plan.deferredTaskIds).toEqual(["t-2", "t-4", "t-5"]);
    expect(plan.deferredPriorityTaskIds).toEqual(["t-4"]);
  });

  it("会过滤空值和不存在的任务 id", () => {
    const plan = createConversationActivityStagePlan({
      taskIds: ["t-1", "", "t-2", "t-2"],
      initialTaskIds: ["", "missing", "t-2"],
      priorityTaskIds: ["missing", "t-1"],
    });

    expect(plan.initialTaskIds).toEqual(["t-2"]);
    expect(plan.initialPriorityTaskIds).toEqual([]);
    expect(plan.deferredTaskIds).toEqual(["t-1"]);
    expect(plan.deferredPriorityTaskIds).toEqual(["t-1"]);
  });
});

