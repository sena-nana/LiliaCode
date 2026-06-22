import { computed, ref } from "vue";
import { describe, expect, it } from "vitest";
import type {
  ChatRuntimeCommand,
  ChatWorkflow,
} from "@lilia/contracts";
import {
  LILIA_BATCH_APPLY_WORKFLOW_TYPE,
  LILIA_COMPACT_WORKFLOW_TYPE,
  LILIA_FIX_SUGGESTION_WORKFLOW_TYPE,
  LILIA_GOAL_WORKFLOW_TYPE,
  LILIA_REVIEW_WORKFLOW_TYPE,
  SESSION_FORK_COMMAND_TYPE,
} from "@lilia/contracts";
import { useLiliaWorkflowActions } from "../src/pages/taskDetail/useLiliaWorkflowActions";

function setupWorkflowActions() {
  const sent: ChatWorkflow[] = [];
  const runtimeSent: ChatRuntimeCommand[] = [];
  const actions = useLiliaWorkflowActions({
    hasContext: computed(() => true),
    isTurnRunning: ref(false),
    blockingPendingAgentActions: computed(() => []),
    attachments: ref([]),
    sendAgentMessage: async (input) => {
      if (input.workflow) sent.push(input.workflow);
      if (input.runtimeCommand) runtimeSent.push(input.runtimeCommand);
    },
  });
  return { actions, sent, runtimeSent };
}

describe("useLiliaWorkflowActions", () => {
  it("compact workflow uses the Lilia protocol", async () => {
    const view = setupWorkflowActions();
    await view.actions.onStartLiliaCompact();
    expect(view.sent).toEqual([{ type: LILIA_COMPACT_WORKFLOW_TYPE }]);
  });

  it("fix suggestion workflow uses the Lilia protocol", async () => {
    const view = setupWorkflowActions();
    await view.actions.onStartLiliaFixSuggestion("检查空状态", [], [], { type: "uncommittedChanges" });
    expect(view.sent).toEqual([{
      type: LILIA_FIX_SUGGESTION_WORKFLOW_TYPE,
      target: { type: "uncommittedChanges" },
      mode: "suggest",
      instructions: "检查空状态",
    }]);
  });

  it("review workflow uses the Lilia protocol", async () => {
    const view = setupWorkflowActions();
    await view.actions.onStartLiliaReview(" 检查回归 ", [], [], {
      type: "baseBranch",
      branch: " main ",
    });
    expect(view.sent).toEqual([{
      type: LILIA_REVIEW_WORKFLOW_TYPE,
      target: { type: "baseBranch", branch: "main" },
      delivery: "inline",
      instructions: "检查回归",
    }]);
  });

  it("batch apply workflow uses the Lilia protocol", async () => {
    const view = setupWorkflowActions();
    await view.actions.onStartLiliaBatchApply({
      sourceTurnId: " turn-1 ",
      sourceKind: "review",
      sourceSummary: " 应用建议 ",
    });
    expect(view.sent).toEqual([{
      type: LILIA_BATCH_APPLY_WORKFLOW_TYPE,
      sourceTurnId: "turn-1",
      sourceKind: "review",
      sourceSummary: "应用建议",
    }]);
  });

  it("session fork uses runtimeCommand", async () => {
    const view = setupWorkflowActions();
    await view.actions.onStartSessionFork();
    expect(view.runtimeSent).toEqual([{ type: SESSION_FORK_COMMAND_TYPE, excludeTurns: true, mode: "fork" }]);
    expect(view.sent).toEqual([]);
  });

  it("goal workflows use the Lilia protocol", async () => {
    const view = setupWorkflowActions();
    await view.actions.onSetLiliaGoal(" ship it ");
    await view.actions.onRefreshLiliaGoal();
    await view.actions.onClearLiliaGoal();
    expect(view.sent).toEqual([
      {
        type: LILIA_GOAL_WORKFLOW_TYPE,
        action: "set",
        objective: "ship it",
        status: "active",
        tokenBudget: null,
      },
      { type: LILIA_GOAL_WORKFLOW_TYPE, action: "refresh" },
      { type: LILIA_GOAL_WORKFLOW_TYPE, action: "clear" },
    ]);
  });

});
