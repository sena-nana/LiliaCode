import { computed, nextTick, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import type { AskUserResult } from "@lilia/contracts";
import { useAskUserInteraction } from "../../composables/useAskUserInteraction";
import { useEditableToolCommand } from "../../composables/useEditableToolCommand";
import type { PendingAsk } from "../../composables/useAskUser";
import { useToolConsentPresentation } from "../../composables/useToolConsentPresentation";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../../services/chat";

interface UseComposerPendingInteractionOptions {
  clearContextState: () => void;
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

export function useComposerPendingInteraction(options: UseComposerPendingInteractionOptions) {
  const pendingText = ref("");
  const toolExpanded = ref(false);
  const toolSubmitting = ref<ToolConsentDecision | null>(null);

  const activeAsk = computed(() => options.pendingAsk.value ?? null);
  const activeToolConsent = computed(() =>
    activeAsk.value ? null : options.toolConsent.value ?? null,
  );
  const hasPending = computed(() => !!activeAsk.value || !!activeToolConsent.value);
  const pendingKey = computed(() => {
    if (activeAsk.value) return `ask:${activeAsk.value.id}`;
    if (activeToolConsent.value) return `tool:${activeToolConsent.value.requestId}`;
    return "none";
  });

  const inputValue = computed({
    get: () => pendingText.value,
    set: (value: string) => {
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
    selectSingleOption,
    toggleMulti,
    submitAsk,
    submitAskFreeform,
    confirmAskNo,
    skipAsk,
    backAsk,
    cancelAsk,
  } = useAskUserInteraction(activeAsk, pendingText, options.resolveAsk);

  const hasPendingPanel = computed(() =>
    !!(activeAsk.value && askQuestion.value) || !!activeToolConsent.value,
  );
  const askUsesInputActions = computed(() =>
    !!activeAsk.value && askQuestion.value?.mode !== "confirm",
  );
  const pendingInputText = computed(() => pendingText.value.trim());
  const hasPendingInputText = computed(() => pendingInputText.value.length > 0);
  const pendingEntryActionsKey = computed(() => {
    if (askUsesInputActions.value) return "ask-input";
    if (askIsPlanApproval.value) return "ask-plan";
    if (activeAsk.value) return "ask-confirm";
    if (activeToolConsent.value) return "tool";
    return "none";
  });

  const {
    toolDanger,
    toolIcon,
    toolHeadline,
    toolInlinePreview,
    toolInputJson,
    toolSubtitle,
  } = useToolConsentPresentation(activeToolConsent);
  const {
    commandDraft: toolCommandDraft,
    isEditingCommand: isEditingToolCommand,
    hasEditableCommand,
    commandIsEmpty: toolCommandIsEmpty,
    updatedCommandInput,
    beginCommandEdit,
    cancelCommandEdit,
  } = useEditableToolCommand(activeToolConsent);

  const inputPlaceholder = computed(() => {
    if (activeToolConsent.value) return "输入拒绝理由，Enter 拒绝此次调用";
    if (activeAsk.value) {
      const q = askQuestion.value;
      if (askIsPlanApproval.value) return "输入修改要求，Enter 退回计划";
      if (q?.mode === "confirm") return "输入取消原因，Enter 返回给 Agent";
      return "补充其他回答";
    }
    return "可向 agent 询问任何事，输入 @ 使用插件或提及文件";
  });

  function modifyPlanApproval() {
    if (!hasPendingInputText.value) return;
    submitAskFreeform();
  }

  function decideToolConsent(decision: ToolConsentDecision, explicitMessage?: string) {
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

  watch(pendingKey, () => {
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
