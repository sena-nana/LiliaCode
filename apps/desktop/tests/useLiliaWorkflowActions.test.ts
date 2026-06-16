import { computed, ref } from "vue";
import { describe, expect, it } from "vitest";
import type {
  ChatRuntimeCommand,
  ChatWorkflow,
  ProviderRuntimeOptions,
} from "@lilia/contracts";
import { useLiliaWorkflowActions } from "../src/pages/taskDetail/useLiliaWorkflowActions";

function setupWorkflowActions() {
  const sent: ChatWorkflow[] = [];
  const runtimeSent: ChatRuntimeCommand[] = [];
  const runtimeOptionsSent: Array<ProviderRuntimeOptions | null | undefined> = [];
  const actions = useLiliaWorkflowActions({
    hasContext: computed(() => true),
    isTurnRunning: ref(false),
    blockingPendingAgentActions: computed(() => []),
    attachments: ref([]),
    sendAgentMessage: async (input) => {
      if (input.workflow) sent.push(input.workflow);
      if (input.runtimeCommand) runtimeSent.push(input.runtimeCommand);
      runtimeOptionsSent.push(input.runtimeOptions);
    },
  });
  return { actions, sent, runtimeSent, runtimeOptionsSent };
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

  it("provider settings is passed through as runtimeCommand", async () => {
    const command: ChatRuntimeCommand = {
      type: "runtime_settings",
      action: "update",
    };
    const runtimeOptions: ProviderRuntimeOptions = {
      common: { model: "gpt-5.5", permission: "ask" },
      provider: {
        codex: { reasoningEffort: "high" },
        claude: { maxTurns: 4 },
      },
    };

    const view = setupWorkflowActions();
    await view.actions.onApplyLiliaProviderSettings(command, runtimeOptions);
    expect(view.runtimeSent).toEqual([command]);
    expect(view.runtimeOptionsSent.at(-1)).toEqual(runtimeOptions);
    expect(view.sent).toEqual([]);
  });

  it("keeps runtime settings alias for existing callers", async () => {
    const command: ChatRuntimeCommand = {
      type: "runtime_settings",
      action: "diagnose",
    };

    const view = setupWorkflowActions();
    await view.actions.onApplyRuntimeSettings(command);
    expect(view.runtimeSent).toEqual([command]);
    expect(view.runtimeOptionsSent.at(-1)).toBeNull();
    expect(view.sent).toEqual([]);
  });
});
