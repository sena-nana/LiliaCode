import { computed, ref } from "vue";
import { describe, expect, it } from "vitest";
import type {
  ChatRuntimeCommand,
  ChatWorkflow,
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
    expect(view.sent).toEqual([{ type: "lilia_compact" }]);
  });

  it("fix suggestion workflow uses the Lilia protocol", async () => {
    const view = setupWorkflowActions();
    await view.actions.onStartLiliaFixSuggestion("检查空状态", [], { type: "uncommittedChanges" });
    expect(view.sent).toEqual([{
      type: "lilia_fix_suggestion",
      target: { type: "uncommittedChanges" },
      mode: "suggest",
      instructions: "检查空状态",
    }]);
  });

  it("session fork uses runtimeCommand", async () => {
    const view = setupWorkflowActions();
    await view.actions.onStartSessionFork();
    expect(view.runtimeSent).toEqual([{ type: "session_fork", excludeTurns: true }]);
    expect(view.sent).toEqual([]);
  });

});
