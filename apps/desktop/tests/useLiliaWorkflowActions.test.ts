import { computed, ref } from "vue";
import { describe, expect, it } from "vitest";
import type {
  ChatComposerState,
  ChatRuntimeCommand,
  ChatWorkflow,
  ProviderRuntimeOptions,
} from "@lilia/contracts";
import { useLiliaWorkflowActions } from "../src/pages/taskDetail/useLiliaWorkflowActions";

function composerState(backend: ChatComposerState["backend"]): ChatComposerState {
  return {
    taskId: "task-1",
    backend,
    model: backend === "codex" ? "gpt-5.5" : "claude-sonnet-4-6",
    planMode: false,
    permission: "ask",
  };
}

function setupWorkflowActions(backend: ChatComposerState["backend"]) {
  const sent: ChatWorkflow[] = [];
  const runtimeSent: ChatRuntimeCommand[] = [];
  const runtimeOptionsSent: Array<ProviderRuntimeOptions | null | undefined> = [];
  const composer = ref<ChatComposerState | null>(composerState(backend));
  const actions = useLiliaWorkflowActions({
    hasContext: computed(() => true),
    composer,
    isTurnRunning: ref(false),
    blockingPendingAgentActions: computed(() => []),
    attachments: ref([]),
    sendAgentMessage: async (
      _content,
      _attachments,
      _guideId,
      workflow,
      runtimeCommand,
      runtimeOptions,
    ) => {
      if (workflow) sent.push(workflow);
      if (runtimeCommand) runtimeSent.push(runtimeCommand);
      runtimeOptionsSent.push(runtimeOptions);
    },
  });
  return { actions, sent, runtimeSent, runtimeOptionsSent };
}

describe("useLiliaWorkflowActions", () => {
  it("compact workflow uses the Lilia protocol for every backend", async () => {
    const codex = setupWorkflowActions("codex");
    await codex.actions.onStartLiliaCompact();
    expect(codex.sent).toEqual([{ type: "lilia_compact" }]);

    const claude = setupWorkflowActions("claude");
    await claude.actions.onStartLiliaCompact();
    expect(claude.sent).toEqual([{ type: "lilia_compact" }]);
  });

  it("fix suggestion workflow uses the Lilia protocol for every backend", async () => {
    const codex = setupWorkflowActions("codex");
    await codex.actions.onStartLiliaFixSuggestion("检查空状态", [], { type: "uncommittedChanges" });
    expect(codex.sent).toEqual([{
      type: "lilia_fix_suggestion",
      target: { type: "uncommittedChanges" },
      mode: "suggest",
      instructions: "检查空状态",
    }]);

    const claude = setupWorkflowActions("claude");
    await claude.actions.onStartLiliaFixSuggestion("", [], { type: "uncommittedChanges" });
    expect(claude.sent).toEqual([{
      type: "lilia_fix_suggestion",
      target: { type: "uncommittedChanges" },
      mode: "suggest",
    }]);
  });

  it("session fork runtime command uses the Lilia protocol for every backend", async () => {
    const codex = setupWorkflowActions("codex");
    await codex.actions.onStartSessionFork();
    expect(codex.runtimeSent).toEqual([{ type: "lilia_session_fork", excludeTurns: true }]);

    const claude = setupWorkflowActions("claude");
    await claude.actions.onStartSessionFork();
    expect(claude.runtimeSent).toEqual([{ type: "lilia_session_fork", excludeTurns: true }]);
  });

  it("provider settings runtime command is passed through for every backend", async () => {
    const command: ChatRuntimeCommand = {
      type: "lilia_provider_settings",
      action: "update",
    };
    const runtimeOptions: ProviderRuntimeOptions = {
      common: { model: "gpt-5.5", permission: "ask" },
      provider: {
        codex: { reasoningEffort: "high" },
        claude: { maxTurns: 4 },
      },
    };

    const codex = setupWorkflowActions("codex");
    await codex.actions.onApplyLiliaProviderSettings(command, runtimeOptions);
    expect(codex.runtimeSent).toEqual([command]);
    expect(codex.runtimeOptionsSent.at(-1)).toEqual(runtimeOptions);

    const claude = setupWorkflowActions("claude");
    await claude.actions.onApplyLiliaProviderSettings(command, runtimeOptions);
    expect(claude.runtimeSent).toEqual([command]);
    expect(claude.runtimeOptionsSent.at(-1)).toEqual(runtimeOptions);
  });
});
