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
} from "@lilia/ui";
import { createLazyLoadState } from "@lilia/ui";
import type {
  ComposerPendingInteractionController,
  ComposerPendingEntryActionsMode,
  UseComposerPendingInteractionOptions,
} from "./useComposerPendingInteraction";

type ComposerPendingInteractionModule = typeof import("./useComposerPendingInteraction");
type ControllerMethodKey = {
  [K in keyof ComposerPendingInteractionController]:
    ComposerPendingInteractionController[K] extends (...args: any[]) => any ? K : never;
}[keyof ComposerPendingInteractionController];
type ControllerMethod<K extends ControllerMethodKey> = Extract<
  ComposerPendingInteractionController[K],
  (...args: any[]) => any
>;

const pendingInteractionModuleLoad = createLazyLoadState<ComposerPendingInteractionModule>(() =>
  measurePerfAsync(
    "chat-composer.pending-controller.module",
    () => import("./useComposerPendingInteraction"),
  )
);

async function loadPendingInteractionModule(): Promise<ComposerPendingInteractionModule> {
  return pendingInteractionModuleLoad.load();
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
  let controllerSeq = 0;
  let disposed = false;

  async function ensureController(): Promise<ComposerPendingInteractionController> {
    if (controllerRef.value) return controllerRef.value;
    if (disposed) throw new Error("composer pending interaction controller disposed");
    const seq = ++controllerSeq;
    const module = await loadPendingInteractionModule();
    if (controllerRef.value) return controllerRef.value;
    if (disposed || seq !== controllerSeq) {
      throw new Error("composer pending interaction controller disposed");
    }
    const scope = effectScope();
    const instance = measurePerfSync(
      "chat-composer.pending-controller.init",
      () => scope.run(() => module.useComposerPendingInteraction(options))!,
    );
    if (disposed || seq !== controllerSeq) {
      scope.stop();
      throw new Error("composer pending interaction controller disposed");
    }
    controllerScope = scope;
    controllerRef.value = instance;
    return instance;
  }

  watch(pendingSource, (active) => {
    if (active) {
      void ensureController().catch(() => undefined);
    }
  }, { immediate: true });

  onBeforeUnmount(() => {
    disposed = true;
    controllerSeq += 1;
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
  const autoDecisionText = controllerReadonlyRef(controller, (value) => value.autoDecisionText, "");
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
  const pendingEntryActionsMode = controllerReadonlyRef(
    controller,
    (value) => value.pendingEntryActionsMode,
    "none" satisfies ComposerPendingEntryActionsMode,
  );
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

  function controllerMethod<K extends ControllerMethodKey>(key: K) {
    return (...args: Parameters<ControllerMethod<K>>): ReturnType<ControllerMethod<K>> | undefined => {
      const method = controllerRef.value?.[key] as ((...args: any[]) => unknown) | undefined;
      return method?.apply(undefined, args as any[]) as ReturnType<ControllerMethod<K>> | undefined;
    };
  }

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
    backAsk: controllerMethod("backAsk"),
    beginCommandEdit: controllerMethod("beginCommandEdit"),
    canAskSubmit,
    canGoPrev,
    cancelAsk: controllerMethod("cancelAsk"),
    cancelCommandEdit: controllerMethod("cancelCommandEdit"),
    clearOptionHighlight: controllerMethod("clearOptionHighlight"),
    confirmAskNo: controllerMethod("confirmAskNo"),
    decideToolConsent: controllerMethod("decideToolConsent"),
    focusOption: controllerMethod("focusOption"),
    hasEditableCommand,
    hasPending,
    pendingInteractionReady,
    hasPendingInputText,
    hasPendingPanel,
    highlightOption: controllerMethod("highlightOption"),
    inputPlaceholder,
    inputValue,
    isEditingToolCommand,
    modifyPlanApproval: controllerMethod("modifyPlanApproval"),
    multiPicks,
    pendingEntryActionsMode,
    pendingEntryActionsKey,
    pendingText,
    pendingKey,
    selectSingleOption: controllerMethod("selectSingleOption"),
    singleFocus,
    singlePick,
    skipAsk: controllerMethod("skipAsk"),
    submitAsk: controllerMethod("submitAsk"),
    submitAskFreeform: controllerMethod("submitAskFreeform"),
    toggleMulti: controllerMethod("toggleMulti"),
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

