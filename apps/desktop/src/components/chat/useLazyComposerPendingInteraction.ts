import {
  computed,
  effectScope,
  onBeforeUnmount,
  shallowRef,
  watch,
  type ComputedRef,
  type EffectScope,
  type WritableComputedRef,
} from "vue";
import {
  measurePerfAsync,
  measurePerfSync,
} from "../../utils/perf";
import type {
  ComposerPendingInteractionController,
  UseComposerPendingInteractionOptions,
} from "./useComposerPendingInteraction";

type ComposerPendingInteractionModule = typeof import("./useComposerPendingInteraction");

let pendingInteractionModuleLoad: Promise<ComposerPendingInteractionModule> | null = null;

async function loadPendingInteractionModule(): Promise<ComposerPendingInteractionModule> {
  if (!pendingInteractionModuleLoad) {
    pendingInteractionModuleLoad = measurePerfAsync(
      "chat-composer.pending-controller.module",
      () => import("./useComposerPendingInteraction"),
    );
  }
  return pendingInteractionModuleLoad;
}

function controllerReadonlyRef<T>(
  controller: ComputedRef<ComposerPendingInteractionController | null>,
  select: (value: ComposerPendingInteractionController) => { value: T },
  fallback: T,
): ComputedRef<T> {
  return computed(() => {
    const current = controller.value;
    return current ? select(current).value : fallback;
  });
}

function controllerWritableRef<T>(
  controller: ComputedRef<ComposerPendingInteractionController | null>,
  select: (value: ComposerPendingInteractionController) => { value: T },
  fallback: T,
): WritableComputedRef<T> {
  return computed({
    get() {
      const current = controller.value;
      return current ? select(current).value : fallback;
    },
    set(value) {
      const current = controller.value;
      if (current) select(current).value = value;
    },
  });
}

export function useLazyComposerPendingInteraction(options: UseComposerPendingInteractionOptions) {
  const controllerRef = shallowRef<ComposerPendingInteractionController | null>(null);
  const controller = computed(() => controllerRef.value);
  const pendingSource = computed(() => !!options.pendingAsk.value || !!options.toolConsent.value);
  let controllerScope: EffectScope | null = null;

  async function ensureController(): Promise<ComposerPendingInteractionController> {
    if (controllerRef.value) return controllerRef.value;
    const module = await loadPendingInteractionModule();
    if (controllerRef.value) return controllerRef.value;
    const scope = effectScope();
    const instance = measurePerfSync(
      "chat-composer.pending-controller.init",
      () => scope.run(() => module.useComposerPendingInteraction(options))!,
    );
    controllerScope = scope;
    controllerRef.value = instance;
    return instance;
  }

  watch(pendingSource, (active) => {
    if (active) {
      void ensureController();
    }
  }, { immediate: true });

  onBeforeUnmount(() => {
    controllerScope?.stop();
    controllerScope = null;
    controllerRef.value = null;
  });

  const activeAsk = controllerReadonlyRef(controller, (value) => value.activeAsk, null);
  const activeAskOptionId = controllerReadonlyRef(controller, (value) => value.activeAskOptionId, null);
  const activeToolConsent = controllerReadonlyRef(controller, (value) => value.activeToolConsent, null);
  const askDismissable = controllerReadonlyRef(controller, (value) => value.askDismissable, false);
  const askFocusedOption = controllerReadonlyRef(controller, (value) => value.askFocusedOption, null);
  const askHasPreview = controllerReadonlyRef(controller, (value) => value.askHasPreview, false);
  const askIndex = controllerReadonlyRef(controller, (value) => value.askIndex, 0);
  const askIsLast = controllerReadonlyRef(controller, (value) => value.askIsLast, false);
  const askIsPlanApproval = controllerReadonlyRef(controller, (value) => value.askIsPlanApproval, false);
  const askOptionsWithId = controllerReadonlyRef(controller, (value) => value.askOptionsWithId, [] as any[]);
  const askOtherSelected = controllerReadonlyRef(controller, (value) => value.askOtherSelected, false);
  const askQuestion = controllerReadonlyRef(controller, (value) => value.askQuestion, null);
  const askTitle = controllerReadonlyRef(controller, (value) => value.askTitle, "");
  const askTotal = controllerReadonlyRef(controller, (value) => value.askTotal, 0);
  const askUsesInputActions = controllerReadonlyRef(controller, (value) => value.askUsesInputActions, false);
  const canAskSubmit = controllerReadonlyRef(controller, (value) => value.canAskSubmit, false);
  const canGoPrev = controllerReadonlyRef(controller, (value) => value.canGoPrev, false);
  const hasEditableCommand = controllerReadonlyRef(controller, (value) => value.hasEditableCommand, false);
  const hasPending = computed(() =>
    controllerRef.value ? controllerRef.value.hasPending.value : pendingSource.value,
  );
  const pendingInteractionReady = computed(() =>
    !pendingSource.value || controllerRef.value !== null,
  );
  const hasPendingInputText = controllerReadonlyRef(controller, (value) => value.hasPendingInputText, false);
  const hasPendingPanel = controllerReadonlyRef(controller, (value) => value.hasPendingPanel, false);
  const inputPlaceholder = controllerReadonlyRef(
    controller,
    (value) => value.inputPlaceholder,
    "可向 agent 询问任何事，输入 @ 使用插件或提及文件",
  );
  const inputValue = controllerWritableRef(controller, (value) => value.inputValue, "");
  const isEditingToolCommand = controllerReadonlyRef(controller, (value) => value.isEditingToolCommand, false);
  const multiPicks = controllerReadonlyRef(controller, (value) => value.multiPicks, new Set<string>());
  const pendingEntryActionsKey = controllerReadonlyRef(controller, (value) => value.pendingEntryActionsKey, "none");
  const pendingText = controllerReadonlyRef(controller, (value) => value.pendingText, "");
  const pendingKey = controllerReadonlyRef(controller, (value) => value.pendingKey, "none");
  const singleFocus = controllerReadonlyRef(controller, (value) => value.singleFocus, null);
  const singlePick = controllerReadonlyRef(controller, (value) => value.singlePick, null);
  const toolCommandDraft = controllerWritableRef(controller, (value) => value.toolCommandDraft, "");
  const toolCommandIsEmpty = controllerReadonlyRef(controller, (value) => value.toolCommandIsEmpty, true);
  const toolDanger = controllerReadonlyRef(controller, (value) => value.toolDanger, false);
  const toolExpanded = controllerWritableRef(controller, (value) => value.toolExpanded, false);
  const toolHeadline = controllerReadonlyRef(controller, (value) => value.toolHeadline, "");
  const toolIcon = controllerReadonlyRef(controller, (value) => value.toolIcon, null);
  const toolInlinePreview = controllerReadonlyRef(controller, (value) => value.toolInlinePreview, null);
  const toolInputJson = controllerReadonlyRef(controller, (value) => value.toolInputJson, "");
  const toolSubmitting = controllerReadonlyRef(controller, (value) => value.toolSubmitting, null);
  const toolSubtitle = controllerReadonlyRef(controller, (value) => value.toolSubtitle, "");

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
    backAsk: () => controllerRef.value?.backAsk(),
    beginCommandEdit: () => controllerRef.value?.beginCommandEdit(),
    canAskSubmit,
    canGoPrev,
    cancelAsk: () => controllerRef.value?.cancelAsk(),
    cancelCommandEdit: () => controllerRef.value?.cancelCommandEdit(),
    clearOptionHighlight: () => controllerRef.value?.clearOptionHighlight(),
    confirmAskNo: () => controllerRef.value?.confirmAskNo(),
    decideToolConsent: (...args: Parameters<ComposerPendingInteractionController["decideToolConsent"]>) =>
      controllerRef.value?.decideToolConsent(...args),
    focusOption: (...args: Parameters<ComposerPendingInteractionController["focusOption"]>) =>
      controllerRef.value?.focusOption(...args),
    hasEditableCommand,
    hasPending,
    pendingInteractionReady,
    hasPendingInputText,
    hasPendingPanel,
    highlightOption: (...args: Parameters<ComposerPendingInteractionController["highlightOption"]>) =>
      controllerRef.value?.highlightOption(...args),
    inputPlaceholder,
    inputValue,
    isEditingToolCommand,
    modifyPlanApproval: () => controllerRef.value?.modifyPlanApproval(),
    multiPicks,
    pendingEntryActionsKey,
    pendingText,
    pendingKey,
    selectSingleOption: (...args: Parameters<ComposerPendingInteractionController["selectSingleOption"]>) =>
      controllerRef.value?.selectSingleOption(...args),
    singleFocus,
    singlePick,
    skipAsk: () => controllerRef.value?.skipAsk(),
    submitAsk: () => controllerRef.value?.submitAsk(),
    submitAskFreeform: (...args: Parameters<ComposerPendingInteractionController["submitAskFreeform"]>) =>
      controllerRef.value?.submitAskFreeform(...args),
    toggleMulti: (...args: Parameters<ComposerPendingInteractionController["toggleMulti"]>) =>
      controllerRef.value?.toggleMulti(...args),
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
