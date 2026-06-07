import { computed, nextTick, ref, watch, type ComputedRef } from "vue";
import type { ChatAttachment } from "@lilia/contracts";
import {
  attachmentPart,
  composerPartsHaveAttachmentPath,
  composerPartsLength,
  composerPartsSearchText,
  normalizeComposerParts,
  partsStartWithWhitespace,
  replaceComposerPartsRange,
  serializeComposerParts,
  splitComposerPartsAt,
  textPart,
  type ComposerAttachmentPart,
  type ComposerPart,
} from "./composerParts";
import { isImageAttachment } from "./imageViewer";

type InlineIconName = "file" | "folder" | "image" | "paperclip" | "x";

const SVG_NS = "http://www.w3.org/2000/svg";

const inlineIconMarkup: Record<InlineIconName, string> = {
  file: '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M16 13H8"/><path d="M16 17H8"/><path d="M10 9H8"/>',
  folder: '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>',
  image: '<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.09-3.09a2 2 0 0 0-2.82 0L6 21"/>',
  paperclip: '<path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
};

export function useComposerRichInput(options: {
  attachments: ComputedRef<ChatAttachment[]>;
  appendAttachmentsToEndKey: ComputedRef<number>;
  hasPending: ComputedRef<boolean>;
  isLargeDirectory: (attachment: ChatAttachment) => boolean;
  removeAttachment: (attachmentId: string) => void;
}) {
  const composerParts = ref<ComposerPart[]>([textPart("")]);
  const richEditor = ref<HTMLDivElement | null>(null);
  const inputSelection = ref(0);
  let observedAppendToEndKey = options.appendAttachmentsToEndKey.value;

  const serializedText = computed(() => serializeComposerParts(composerParts.value));
  const searchText = computed(() => composerPartsSearchText(composerParts.value));
  const isEmpty = computed(() => serializedText.value.length === 0);

  function hasAttachmentPath(path: string): boolean {
    return composerPartsHaveAttachmentPath(composerParts.value, path);
  }

  function attachmentFromComposerOrProps(id: string): ChatAttachment | null {
    const fromProps = options.attachments.value.find((attachment) => attachment.id === id);
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
      options.isLargeDirectory(attachment) ? "chat-file-reference--warning" : "",
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

    if (options.isLargeDirectory(attachment)) {
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

  function renderFromParts() {
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

  function captureSelectionOffset(): number {
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) return inputSelection.value;
    return richSelectionPointOffset(selection.focusNode, selection.focusOffset) ?? inputSelection.value;
  }

  function captureSelectionRange(): { start: number; end: number } | null {
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

  function findDomPosition(root: Node, offset: number): { node: Node; offset: number } {
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

  function focusAt(offset = inputSelection.value) {
    void nextTick(() => {
      const editor = richEditor.value;
      if (!editor) return;
      editor.focus();
      const position = findDomPosition(editor, offset);
      const range = document.createRange();
      range.setStart(position.node, position.offset);
      range.collapse(true);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      inputSelection.value = offset;
    });
  }

  function collectPartsFromNode(node: Node, parts: ComposerPart[]) {
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
      collectPartsFromNode(child, parts);
    }
  }

  function isBrowserEmptyEditor(editor: HTMLElement): boolean {
    return editor.childNodes.length === 1 && editor.firstChild instanceof HTMLBRElement;
  }

  function readPartsFromEditor(): ComposerPart[] {
    const editor = richEditor.value;
    if (!editor) return composerParts.value;
    if (isBrowserEmptyEditor(editor)) {
      return [textPart("")];
    }
    const parts: ComposerPart[] = [];
    for (const child of Array.from(editor.childNodes)) {
      collectPartsFromNode(child, parts);
    }
    return normalizeComposerParts(parts);
  }

  function syncRemovedInlineAttachments(nextParts: ComposerPart[]) {
    const nextPaths = new Set(nextParts
      .filter((part): part is ComposerAttachmentPart => part.type === "attachment")
      .map((part) => part.attachment.path));
    for (const attachment of options.attachments.value) {
      if (!nextPaths.has(attachment.path)) {
        options.removeAttachment(attachment.id);
      }
    }
  }

  function hasFocus(): boolean {
    const editor = richEditor.value;
    const active = document.activeElement;
    return !!editor && !!active && (active === editor || editor.contains(active));
  }

  function onInput() {
    const cursor = captureSelectionOffset();
    const nextParts = readPartsFromEditor();
    composerParts.value = nextParts;
    inputSelection.value = Math.min(cursor, composerPartsLength(nextParts));
    syncRemovedInlineAttachments(nextParts);
    if (!hasFocus()) focusAt(inputSelection.value);
  }

  function onSelection() {
    inputSelection.value = captureSelectionOffset();
  }

  function replaceRange(start: number, end: number, replacement: ComposerPart[]): number {
    const next = replaceComposerPartsRange(composerParts.value, start, end, replacement);
    composerParts.value = next.parts;
    inputSelection.value = next.cursor;
    renderFromParts();
    focusAt(inputSelection.value);
    return inputSelection.value;
  }

  function removeInlineAttachment(attachmentId: string) {
    const target = composerParts.value.find((part) =>
      part.type === "attachment" && part.attachment.id === attachmentId
    );
    composerParts.value = normalizeComposerParts(composerParts.value.filter((part) =>
      !(part.type === "attachment" && part.attachment.id === attachmentId)
    ));
    if (target?.type === "attachment") options.removeAttachment(target.attachment.id);
    inputSelection.value = Math.min(inputSelection.value, composerPartsLength(composerParts.value));
    renderFromParts();
    focusAt(inputSelection.value);
  }

  function insertAttachmentReference(attachment: ChatAttachment, offset = inputSelection.value): boolean {
    if (attachment.exists === false || hasAttachmentPath(attachment.path)) return false;
    const [before, after] = splitComposerPartsAt(composerParts.value, offset);
    const replacement: ComposerPart[] = [attachmentPart(attachment)];
    if (!partsStartWithWhitespace(after)) replacement.push(textPart(" "));
    composerParts.value = normalizeComposerParts([...before, ...replacement, ...after]);
    inputSelection.value = composerPartsLength([...before, ...replacement]);
    renderFromParts();
    focusAt(inputSelection.value);
    return true;
  }

  function externalInsertionOffset(): number {
    return hasFocus() ? captureSelectionOffset() : composerPartsLength(composerParts.value);
  }

  function resetInput() {
    const retainedAttachments = options.attachments.value;
    composerParts.value = retainedAttachments.length
      ? normalizeComposerParts([
        ...retainedAttachments.map(attachmentPart),
        textPart(" "),
      ])
      : [textPart("")];
    inputSelection.value = composerPartsLength(composerParts.value);
    renderFromParts();
  }

  function replaceWithText(text: string) {
    composerParts.value = [textPart(text)];
    inputSelection.value = text.length;
    renderFromParts();
    focusAt(inputSelection.value);
  }

  function setEditor(element: HTMLDivElement | null) {
    richEditor.value = element;
  }

  watch(richEditor, () => {
    renderFromParts();
  });

  watch(
    () => [
      options.attachments.value.map((attachment) => `${attachment.id}:${attachment.path}`).join("\n"),
      options.appendAttachmentsToEndKey.value,
    ] as const,
    () => {
      if (options.hasPending.value) return;
      const nextAppendToEndKey = options.appendAttachmentsToEndKey.value;
      const shouldAppendToEnd = nextAppendToEndKey !== observedAppendToEndKey;
      observedAppendToEndKey = nextAppendToEndKey;
      const incoming = options.attachments.value;
      const incomingPaths = new Set(incoming.map((attachment) => attachment.path));
      const filtered = composerParts.value.filter((part) =>
        part.type === "text" || incomingPaths.has(part.attachment.path)
      );
      if (filtered.length !== composerParts.value.length) {
        composerParts.value = normalizeComposerParts(filtered);
        inputSelection.value = Math.min(inputSelection.value, composerPartsLength(composerParts.value));
        renderFromParts();
      }

      let offset = shouldAppendToEnd
        ? composerPartsLength(composerParts.value)
        : externalInsertionOffset();
      for (const attachment of incoming) {
        if (hasAttachmentPath(attachment.path)) continue;
        if (insertAttachmentReference(attachment, offset)) {
          offset = inputSelection.value;
        }
      }
    },
    { immediate: true },
  );

  return {
    inputSelection,
    serializedText,
    searchText,
    isEmpty,
    setEditor,
    onInput,
    onSelection,
    replaceRange,
    replaceWithText,
    insertAttachmentReference,
    resetInput,
    captureSelectionOffset,
    captureSelectionRange,
    hasAttachmentPath,
  };
}
