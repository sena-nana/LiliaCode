import { convertFileSrc } from "@tauri-apps/api/core";
import type { ChatAttachment } from "@lilia/contracts";

export interface ChatImageViewerSource {
  src: string;
  name?: string | null;
  path?: string | null;
  mime?: string | null;
  size?: number | null;
}

export function isImageAttachment(attachment: ChatAttachment): boolean {
  return attachment.exists !== false && !!attachment.mime?.startsWith("image/");
}

export function attachmentImageSrc(attachment: ChatAttachment): string | null {
  if (!isImageAttachment(attachment)) return null;
  try {
    return convertFileSrc(attachment.path);
  } catch {
    return null;
  }
}

export function imageViewerSourceFromAttachment(
  attachment: ChatAttachment,
): ChatImageViewerSource | null {
  const src = attachmentImageSrc(attachment);
  if (!src) return null;
  return {
    src,
    name: attachment.name,
    path: attachment.path,
    mime: attachment.mime ?? null,
    size: attachment.size ?? null,
  };
}
