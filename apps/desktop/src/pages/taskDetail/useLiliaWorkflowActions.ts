import type { ComputedRef, Ref } from "vue";
import type {
  ChatAttachment,
  ChatRuntimeCommand,
  ChatWorkflow,
  LiliaReviewTarget,
  ProviderRuntimeOptions,
} from "@lilia/contracts";
import type { LiliaBatchApplyInput } from "../../components/chat/liliaBatchApply";
import type { PendingAgentAction } from "../../composables/usePendingAgentActions";

export interface LiliaWorkflowSendAgentMessageInput {
  turn: {
    content: string;
    outgoingAttachments?: ChatAttachment[];
    guideId?: string;
    titleContent?: string;
  };
  workflow?: ChatWorkflow | null;
  runtimeCommand?: ChatRuntimeCommand | null;
  runtimeOptions?: ProviderRuntimeOptions | null;
}

export function useLiliaWorkflowActions(options: {
  hasContext: ComputedRef<boolean>;
  isTurnRunning: Ref<boolean>;
  blockingPendingAgentActions: ComputedRef<PendingAgentAction[]>;
  attachments: Ref<ChatAttachment[]>;
  sendAgentMessage: (input: LiliaWorkflowSendAgentMessageInput) => Promise<void>;
}) {
  function canStartLiliaWorkflow() {
    return options.hasContext.value &&
      !options.isTurnRunning.value &&
      options.blockingPendingAgentActions.value.length === 0;
  }

  async function sendHandledLiliaWorkflow(
    content: string,
    outgoingAttachments: ChatAttachment[],
    workflow: ChatWorkflow,
    clearComposerAttachments = false,
  ) {
    if (!canStartLiliaWorkflow()) return;
    try {
      await options.sendAgentMessage({ turn: { content, outgoingAttachments }, workflow });
      if (clearComposerAttachments) options.attachments.value = [];
    } catch {
      return;
    }
  }

  async function onStartLiliaReview(
    content: string,
    outgoingAttachments: ChatAttachment[],
    target: LiliaReviewTarget,
  ) {
    const workflow: ChatWorkflow = {
      type: "lilia_review",
      target,
      delivery: "inline",
    };
    const instructions = content.trim();
    if (instructions) workflow.instructions = instructions;
    await sendLiliaWorkflow(workflow, instructions, outgoingAttachments, true);
  }

  async function onStartLiliaFixSuggestion(
    content: string,
    outgoingAttachments: ChatAttachment[],
    target: LiliaReviewTarget,
  ) {
    const workflow: ChatWorkflow = {
      type: "lilia_fix_suggestion",
      target,
      mode: "suggest",
    };
    const instructions = content.trim();
    if (instructions) workflow.instructions = instructions;
    await sendLiliaWorkflow(workflow, instructions, outgoingAttachments, true);
  }

  async function sendLiliaWorkflow(
    workflow: ChatWorkflow,
    content = "",
    outgoingAttachments: ChatAttachment[] = [],
    clearComposerAttachments = false,
  ) {
    await sendHandledLiliaWorkflow(
      content,
      outgoingAttachments,
      workflow,
      clearComposerAttachments,
    );
  }

  async function onStartLiliaCompact() {
    await sendLiliaWorkflow({ type: "lilia_compact" });
  }

  async function onStartSessionFork() {
    await options.sendAgentMessage({
      turn: {
        content: "",
        outgoingAttachments: [],
      },
      runtimeCommand: { type: "session_fork", excludeTurns: true },
    });
  }

  async function onStartLiliaBatchApply(input: LiliaBatchApplyInput) {
    const sourceSummary = input.sourceSummary.trim();
    if (!sourceSummary) return;
    await sendLiliaWorkflow({
      type: "lilia_batch_apply",
      sourceTurnId: input.sourceTurnId,
      sourceKind: input.sourceKind,
      sourceSummary,
    });
  }

  async function onSetLiliaGoal(objective: string) {
    const trimmed = objective.trim();
    if (!trimmed) return;
    await sendLiliaWorkflow({
      type: "lilia_goal",
      action: "set",
      objective: trimmed,
      status: "active",
      tokenBudget: null,
    });
  }

  async function onRefreshLiliaGoal() {
    await sendLiliaWorkflow({
      type: "lilia_goal",
      action: "refresh",
    });
  }

  async function onClearLiliaGoal() {
    await sendLiliaWorkflow({
      type: "lilia_goal",
      action: "clear",
    });
  }

  async function onApplyRuntimeSettings(
    runtimeCommand: Extract<ChatRuntimeCommand, { type: "runtime_settings" }>,
    runtimeOptions?: ProviderRuntimeOptions | null,
  ) {
    await options.sendAgentMessage({
      turn: {
        content: "",
        outgoingAttachments: [],
      },
      runtimeCommand,
      runtimeOptions: runtimeOptions ?? null,
    });
  }

  return {
    onStartLiliaReview,
    onStartLiliaFixSuggestion,
    onStartLiliaCompact,
    onStartSessionFork,
    onStartLiliaBatchApply,
    onSetLiliaGoal,
    onRefreshLiliaGoal,
    onClearLiliaGoal,
    onApplyRuntimeSettings,
  };
}
