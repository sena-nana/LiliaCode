import { ref, type Ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ChatAttachment } from "@lilia/contracts";
import type { ChatImageViewerSource } from "../../components/chat/imageViewer";
import {
  describeAttachments,
  pickAttachmentFiles,
} from "../../services/chat";

export interface DropPoint {
  x: number;
  y: number;
}

export interface DropPayload {
  type: string;
  paths: string[];
  position: DropPoint | null;
}

export function isPointInsideElement(
  point: DropPoint | null,
  element: HTMLElement | null,
): boolean {
  if (!point || !element) return false;
  const rect = element.getBoundingClientRect();
  return point.x >= rect.left &&
    point.x <= rect.right &&
    point.y >= rect.top &&
    point.y <= rect.bottom;
}

export function readDropPayload(payload: unknown): DropPayload | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const row = payload as Record<string, unknown>;
  const paths = Array.isArray(row.paths)
    ? row.paths.filter((path): path is string => typeof path === "string")
    : [];
  const position = row.position && typeof row.position === "object" && !Array.isArray(row.position)
    ? row.position as Record<string, unknown>
    : null;
  const x = typeof position?.x === "number" ? position.x : null;
  const y = typeof position?.y === "number" ? position.y : null;
  return {
    type: typeof row.type === "string" ? row.type : "",
    paths,
    position: x === null || y === null ? null : { x, y },
  };
}

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

  function canAcceptFileDropAt(point: DropPoint | null): boolean {
    if (!options.hasContext()) return false;
    if (!options.canAcceptInteractiveDrop()) return false;
    if (!isPointInsideElement(point, options.chatPageRef.value)) return false;
    const sidebar = options.chatPageRef.value?.querySelector(".chat-sidebar");
    return !(sidebar instanceof HTMLElement && isPointInsideElement(point, sidebar));
  }

  async function normalizeDropPoint(point: DropPoint | null): Promise<DropPoint | null> {
    if (!point) return null;
    try {
      const scaleFactor = await appWindow.scaleFactor();
      if (!Number.isFinite(scaleFactor) || scaleFactor <= 0) return point;
      return {
        x: point.x / scaleFactor,
        y: point.y / scaleFactor,
      };
    } catch {
      return point;
    }
  }

  async function addAttachmentsFromPaths(paths: string[], appendToEnd = false) {
    const uniquePaths = paths.filter((path, index) =>
      paths.indexOf(path) === index &&
      !attachments.value.some((attachment) => attachment.path === path)
    );
    if (uniquePaths.length === 0) return;
    try {
      const described = await describeAttachments(uniquePaths);
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
    if (attachment.exists === false) return;
    if (attachments.value.some((item) => item.path === attachment.path)) return;
    attachments.value = [...attachments.value, attachment];
  }

  async function onPickAttachments() {
    try {
      const paths = await pickAttachmentFiles();
      await addAttachmentsFromPaths(paths);
    } catch (err) {
      console.error("[chat] pickAttachmentFiles failed", err);
    }
  }

  function removeAttachment(attachmentId: string) {
    attachments.value = attachments.value.filter((attachment) => attachment.id !== attachmentId);
  }

  async function installDragDropListener(): Promise<UnlistenFn> {
    return await getCurrentWebview().onDragDropEvent(async (event) => {
      const drop = readDropPayload(event.payload);
      if (!drop) return;
      if (drop.type === "leave") {
        fileDropActive.value = false;
        return;
      }
      const point = await normalizeDropPoint(drop.position);
      const canAccept = canAcceptFileDropAt(point);
      fileDropActive.value = (drop.type === "enter" || drop.type === "over") && canAccept;
      if (drop.type !== "drop") return;
      fileDropActive.value = false;
      if (!canAccept || drop.paths.length === 0) return;
      await addAttachmentsFromPaths(drop.paths, true);
    });
  }

  function resetAttachments() {
    attachments.value = [];
    fileDropActive.value = false;
    viewingImage.value = null;
  }

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
