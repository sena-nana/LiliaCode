import {
  serializeChatAttachmentReference,
  serializeConversationReference,
  type ChatAttachment,
  type ChatConversationReference,
} from "@lilia/contracts";

export const ATTACHMENT_OBJECT_CHAR = "\uFFFC";
export const CONVERSATION_REFERENCE_OBJECT_CHAR = "\uFFFD";

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

export interface ComposerConversationReferencePart {
  id: string;
  type: "conversationReference";
  reference: ChatConversationReference;
}

export type ComposerPart =
  | ComposerTextPart
  | ComposerAttachmentPart
  | ComposerConversationReferencePart;

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

export function conversationReferencePart(
  reference: ChatConversationReference,
): ComposerConversationReferencePart {
  return {
    id: nextComposerPartId("conversation-reference"),
    type: "conversationReference",
    reference,
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

export function serializeComposerParts(parts: ComposerPart[]): string {
  return parts
    .map((part) => {
      if (part.type === "text") return part.text;
      if (part.type === "attachment") return serializeChatAttachmentReference(part.attachment);
      return serializeConversationReference(part.reference);
    })
    .join("");
}

export function composerPartsSearchText(parts: ComposerPart[]): string {
  return parts
    .map((part) => {
      if (part.type === "text") return part.text;
      if (part.type === "attachment") return ATTACHMENT_OBJECT_CHAR;
      return CONVERSATION_REFERENCE_OBJECT_CHAR;
    })
    .join("");
}

export function composerPartsHaveAttachmentPath(parts: ComposerPart[], path: string): boolean {
  return parts.some((part) => part.type === "attachment" && part.attachment.path === path);
}

export function composerPartsHaveConversationReference(
  parts: ComposerPart[],
  taskId: string,
): boolean {
  return parts.some((part) => part.type === "conversationReference" && part.reference.taskId === taskId);
}

