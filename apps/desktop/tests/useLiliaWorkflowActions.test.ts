import { computed, ref } from "vue";
import { describe, expect, it } from "vitest";
import type { ChatComposerState, ChatWorkflow } from "@lilia/contracts";
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
  const composer = ref<ChatComposerState | null>(composerState(backend));
  const actions = useLiliaWorkflowActions({
    hasContext: computed(() => true),
    composer,
    isTurnRunning: ref(false),
    blockingPendingAgentActions: computed(() => []),
    attachments: ref([]),
    sendAgentMessage: async (_content, _attachments, _guideId, workflow) => {
      if (workflow) sent.push(workflow);
    },
  });
  return { actions, sent };
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

  it("session fork workflow uses the Lilia protocol for every backend", async () => {
    const codex = setupWorkflowActions("codex");
    await codex.actions.onStartSessionFork();
    expect(codex.sent).toEqual([{ type: "lilia_session_fork", excludeTurns: true }]);

    const claude = setupWorkflowActions("claude");
    await claude.actions.onStartSessionFork();
    expect(claude.sent).toEqual([{ type: "lilia_session_fork", excludeTurns: true }]);
  });

  it("provider settings workflow is passed through for every backend", async () => {
    const workflow: ChatWorkflow = {
      type: "lilia_provider_settings",
      action: "update",
      common: { model: "gpt-5.5", permission: "ask" },
      codex: { reasoningEffort: "high" },
      claude: { maxTurns: 4 },
    };

    const codex = setupWorkflowActions("codex");
    await codex.actions.onApplyLiliaProviderSettings(workflow);
    expect(codex.sent).toEqual([workflow]);

    const claude = setupWorkflowActions("claude");
    await claude.actions.onApplyLiliaProviderSettings(workflow);
    expect(claude.sent).toEqual([workflow]);
  });
});
