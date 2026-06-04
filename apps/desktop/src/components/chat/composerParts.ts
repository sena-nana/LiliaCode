import type { ChatAttachment } from "@lilia/contracts";
import { isImageAttachment } from "./imageViewer";

export const ATTACHMENT_OBJECT_CHAR = "\uFFFC";

export interface MentionRange {
  start: number;
  end: number;
  query: string;
}

export interface ComposerTextPart {
  id: string;
  type: "text";
  text: string;
}

export interface ComposerAttachmentPart {
  id: string;
  type: "attachment";
  attachment: ChatAttachment;
}

export type ComposerPart = ComposerTextPart | ComposerAttachmentPart;

let composerPartSeq = 0;

function nextComposerPartId(prefix: string) {
  composerPartSeq += 1;
  return `${prefix}-${composerPartSeq}`;
}

export function textPart(text: string): ComposerTextPart {
  return {
    id: nextComposerPartId("text"),
    type: "text",
    text,
  };
}

export function attachmentPart(attachment: ChatAttachment): ComposerAttachmentPart {
  return {
    id: nextComposerPartId("attachment"),
    type: "attachment",
    attachment,
  };
}

export function composerPartLength(part: ComposerPart): number {
  return part.type === "text" ? part.text.length : 1;
}

export function composerPartsLength(parts: ComposerPart[]): number {
  return parts.reduce((total, part) => total + composerPartLength(part), 0);
}

export function normalizeComposerParts(parts: ComposerPart[]): ComposerPart[] {
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

export function splitComposerPartsAt(
  parts: ComposerPart[],
  offset: number,
): [ComposerPart[], ComposerPart[]] {
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

export function partsStartWithWhitespace(parts: ComposerPart[]): boolean {
  const first = parts.find((part) => part.type !== "text" || part.text.length > 0);
  return first?.type === "text" ? /^\s/.test(first.text) : false;
}

export function replaceComposerPartsRange(
  parts: ComposerPart[],
  start: number,
  end: number,
  replacement: ComposerPart[],
): { parts: ComposerPart[]; cursor: number } {
  const [before, rest] = splitComposerPartsAt(parts, start);
  const [, after] = splitComposerPartsAt(rest, Math.max(0, end - start));
  const nextParts = normalizeComposerParts([...before, ...replacement, ...after]);
  const nextCursor = composerPartsLength([...before, ...replacement]);
  return {
    parts: nextParts,
    cursor: Math.min(nextCursor, composerPartsLength(nextParts)),
  };
}

export function referenceKindLabel(attachment: ChatAttachment): string {
  if (isImageAttachment(attachment)) return "图片引用";
  if (attachment.kind === "directory") return "目录引用";
  return "文件引用";
}

export function serializeAttachmentReference(attachment: ChatAttachment): string {
  return `[${referenceKindLabel(attachment)}: ${attachment.name} | ${attachment.path}]`;
}

export function serializeComposerParts(parts: ComposerPart[]): string {
  return parts
    .map((part) => part.type === "text"
      ? part.text
      : serializeAttachmentReference(part.attachment))
    .join("");
}

export function composerPartsSearchText(parts: ComposerPart[]): string {
  return parts
    .map((part) => part.type === "text" ? part.text : ATTACHMENT_OBJECT_CHAR)
    .join("");
}

export function composerPartsHaveAttachmentPath(parts: ComposerPart[], path: string): boolean {
  return parts.some((part) => part.type === "attachment" && part.attachment.path === path);
}
