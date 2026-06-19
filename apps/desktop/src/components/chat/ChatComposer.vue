<script setup lang="ts">
/**
 * Composer：富文本输入 + pending Agent 交互 + 工具栏组合层。
 */

import {
  computed,
  defineAsyncComponent,
  effectScope,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  watch,
  type Component,
  type EffectScope,
  type ShallowRef,
} from "vue";
import {
  ArrowUp,
  FileText,
  Folder,
  Image,
  Paperclip,
  Square,
} from "lucide-vue-next";
import type {
  AskUserResult,
  ChatAttachment,
  ChatComposerState,
  ChatConversationReference,
  ChatContextSearchResult,
  ChatContextUsage,
  ChatModelOption,
  ChatSlashCommandWorkflow,
  LiliaReviewTarget,
  PermissionMode,
} from "@lilia/contracts";
import type { PendingAsk } from "../../composables/useAskUser";
import type { SearchResult } from "../../services/sessionSearch";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../../services/chat";
import { previewAutoModelSelection } from "../../services/modelSelection";
import ComposerRichInput from "./ComposerRichInput.vue";
import { textPart } from "./composerParts";
import {
  contextInlinePath,
  readContextMentionRange,
  readConversationReferenceRange,
  readSlashCommandRange,
} from "./composerTriggerRanges";
import { pasteHasFileItems, pastedPlainText } from "./composerPasteEvent";
import { useComposerRichInput } from "./useComposerRichInput";
import {
  imageViewerSourceFromAttachment,
  isImageAttachment,
  type ChatImageViewerSource,
} from "./imageViewer";
import { useLazyComposerPendingInteraction } from "./useLazyComposerPendingInteraction";
import type {
  ComposerSlashCommandItem,
  ComposerSlashTargetItem,
  ComposerWorkflowSlashKind,
} from "./useComposerSlashCommands";
import {
  cancelIdleRun,
  measurePerfAsync,
  measurePerfSync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../utils/perf";

const ComposerContextPanel = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "chat-composer.context-panel.load",
    async () => (await import("./ComposerContextPanel.vue")).default,
  ),
});

const ComposerConversationReferencePanel = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "chat-composer.conversation-panel.load",
    async () => (await import("./ComposerConversationReferencePanel.vue")).default,
  ),
});

const ComposerPendingPanel = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "chat-composer.pending-panel.load",
    async () => (await import("./ComposerPendingPanel.vue")).default,
  ),
});

const ComposerSlashCommandPanel = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "chat-composer.slash-panel.load",
    async () => (await import("./ComposerSlashCommandPanel.vue")).default,
  ),
});

let composerToolbarLoad: Promise<Component> | null = null;
let composerPendingEntryActionsLoad: Promise<Component> | null = null;
let composerConversationSearchModuleLoad:
  Promise<typeof import("./useComposerConversationSearch")> | null = null;
let composerContextSearchModuleLoad:
  Promise<typeof import("./useComposerContextSearch")> | null = null;
let composerSlashCommandsModuleLoad:
  Promise<typeof import("./useComposerSlashCommands")> | null = null;
let composerPasteModuleLoad: Promise<typeof import("./useComposerPaste")> | null = null;

async function loadComposerToolbar(): Promise<Component> {
  if (!composerToolbarLoad) {
    composerToolbarLoad = measurePerfAsync(
      "chat-composer.toolbar.load",
      async () => (await import("./ComposerToolbar.vue")).default,
    );
  }
  return composerToolbarLoad;
}

async function loadComposerPendingEntryActions(): Promise<Component> {
  if (!composerPendingEntryActionsLoad) {
    composerPendingEntryActionsLoad = measurePerfAsync(
      "chat-composer.pending-entry-actions.load",
      async () => (await import("./ComposerPendingEntryActions.vue")).default,
    );
  }
  return composerPendingEntryActionsLoad;
}

async function loadComposerConversationSearchModule() {
  if (!composerConversationSearchModuleLoad) {
    composerConversationSearchModuleLoad = measurePerfAsync(
      "chat-composer.conversation-search.module.load",
      async () => await import("./useComposerConversationSearch"),
    );
  }
  return composerConversationSearchModuleLoad;
}

async function loadComposerContextSearchModule() {
  if (!composerContextSearchModuleLoad) {
    composerContextSearchModuleLoad = measurePerfAsync(
      "chat-composer.context-search.module.load",
      async () => await import("./useComposerContextSearch"),
    );
  }
  return composerContextSearchModuleLoad;
}

async function loadComposerSlashCommandsModule() {
  if (!composerSlashCommandsModuleLoad) {
    composerSlashCommandsModuleLoad = measurePerfAsync(
      "chat-composer.slash-search.module.load",
      async () => await import("./useComposerSlashCommands"),
    );
  }
  return composerSlashCommandsModuleLoad;
}

async function loadComposerPasteModule() {
  if (!composerPasteModuleLoad) {
    composerPasteModuleLoad = measurePerfAsync(
      "chat-composer.paste.module.load",
      async () => await import("./useComposerPaste"),
    );
  }
  return composerPasteModuleLoad;
}

const props = defineProps<{
  state: ChatComposerState;
  modelOptions?: ChatModelOption[];
  attachments?: ChatAttachment[];
  appendAttachmentsToEndKey?: number;
  projectCwd?: string | null;
  /** 上一轮还在 streaming 时为 true，发送会进入调度队列。 */
  sending?: boolean;
  compactDisabled?: boolean;
  contextUsage?: ChatContextUsage | null;
  pendingAsk?: PendingAsk | null;
  toolConsent?: ToolConsentRequest | null;
  restoreDraftKey?: number;
  restoreDraftContent?: string;
  restoreDraftConversationReferences?: ChatConversationReference[];
  insertDraftTextKey?: number;
  insertDraftTextContent?: string;
}>();

const emit = defineEmits<{
  send: [content: string, attachments: ChatAttachment[], conversationReferences: ChatConversationReference[]];
  "start-lilia-review": [
    content: string,
    attachments: ChatAttachment[],
    conversationReferences: ChatConversationReference[],
    target: LiliaReviewTarget,
  ];
  "start-lilia-fix-suggestion": [
    content: string,
    attachments: ChatAttachment[],
    conversationReferences: ChatConversationReference[],
    target: LiliaReviewTarget,
  ];
  "start-lilia-compact": [];
  "execute-slash-command": [workflow: ChatSlashCommandWorkflow];
  "update:state": [next: ChatComposerState];
  "remove-attachment": [attachmentId: string];
  "pick-attachments": [];
  "add-context-attachment": [attachment: ChatAttachment];
  "resolve-ask-user": [result: AskUserResult];
  "resolve-tool-consent": [
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ];
  "open-image": [image: ChatImageViewerSource];
  "draft-empty-change": [empty: boolean];
  interrupt: [];
}>();

const COMPOSER_INPUT_LINE_HEIGHT = 22;
const COMPOSER_INPUT_VERTICAL_PADDING = 8;
const COMPOSER_INPUT_MAX_ROWS = 3;
const COMPOSER_INPUT_MIN_HEIGHT = COMPOSER_INPUT_LINE_HEIGHT + COMPOSER_INPUT_VERTICAL_PADDING;
const COMPOSER_INPUT_MAX_HEIGHT =
  COMPOSER_INPUT_LINE_HEIGHT * COMPOSER_INPUT_MAX_ROWS + COMPOSER_INPUT_VERTICAL_PADDING;
const COMPOSER_INPUT_TRANSITION_MS = 160;
const COMPOSER_ACTION_BLOCK_MS = 320;
const LARGE_DIRECTORY_FILE_COUNT = 200;
const LARGE_DIRECTORY_TOTAL_SIZE = 20 * 1024 * 1024;
const DIRECT_PASTE_TEXT_THRESHOLD = 2000;
const EMPTY_CONVERSATION_RESULTS: SearchResult[] = [];
const EMPTY_CONTEXT_RESULTS: ChatContextSearchResult[] = [];
const EMPTY_SLASH_RESULTS: ComposerSlashCommandItem[] = [];
const EMPTY_SLASH_TARGETS: ComposerSlashTargetItem[] = [];

type ValueRef<T> = { value: T };
type ConversationSearchController = {
  panelOpen: ValueRef<boolean>;
  results: ValueRef<SearchResult[]>;
  activeIndex: ValueRef<number>;
  loading: ValueRef<boolean>;
  error: ValueRef<string | null>;
  handleKeydown: (event: KeyboardEvent) => boolean;
  selectResult: (result: SearchResult | null) => void;
  activateResult: (index: number) => void;
  clear: () => void;
  resetSuppression: () => void;
  noteInputChanged: () => void;
};
type ContextSearchController = {
  panelOpen: ValueRef<boolean>;
  results: ValueRef<ChatContextSearchResult[]>;
  activeIndex: ValueRef<number>;
  loading: ValueRef<boolean>;
  error: ValueRef<string | null>;
  missingPath: ValueRef<string | null>;
  showMissingPath: ValueRef<boolean>;
  handleKeydown: (event: KeyboardEvent) => boolean;
  selectResult: (result: ChatContextSearchResult | null) => void;
  activateResult: (index: number) => void;
  clear: () => void;
  resetSuppression: () => void;
  noteInputChanged: () => void;
};
type SlashCommandsController = {
  panelOpen: ValueRef<boolean>;
  results: ValueRef<ComposerSlashCommandItem[]>;
  targetItems: ComposerSlashTargetItem[];
  activeWorkflowKind: ValueRef<ComposerWorkflowSlashKind | null>;
  activeIndex: ValueRef<number>;
  loading: ValueRef<boolean>;
  error: ValueRef<string | null>;
  handleKeydown: (event: KeyboardEvent) => boolean;
  selectResult: (result?: ComposerSlashCommandItem | null) => void;
  selectTarget: (item?: ComposerSlashTargetItem | null) => void;
  activateResult: (index: number) => void;
  clear: () => void;
  noteInputChanged: () => void;
};
type SpecialInputControllerKind = "conversation" | "context" | "slash";

function composerPerfDetail(state: ChatComposerState) {
  return `${state.taskId}:${state.backend}`;
}

const textarea = ref<HTMLTextAreaElement | null>(null);
const textareaMeasure = ref<HTMLTextAreaElement | null>(null);
const actionsBlocked = ref(false);
let resizeFrameId: number | null = null;
let overflowTimerId: number | null = null;
let actionBlockTimerId: number | null = null;
let warmToolbarHandle: number | null = null;
let composerDisposed = false;
let composerChromeRequested = false;
let specialInputRefreshScheduled = false;
let specialInputRefreshSeq = 0;
const composerToolbarComponent: ShallowRef<Component | null> = shallowRef(null);
const composerPendingEntryActionsComponent: ShallowRef<Component | null> = shallowRef(null);
let conversationSearchScope: EffectScope | null = null;
let contextSearchScope: EffectScope | null = null;
let slashCommandsScope: EffectScope | null = null;
let conversationSearchControllerLoad: Promise<ConversationSearchController> | null = null;
let contextSearchControllerLoad: Promise<ContextSearchController> | null = null;
let slashCommandsControllerLoad: Promise<SlashCommandsController> | null = null;
const specialInputControllerIntentSeq: Record<SpecialInputControllerKind, number> = {
  conversation: 0,
  context: 0,
  slash: 0,
};
const conversationSearchController = shallowRef<ConversationSearchController | null>(null);
const contextSearchController = shallowRef<ContextSearchController | null>(null);
const slashCommandsController = shallowRef<SlashCommandsController | null>(null);
let composerPasteLoad: Promise<{ onPaste: (event: ClipboardEvent) => void }> | null = null;
let composerPasteHandler: { onPaste: (event: ClipboardEvent) => void } | null = null;

let clearComposerContextState = () => {};

const pendingInteractionController = measurePerfSync(
  "chat-composer.setup.pending",
  () => useLazyComposerPendingInteraction({
    clearContextState: () => clearComposerContextState(),
    pendingAsk: computed(() => props.pendingAsk),
    queueResize,
    resolveAsk: (result) => emit("resolve-ask-user", result),
    resolveToolConsent: (decision, message, updatedInput) => {
      if (updatedInput) {
        emit("resolve-tool-consent", decision, message, updatedInput);
      } else {
        emit("resolve-tool-consent", decision, message);
      }
    },
    toolConsent: computed(() => props.toolConsent),
  }),
  { detail: composerPerfDetail(props.state) },
);

const {
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
  pendingInteractionReady,
  hasPendingInputText,
  hasPendingPanel,
  highlightOption,
  inputPlaceholder,
  inputValue,
  isEditingToolCommand,
  modifyPlanApproval,
  multiPicks,
  pendingEntryActionsKey,
  pendingText,
  pendingKey,
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
} = pendingInteractionController;

const attachmentsForView = computed(() => props.attachments ?? []);
const restoreDraftConversationReferences = computed(() => props.restoreDraftConversationReferences ?? []);
const appendAttachmentsToEndKey = computed(() => props.appendAttachmentsToEndKey ?? 0);
const suppressedAttachmentIds = ref<Set<string>>(new Set());
const unsuppressedAttachmentsForSend = computed(() =>
  attachmentsForView.value.filter((attachment) => !suppressedAttachmentIds.value.has(attachment.id))
);
const contextUsageForToolbar = computed(() =>
  props.contextUsage?.backend === props.state.backend ? props.contextUsage : null,
);

const richInput = measurePerfSync(
  "chat-composer.setup.rich-input",
  () => useComposerRichInput({
    attachments: attachmentsForView,
    appendAttachmentsToEndKey,
    hasPending,
    isLargeDirectory,
    removeAttachment: (attachmentId) => emit("remove-attachment", attachmentId),
  }),
  { detail: composerPerfDetail(props.state) },
);

async function ensureConversationSearchLoaded() {
  if (conversationSearchController.value) return conversationSearchController.value;
  if (!conversationSearchControllerLoad) {
    conversationSearchControllerLoad = measurePerfAsync(
      "chat-composer.conversation-search.controller.load",
      async () => {
        const module = await loadComposerConversationSearchModule();
        const scope = effectScope();
        const controller = scope.run(() => module.useComposerConversationSearch({
          richInput,
          hasPending,
        }));
        if (!controller) {
          scope.stop();
          throw new Error("failed to initialize conversation search");
        }
        if (composerDisposed) {
          scope.stop();
          return controller;
        }
        conversationSearchScope?.stop();
        conversationSearchScope = scope;
        conversationSearchController.value = controller;
        controller.noteInputChanged();
        return controller;
      },
    ).catch((error) => {
      conversationSearchControllerLoad = null;
      throw error;
    });
  }
  return conversationSearchControllerLoad;
}

async function ensureContextSearchLoaded() {
  if (contextSearchController.value) return contextSearchController.value;
  if (!contextSearchControllerLoad) {
    contextSearchControllerLoad = measurePerfAsync(
      "chat-composer.context-search.controller.load",
      async () => {
        const module = await loadComposerContextSearchModule();
        const scope = effectScope();
        const controller = scope.run(() => module.useComposerContextSearch({
          richInput,
          projectCwd: computed(() => props.projectCwd),
          hasPending,
          addContextAttachment: (attachment) => emit("add-context-attachment", attachment),
        }));
        if (!controller) {
          scope.stop();
          throw new Error("failed to initialize context search");
        }
        if (composerDisposed) {
          scope.stop();
          return controller;
        }
        contextSearchScope?.stop();
        contextSearchScope = scope;
        contextSearchController.value = controller;
        controller.noteInputChanged();
        return controller;
      },
    ).catch((error) => {
      contextSearchControllerLoad = null;
      throw error;
    });
  }
  return contextSearchControllerLoad;
}

async function ensureSlashCommandsLoaded() {
  if (slashCommandsController.value) return slashCommandsController.value;
  if (!slashCommandsControllerLoad) {
    slashCommandsControllerLoad = measurePerfAsync(
      "chat-composer.slash-search.controller.load",
      async () => {
        const module = await loadComposerSlashCommandsModule();
        const scope = effectScope();
        const controller = scope.run(() => module.useComposerSlashCommands({
          richInput,
          projectCwd: computed(() => props.projectCwd),
          hasPending,
          executeCommand: (workflow) => {
            emit("execute-slash-command", workflow);
            richInput.resetInput();
            clearComposerContextState();
          },
          startWorkflow: (kind, target) =>
            kind === "review"
              ? startLiliaReview(target)
              : startLiliaFixSuggestion(target),
        }));
        if (!controller) {
          scope.stop();
          throw new Error("failed to initialize slash commands");
        }
        if (composerDisposed) {
          scope.stop();
          return controller;
        }
        slashCommandsScope?.stop();
        slashCommandsScope = scope;
        slashCommandsController.value = controller;
        controller.noteInputChanged();
        return controller;
      },
    ).catch((error) => {
      slashCommandsControllerLoad = null;
      throw error;
    });
  }
  return slashCommandsControllerLoad;
}

const {
  conversationSearch,
  contextSearch,
  slashCommands,
} = measurePerfSync(
  "chat-composer.setup.controller-proxies",
  () => ({
    conversationSearch: {
      panelOpen: computed(() => conversationSearchController.value?.panelOpen.value ?? false),
      results: computed(() => conversationSearchController.value?.results.value ?? EMPTY_CONVERSATION_RESULTS),
      activeIndex: computed(() => conversationSearchController.value?.activeIndex.value ?? 0),
      loading: computed(() => conversationSearchController.value?.loading.value ?? false),
      error: computed(() => conversationSearchController.value?.error.value ?? null),
      activateResult: (index: number) => conversationSearchController.value?.activateResult(index),
      selectResult: (result?: SearchResult | null) => conversationSearchController.value?.selectResult(result ?? null),
      clear: () => conversationSearchController.value?.clear(),
      resetSuppression: () => conversationSearchController.value?.resetSuppression(),
      noteInputChanged: () => conversationSearchController.value?.noteInputChanged(),
      handleKeydown: (event: KeyboardEvent) => conversationSearchController.value?.handleKeydown(event) ?? false,
    },
    contextSearch: {
      panelOpen: computed(() => contextSearchController.value?.panelOpen.value ?? false),
      results: computed(() => contextSearchController.value?.results.value ?? EMPTY_CONTEXT_RESULTS),
      activeIndex: computed(() => contextSearchController.value?.activeIndex.value ?? 0),
      loading: computed(() => contextSearchController.value?.loading.value ?? false),
      error: computed(() => contextSearchController.value?.error.value ?? null),
      missingPath: computed(() => contextSearchController.value?.missingPath.value ?? null),
      showMissingPath: computed(() => contextSearchController.value?.showMissingPath.value ?? false),
      activateResult: (index: number) => contextSearchController.value?.activateResult(index),
      selectResult: (result?: ChatContextSearchResult | null) => contextSearchController.value?.selectResult(result ?? null),
      clear: () => contextSearchController.value?.clear(),
      resetSuppression: () => contextSearchController.value?.resetSuppression(),
      noteInputChanged: () => contextSearchController.value?.noteInputChanged(),
      handleKeydown: (event: KeyboardEvent) => contextSearchController.value?.handleKeydown(event) ?? false,
    },
    slashCommands: {
      panelOpen: computed(() => slashCommandsController.value?.panelOpen.value ?? false),
      results: computed(() => slashCommandsController.value?.results.value ?? EMPTY_SLASH_RESULTS),
      targetItems: computed(() => slashCommandsController.value?.targetItems ?? EMPTY_SLASH_TARGETS),
      activeWorkflowKind: computed(
        () => slashCommandsController.value?.activeWorkflowKind.value ?? null,
      ),
      activeIndex: computed(() => slashCommandsController.value?.activeIndex.value ?? 0),
      loading: computed(() => slashCommandsController.value?.loading.value ?? false),
      error: computed(() => slashCommandsController.value?.error.value ?? null),
      activateResult: (index: number) => slashCommandsController.value?.activateResult(index),
      selectResult: (result?: ComposerSlashCommandItem | null) => slashCommandsController.value?.selectResult(result),
      selectTarget: (item?: ComposerSlashTargetItem | null) => slashCommandsController.value?.selectTarget(item),
      clear: () => slashCommandsController.value?.clear(),
      noteInputChanged: () => slashCommandsController.value?.noteInputChanged(),
      handleKeydown: (event: KeyboardEvent) => slashCommandsController.value?.handleKeydown(event) ?? false,
    },
  }),
  { detail: composerPerfDetail(props.state) },
);

clearComposerContextState = () => {
  conversationSearch.clear();
  conversationSearch.resetSuppression();
  contextSearch.clear();
  contextSearch.resetSuppression();
  slashCommands.clear();
};

async function ensureComposerPasteLoaded() {
  if (composerPasteHandler) return composerPasteHandler;
  if (!composerPasteLoad) {
    composerPasteLoad = measurePerfAsync(
      "chat-composer.paste.handler.load",
      async () => {
        const module = await loadComposerPasteModule();
        const handler = module.useComposerPaste({
          richInput,
          clearContextSearch: clearComposerContextState,
          addContextAttachment: (attachment) => emit("add-context-attachment", attachment),
          hasPending: () => hasPending.value,
        });
        composerPasteHandler = handler;
        return handler;
      },
    ).catch((error) => {
      composerPasteLoad = null;
      throw error;
    });
  }
  return composerPasteLoad;
}

const previewAttachments = computed(() => attachmentsForView.value.filter(isImageAttachment));
const autoModelPreview = computed(() =>
  previewAutoModelSelection({
    backend: props.state.backend,
    modelOptions: props.modelOptions ?? [],
    composer: props.state,
    prompt: hasPending.value ? inputValue.value : richInput.serializedText.value,
    attachments: attachmentsForView.value,
    conversationReferences: richInput.conversationReferences.value,
    contextUsage: contextUsageForToolbar.value,
    workflow: null,
    runtimeCommand: null,
  }),
);

function invalidateSpecialInputControllerLoad(kind: SpecialInputControllerKind) {
  specialInputControllerIntentSeq[kind] += 1;
}

function scheduleSpecialInputControllerLoad(
  kind: SpecialInputControllerKind,
  text: string,
  cursor: number,
  shouldLoad: (text: string, cursor: number) => boolean,
  load: () => Promise<unknown>,
) {
  const seq = ++specialInputControllerIntentSeq[kind];
  if (composerDisposed || hasPending.value || seq !== specialInputControllerIntentSeq[kind]) return;
  if (!shouldLoad(text, cursor)) return;
  void load();
}

function maybeLoadSpecialInputControllers() {
  if (hasPending.value) return;
  const text = richInput.searchText.value;
  const cursor = richInput.inputSelection.value;
  if (readConversationReferenceRange(text, cursor)) {
    scheduleSpecialInputControllerLoad(
      "conversation",
      text,
      cursor,
      (nextText, nextCursor) => !!readConversationReferenceRange(nextText, nextCursor),
      ensureConversationSearchLoaded,
    );
  } else {
    invalidateSpecialInputControllerLoad("conversation");
  }
  if (readContextMentionRange(text, cursor)) {
    scheduleSpecialInputControllerLoad(
      "context",
      text,
      cursor,
      (nextText, nextCursor) => !!readContextMentionRange(nextText, nextCursor),
      ensureContextSearchLoaded,
    );
  } else {
    invalidateSpecialInputControllerLoad("context");
  }
  if (readSlashCommandRange(text, cursor)) {
    scheduleSpecialInputControllerLoad(
      "slash",
      text,
      cursor,
      (nextText, nextCursor) => !!readSlashCommandRange(nextText, nextCursor),
      ensureSlashCommandsLoaded,
    );
  } else {
    invalidateSpecialInputControllerLoad("slash");
  }
}

function scheduleSpecialInputControllerRefresh() {
  specialInputRefreshSeq += 1;
  if (specialInputRefreshScheduled) return;
  specialInputRefreshScheduled = true;
  scheduleAfterPaint(() => {
    specialInputRefreshScheduled = false;
    if (composerDisposed || hasPending.value) return;
    const refreshSeq = specialInputRefreshSeq;
    measurePerfSync(
      "chat-composer.special-input.refresh",
      maybeLoadSpecialInputControllers,
      { detail: composerPerfDetail(props.state) },
    );
    if (refreshSeq !== specialInputRefreshSeq) {
      scheduleSpecialInputControllerRefresh();
    }
  });
}

const canSend = computed(() => {
  if (activeToolConsent.value || activeAsk.value) return pendingText.value.trim().length > 0;
  return richInput.plainText.value.trim().length > 0 ||
    richInput.conversationReferences.value.length > 0 ||
    unsuppressedAttachmentsForSend.value.length > 0;
});

const canInterrupt = computed(() =>
  props.sending === true &&
  !hasPending.value &&
  !canSend.value,
);

const canSubmitEntry = computed(() => canSend.value || canInterrupt.value);
const hasPendingComposerUi = computed(() => hasPending.value && pendingInteractionReady.value);
const toolIconForPanel = computed<Component>(() => toolIcon.value ?? FileText);

async function ensureComposerToolbarLoaded() {
  if (composerToolbarComponent.value) return composerToolbarComponent.value;
  const component = await loadComposerToolbar();
  if (!composerDisposed) composerToolbarComponent.value = component;
  return component;
}

async function ensureComposerPendingEntryActionsLoaded() {
  if (composerPendingEntryActionsComponent.value) return composerPendingEntryActionsComponent.value;
  const component = await loadComposerPendingEntryActions();
  if (!composerDisposed) composerPendingEntryActionsComponent.value = component;
  return component;
}

function cancelToolbarWarmup() {
  if (warmToolbarHandle === null) return;
  cancelIdleRun(warmToolbarHandle);
  warmToolbarHandle = null;
}

function requestComposerToolbar() {
  if (composerDisposed || hasPending.value || composerToolbarComponent.value) return;
  void ensureComposerToolbarLoaded().catch(() => {
    // toolbar 按需加载失败时保留 fallback 发送按钮兜底。
  });
}

function requestComposerChrome() {
  composerChromeRequested = true;
  requestComposerToolbar();
}

function scheduleToolbarWarmup() {
  if (composerDisposed || hasPending.value || composerToolbarComponent.value || warmToolbarHandle !== null) return;
  scheduleAfterPaint(() => {
    if (composerDisposed || hasPending.value || composerToolbarComponent.value || warmToolbarHandle !== null) return;
    warmToolbarHandle = runWhenIdle(() => {
      scheduleAfterPaint(() => {
        if (composerDisposed || hasPending.value || composerToolbarComponent.value) return;
        warmToolbarHandle = runWhenIdle(() => {
          warmToolbarHandle = null;
          requestComposerToolbar();
        });
      });
    });
  });
}

const interactionPhaseKey = computed(() => {
  if (activeAsk.value) {
    const questionId = askQuestion.value?.id ?? "unknown";
    return `ask:${activeAsk.value.id}:${questionId}`;
  }
  if (activeToolConsent.value) return `tool:${activeToolConsent.value.requestId}`;
  return "input";
});

onMounted(() => {
  composerDisposed = false;
  if (hasPendingComposerUi.value) {
    void ensureComposerPendingEntryActionsLoaded();
  }
  scheduleToolbarWarmup();
});

watch(hasPending, (pending) => {
  if (!pending && composerChromeRequested) {
    requestComposerToolbar();
    return;
  }
  if (!pending) scheduleToolbarWarmup();
}, { immediate: true });

watch(hasPendingComposerUi, (available) => {
  if (available) {
    void ensureComposerPendingEntryActionsLoaded();
  }
}, { immediate: true });

watch(
  () => [
    previewAttachments.value.length,
    Boolean(contextUsageForToolbar.value),
    props.state.planMode,
    props.state.goalMode,
  ] as const,
  ([attachmentsCount, hasContextUsage, planMode, goalMode]) => {
    if (attachmentsCount > 0 || hasContextUsage || planMode || goalMode) {
      requestComposerChrome();
    }
  },
  { immediate: true },
);

const sendTitle = computed(() => {
  if (activeToolConsent.value) return "发送拒绝备注（Enter）";
  if (activeAsk.value) {
    if (askIsPlanApproval.value) return "发送计划修改要求（Enter）";
    return "发送取消原因（Enter）";
  }
  if (canInterrupt.value) return "打断 Agent";
  return props.sending ? "加入调度队列（Enter）" : "发送（Enter）";
});

const sendAriaLabel = computed(() => {
  if (activeToolConsent.value) return "发送拒绝备注";
  if (activeAsk.value) {
    if (askIsPlanApproval.value) return "发送计划修改要求";
    return "发送取消原因";
  }
  if (canInterrupt.value) return "打断 Agent";
  return props.sending ? "加入调度队列" : "发送";
});

const permissionOptions = [
  { value: "full" as PermissionMode, label: "完全访问", hint: "无需逐条确认" },
  { value: "ask" as PermissionMode, label: "询问", hint: "敏感操作前询问" },
  { value: "readonly" as PermissionMode, label: "只读", hint: "禁止写操作" },
];

function updateInputSelection() {
  if (hasPending.value) {
    const el = textarea.value;
    richInput.inputSelection.value = el?.selectionStart ?? pendingText.value.length;
    return;
  }
  richInput.inputSelection.value = richInput.captureSelectionOffset();
}

function onInputEvent() {
  measurePerfSync("chat-composer.pending-input.update", () => {
    updateInputSelection();
    conversationSearch.noteInputChanged();
    contextSearch.noteInputChanged();
    scheduleSpecialInputControllerRefresh();
  }, { detail: composerPerfDetail(props.state) });
}

function onSelectionEvent() {
  measurePerfSync("chat-composer.pending-selection.update", () => {
    updateInputSelection();
    scheduleSpecialInputControllerRefresh();
  }, { detail: composerPerfDetail(props.state) });
}

function onRichInput() {
  measurePerfSync("chat-composer.rich-input.update", () => {
    richInput.onInput();
    conversationSearch.noteInputChanged();
    contextSearch.noteInputChanged();
    slashCommands.noteInputChanged();
    scheduleSpecialInputControllerRefresh();
  }, { detail: composerPerfDetail(props.state) });
}

function onRichSelectionEvent() {
  measurePerfSync("chat-composer.rich-selection.update", () => {
    richInput.onSelection();
    scheduleSpecialInputControllerRefresh();
  }, { detail: composerPerfDetail(props.state) });
}

function onRichPaste(event: ClipboardEvent) {
  if (hasPending.value) return;
  const hasFiles = pasteHasFileItems(event);
  const plainText = pastedPlainText(event);
  if (!hasFiles && !plainText) return;
  event.preventDefault();
  if (!hasFiles && plainText.length < DIRECT_PASTE_TEXT_THRESHOLD) {
    const range = richInput.captureSelectionRange();
    const offset = range?.start ?? richInput.captureSelectionOffset();
    const end = range?.end ?? offset;
    richInput.inputSelection.value = offset;
    clearComposerContextState();
    richInput.replaceRange(offset, end, [textPart(plainText)]);
    return;
  }
  void ensureComposerPasteLoaded().then((paste) => {
    paste.onPaste(event);
  }).catch((error) => {
    console.error("[chat] load paste handler failed", error);
  });
}

function openAttachmentImage(attachment: ChatAttachment) {
  const source = imageViewerSourceFromAttachment(attachment);
  if (source) emit("open-image", source);
}

function contextAttachmentIcon(attachment: ChatAttachment) {
  if (isImageAttachment(attachment)) return Image;
  if (attachment.kind === "directory") return Folder;
  if (attachment.kind === "file") return FileText;
  return Paperclip;
}

function formatBytes(size: number | null | undefined): string {
  if (typeof size !== "number" || !Number.isFinite(size)) return "大小未知";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${Math.round(size / 1024)} KB`;
  return `${Math.round(size / (1024 * 1024))} MB`;
}

function isLargeDirectory(attachment: ChatAttachment): boolean {
  const directory = attachment.directory;
  if (!directory) return false;
  return directory.fileCount >= LARGE_DIRECTORY_FILE_COUNT ||
    directory.totalSize >= LARGE_DIRECTORY_TOTAL_SIZE ||
    directory.truncated;
}

function directorySummary(attachment: ChatAttachment): string | null {
  const directory = attachment.directory;
  if (!directory) return null;
  const parts = [
    `${directory.fileCount} 个文件`,
    `${directory.directoryCount} 个目录`,
    formatBytes(directory.totalSize),
  ];
  if (directory.truncated) parts.push("未完全统计");
  if (directory.unreadableCount > 0) parts.push(`${directory.unreadableCount} 处不可读`);
  return parts.join(" · ");
}

function attachmentMetaLabel(attachment: ChatAttachment): string {
  if (attachment.exists === false) return "路径不存在";
  if (attachment.kind === "directory") {
    const summary = directorySummary(attachment);
    return isLargeDirectory(attachment)
      ? `目录较大${summary ? ` · ${summary}` : ""}`
      : summary ?? "目录";
  }
  if (isImageAttachment(attachment)) return "图片";
  if (attachment.kind === "file") return formatBytes(attachment.size);
  return "未知路径";
}

function patch(next: Partial<ChatComposerState>) {
  emit("update:state", { ...props.state, ...next });
}

function setPermission(v: PermissionMode) { patch({ permission: v }); }
function togglePlanMode() { patch({ planMode: !props.state.planMode }); }
function toggleGoalMode() { patch({ goalMode: !props.state.goalMode }); }

function fillSuggestionPrompt(prompt: string) {
  richInput.replaceRange(
    0,
    richInput.serializedText.value.length,
    [textPart(prompt)],
  );
  clearComposerContextState();
  focusInput();
}

function blockActionsBriefly() {
  actionsBlocked.value = true;
  if (actionBlockTimerId !== null) window.clearTimeout(actionBlockTimerId);
  actionBlockTimerId = window.setTimeout(() => {
    actionsBlocked.value = false;
    actionBlockTimerId = null;
  }, COMPOSER_ACTION_BLOCK_MS);
}

function send() {
  const value = hasPending.value ? inputValue.value.trim() : richInput.serializedText.value.trim();
  const outgoingConversationReferences = richInput.conversationReferences.value;
  const outgoingAttachments = attachmentsForView.value;
  if (activeToolConsent.value) {
    if (!value) return;
    decideToolConsent("deny", value);
    return;
  }
  if (activeAsk.value) {
    if (askUsesInputActions.value) submitAsk();
    else submitAskFreeform(value);
    return;
  }

  if (!value && unsuppressedAttachmentsForSend.value.length === 0) return;
  suppressedAttachmentIds.value = new Set(outgoingAttachments.map((attachment) => attachment.id));
  emit("send", value, outgoingAttachments, outgoingConversationReferences);
  richInput.resetInput();
  clearComposerContextState();
}

function submitEntry() {
  if (canInterrupt.value) {
    emit("interrupt");
    return;
  }
  send();
}

function startLiliaReview(target: LiliaReviewTarget) {
  if (hasPending.value) return;
  const value = richInput.serializedText.value.trim();
  emit("start-lilia-review", value, attachmentsForView.value, richInput.conversationReferences.value, target);
  richInput.resetInput();
  clearComposerContextState();
}

function startLiliaFixSuggestion(target: LiliaReviewTarget) {
  if (hasPending.value) return;
  const value = richInput.serializedText.value.trim();
  emit(
    "start-lilia-fix-suggestion",
    value,
    attachmentsForView.value,
    richInput.conversationReferences.value,
    target,
  );
  richInput.resetInput();
  clearComposerContextState();
}

function getDraftSnapshot() {
  return {
    content: hasPending.value ? inputValue.value : richInput.plainText.value,
    conversationReferences: richInput.conversationReferences.value,
  };
}

function focusInput() {
  if (hasPending.value) {
    const el = textarea.value;
    if (!el) return;
    el.focus();
    const offset = inputValue.value.length;
    el.setSelectionRange(offset, offset);
    return;
  }
  richInput.focusAt(richInput.serializedText.value.length);
}

function triggerConversationReference() {
  if (hasPending.value) {
    const el = textarea.value;
    const start = el?.selectionStart ?? inputValue.value.length;
    const end = el?.selectionEnd ?? start;
    const prefix = start > 0 && !/\s/.test(inputValue.value[start - 1] ?? "") ? " #" : "#";
    inputValue.value = `${inputValue.value.slice(0, start)}${prefix}${inputValue.value.slice(end)}`;
    const nextOffset = start + prefix.length;
    void nextTick(() => {
      textarea.value?.focus();
      textarea.value?.setSelectionRange(nextOffset, nextOffset);
      queueResize();
    });
    return;
  }
  const offset = richInput.captureSelectionOffset();
  const current = richInput.searchText.value;
  const prefix = offset > 0 && !/\s/.test(current[offset - 1] ?? "") ? " #" : "#";
  richInput.replaceRange(offset, offset, [textPart(prefix)]);
  conversationSearch.noteInputChanged();
  void ensureConversationSearchLoaded();
}

function onKeydown(e: KeyboardEvent) {
  updateInputSelection();
  if (e.isComposing) return;
  if (conversationSearch.handleKeydown(e)) return;
  if (contextSearch.handleKeydown(e)) return;
  if (slashCommands.handleKeydown(e)) return;
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    send();
  }
}

function onRichKeydown(e: KeyboardEvent) {
  richInput.inputSelection.value = richInput.captureSelectionOffset();
  if (e.isComposing) return;
  if (conversationSearch.handleKeydown(e)) return;
  if (contextSearch.handleKeydown(e)) return;
  if (slashCommands.handleKeydown(e)) return;
  if (e.key === "Enter" && e.shiftKey) {
    e.preventDefault();
    richInput.replaceRange(
      richInput.inputSelection.value,
      richInput.inputSelection.value,
      [textPart("\n")],
    );
    conversationSearch.noteInputChanged();
    contextSearch.noteInputChanged();
    return;
  }
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    send();
  }
}

function queueResize() {
  if (resizeFrameId !== null) return;
  resizeFrameId = window.requestAnimationFrame(() => {
    resizeFrameId = null;
    resize();
  });
}

function measureInputScrollHeight(el: HTMLTextAreaElement): number {
  const measure = textareaMeasure.value;
  if (!measure) return el.scrollHeight;
  measure.value = el.value || " ";
  measure.style.width = `${el.clientWidth || el.getBoundingClientRect().width}px`;

  return measure.scrollHeight;
}

function resize() {
  const el = textarea.value;
  if (!el) return;
  const currentHeight =
    el.getBoundingClientRect().height ||
    Number.parseFloat(el.style.height) ||
    COMPOSER_INPUT_MIN_HEIGHT;
  const scrollHeight = measureInputScrollHeight(el);
  const nextHeight = Math.min(
    Math.max(scrollHeight, COMPOSER_INPUT_MIN_HEIGHT),
    COMPOSER_INPUT_MAX_HEIGHT,
  );
  if (Math.abs(currentHeight - nextHeight) >= 1) {
    el.style.height = `${currentHeight}px`;
    void el.offsetHeight;
    el.style.height = `${nextHeight}px`;
  } else {
    el.style.height = `${nextHeight}px`;
  }

  if (overflowTimerId !== null) window.clearTimeout(overflowTimerId);
  overflowTimerId = null;
  el.style.overflowY = "hidden";
  el.scrollTop = 0;
  if (scrollHeight > COMPOSER_INPUT_MAX_HEIGHT) {
    overflowTimerId = window.setTimeout(() => {
      if (textarea.value !== el) return;
      el.style.overflowY = "auto";
      el.scrollTop = scrollHeight;
      overflowTimerId = null;
    }, COMPOSER_INPUT_TRANSITION_MS);
  }
}

function onInlineKeydown(e: KeyboardEvent) {
  const q = askQuestion.value;
  if (!q) return;
  if (e.key === "Escape" && askDismissable.value) {
    e.preventDefault();
    cancelAsk();
    return;
  }
  if (e.target instanceof HTMLTextAreaElement) return;
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    submitAsk();
    return;
  }
  if (q.mode === "confirm") return;
  const list = askOptionsWithId.value;
  const allIds = list.map((o) => o.id);
  if (allIds.length === 0) return;
  const cur = singleFocus.value ?? singlePick.value ?? allIds[0];
  const i = allIds.indexOf(cur);
  if (e.key === "ArrowDown") {
    e.preventDefault();
    highlightOption(allIds[(i + 1) % allIds.length]);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    highlightOption(allIds[(i - 1 + allIds.length) % allIds.length]);
  } else if (e.key === " " && q.mode === "single") {
    e.preventDefault();
    selectSingleOption(cur);
  } else if (e.key === " " && q.mode === "multi") {
    e.preventDefault();
    toggleMulti(cur);
  }
}

watch(inputValue, () => {
  void nextTick(queueResize);
});

watch(interactionPhaseKey, blockActionsBriefly);

watch(
  () => !hasPending.value && richInput.isEmpty.value,
  (empty) => emit("draft-empty-change", empty),
  { immediate: true },
);

watch(
  [attachmentsForView, () => props.sending],
  ([attachments, sending]) => {
    if (suppressedAttachmentIds.value.size === 0) return;
    if (!sending || attachments.length === 0) {
      suppressedAttachmentIds.value = new Set();
      return;
    }
    const attachmentIds = new Set(attachments.map((attachment) => attachment.id));
    const nextSuppressed = new Set(
      Array.from(suppressedAttachmentIds.value).filter((id) => attachmentIds.has(id)),
    );
    if (nextSuppressed.size !== suppressedAttachmentIds.value.size) {
      suppressedAttachmentIds.value = nextSuppressed;
    }
  },
  { immediate: true },
);

watch(
  () => props.restoreDraftKey ?? 0,
  (key, previousKey) => {
    if (key === previousKey || key <= 0 || hasPending.value) return;
    richInput.replaceWithText(
      props.restoreDraftContent ?? "",
      attachmentsForView.value,
      restoreDraftConversationReferences.value,
    );
    clearComposerContextState();
  },
  { immediate: true },
);

watch(
  () => props.insertDraftTextKey ?? 0,
  (key, previousKey) => {
    if (key === previousKey || key <= 0) return;
    const text = props.insertDraftTextContent ?? "";
    if (!text) return;
    if (hasPending.value) {
      const el = textarea.value;
      const start = el?.selectionStart ?? inputValue.value.length;
      const end = el?.selectionEnd ?? start;
      inputValue.value = `${inputValue.value.slice(0, start)}${text}${inputValue.value.slice(end)}`;
      const nextOffset = start + text.length;
      richInput.inputSelection.value = nextOffset;
      void nextTick(() => {
        textarea.value?.focus();
        textarea.value?.setSelectionRange(nextOffset, nextOffset);
        queueResize();
      });
      return;
    }
    const offset = richInput.serializedText.value.length;
    richInput.replaceRange(offset, offset, [textPart(text)]);
    clearComposerContextState();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  composerDisposed = true;
  composerChromeRequested = false;
  cancelToolbarWarmup();
  conversationSearchScope?.stop();
  contextSearchScope?.stop();
  slashCommandsScope?.stop();
  conversationSearchScope = null;
  contextSearchScope = null;
  slashCommandsScope = null;
  conversationSearchController.value = null;
  contextSearchController.value = null;
  slashCommandsController.value = null;
  if (resizeFrameId !== null) {
    window.cancelAnimationFrame(resizeFrameId);
    resizeFrameId = null;
  }
  if (overflowTimerId !== null) {
    window.clearTimeout(overflowTimerId);
    overflowTimerId = null;
  }
  if (actionBlockTimerId !== null) {
    window.clearTimeout(actionBlockTimerId);
    actionBlockTimerId = null;
  }
});

defineExpose({ focusInput, getDraftSnapshot, fillSuggestionPrompt, triggerConversationReference });
</script>

<template>
  <div
    class="chat-composer"
    @pointerenter="requestComposerChrome"
    @focusin="requestComposerChrome"
  >
    <Transition name="chat-composer-pending-panel">
      <ComposerPendingPanel
        v-if="hasPendingPanel && pendingInteractionReady"
        :key="pendingKey"
        :active-ask="activeAsk"
        :ask-question="askQuestion"
        :ask-title="askTitle"
        :ask-index="askIndex"
        :ask-total="askTotal"
        :ask-dismissable="askDismissable"
        :ask-is-plan-approval="askIsPlanApproval"
        :ask-options-with-id="askOptionsWithId"
        :ask-has-preview="askHasPreview"
        :ask-focused-option="askFocusedOption"
        :active-ask-option-id="activeAskOptionId"
        :single-pick="singlePick"
        :multi-picks="multiPicks"
        :can-go-prev="canGoPrev"
        :active-tool-consent="activeToolConsent"
        :tool-danger="toolDanger"
        :tool-icon="toolIconForPanel"
        :tool-headline="toolHeadline"
        :tool-inline-preview="toolInlinePreview"
        :tool-input-json="toolInputJson"
        :tool-subtitle="toolSubtitle"
        :tool-expanded="toolExpanded"
        :is-editing-tool-command="isEditingToolCommand"
        :has-editable-command="hasEditableCommand"
        :tool-command-draft="toolCommandDraft"
        :actions-blocked="actionsBlocked"
        @keydown="onInlineKeydown"
        @cancel-ask="cancelAsk"
        @highlight-option="highlightOption"
        @clear-option-highlight="clearOptionHighlight"
        @focus-option="focusOption"
        @select-single-option="selectSingleOption"
        @toggle-multi="toggleMulti"
        @skip-ask="skipAsk"
        @back-ask="backAsk"
        @confirm-ask-no="confirmAskNo"
        @submit-ask="submitAsk"
        @update-tool-expanded="toolExpanded = $event"
        @update-tool-command-draft="toolCommandDraft = $event"
        @begin-command-edit="beginCommandEdit"
      />
    </Transition>

    <Transition name="chat-composer-stack">
      <ComposerConversationReferencePanel
        v-if="conversationSearch.panelOpen.value"
        :results="conversationSearch.results.value"
        :active-index="conversationSearch.activeIndex.value"
        :loading="conversationSearch.loading.value"
        :error="conversationSearch.error.value"
        @activate="conversationSearch.activateResult"
        @select="conversationSearch.selectResult"
      />
    </Transition>

    <Transition name="chat-composer-stack">
      <ComposerContextPanel
        v-if="contextSearch.panelOpen.value"
        :results="contextSearch.results.value"
        :active-index="contextSearch.activeIndex.value"
        :loading="contextSearch.loading.value"
        :error="contextSearch.error.value"
        :missing-path="contextSearch.missingPath.value"
        :show-missing-path="contextSearch.showMissingPath.value"
        :is-large-directory="isLargeDirectory"
        :attachment-meta-label="attachmentMetaLabel"
        :context-inline-path="contextInlinePath"
        :context-attachment-icon="contextAttachmentIcon"
        @activate="contextSearch.activateResult"
        @select="contextSearch.selectResult"
      />
    </Transition>

    <Transition name="chat-composer-stack">
      <ComposerSlashCommandPanel
        v-if="slashCommands.panelOpen.value"
        :results="slashCommands.results.value"
        :target-items="slashCommands.targetItems.value"
        :active-workflow-kind="slashCommands.activeWorkflowKind.value"
        :active-index="slashCommands.activeIndex.value"
        :loading="slashCommands.loading.value"
        :error="slashCommands.error.value"
        @activate="slashCommands.activateResult"
        @select="slashCommands.selectResult"
        @select-target="slashCommands.selectTarget"
      />
    </Transition>

    <div
      class="chat-composer__entry-row"
      :class="{ 'chat-composer__entry-row--pending': hasPending }"
    >
      <textarea
        v-if="hasPendingComposerUi && (!askUsesInputActions || askOtherSelected)"
        ref="textareaMeasure"
        class="chat-composer__input chat-composer__input-measure"
        rows="1"
        tabindex="-1"
        aria-hidden="true"
      />
      <textarea
        v-if="hasPendingComposerUi && (!askUsesInputActions || askOtherSelected)"
        ref="textarea"
        v-model="inputValue"
        class="chat-composer__input"
        rows="1"
        :placeholder="inputPlaceholder"
        @input="onInputEvent"
        @click="onSelectionEvent"
        @keydown="onKeydown"
        @keyup="onSelectionEvent"
        @select="onSelectionEvent"
      />
      <ComposerRichInput
        v-else-if="!hasPending"
        :placeholder="inputPlaceholder"
        :is-empty="richInput.isEmpty.value"
        @editor="richInput.setEditor"
        @input="onRichInput"
        @selection="onRichSelectionEvent"
        @keydown="onRichKeydown"
        @paste="onRichPaste"
      />
      <div
        v-else-if="hasPending"
        class="chat-composer__pending-loading"
      >
        正在载入交互…
      </div>

      <Transition name="chat-composer-entry-actions" mode="out-in">
        <component
          :is="composerPendingEntryActionsComponent"
          v-if="hasPendingComposerUi && composerPendingEntryActionsComponent"
          :key="pendingEntryActionsKey"
          :ask-uses-input-actions="askUsesInputActions"
          :ask-question-skippable="askQuestion?.skippable !== false"
          :ask-total="askTotal"
          :can-go-prev="canGoPrev"
          :can-ask-submit="canAskSubmit"
          :ask-is-last="askIsLast"
          :ask-is-plan-approval="askIsPlanApproval"
          :has-pending-input-text="hasPendingInputText"
          :has-tool-consent="!!activeToolConsent"
          :tool-submitting="toolSubmitting"
          :is-editing-tool-command="isEditingToolCommand"
          :tool-danger="toolDanger"
          :tool-command-is-empty="toolCommandIsEmpty"
          :can-interrupt="canInterrupt"
          :can-submit-entry="canSubmitEntry"
          :actions-blocked="actionsBlocked"
          :send-title="sendTitle"
          :send-aria-label="sendAriaLabel"
          @skip-ask="skipAsk"
          @back-ask="backAsk"
          @submit-ask="submitAsk"
          @modify-plan-approval="modifyPlanApproval"
          @cancel-tool-command-edit="cancelCommandEdit"
          @decide-tool-consent="decideToolConsent"
          @submit-entry="submitEntry"
        />
        <div v-else-if="hasPendingComposerUi" class="chat-composer__entry-actions">
          <button
            v-if="askUsesInputActions && askQuestion?.skippable !== false && askTotal > 1"
            type="button"
            class="ui-button ui-button--ghost composer-inline__skip composer-inline__btn"
            :disabled="actionsBlocked"
            @click="skipAsk"
          >
            跳过
          </button>

          <div v-if="askUsesInputActions" class="chat-composer__pending-actions">
            <button
              v-if="canGoPrev"
              type="button"
              class="ui-button ui-button--ghost composer-inline__btn"
              :disabled="actionsBlocked"
              @click="backAsk"
            >
              上一题
            </button>
            <button
              type="button"
              class="ui-button ui-button--primary composer-inline__btn"
              :disabled="actionsBlocked || !canAskSubmit"
              @click="submitAsk"
            >
              {{ askIsLast ? "完成" : "继续" }}
            </button>
          </div>

          <div v-else-if="askIsPlanApproval" class="chat-composer__pending-actions">
            <button
              type="button"
              class="ui-button ui-button--ghost composer-inline__btn"
              :disabled="actionsBlocked || !hasPendingInputText"
              @click="modifyPlanApproval"
            >
              {{ hasPendingInputText ? "修改" : "忽略" }}
            </button>
            <button
              type="button"
              class="ui-button ui-button--primary composer-inline__btn"
              :disabled="actionsBlocked"
              @click="submitAsk"
            >
              同意
            </button>
          </div>

          <div v-else-if="activeToolConsent" class="chat-composer__pending-actions">
            <button
              type="button"
              class="ui-button ui-button--ghost composer-inline__btn"
              :disabled="actionsBlocked || toolSubmitting !== null || (!isEditingToolCommand && !hasPendingInputText)"
              @click="isEditingToolCommand ? cancelCommandEdit() : decideToolConsent('deny')"
            >
              {{
                toolSubmitting === "deny"
                  ? "处理中..."
                  : isEditingToolCommand
                    ? "取消"
                    : hasPendingInputText
                      ? "修改"
                      : "忽略"
              }}
            </button>
            <button
              type="button"
              class="composer-inline__btn"
              :class="toolDanger ? 'ui-button ui-button--ghost ui-button--danger' : 'ui-button ui-button--primary'"
              :disabled="actionsBlocked || toolSubmitting !== null || toolCommandIsEmpty"
              @click="decideToolConsent('allow')"
            >
              {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
            </button>
          </div>

          <button
            v-if="!askUsesInputActions && !askIsPlanApproval && !activeToolConsent"
            type="button"
            class="chat-composer__send"
            :class="{ 'chat-composer__send--interrupt': canInterrupt }"
            :disabled="actionsBlocked || !canSubmitEntry"
            :title="sendTitle"
            :aria-label="sendAriaLabel"
            @click="submitEntry"
          >
            <component :is="canInterrupt ? Square : ArrowUp" :size="16" aria-hidden="true" />
          </button>
        </div>
      </Transition>
    </div>

    <Transition name="chat-composer-stack">
      <component
        :is="composerToolbarComponent"
        v-if="!hasPending && composerToolbarComponent"
        :state="state"
        :model-options="modelOptions ?? []"
        :auto-model-preview="autoModelPreview"
        :permission-options="permissionOptions"
        :preview-attachments="previewAttachments"
        :can-interrupt="canInterrupt"
        :can-submit-entry="canSubmitEntry"
        :actions-blocked="actionsBlocked"
        :compact-disabled="compactDisabled === true || sending === true || hasPending"
        :context-usage="contextUsageForToolbar"
        :send-title="sendTitle"
        :send-aria-label="sendAriaLabel"
        @pick-attachments="emit('pick-attachments')"
        @reference-conversation="triggerConversationReference"
        @set-permission="setPermission"
        @update-composer="patch"
        @toggle-plan-mode="togglePlanMode"
        @toggle-goal-mode="toggleGoalMode"
        @start-lilia-compact="emit('start-lilia-compact')"
        @submit-entry="submitEntry"
        @open-image="openAttachmentImage"
      />
      <button
        v-else-if="!hasPending"
        type="button"
        class="chat-composer__send chat-composer__send--toolbar-fallback"
        :class="{ 'chat-composer__send--interrupt': canInterrupt }"
        :disabled="actionsBlocked || !canSubmitEntry"
        :title="sendTitle"
        :aria-label="sendAriaLabel"
        @click="submitEntry"
      >
        <component :is="canInterrupt ? Square : ArrowUp" :size="16" aria-hidden="true" />
      </button>
    </Transition>
  </div>
</template>
