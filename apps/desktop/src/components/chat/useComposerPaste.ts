import type { ChatAttachment } from "@lilia/contracts";
import { measurePerfAsync } from "../../utils/perf";
import { textPart } from "./composerParts";
import { pasteHasFileItems, pastedImageFiles, pastedPlainText } from "./composerPasteEvent";
import type { useComposerRichInput } from "./useComposerRichInput";

const LONG_PASTE_TEXT_THRESHOLD = 2000;

type ComposerRichInput = ReturnType<typeof useComposerRichInput>;
type ComposerPasteDeps = {
  describeAttachments: typeof import("../../services/chat").describeAttachments;
  readClipboardFilePaths: typeof import("../../services/chat").readClipboardFilePaths;
  saveClipboardImage: typeof import("../../services/chat").saveClipboardImage;
  saveClipboardText: typeof import("../../services/chat").saveClipboardText;
};

let composerPasteDepsLoad: Promise<ComposerPasteDeps> | null = null;

async function loadComposerPasteDeps(): Promise<ComposerPasteDeps> {
  if (!composerPasteDepsLoad) {
    composerPasteDepsLoad = measurePerfAsync(
      "chat-composer.paste.load",
      async () => {
        const {
          describeAttachments,
          readClipboardFilePaths,
          saveClipboardImage,
          saveClipboardText,
        } = await import("../../services/chat");
        return {
          describeAttachments,
          readClipboardFilePaths,
          saveClipboardImage,
          saveClipboardText,
        };
      },
    );
  }
  return composerPasteDepsLoad;
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

async function savePastedImages(
  deps: ComposerPasteDeps,
  files: File[],
): Promise<ChatAttachment[]> {
  return measurePerfAsync(
    "chat-composer.paste.images.save",
    () => Promise.all(files.map(async (file) => {
      const bytesBase64 = await fileToBase64(file);
      return deps.saveClipboardImage({
        mime: file.type || null,
        bytesBase64,
        name: file.name || null,
      });
    })),
  );
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
    const deps = await loadComposerPasteDeps();
    return measurePerfAsync(
      "chat-composer.paste.describe",
      () => deps.describeAttachments(uniquePaths),
    );
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
      const deps = await loadComposerPasteDeps();
      const paths = await measurePerfAsync(
        "chat-composer.paste.clipboard-paths",
        () => deps.readClipboardFilePaths(),
      );
      const pathAttachments = await attachmentsFromPastedFilePaths(paths);
      const imageAttachments = pathAttachments.length > 0 ? [] : await savePastedImages(deps, imageFiles);
      await insertPastedAttachments([...pathAttachments, ...imageAttachments], offset);
    } catch (err) {
      console.error("[chat] paste context failed", err);
    }
  }

  async function handleLongTextPaste(text: string, offset: number) {
    try {
      const deps = await loadComposerPasteDeps();
      const attachment = await measurePerfAsync(
        "chat-composer.paste.long-text.save",
        () => deps.saveClipboardText({ text }),
      );
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
