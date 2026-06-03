<script setup lang="ts">
/**
 * Composer：textarea 自动撑高（最多 3 行）+ 一排 chip。
 * 挂起态会把工具授权、Agent 提问和计划确认收进输入框内部。
 */

import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  ArrowLeft,
  ArrowRight,
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
  ChatContextSearchResult,
  ChatComposerState,
  PermissionMode,
} from "@lilia/contracts";
import { useAskUserInteraction } from "../../composables/useAskUserInteraction";
import { useEditableToolCommand } from "../../composables/useEditableToolCommand";
import type { PendingAsk } from "../../composables/useAskUser";
import { useToolConsentPresentation } from "../../composables/useToolConsentPresentation";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../../services/chat";
import {
  describeAttachments,
  readClipboardFilePaths,
  saveClipboardImage,
  saveClipboardText,
  searchContextAttachments,
} from "../../services/chat";
import ComposerContextPanel from "./ComposerContextPanel.vue";
import ComposerPendingPanel from "./ComposerPendingPanel.vue";
import ComposerRichInput from "./ComposerRichInput.vue";
import ComposerToolbar from "./ComposerToolbar.vue";
import {
  imageViewerSourceFromAttachment,
  isImageAttachment,
  type ChatImageViewerSource,
} from "./imageViewer";

const props = defineProps<{
  state: ChatComposerState;
  attachments?: ChatAttachment[];
  appendAttachmentsToEndKey?: number;
  projectCwd?: string | null;
  /** 上一轮还在 streaming 时为 true，发送会进入调度队列。 */
  sending?: boolean;
  pendingAsk?: PendingAsk | null;
  toolConsent?: ToolConsentRequest | null;
}>();

const emit = defineEmits<{
  send: [content: string, attachments: ChatAttachment[]];
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
  interrupt: [];
}>();

const COMPOSER_INPUT_LINE_HEIGHT = 22;
const COMPOSER_INPUT_VERTICAL_PADDING = 8;
const COMPOSER_INPUT_MAX_ROWS = 3;
const COMPOSER_INPUT_MIN_HEIGHT = COMPOSER_INPUT_LINE_HEIGHT + COMPOSER_INPUT_VERTICAL_PADDING;
const COMPOSER_INPUT_MAX_HEIGHT =
  COMPOSER_INPUT_LINE_HEIGHT * COMPOSER_INPUT_MAX_ROWS + COMPOSER_INPUT_VERTICAL_PADDING;
const COMPOSER_INPUT_TRANSITION_MS = 160;
const CONTEXT_SEARCH_LIMIT = 12;
const LONG_PASTE_TEXT_THRESHOLD = 2000;
const LARGE_DIRECTORY_FILE_COUNT = 200;
const LARGE_DIRECTORY_TOTAL_SIZE = 20 * 1024 * 1024;

interface MentionRange {
  start: number;
  end: number;
  query: string;
}

interface ComposerTextPart {
  id: string;
  type: "text";
  text: string;
}

interface ComposerAttachmentPart {
  id: string;
  type: "attachment";
  attachment: ChatAttachment;
}

type ComposerPart = ComposerTextPart | ComposerAttachmentPart;

const ATTACHMENT_OBJECT_CHAR = "\uFFFC";
const SVG_NS = "http://www.w3.org/2000/svg";

const inlineIconMarkup = {
  file: '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M16 13H8"/><path d="M16 17H8"/><path d="M10 9H8"/>',
  folder: '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>',
  image: '<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.09-3.09a2 2 0 0 0-2.82 0L6 21"/>',
  paperclip: '<path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
} as const;

type InlineIconName = keyof typeof inlineIconMarkup;

let composerPartSeq = 0;

function nextComposerPartId(prefix: string) {
  composerPartSeq += 1;
  return `${prefix}-${composerPartSeq}`;
}

function textPart(text: string): ComposerTextPart {
  return {
    id: nextComposerPartId("text"),
    type: "text",
    text,
  };
}

function attachmentPart(attachment: ChatAttachment): ComposerAttachmentPart {
  return {
    id: nextComposerPartId("attachment"),
    type: "attachment",
    attachment,
  };
}

const composerParts = ref<ComposerPart[]>([textPart("")]);
const pendingText = ref("");
const textarea = ref<HTMLTextAreaElement | null>(null);
const textareaMeasure = ref<HTMLTextAreaElement | null>(null);
const richEditor = ref<HTMLDivElement | null>(null);
const inputSelection = ref(0);
const contextResults = ref<ChatContextSearchResult[]>([]);
const contextSearchLoading = ref(false);
const contextSearchError = ref<string | null>(null);
const contextMissingPath = ref<string | null>(null);
const contextActiveIndex = ref(0);
const contextSuppressedKey = ref<string | null>(null);
const contextNoMatchSuppression = ref<{
  start: number;
  query: string;
} | null>(null);
const contextUserInteracted = ref(false);
let contextSearchSeq = 0;
let resizeFrameId: number | null = null;
let overflowTimerId: number | null = null;
let observedAppendToEndKey = props.appendAttachmentsToEndKey ?? 0;

const toolExpanded = ref(false);
const toolSubmitting = ref<ToolConsentDecision | null>(null);

const activeAsk = computed(() => props.pendingAsk ?? null);
const activeToolConsent = computed(() =>
  activeAsk.value ? null : props.toolConsent ?? null,
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
} = useAskUserInteraction(
  activeAsk,
  pendingText,
  (result) => emit("resolve-ask-user", result),
);

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

const composerSearchText = computed(() =>
  composerParts.value
    .map((part) => part.type === "text" ? part.text : ATTACHMENT_OBJECT_CHAR)
    .join(""),
);
const composerSerializedText = computed(() => serializeComposerParts(composerParts.value));
const composerIsEmpty = computed(() => composerSerializedText.value.length === 0);
const previewAttachments = computed(() => (props.attachments ?? []).filter(isImageAttachment));

const canSend = computed(() => {
  if (activeToolConsent.value || activeAsk.value) return pendingText.value.trim().length > 0;
  return composerSerializedText.value.trim().length > 0 || (props.attachments?.length ?? 0) > 0;
});

const canInterrupt = computed(() =>
  props.sending === true &&
  !hasPending.value &&
  !canSend.value,
);

const canSubmitEntry = computed(() => canSend.value || canInterrupt.value);

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

const mentionRange = computed<MentionRange | null>(() => {
  if (hasPending.value) return null;
  return readMentionRange(composerSearchText.value, inputSelection.value);
});
const mentionKey = computed(() => {
  const range = mentionRange.value;
  return range ? `${range.start}:${range.end}:${range.query}` : null;
});
const contextPanelOpen = computed(() =>
  mentionRange.value !== null &&
  !isContextAutoSuppressed(mentionRange.value) &&
  mentionKey.value !== contextSuppressedKey.value,
);
const contextActiveResult = computed(() =>
  contextResults.value[contextActiveIndex.value] ?? null,
);
const contextShowMissingPath = computed(() =>
  contextPanelOpen.value &&
  !contextSearchLoading.value &&
  contextResults.value.length === 0 &&
  !!contextMissingPath.value,
);

function readMentionRange(text: string, cursor: number): MentionRange | null {
  const end = Math.min(Math.max(cursor, 0), text.length);
  const prefix = text.slice(0, end);
  const start = prefix.lastIndexOf("@");
  if (start < 0) return null;
  const query = text.slice(start + 1, end);
  if (query.length > 240 || /[\n\r]/.test(query)) return null;
  if (/\s/.test(query) && !isAbsolutePathLike(query)) return null;
  return { start, end, query };
}

function isContextAutoSuppressed(range: MentionRange): boolean {
  const suppression = contextNoMatchSuppression.value;
  if (!suppression || suppression.start !== range.start) return false;
  if (range.query.trim().length === 0 || isContextPathQueryLike(range.query)) return false;
  return range.query.length > suppression.query.length &&
    range.query.startsWith(suppression.query);
}

function isContextPathQueryLike(value: string): boolean {
  return value.includes("/") || value.includes("\\");
}

function updateInputSelection() {
  if (hasPending.value) {
    const el = textarea.value;
    inputSelection.value = el?.selectionStart ?? pendingText.value.length;
    return;
  }
  inputSelection.value = captureRichSelectionOffset();
}

function onInputEvent() {
  updateInputSelection();
  contextSuppressedKey.value = null;
}

function onSelectionEvent() {
  updateInputSelection();
}

function setRichEditor(element: HTMLDivElement | null) {
  richEditor.value = element;
}

function isAbsolutePathLike(value: string): boolean {
  const trimmed = value.trim();
  return /^[a-zA-Z]:[\\/]/.test(trimmed) ||
    trimmed.startsWith("\\\\") ||
    trimmed.startsWith("/");
}

function composerPartLength(part: ComposerPart): number {
  return part.type === "text" ? part.text.length : 1;
}

function composerPartsLength(parts = composerParts.value): number {
  return parts.reduce((total, part) => total + composerPartLength(part), 0);
}

function normalizeComposerParts(parts: ComposerPart[]): ComposerPart[] {
  const normalized: ComposerPart[] = [];
  for (const part of parts) {
    if (part.type === "text") {
      const previous = normalized[normalized.length - 1];
      if (previous?.type === "text") {
        previous.text += part.text;
      } else if (part.text || normalized.length === 0) {
        normalized.push({ ...part, id: part.id || nextComposerPartId("text") });
      }
      continue;
    }
    normalized.push(part);
  }
  if (normalized.length === 0) normalized.push(textPart(""));
  return normalized;
}

function splitComposerPartsAt(parts: ComposerPart[], offset: number): [ComposerPart[], ComposerPart[]] {
  const before: ComposerPart[] = [];
  const after: ComposerPart[] = [];
  let remaining = Math.max(0, offset);
  let split = false;

  for (const part of parts) {
    if (split) {
      after.push(part);
      continue;
    }
    if (part.type === "text") {
      if (remaining < part.text.length) {
        const left = part.text.slice(0, remaining);
        const right = part.text.slice(remaining);
        if (left) before.push(textPart(left));
        if (right) after.push(textPart(right));
        split = true;
      } else {
        before.push(part);
        remaining -= part.text.length;
      }
      continue;
    }
    if (remaining <= 0) {
      after.push(part);
      split = true;
    } else {
      before.push(part);
      remaining -= 1;
    }
  }

  return [normalizeComposerParts(before), normalizeComposerParts(after)];
}

function partsStartWithWhitespace(parts: ComposerPart[]): boolean {
  const first = parts.find((part) => part.type !== "text" || part.text.length > 0);
  return first?.type === "text" ? /^\s/.test(first.text) : false;
}

function replaceComposerRange(start: number, end: number, replacement: ComposerPart[]): number {
  const [before, rest] = splitComposerPartsAt(composerParts.value, start);
  const [, after] = splitComposerPartsAt(rest, Math.max(0, end - start));
  const nextParts = normalizeComposerParts([...before, ...replacement, ...after]);
  const nextCursor = composerPartsLength([...before, ...replacement]);
  composerParts.value = nextParts;
  inputSelection.value = Math.min(nextCursor, composerPartsLength(nextParts));
  renderRichEditorFromParts();
  focusRichEditorAt(inputSelection.value);
  return inputSelection.value;
}

function referenceKindLabel(attachment: ChatAttachment): string {
  if (isImageAttachment(attachment)) return "图片引用";
  if (attachment.kind === "directory") return "目录引用";
  return "文件引用";
}

function serializeAttachmentReference(attachment: ChatAttachment): string {
  return `[${referenceKindLabel(attachment)}: ${attachment.name} | ${attachment.path}]`;
}

function serializeComposerParts(parts: ComposerPart[]): string {
  return parts
    .map((part) => part.type === "text"
      ? part.text
      : serializeAttachmentReference(part.attachment))
    .join("");
}

function composerHasAttachmentPath(path: string): boolean {
  return composerParts.value.some((part) =>
    part.type === "attachment" && part.attachment.path === path
  );
}

function attachmentFromComposerOrProps(id: string): ChatAttachment | null {
  const fromProps = props.attachments?.find((attachment) => attachment.id === id);
  if (fromProps) return fromProps;
  const fromComposer = composerParts.value.find((part) =>
    part.type === "attachment" && part.attachment.id === id
  );
  return fromComposer?.type === "attachment" ? fromComposer.attachment : null;
}

function attachmentIconName(attachment: ChatAttachment): InlineIconName {
  if (isImageAttachment(attachment)) return "image";
  if (attachment.kind === "directory") return "folder";
  if (attachment.kind === "file") return "file";
  return "paperclip";
}

function createInlineSvgIcon(name: InlineIconName, size = 14): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.innerHTML = inlineIconMarkup[name];
  return svg;
}

function createAttachmentReferenceElement(attachment: ChatAttachment): HTMLElement {
  const chip = document.createElement("span");
  chip.className = [
    "chat-file-reference",
    isLargeDirectory(attachment) ? "chat-file-reference--warning" : "",
  ].filter(Boolean).join(" ");
  chip.contentEditable = "false";
  chip.dataset.attachmentId = attachment.id;
  chip.title = attachment.path;

  const icon = document.createElement("span");
  icon.className = "chat-file-reference__icon";
  icon.setAttribute("aria-hidden", "true");
  icon.append(createInlineSvgIcon(attachmentIconName(attachment), 13));
  chip.append(icon);

  const main = document.createElement("span");
  main.className = "chat-file-reference__main";
  const name = document.createElement("span");
  name.className = "chat-file-reference__name";
  name.textContent = attachment.name;
  main.append(name);
  chip.append(main);

  if (isLargeDirectory(attachment)) {
    const meta = document.createElement("span");
    meta.className = "chat-file-reference__meta";
    meta.textContent = "目录较大";
    chip.append(meta);
  }

  const remove = document.createElement("button");
  remove.type = "button";
  remove.className = "chat-file-reference__remove";
  remove.setAttribute("aria-label", `移除文件引用 ${attachment.name}`);
  remove.append(createInlineSvgIcon("x", 12));
  remove.addEventListener("mousedown", (event) => {
    event.preventDefault();
    event.stopPropagation();
  });
  remove.addEventListener("click", (event) => {
    event.preventDefault();
    event.stopPropagation();
    removeInlineAttachment(attachment.id);
  });
  chip.append(remove);

  return chip;
}

function renderRichEditorFromParts() {
  const editor = richEditor.value;
  if (!editor) return;
  const fragment = document.createDocumentFragment();
  for (const part of composerParts.value) {
    if (part.type === "text") {
      if (part.text) fragment.append(document.createTextNode(part.text));
    } else {
      fragment.append(createAttachmentReferenceElement(part.attachment));
    }
  }
  editor.replaceChildren(fragment);
}

function nodeComposerLength(node: Node): number {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent?.length ?? 0;
  if (node.nodeType !== Node.ELEMENT_NODE && node.nodeType !== Node.DOCUMENT_FRAGMENT_NODE) {
    return 0;
  }
  const element = node instanceof HTMLElement ? node : null;
  if (element?.dataset.attachmentId) return 1;
  if (element?.tagName === "BR") return 1;
  return Array.from(node.childNodes).reduce((total, child) => total + nodeComposerLength(child), 0);
}

function captureRichSelectionOffset(): number {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return inputSelection.value;
  return richSelectionPointOffset(selection.focusNode, selection.focusOffset) ?? inputSelection.value;
}

function richSelectionPointOffset(node: Node | null, offset: number): number | null {
  const editor = richEditor.value;
  if (!editor || !node || !editor.contains(node)) return null;
  const range = document.createRange();
  range.selectNodeContents(editor);
  try {
    range.setEnd(node, offset);
  } catch {
    return null;
  }
  return nodeComposerLength(range.cloneContents());
}

function captureRichSelectionRange(): { start: number; end: number } | null {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return null;
  const anchor = richSelectionPointOffset(selection.anchorNode, selection.anchorOffset);
  const focus = richSelectionPointOffset(selection.focusNode, selection.focusOffset);
  if (anchor === null || focus === null) return null;
  return {
    start: Math.min(anchor, focus),
    end: Math.max(anchor, focus),
  };
}

function childIndex(node: Node): number {
  return node.parentNode ? Array.from<Node>(node.parentNode.childNodes).indexOf(node) : 0;
}

function findRichDomPosition(root: Node, offset: number): { node: Node; offset: number } {
  let remaining = Math.max(0, offset);

  function walk(node: Node): { node: Node; offset: number } | null {
    if (node.nodeType === Node.TEXT_NODE) {
      const length = node.textContent?.length ?? 0;
      if (remaining <= length) return { node, offset: remaining };
      remaining -= length;
      return null;
    }

    if (node instanceof HTMLElement && node.dataset.attachmentId) {
      const parent = node.parentNode ?? root;
      const index = childIndex(node);
      if (remaining <= 0) return { node: parent, offset: index };
      if (remaining <= 1) return { node: parent, offset: index + 1 };
      remaining -= 1;
      return null;
    }

    for (const child of Array.from(node.childNodes)) {
      const found = walk(child);
      if (found) return found;
    }
    return null;
  }

  return walk(root) ?? { node: root, offset: root.childNodes.length };
}

function focusRichEditorAt(offset = inputSelection.value) {
  void nextTick(() => {
    const editor = richEditor.value;
    if (!editor) return;
    editor.focus();
    const position = findRichDomPosition(editor, offset);
    const range = document.createRange();
    range.setStart(position.node, position.offset);
    range.collapse(true);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
    inputSelection.value = offset;
  });
}

function collectComposerPartsFromNode(node: Node, parts: ComposerPart[]) {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = node.textContent ?? "";
    if (text) parts.push(textPart(text));
    return;
  }
  if (!(node instanceof HTMLElement)) return;
  const attachmentId = node.dataset.attachmentId;
  if (attachmentId) {
    const attachment = attachmentFromComposerOrProps(attachmentId);
    if (attachment) parts.push(attachmentPart(attachment));
    return;
  }
  if (node.tagName === "BR") {
    parts.push(textPart("\n"));
    return;
  }
  for (const child of Array.from(node.childNodes)) {
    collectComposerPartsFromNode(child, parts);
  }
}

function readComposerPartsFromEditor(): ComposerPart[] {
  const editor = richEditor.value;
  if (!editor) return composerParts.value;
  if (isBrowserEmptyRichEditor(editor)) {
    return [textPart("")];
  }
  const parts: ComposerPart[] = [];
  for (const child of Array.from(editor.childNodes)) {
    collectComposerPartsFromNode(child, parts);
  }
  return normalizeComposerParts(parts);
}

function isBrowserEmptyRichEditor(editor: HTMLElement): boolean {
  return editor.childNodes.length === 1 && editor.firstChild instanceof HTMLBRElement;
}

function syncRemovedInlineAttachments(nextParts: ComposerPart[]) {
  const nextPaths = new Set(nextParts
    .filter((part): part is ComposerAttachmentPart => part.type === "attachment")
    .map((part) => part.attachment.path));
  for (const attachment of props.attachments ?? []) {
    if (!nextPaths.has(attachment.path)) {
      emit("remove-attachment", attachment.id);
    }
  }
}

function onRichInput() {
  const cursor = captureRichSelectionOffset();
  const nextParts = readComposerPartsFromEditor();
  composerParts.value = nextParts;
  inputSelection.value = Math.min(cursor, composerPartsLength(nextParts));
  contextSuppressedKey.value = null;
  contextUserInteracted.value = false;
  syncRemovedInlineAttachments(nextParts);
  if (!richEditorHasFocus()) focusRichEditorAt(inputSelection.value);
}

function onRichSelectionEvent() {
  inputSelection.value = captureRichSelectionOffset();
}

function removeInlineAttachment(attachmentId: string) {
  const target = composerParts.value.find((part) =>
    part.type === "attachment" && part.attachment.id === attachmentId
  );
  composerParts.value = normalizeComposerParts(composerParts.value.filter((part) =>
    !(part.type === "attachment" && part.attachment.id === attachmentId)
  ));
  if (target?.type === "attachment") emit("remove-attachment", target.attachment.id);
  inputSelection.value = Math.min(inputSelection.value, composerPartsLength());
  renderRichEditorFromParts();
  focusRichEditorAt(inputSelection.value);
}

function insertAttachmentReference(attachment: ChatAttachment, offset = inputSelection.value): boolean {
  if (attachment.exists === false || composerHasAttachmentPath(attachment.path)) return false;
  const [before, after] = splitComposerPartsAt(composerParts.value, offset);
  const replacement: ComposerPart[] = [attachmentPart(attachment)];
  if (!partsStartWithWhitespace(after)) replacement.push(textPart(" "));
  composerParts.value = normalizeComposerParts([...before, ...replacement, ...after]);
  inputSelection.value = composerPartsLength([...before, ...replacement]);
  renderRichEditorFromParts();
  focusRichEditorAt(inputSelection.value);
  return true;
}

function pastedImageFiles(event: ClipboardEvent): File[] {
  const items = Array.from(event.clipboardData?.items ?? []);
  return items
    .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
    .map((item) => item.getAsFile())
    .filter((file): file is File => file !== null);
}

function pasteHasFileItems(event: ClipboardEvent): boolean {
  const data = event.clipboardData;
  if (!data) return false;
  return Array.from(data.items).some((item) => item.kind === "file") || data.files.length > 0;
}

function htmlToPlainText(html: string): string {
  const template = document.createElement("template");
  template.innerHTML = html;
  return template.content.textContent ?? "";
}

function pastedPlainText(event: ClipboardEvent): string {
  const data = event.clipboardData;
  if (!data) return "";
  return data.getData("text/plain") || htmlToPlainText(data.getData("text/html"));
}

async function fileToBase64(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.slice(index, index + chunkSize));
  }
  return window.btoa(binary);
}

async function savePastedImages(files: File[]): Promise<ChatAttachment[]> {
  return Promise.all(files.map(async (file) => {
    const bytesBase64 = await fileToBase64(file);
    return saveClipboardImage({
      mime: file.type || null,
      bytesBase64,
      name: file.name || null,
    });
  }));
}

async function attachmentsFromPastedFilePaths(paths: string[]): Promise<ChatAttachment[]> {
  const uniquePaths = paths.filter((path, index) =>
    path.trim().length > 0 &&
    paths.indexOf(path) === index &&
    !composerHasAttachmentPath(path)
  );
  if (uniquePaths.length === 0) return [];
  return describeAttachments(uniquePaths);
}

async function insertPastedAttachments(attachments: ChatAttachment[], offset: number) {
  let nextOffset = offset;
  for (const attachment of attachments) {
    if (insertAttachmentReference(attachment, nextOffset)) {
      emit("add-context-attachment", attachment);
      nextOffset = inputSelection.value;
    }
  }
}

async function handleRichPaste(imageFiles: File[], offset: number) {
  try {
    const paths = await readClipboardFilePaths();
    const pathAttachments = await attachmentsFromPastedFilePaths(paths);
    const imageAttachments = pathAttachments.length > 0 ? [] : await savePastedImages(imageFiles);
    await insertPastedAttachments([...pathAttachments, ...imageAttachments], offset);
  } catch (err) {
    console.error("[chat] paste context failed", err);
  }
}

async function handleLongTextPaste(text: string, offset: number) {
  try {
    const attachment = await saveClipboardText({ text });
    await insertPastedAttachments([attachment], offset);
  } catch (err) {
    console.error("[chat] paste context failed", err);
  }
}

function onRichPaste(event: ClipboardEvent) {
  if (hasPending.value) return;
  const hasFiles = pasteHasFileItems(event);
  const plainText = pastedPlainText(event);
  if (!hasFiles && !plainText) return;
  event.preventDefault();
  const range = captureRichSelectionRange();
  let offset = range?.start ?? captureRichSelectionOffset();
  const end = range?.end ?? offset;
  inputSelection.value = offset;
  contextSuppressedKey.value = null;
  clearContextSearch();
  if (!hasFiles) {
    if (plainText.length < LONG_PASTE_TEXT_THRESHOLD) {
      replaceComposerRange(offset, end, [textPart(plainText)]);
      return;
    }
    if (end > offset) {
      offset = replaceComposerRange(offset, end, []);
    }
    void handleLongTextPaste(plainText, offset);
    return;
  }
  if (end > offset) {
    offset = replaceComposerRange(offset, end, []);
  }
  const imageFiles = pastedImageFiles(event);
  void handleRichPaste(imageFiles, offset);
}

function richEditorHasFocus(): boolean {
  const editor = richEditor.value;
  const active = document.activeElement;
  return !!editor && !!active && (active === editor || editor.contains(active));
}

function externalAttachmentInsertionOffset(): number {
  return richEditorHasFocus() ? captureRichSelectionOffset() : composerPartsLength();
}

function resetComposerInput() {
  const retainedAttachments = props.attachments ?? [];
  composerParts.value = retainedAttachments.length
    ? normalizeComposerParts([
      ...retainedAttachments.map(attachmentPart),
      textPart(" "),
    ])
    : [textPart("")];
  inputSelection.value = composerPartsLength();
  renderRichEditorFromParts();
  clearContextSearch();
  contextSuppressedKey.value = null;
  contextNoMatchSuppression.value = null;
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

function compactPathLabel(value: string): string {
  return value.replace(/\\/g, "/");
}

function contextInlinePath(result: ChatContextSearchResult): string | null {
  const path = compactPathLabel(result.relativePath);
  return path && path !== result.attachment.name ? path : null;
}

function clearContextSearch() {
  contextResults.value = [];
  contextSearchLoading.value = false;
  contextSearchError.value = null;
  contextMissingPath.value = null;
  contextActiveIndex.value = 0;
  contextUserInteracted.value = false;
}

async function refreshContextSearch(range: MentionRange | null) {
  const seq = ++contextSearchSeq;
  if (!contextPanelOpen.value || !range) {
    clearContextSearch();
    return;
  }
  const query = range.query.trim();
  contextSearchLoading.value = true;
  contextSearchError.value = null;
  contextMissingPath.value = null;
  try {
    let results: ChatContextSearchResult[] = [];
    let missingPath: string | null = null;
    let searchError: string | null = null;
    if (isAbsolutePathLike(query)) {
      const [attachment] = await describeAttachments([query]);
      if (attachment?.exists !== false) {
        results = [{
          attachment,
          relativePath: compactPathLabel(attachment.path),
          matchedBy: "path",
        }];
      } else if (attachment) {
        missingPath = attachment.path;
      } else {
        missingPath = query;
      }
    } else {
      const cwd = props.projectCwd?.trim();
      if (!cwd) {
        searchError = "没有可搜索的项目目录";
      } else {
        results = await searchContextAttachments(cwd, query, CONTEXT_SEARCH_LIMIT);
      }
    }
    if (seq !== contextSearchSeq) return;
    contextResults.value = results;
    contextMissingPath.value = missingPath;
    contextSearchError.value = searchError;
    contextActiveIndex.value = 0;
    contextUserInteracted.value = false;
    contextNoMatchSuppression.value = results.length === 0 &&
      !missingPath &&
      !searchError &&
      query.length > 0 &&
      !isContextPathQueryLike(range.query)
      ? { start: range.start, query: range.query }
      : null;
  } catch (err) {
    if (seq !== contextSearchSeq) return;
    contextSearchError.value = `搜索失败：${String(err)}`;
    contextNoMatchSuppression.value = null;
  } finally {
    if (seq === contextSearchSeq) {
      contextSearchLoading.value = false;
    }
  }
}

function suppressContextPanel() {
  contextSuppressedKey.value = mentionKey.value;
  clearContextSearch();
}

function replaceMentionWithText(range: MentionRange, text: string) {
  replaceComposerRange(range.start, range.end, [textPart(text)]);
  contextSuppressedKey.value = null;
}

function selectContextResult(result: ChatContextSearchResult | null) {
  if (!result || result.attachment.exists === false) return;
  const range = mentionRange.value;
  if (!range) return;
  const alreadyInserted = composerHasAttachmentPath(result.attachment.path);
  const replacement = alreadyInserted ? [] : [attachmentPart(result.attachment), textPart(" ")];
  replaceComposerRange(range.start, range.end, replacement);
  contextSuppressedKey.value = null;
  clearContextSearch();
  if (!alreadyInserted) emit("add-context-attachment", result.attachment);
}

function navigateContextDirectory(result: ChatContextSearchResult | null): boolean {
  if (!result || result.attachment.exists === false || result.attachment.kind !== "directory") {
    return false;
  }
  const range = mentionRange.value;
  if (!range) return false;
  const relativePath = compactPathLabel(result.relativePath);
  const nextQuery = relativePath.endsWith("/") ? relativePath : `${relativePath}/`;
  replaceMentionWithText(range, `@${nextQuery}`);
  contextUserInteracted.value = true;
  return true;
}

function parentContextQuery(query: string): string | null {
  const normalized = compactPathLabel(query.trim()).replace(/^\.\//, "");
  if (!normalized.includes("/") && !normalized.endsWith("/")) return null;
  const current = normalized.replace(/\/+$/, "");
  if (!current) return null;
  const slash = current.lastIndexOf("/");
  return slash < 0 ? "" : `${current.slice(0, slash)}/`;
}

function retreatContextDirectory(): boolean {
  const range = mentionRange.value;
  if (!range) return false;
  const parentQuery = parentContextQuery(range.query);
  if (parentQuery === null) return false;
  replaceMentionWithText(range, `@${parentQuery}`);
  contextUserInteracted.value = true;
  return true;
}

function moveContextActive(delta: number) {
  if (contextResults.value.length === 0) return;
  contextActiveIndex.value =
    (contextActiveIndex.value + delta + contextResults.value.length) %
    contextResults.value.length;
  contextUserInteracted.value = true;
}

function activateContextResult(index: number) {
  contextActiveIndex.value = index;
  contextUserInteracted.value = true;
}

function handleContextKeydown(e: KeyboardEvent): boolean {
  if (!contextPanelOpen.value) return false;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    moveContextActive(1);
    return true;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    moveContextActive(-1);
    return true;
  }
  if (e.key === "Enter") {
    const result = contextActiveResult.value;
    const query = mentionRange.value?.query.trim() ?? "";
    if (result && (query.length > 0 || contextUserInteracted.value)) {
      e.preventDefault();
      selectContextResult(result);
      return true;
    }
    if (query.length > 0 &&
      (contextSearchLoading.value || contextShowMissingPath.value || contextResults.value.length === 0)) {
      e.preventDefault();
      return true;
    }
    return false;
  }
  if (e.key === "Tab") {
    e.preventDefault();
    contextUserInteracted.value = true;
    navigateContextDirectory(contextActiveResult.value);
    return true;
  }
  if (e.key === "Escape") {
    e.preventDefault();
    if (retreatContextDirectory()) return true;
    suppressContextPanel();
    return true;
  }
  return false;
}

function patch(next: Partial<ChatComposerState>) {
  emit("update:state", { ...props.state, ...next });
}

function setPermission(v: PermissionMode) { patch({ permission: v }); }
function togglePlanMode() { patch({ planMode: !props.state.planMode }); }

function send() {
  const value = hasPending.value ? inputValue.value.trim() : composerSerializedText.value.trim();
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

  const attachments = props.attachments ?? [];
  if (!value && attachments.length === 0) return;
  emit("send", value, attachments);
  resetComposerInput();
}

function submitEntry() {
  if (canInterrupt.value) {
    emit("interrupt");
    return;
  }
  send();
}

function onKeydown(e: KeyboardEvent) {
  updateInputSelection();
  if (e.isComposing) return;
  if (handleContextKeydown(e)) return;
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    send();
  }
}

function onRichKeydown(e: KeyboardEvent) {
  inputSelection.value = captureRichSelectionOffset();
  if (e.isComposing) return;
  if (handleContextKeydown(e)) return;
  if (e.key === "Enter" && e.shiftKey) {
    e.preventDefault();
    replaceComposerRange(inputSelection.value, inputSelection.value, [textPart("\n")]);
    contextSuppressedKey.value = null;
    contextUserInteracted.value = false;
    return;
  }
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    send();
  }
}

function modifyPlanApproval() {
  if (!hasPendingInputText.value) return;
  submitAskFreeform();
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
    emit("resolve-tool-consent", decision, message, updatedInput);
  } else {
    emit("resolve-tool-consent", decision, message);
  }
  if (decision === "deny") pendingText.value = "";
}

watch(inputValue, () => {
  void nextTick(queueResize);
});

watch(richEditor, () => {
  renderRichEditorFromParts();
});

watch(
  () => [
    (props.attachments ?? []).map((attachment) => `${attachment.id}:${attachment.path}`).join("\n"),
    props.appendAttachmentsToEndKey ?? 0,
  ] as const,
  () => {
    if (hasPending.value) return;
    const nextAppendToEndKey = props.appendAttachmentsToEndKey ?? 0;
    const shouldAppendToEnd = nextAppendToEndKey !== observedAppendToEndKey;
    observedAppendToEndKey = nextAppendToEndKey;
    const incoming = props.attachments ?? [];
    const incomingPaths = new Set(incoming.map((attachment) => attachment.path));
    const filtered = composerParts.value.filter((part) =>
      part.type === "text" || incomingPaths.has(part.attachment.path)
    );
    if (filtered.length !== composerParts.value.length) {
      composerParts.value = normalizeComposerParts(filtered);
      inputSelection.value = Math.min(inputSelection.value, composerPartsLength());
      renderRichEditorFromParts();
    }

    let offset = shouldAppendToEnd ? composerPartsLength() : externalAttachmentInsertionOffset();
    for (const attachment of incoming) {
      if (composerHasAttachmentPath(attachment.path)) continue;
      if (insertAttachmentReference(attachment, offset)) {
        offset = inputSelection.value;
      }
    }
  },
  { immediate: true },
);

watch(
  () => [
    contextPanelOpen.value,
    mentionRange.value?.start ?? -1,
    mentionRange.value?.end ?? -1,
    mentionRange.value?.query ?? "",
    props.projectCwd ?? "",
  ] as const,
  () => {
    void refreshContextSearch(mentionRange.value);
  },
  { immediate: true },
);

watch(pendingKey, () => {
  if (!activeAsk.value) pendingText.value = "";
  toolExpanded.value = false;
  toolSubmitting.value = null;
  contextSuppressedKey.value = null;
  clearContextSearch();
  void nextTick(queueResize);
}, { immediate: true });

watch(
  () => askQuestion.value?.id,
  () => {
    void nextTick(queueResize);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  contextSearchSeq += 1;
  if (resizeFrameId !== null) {
    window.cancelAnimationFrame(resizeFrameId);
    resizeFrameId = null;
  }
  if (overflowTimerId !== null) {
    window.clearTimeout(overflowTimerId);
    overflowTimerId = null;
  }
});
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
        :is-empty="composerIsEmpty"
        @editor="setRichEditor"
        @input="onRichInput"
        @selection="onRichSelectionEvent"
        @keydown="onRichKeydown"
        @paste="onRichPaste"
      />

      <Transition name="chat-composer-entry-actions" mode="out-in">
        <div
          v-if="hasPending"
          :key="pendingEntryActionsKey"
          class="chat-composer__entry-actions"
        >
          <button
            v-if="askUsesInputActions && askQuestion?.skippable !== false && askTotal > 1"
            type="button"
            class="ghost composer-inline__skip composer-inline__btn"
            @click="skipAsk"
          >
            跳过
          </button>

          <div v-if="askUsesInputActions" class="chat-composer__pending-actions">
            <button
              v-if="canGoPrev"
              type="button"
              class="ghost composer-inline__btn"
              @click="backAsk"
            >
              <ArrowLeft :size="13" aria-hidden="true" />
              上一题
            </button>
            <button
              type="button"
              class="primary composer-inline__btn"
              :disabled="!canAskSubmit"
              @click="submitAsk"
            >
              {{ askIsLast ? "完成" : "继续" }}
              <ArrowRight v-if="!askIsLast" :size="13" aria-hidden="true" />
            </button>
          </div>

          <div v-else-if="askIsPlanApproval" class="chat-composer__pending-actions">
            <button
              type="button"
              class="ghost composer-inline__btn"
              :disabled="!hasPendingInputText"
              @click="modifyPlanApproval"
            >
              {{ hasPendingInputText ? "修改" : "忽略" }}
            </button>
            <button
              type="button"
              class="primary composer-inline__btn"
              @click="submitAsk"
            >
              同意
            </button>
          </div>

          <div v-else-if="activeToolConsent" class="chat-composer__pending-actions">
            <button
              type="button"
              class="ghost composer-inline__btn"
              :disabled="toolSubmitting !== null || (!isEditingToolCommand && !hasPendingInputText)"
              @click="isEditingToolCommand ? cancelCommandEdit() : decideToolConsent('deny')"
            >
              {{ toolSubmitting === "deny" ? "处理中..." : isEditingToolCommand ? "取消" : hasPendingInputText ? "修改" : "忽略" }}
            </button>
            <button
              type="button"
              class="composer-inline__btn"
              :class="toolDanger ? 'ghost danger' : 'primary'"
              :disabled="toolSubmitting !== null || toolCommandIsEmpty"
              @click="decideToolConsent('allow')"
            >
              {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
            </button>
          </div>

          <button
            v-if="!hasPending || (!askUsesInputActions && !askIsPlanApproval && !activeToolConsent)"
            type="button"
            class="chat-composer__send"
            :class="{ 'chat-composer__send--interrupt': canInterrupt }"
            :disabled="!canSubmitEntry"
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
      <ComposerContextPanel
        v-if="contextPanelOpen"
        :results="contextResults"
        :active-index="contextActiveIndex"
        :loading="contextSearchLoading"
        :error="contextSearchError"
        :missing-path="contextMissingPath"
        :show-missing-path="contextShowMissingPath"
        :is-large-directory="isLargeDirectory"
        :attachment-meta-label="attachmentMetaLabel"
        :context-inline-path="contextInlinePath"
        :context-attachment-icon="contextAttachmentIcon"
        @activate="activateContextResult"
        @select="selectContextResult"
      />
    </Transition>

    <Transition name="chat-composer-stack">
      <ComposerToolbar
        v-if="!hasPending"
        :state="state"
        :permission-options="permissionOptions"
        :preview-attachments="previewAttachments"
        :can-interrupt="canInterrupt"
        :can-submit-entry="canSubmitEntry"
        :send-title="sendTitle"
        :send-aria-label="sendAriaLabel"
        @pick-attachments="emit('pick-attachments')"
        @set-permission="setPermission"
        @toggle-plan-mode="togglePlanMode"
        @submit-entry="submitEntry"
        @open-image="openAttachmentImage"
      />
    </Transition>
  </div>
</template>
