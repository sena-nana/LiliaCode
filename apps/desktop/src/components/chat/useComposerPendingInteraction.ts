import { computed, nextTick, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import {
  DEFAULT_ASK_USER_MODE,
  TOOL_CONSENT_INTERACTION_KIND,
  type AskUserResult,
} from "@lilia/contracts";
import {
  pendingAgentActionAutoDecisionKey,
  pendingAgentActionAutoDecisionLabel,
  pendingAgentActionAutoResolution,
  pendingAgentActionResolutionSubmittingTarget,
  pendingAskAgentAction,
  toolConsentAgentAction,
  type PendingAgentAction,
  type PendingAgentActionAutoResolutionInput,
  type PendingAgentActionSubmittingTarget,
} from "../../composables/pendingAgentActions";
import { useAskUserInteraction } from "../../composables/useAskUserInteraction";
import { useEditableToolCommand } from "../../composables/useEditableToolCommand";
import {
  pendingAskInteractionKey,
  type PendingAsk,
} from "../../composables/useAskUser";
import { useToolConsentPresentation } from "../../composables/useToolConsentPresentation";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../../services/chat";
import {
  recommendedAskUserResult,
  useFreeImplementationCountdown,
} from "./freeImplementationMode";

export interface UseComposerPendingInteractionOptions {
  clearContextState: () => void;
  freeImplementation: ComputedRef<boolean>;
  pendingAsk: ComputedRef<PendingAsk | null | undefined>;
  queueResize: () => void;
  resolveAsk: (result: AskUserResult) => void;
  resolveToolConsent: (
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ) => void;
  toolConsent: ComputedRef<ToolConsentRequest | null | undefined>;
}

export function composerPendingInteractionKey(
  pendingAsk: PendingAsk | null | undefined,
  toolConsent: ToolConsentRequest | null | undefined,
): string {
  if (pendingAsk) return pendingAskInteractionKey(pendingAsk);
  if (toolConsent) return `tool:${toolConsent.requestId}`;
  return "none";
}

export type ComposerPendingEntryActionsMode =
  | "ask-input"
  | "ask-plan"
  | "ask-confirm"
  | "tool"
  | "none";

type ComposerPendingEntryActionsModeInput = {
  askUsesInputActions: boolean;
  askIsPlanApproval: boolean;
  hasAsk: boolean;
  hasToolConsent: boolean;
};

export function composerPendingEntryActionsMode(
  input: ComposerPendingEntryActionsModeInput,
): ComposerPendingEntryActionsMode {
  if (input.askUsesInputActions) return "ask-input";
  if (input.askIsPlanApproval) return "ask-plan";
  if (input.hasAsk) return "ask-confirm";
  if (input.hasToolConsent) return "tool";
  return "none";
}

export type ComposerPendingAutoResolution =
  | {
      target: "ask_user";
      submittingTarget: PendingAgentActionSubmittingTarget;
      result: AskUserResult;
    }
  | {
      target: "tool_consent";
      submittingTarget: PendingAgentActionSubmittingTarget;
      decision: ToolConsentDecision;
      message?: string;
      updatedInput?: ToolConsentUpdatedInput;
    };

export function composerPendingAutoResolution(
  action: PendingAgentAction | null,
  input: PendingAgentActionAutoResolutionInput,
): ComposerPendingAutoResolution | null {
  if (!action) return null;
  const resolution = pendingAgentActionAutoResolution(action, input);
  if (!resolution) return null;
  if ("result" in resolution) {
    return {
      target: "ask_user",
      submittingTarget: pendingAgentActionResolutionSubmittingTarget(resolution),
      result: resolution.result,
    };
  }
  if (resolution.kind === TOOL_CONSENT_INTERACTION_KIND) {
    return {
      target: "tool_consent",
      submittingTarget: pendingAgentActionResolutionSubmittingTarget(resolution),
      decision: resolution.decision,
      message: resolution.message,
      updatedInput: resolution.updatedInput,
    };
  }
  return null;
}

export function useComposerPendingInteraction(options: UseComposerPendingInteractionOptions) {
  const pendingText = ref("");
  const toolExpanded = ref(false);
  const toolSubmitting = ref<ToolConsentDecision | null>(null);

  const activeAsk = computed(() => options.pendingAsk.value ?? null);
  const activeToolConsent = computed(() =>
    activeAsk.value ? null : options.toolConsent.value ?? null,
  );
  const activePendingAction = computed(() => {
    if (activeAsk.value) return pendingAskAgentAction(activeAsk.value);
    if (activeToolConsent.value) return toolConsentAgentAction(activeToolConsent.value);
    return null;
  });
  const hasPending = computed(() => !!activeAsk.value || !!activeToolConsent.value);
  const pendingKey = computed(() =>
    composerPendingInteractionKey(activeAsk.value, activeToolConsent.value),
  );

  const inputValue = computed({
    get: () => pendingText.value,
    set: (value: string) => {
      cancelAutoDecision();
      pendingText.value = value;
    },
  });

  const {
    askIndex,
    askTotal,
    askQuestion,
    askDismissable,
    askIsLast,
    canGoPrev,
    askTitle,
    askIsPlanApproval,
    askOptionsWithId,
    askHasPreview,
    askFocusedOption,
    askOtherSelected,
    canAskSubmit,
    singleFocus,
    activeOptionId: activeAskOptionId,
    singlePick,
    multiPicks,
    focusOption,
    highlightOption,
    clearOptionHighlight,
    selectSingleOption: selectSingleOptionBase,
    toggleMulti: toggleMultiBase,
    submitAsk: submitAskBase,
    submitAskFreeform: submitAskFreeformBase,
    confirmAskNo: confirmAskNoBase,
    skipAsk: skipAskBase,
    backAsk: backAskBase,
    cancelAsk: cancelAskBase,
  } = useAskUserInteraction(activeAsk, pendingText, options.resolveAsk);

  const hasPendingPanel = computed(() =>
    !!(activeAsk.value && askQuestion.value) || !!activeToolConsent.value,
  );
  const askUsesInputActions = computed(() =>
    !!activeAsk.value && askQuestion.value?.mode !== DEFAULT_ASK_USER_MODE,
  );
  const pendingInputText = computed(() => pendingText.value.trim());
  const hasPendingInputText = computed(() => pendingInputText.value.length > 0);
  const pendingEntryActionsMode = computed(() =>
    composerPendingEntryActionsMode({
      askUsesInputActions: askUsesInputActions.value,
      askIsPlanApproval: askIsPlanApproval.value,
      hasAsk: !!activeAsk.value,
      hasToolConsent: !!activeToolConsent.value,
    }),
  );
  const pendingEntryActionsKey = computed(() => pendingEntryActionsMode.value);

  const {
    toolDanger,
    toolIcon,
    toolHeadline,
    toolInlinePreview,
    toolInputJson,
    toolSubtitle,
  } = useToolConsentPresentation(activeToolConsent);
  const {
    commandDraft: toolCommandDraftBase,
    isEditingCommand: isEditingToolCommand,
    hasEditableCommand,
    commandIsEmpty: toolCommandIsEmpty,
    updatedCommandInput,
    beginCommandEdit: beginCommandEditBase,
    cancelCommandEdit: cancelCommandEditBase,
  } = useEditableToolCommand(activeToolConsent);
  const toolCommandDraft = computed({
    get: () => toolCommandDraftBase.value,
    set: (value: string) => {
      if (value !== toolCommandDraftBase.value) cancelAutoDecision();
      toolCommandDraftBase.value = value;
    },
  });

  const inputPlaceholder = computed(() => {
    if (activeToolConsent.value) return "输入拒绝理由，Enter 拒绝此次调用";
    if (activeAsk.value) {
      const q = askQuestion.value;
      if (askIsPlanApproval.value) return "输入修改要求，Enter 退回计划";
      if (q?.mode === DEFAULT_ASK_USER_MODE) return "输入取消原因，Enter 返回给 Agent";
      return "补充其他回答";
    }
    return "可向 agent 询问任何事，输入 @ 使用插件或提及文件";
  });

  function modifyPlanApproval() {
    cancelAutoDecision();
    if (!hasPendingInputText.value) return;
    submitAskFreeformBase();
  }

  function decideToolConsent(
    decision: ToolConsentDecision,
    explicitMessage?: string,
    source: "manual" | "auto" = "manual",
  ) {
    if (source === "manual") cancelAutoDecision();
    const c = activeToolConsent.value;
    if (!c || toolSubmitting.value) return;
    if (decision === "allow" && toolCommandIsEmpty.value) return;
    toolSubmitting.value = decision;
    const message = decision === "deny"
      ? explicitMessage?.trim() || pendingInputText.value || "用户拒绝了此次工具调用"
      : undefined;
    const updatedInput = decision === "allow" ? updatedCommandInput.value : undefined;
    if (updatedInput) {
      options.resolveToolConsent(decision, message, updatedInput);
    } else {
      options.resolveToolConsent(decision, message);
    }
    if (decision === "deny") pendingText.value = "";
  }

  function runManualAction(action: () => void) {
    cancelAutoDecision();
    action();
  }

  function runManualActionWith<T>(action: (value: T) => void, value: T) {
    cancelAutoDecision();
    action(value);
  }

  function submitAsk() {
    runManualAction(submitAskBase);
  }

  function submitAskFreeform(value?: string) {
    runManualAction(() => submitAskFreeformBase(value));
  }

  function confirmAskNo() {
    runManualAction(confirmAskNoBase);
  }

  function skipAsk() {
    runManualAction(skipAskBase);
  }

  function backAsk() {
    runManualAction(backAskBase);
  }

  function cancelAsk() {
    runManualAction(cancelAskBase);
  }

  function selectSingleOption(id: string) {
    runManualActionWith(selectSingleOptionBase, id);
  }

  function toggleMulti(id: string) {
    runManualActionWith(toggleMultiBase, id);
  }

  function beginCommandEdit() {
    runManualAction(beginCommandEditBase);
  }

  function cancelCommandEdit() {
    runManualAction(cancelCommandEditBase);
  }

  function autoDecisionKeyForCurrentState(): string {
    const action = activePendingAction.value;
    if (!action) return "";
    return pendingAgentActionAutoDecisionKey(action, {
      askHasRecommendedResult: !!recommendedAskUserResult(activeAsk.value?.spec),
      askQuestionId: askQuestion.value?.id,
      editingToolCommand: isEditingToolCommand.value,
      toolDanger: toolDanger.value,
      toolSubmitting: !!toolSubmitting.value,
      toolCommandIsEmpty: toolCommandIsEmpty.value,
    });
  }

  const autoDecisionKey = computed(autoDecisionKeyForCurrentState);

  function autoDecisionLabelForCurrentState(): string {
    return activePendingAction.value
      ? pendingAgentActionAutoDecisionLabel(activePendingAction.value)
      : "";
  }

  function runAutoDecision() {
    const action = activePendingAction.value;
    const askResult = activeAsk.value
      ? recommendedAskUserResult(activeAsk.value.spec)
      : null;
    const resolution = composerPendingAutoResolution(action, {
      askHasRecommendedResult: !!askResult,
      askQuestionId: askQuestion.value?.id,
      askResult,
      editingToolCommand: isEditingToolCommand.value,
      toolCommandIsEmpty: toolCommandIsEmpty.value,
      toolDanger: toolDanger.value,
      toolSubmitting: !!toolSubmitting.value,
      toolUpdatedInput: updatedCommandInput.value,
    });
    if (!resolution) return;
    if (resolution.target === "ask_user") {
      options.resolveAsk(resolution.result);
      return;
    }
    if (resolution.submittingTarget === "tool") {
      toolSubmitting.value = resolution.decision;
    }
    options.resolveToolConsent(resolution.decision, resolution.message, resolution.updatedInput);
  }

  const {
    text: autoDecisionText,
    cancel: cancelAutoDecision,
    reset: resetAutoDecision,
  } = useFreeImplementationCountdown({
    enabled: options.freeImplementation,
    decisionKey: autoDecisionKey,
    decisionLabel: autoDecisionLabelForCurrentState,
    runDecision: runAutoDecision,
  });

  watch(pendingKey, () => {
    resetAutoDecision();
    if (!activeAsk.value) pendingText.value = "";
    toolExpanded.value = false;
    toolSubmitting.value = null;
    options.clearContextState();
    void nextTick(options.queueResize);
  }, { immediate: true });

  watch(
    () => askQuestion.value?.id,
    () => {
      void nextTick(options.queueResize);
    },
    { immediate: true },
  );

  return {
    activeAsk,
    activeAskOptionId,
    activeToolConsent,
    askDismissable,
    askFocusedOption,
    askHasPreview,
    askIndex,
    askIsLast,
    askIsPlanApproval,
    askOptionsWithId,
    askOtherSelected,
    askQuestion,
    askTitle,
    askTotal,
    askUsesInputActions,
    autoDecisionText,
    backAsk,
    beginCommandEdit,
    canAskSubmit,
    canGoPrev,
    cancelAsk,
    cancelCommandEdit,
    clearOptionHighlight,
    confirmAskNo,
    decideToolConsent,
    focusOption,
    hasEditableCommand,
    hasPending,
    hasPendingInputText,
    hasPendingPanel,
    highlightOption,
    inputPlaceholder,
    inputValue,
    isEditingToolCommand,
    modifyPlanApproval,
    multiPicks,
    pendingEntryActionsMode,
    pendingEntryActionsKey,
    pendingInputText,
    pendingKey,
    pendingText,
    selectSingleOption,
    singleFocus,
    singlePick,
    skipAsk,
    submitAsk,
    submitAskFreeform,
    toggleMulti,
    toolCommandDraft,
    toolCommandIsEmpty,
    toolDanger,
    toolExpanded,
    toolHeadline,
    toolIcon,
    toolInlinePreview,
    toolInputJson,
    toolSubmitting,
    toolSubtitle,
  };
}

export type ComposerPendingInteractionController = ReturnType<typeof useComposerPendingInteraction>;

