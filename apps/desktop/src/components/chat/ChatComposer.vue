<script setup lang="ts">
/**
 * Composer：富文本输入 + pending Agent 交互 + 工具栏组合层。
 */

import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  FileText,
  Folder,
  Image,
  Paperclip,
} from "lucide-vue-next";
import type {
  AskUserResult,
  ChatAttachment,
  ChatComposerState,
  ChatConversationReference,
  ChatContextUsage,
  ChatSlashCommandWorkflow,
  LiliaReviewTarget,
  PermissionMode,
} from "@lilia/contracts";
import type { PendingAsk } from "../../composables/useAskUser";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../../services/chat";
import ComposerContextPanel from "./ComposerContextPanel.vue";
import ComposerConversationReferencePanel from "./ComposerConversationReferencePanel.vue";
import ComposerPendingEntryActions from "./ComposerPendingEntryActions.vue";
import ComposerPendingPanel from "./ComposerPendingPanel.vue";
import ComposerRichInput from "./ComposerRichInput.vue";
import ComposerSlashCommandPanel from "./ComposerSlashCommandPanel.vue";
import ComposerToolbar from "./ComposerToolbar.vue";
import { textPart } from "./composerParts";
import {
  contextInlinePath,
  useComposerContextSearch,
} from "./useComposerContextSearch";
import { useComposerConversationSearch } from "./useComposerConversationSearch";
import {
  useComposerSlashCommands,
} from "./useComposerSlashCommands";
import { useComposerPaste } from "./useComposerPaste";
import { useComposerRichInput } from "./useComposerRichInput";
import {
  imageViewerSourceFromAttachment,
  isImageAttachment,
  type ChatImageViewerSource,
} from "./imageViewer";
import { useComposerPendingInteraction } from "./useComposerPendingInteraction";

const props = defineProps<{
  state: ChatComposerState;
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

const textarea = ref<HTMLTextAreaElement | null>(null);
const textareaMeasure = ref<HTMLTextAreaElement | null>(null);
const actionsBlocked = ref(false);
let resizeFrameId: number | null = null;
let overflowTimerId: number | null = null;
let actionBlockTimerId: number | null = null;

let clearComposerContextState = () => {};

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
} = useComposerPendingInteraction({
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
});

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

const richInput = useComposerRichInput({
  attachments: attachmentsForView,
  appendAttachmentsToEndKey,
  hasPending,
  isLargeDirectory,
  removeAttachment: (attachmentId) => emit("remove-attachment", attachmentId),
});

const conversationSearch = useComposerConversationSearch({
  richInput,
  hasPending,
});

const contextSearch = useComposerContextSearch({
  richInput,
  projectCwd: computed(() => props.projectCwd),
  hasPending,
  addContextAttachment: (attachment) => emit("add-context-attachment", attachment),
});

const slashCommands = useComposerSlashCommands({
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
});

clearComposerContextState = () => {
  conversationSearch.clear();
  conversationSearch.resetSuppression();
  contextSearch.clear();
  contextSearch.resetSuppression();
  slashCommands.clear();
};

const paste = useComposerPaste({
  richInput,
  clearContextSearch: clearComposerContextState,
  addContextAttachment: (attachment) => emit("add-context-attachment", attachment),
  hasPending: () => hasPending.value,
});

const previewAttachments = computed(() => attachmentsForView.value.filter(isImageAttachment));

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

const interactionPhaseKey = computed(() => {
  if (activeAsk.value) {
    const questionId = askQuestion.value?.id ?? "unknown";
    return `ask:${activeAsk.value.id}:${questionId}`;
  }
  if (activeToolConsent.value) return `tool:${activeToolConsent.value.requestId}`;
  return "input";
});

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
  updateInputSelection();
  conversationSearch.noteInputChanged();
  contextSearch.noteInputChanged();
}

function onSelectionEvent() {
  updateInputSelection();
}

function onRichInput() {
  richInput.onInput();
  conversationSearch.noteInputChanged();
  contextSearch.noteInputChanged();
  slashCommands.noteInputChanged();
}

function onRichSelectionEvent() {
  richInput.onSelection();
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
  <div class="chat-composer">
    <Transition name="chat-composer-pending-panel">
      <ComposerPendingPanel
        v-if="hasPendingPanel"
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
        :tool-icon="toolIcon"
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
        :target-items="slashCommands.targetItems"
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
        v-if="hasPending && (!askUsesInputActions || askOtherSelected)"
        ref="textareaMeasure"
        class="chat-composer__input chat-composer__input-measure"
        rows="1"
        tabindex="-1"
        aria-hidden="true"
      />
      <textarea
        v-if="hasPending && (!askUsesInputActions || askOtherSelected)"
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
        @paste="paste.onPaste"
      />

      <Transition name="chat-composer-entry-actions" mode="out-in">
        <ComposerPendingEntryActions
          v-if="hasPending"
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
      </Transition>
    </div>

    <Transition name="chat-composer-stack">
      <ComposerToolbar
        v-if="!hasPending"
        :state="state"
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
        @toggle-plan-mode="togglePlanMode"
        @toggle-goal-mode="toggleGoalMode"
        @start-lilia-compact="emit('start-lilia-compact')"
        @submit-entry="submitEntry"
        @open-image="openAttachmentImage"
      />
    </Transition>
  </div>
</template>
