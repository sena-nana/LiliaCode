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
  it("session fork workflow uses the Lilia protocol for every backend", async () => {
    const codex = setupWorkflowActions("codex");
    await codex.actions.onStartSessionFork();
    expect(codex.sent).toEqual([{ type: "lilia_session_fork", excludeTurns: true }]);

    const claude = setupWorkflowActions("claude");
    await claude.actions.onStartSessionFork();
    expect(claude.sent).toEqual([{ type: "lilia_session_fork", excludeTurns: true }]);
  });
});
