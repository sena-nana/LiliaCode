import type { ComputedRef, Ref } from "vue";
import type {
  ChatAttachment,
  ChatConversationReference,
  ChatRuntimeCommand,
  ChatWorkflow,
  LiliaReviewTarget,
  ProviderRuntimeOptions,
} from "@lilia/contracts";
import {
  createLiliaBatchApplyWorkflow,
  createLiliaCompactWorkflow,
  createLiliaFixSuggestionWorkflow,
  createLiliaGoalWorkflow,
  createLiliaReviewWorkflow,
  createSessionForkCommand,
} from "@lilia/contracts";
import type { LiliaBatchApplyInput } from "@lilia/contracts";
import type { PendingAgentAction } from "../../composables/pendingAgentActions";

export interface LiliaWorkflowSendAgentMessageInput {
  turn: {
    content: string;
    outgoingAttachments?: ChatAttachment[];
    outgoingConversationReferences?: ChatConversationReference[];
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
    outgoingConversationReferences: ChatConversationReference[],
    workflow: ChatWorkflow,
    clearComposerAttachments = false,
  ) {
    if (!canStartLiliaWorkflow()) return;
    try {
      await options.sendAgentMessage({
        turn: { content, outgoingAttachments, outgoingConversationReferences },
        workflow,
      });
      if (clearComposerAttachments) options.attachments.value = [];
    } catch {
      return;
    }
  }

  async function onStartLiliaReview(
    content: string,
    outgoingAttachments: ChatAttachment[],
    outgoingConversationReferences: ChatConversationReference[],
    target: LiliaReviewTarget,
  ) {
    const workflow = createLiliaReviewWorkflow(target, {
      instructions: content,
      delivery: "inline",
    });
    const instructions = content.trim();
    await sendLiliaWorkflow(
      workflow,
      instructions,
      outgoingAttachments,
      outgoingConversationReferences,
      true,
    );
  }

  async function onStartLiliaFixSuggestion(
    content: string,
    outgoingAttachments: ChatAttachment[],
    outgoingConversationReferences: ChatConversationReference[],
    target: LiliaReviewTarget,
  ) {
    const workflow = createLiliaFixSuggestionWorkflow(target, {
      instructions: content,
      mode: "suggest",
    });
    const instructions = content.trim();
    await sendLiliaWorkflow(
      workflow,
      instructions,
      outgoingAttachments,
      outgoingConversationReferences,
      true,
    );
  }

  async function sendLiliaWorkflow(
    workflow: ChatWorkflow,
    content = "",
    outgoingAttachments: ChatAttachment[] = [],
    outgoingConversationReferences: ChatConversationReference[] = [],
    clearComposerAttachments = false,
  ) {
    await sendHandledLiliaWorkflow(
      content,
      outgoingAttachments,
      outgoingConversationReferences,
      workflow,
      clearComposerAttachments,
    );
  }

  async function onStartLiliaCompact() {
    await sendLiliaWorkflow(createLiliaCompactWorkflow());
  }

  async function onStartSessionFork() {
    await options.sendAgentMessage({
      turn: {
        content: "",
        outgoingAttachments: [],
      },
      runtimeCommand: createSessionForkCommand(),
    });
  }

  async function onStartLiliaBatchApply(input: LiliaBatchApplyInput) {
    const sourceSummary = input.sourceSummary.trim();
    if (!sourceSummary) return;
    await sendLiliaWorkflow(createLiliaBatchApplyWorkflow({
      ...input,
      sourceSummary,
    }));
  }

  async function onSetLiliaGoal(objective: string) {
    const trimmed = objective.trim();
    if (!trimmed) return;
    await sendLiliaWorkflow(createLiliaGoalWorkflow("set", {
      objective: trimmed,
      status: "active",
      tokenBudget: null,
    }));
  }

  async function onRefreshLiliaGoal() {
    await sendLiliaWorkflow(createLiliaGoalWorkflow("refresh"));
  }

  async function onClearLiliaGoal() {
    await sendLiliaWorkflow(createLiliaGoalWorkflow("clear"));
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
  };
}
