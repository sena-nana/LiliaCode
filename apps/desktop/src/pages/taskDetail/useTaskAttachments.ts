import { onScopeDispose, ref, type Ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ChatAttachment } from "@lilia/contracts";
import type { ChatImageViewerSource } from "../../components/chat/imageViewer";
import {
  describeAttachments,
  pickAttachmentFiles,
} from "../../services/chat";
import {
  isPointInsideElement,
  normalizeDropPoint as normalizeWebviewDropPoint,
  readDropPayload,
  type DropPoint,
} from "../../composables/webviewDrop";

export function useTaskAttachments(options: {
  chatPageRef: Ref<HTMLElement | null>;
  attachments?: Ref<ChatAttachment[]>;
  hasContext: () => boolean;
  canAcceptInteractiveDrop: () => boolean;
}) {
  const attachments = options.attachments ?? ref<ChatAttachment[]>([]);
  const viewingImage = ref<ChatImageViewerSource | null>(null);
  const droppedAttachmentAppendKey = ref(0);
  const fileDropActive = ref(false);
  const appWindow = getCurrentWindow();
  let disposed = false;

  function canAcceptFileDropAt(point: DropPoint | null): boolean {
    if (!options.hasContext()) return false;
    if (!options.canAcceptInteractiveDrop()) return false;
    if (!isPointInsideElement(point, options.chatPageRef.value)) return false;
    const sidebar = options.chatPageRef.value?.querySelector(".chat-sidebar");
    return !(sidebar instanceof HTMLElement && isPointInsideElement(point, sidebar));
  }

  async function normalizeDropPoint(point: DropPoint | null): Promise<DropPoint | null> {
    if (disposed) return null;
    const normalized = await normalizeWebviewDropPoint(point, () => appWindow.scaleFactor());
    return disposed ? null : normalized;
  }

  async function addAttachmentsFromPaths(paths: string[], appendToEnd = false) {
    if (disposed) return;
    const uniquePaths = paths.filter((path, index) =>
      paths.indexOf(path) === index &&
      !attachments.value.some((attachment) => attachment.path === path)
    );
    if (uniquePaths.length === 0) return;
    try {
      const described = await describeAttachments(uniquePaths);
      if (disposed) return;
      const existing = new Set(attachments.value.map((attachment) => attachment.path));
      const nextAttachments = described.filter((attachment) => !existing.has(attachment.path));
      if (appendToEnd && nextAttachments.length > 0) {
        droppedAttachmentAppendKey.value += 1;
      }
      attachments.value = [
        ...attachments.value,
        ...nextAttachments,
      ];
    } catch (err) {
      console.error("[chat] describeAttachments failed", err);
    }
  }

  function addContextAttachment(attachment: ChatAttachment) {
    if (disposed) return;
    if (attachment.exists === false) return;
    if (attachments.value.some((item) => item.path === attachment.path)) return;
    attachments.value = [...attachments.value, attachment];
  }

  async function onPickAttachments() {
    if (disposed) return;
    try {
      const paths = await pickAttachmentFiles();
      if (disposed) return;
      await addAttachmentsFromPaths(paths);
    } catch (err) {
      if (disposed) return;
      console.error("[chat] pickAttachmentFiles failed", err);
    }
  }

  function removeAttachment(attachmentId: string) {
    if (disposed) return;
    attachments.value = attachments.value.filter((attachment) => attachment.id !== attachmentId);
  }

  async function installDragDropListener(): Promise<UnlistenFn> {
    if (disposed) return () => {};
    const unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
      if (disposed) return;
      const drop = readDropPayload(event.payload);
      if (!drop) return;
      if (drop.type === "leave") {
        fileDropActive.value = false;
        return;
      }
      const point = await normalizeDropPoint(drop.position);
      if (disposed) return;
      const canAccept = canAcceptFileDropAt(point);
      fileDropActive.value = (drop.type === "enter" || drop.type === "over") && canAccept;
      if (drop.type !== "drop") return;
      fileDropActive.value = false;
      if (!canAccept || drop.paths.length === 0) return;
      await addAttachmentsFromPaths(drop.paths, true);
    });
    if (disposed) {
      unlisten();
      return () => {};
    }
    return unlisten;
  }

  function resetAttachments() {
    if (disposed) return;
    attachments.value = [];
    fileDropActive.value = false;
    viewingImage.value = null;
  }

  onScopeDispose(() => {
    disposed = true;
  });

  return {
    attachments,
    viewingImage,
    droppedAttachmentAppendKey,
    fileDropActive,
    addAttachmentsFromPaths,
    addContextAttachment,
    onPickAttachments,
    removeAttachment,
    installDragDropListener,
    resetAttachments,
  };
}

