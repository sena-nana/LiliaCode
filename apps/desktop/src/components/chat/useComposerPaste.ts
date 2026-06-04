import type { ChatAttachment } from "@lilia/contracts";
import {
  describeAttachments,
  readClipboardFilePaths,
  saveClipboardImage,
  saveClipboardText,
} from "../../services/chat";
import { textPart } from "./composerParts";
import type { useComposerRichInput } from "./useComposerRichInput";

const LONG_PASTE_TEXT_THRESHOLD = 2000;

type ComposerRichInput = ReturnType<typeof useComposerRichInput>;

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

export function useComposerPaste(options: {
  richInput: ComposerRichInput;
  clearContextSearch: () => void;
  addContextAttachment: (attachment: ChatAttachment) => void;
  hasPending: () => boolean;
}) {
  async function attachmentsFromPastedFilePaths(paths: string[]): Promise<ChatAttachment[]> {
    const uniquePaths = paths.filter((path, index) =>
      path.trim().length > 0 &&
      paths.indexOf(path) === index &&
      !options.richInput.hasAttachmentPath(path)
    );
    if (uniquePaths.length === 0) return [];
    return describeAttachments(uniquePaths);
  }

  async function insertPastedAttachments(attachments: ChatAttachment[], offset: number) {
    let nextOffset = offset;
    for (const attachment of attachments) {
      if (options.richInput.insertAttachmentReference(attachment, nextOffset)) {
        options.addContextAttachment(attachment);
        nextOffset = options.richInput.inputSelection.value;
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

  function onPaste(event: ClipboardEvent) {
    if (options.hasPending()) return;
    const hasFiles = pasteHasFileItems(event);
    const plainText = pastedPlainText(event);
    if (!hasFiles && !plainText) return;
    event.preventDefault();
    const range = options.richInput.captureSelectionRange();
    let offset = range?.start ?? options.richInput.captureSelectionOffset();
    const end = range?.end ?? offset;
    options.richInput.inputSelection.value = offset;
    options.clearContextSearch();
    if (!hasFiles) {
      if (plainText.length < LONG_PASTE_TEXT_THRESHOLD) {
        options.richInput.replaceRange(offset, end, [textPart(plainText)]);
        return;
      }
      if (end > offset) {
        offset = options.richInput.replaceRange(offset, end, []);
      }
      void handleLongTextPaste(plainText, offset);
      return;
    }
    if (end > offset) {
      offset = options.richInput.replaceRange(offset, end, []);
    }
    const imageFiles = pastedImageFiles(event);
    void handleRichPaste(imageFiles, offset);
  }

  return {
    onPaste,
  };
}
