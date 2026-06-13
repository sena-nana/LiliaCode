import type { ComputedRef, Ref } from "vue";
import type {
  ChatAttachment,
  ChatWorkflow,
  CodexReviewTarget,
} from "@lilia/contracts";
import type { CodexBatchApplyInput } from "../../components/chat/codexBatchApply";
import type { PendingAgentAction } from "../../composables/usePendingAgentActions";

export function useCodexWorkflowActions(options: {
  hasContext: ComputedRef<boolean>;
  isTurnRunning: Ref<boolean>;
  blockingPendingAgentActions: ComputedRef<PendingAgentAction[]>;
  attachments: Ref<ChatAttachment[]>;
  sendAgentMessage: (
    content: string,
    outgoingAttachments?: ChatAttachment[],
    guideId?: string,
    workflow?: ChatWorkflow | null,
  ) => Promise<void>;
}) {
  function canStartCodexWorkflow() {
    return options.hasContext.value &&
      !options.isTurnRunning.value &&
      options.blockingPendingAgentActions.value.length === 0;
  }

  async function sendHandledCodexWorkflow(
    content: string,
    outgoingAttachments: ChatAttachment[],
    workflow: ChatWorkflow,
    clearComposerAttachments = false,
  ) {
    if (!canStartCodexWorkflow()) return;
    try {
      await options.sendAgentMessage(content, outgoingAttachments, undefined, workflow);
      if (clearComposerAttachments) options.attachments.value = [];
    } catch {
      return;
    }
  }

  async function onStartCodexReview(
    content: string,
    outgoingAttachments: ChatAttachment[],
    target: CodexReviewTarget,
  ) {
    const workflow: ChatWorkflow = {
      type: "codex_review",
      target,
      delivery: "inline",
    };
    const instructions = content.trim();
    if (instructions) workflow.instructions = instructions;
    await sendHandledCodexWorkflow(instructions, outgoingAttachments, workflow, true);
  }

  async function onStartCodexFixSuggestion(
    content: string,
    outgoingAttachments: ChatAttachment[],
    target: CodexReviewTarget,
  ) {
    const workflow: ChatWorkflow = {
      type: "codex_fix_suggestion",
      target,
      mode: "suggest",
    };
    const instructions = content.trim();
    if (instructions) workflow.instructions = instructions;
    await sendHandledCodexWorkflow(instructions, outgoingAttachments, workflow, true);
  }

  async function sendCodexWorkflow(workflow: ChatWorkflow) {
    await sendHandledCodexWorkflow("", [], workflow);
  }

  async function onStartCodexCompact() {
    await sendCodexWorkflow({ type: "codex_compact" });
  }

  async function onStartCodexBatchApply(input: CodexBatchApplyInput) {
    const sourceSummary = input.sourceSummary.trim();
    if (!sourceSummary) return;
    await sendCodexWorkflow({
      type: "codex_batch_apply",
      sourceTurnId: input.sourceTurnId,
      sourceKind: input.sourceKind,
      sourceSummary,
    });
  }

  async function onSetCodexGoal(objective: string) {
    const trimmed = objective.trim();
    if (!trimmed) return;
    await sendCodexWorkflow({
      type: "codex_goal",
      action: "set",
      objective: trimmed,
      status: "active",
      tokenBudget: null,
    });
  }

  async function onRefreshCodexGoal() {
    await sendCodexWorkflow({
      type: "codex_goal",
      action: "refresh",
    });
  }

  async function onClearCodexGoal() {
    await sendCodexWorkflow({
      type: "codex_goal",
      action: "clear",
    });
  }

  return {
    onStartCodexReview,
    onStartCodexFixSuggestion,
    onStartCodexCompact,
    onStartCodexBatchApply,
    onSetCodexGoal,
    onRefreshCodexGoal,
    onClearCodexGoal,
  };
}
