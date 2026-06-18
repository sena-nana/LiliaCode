import type { ChatAttachment, ChatConversationReference, ChatMessage } from "@lilia/contracts";
import { conversationReferencePattern } from "../../services/chatConversationReferences";
import { isImageAttachment } from "./imageViewer";
import { measurePerfSync } from "../../utils/perf";

export type ChatBubbleMessage = ChatMessage & { streaming?: boolean; queued?: boolean };

export type MessageSegment =
  | { type: "text"; text: string }
  | { type: "attachment"; attachment: ChatAttachment }
  | { type: "conversationReference"; reference: ChatConversationReference };

export type ChatBubbleDisplay = {
  segments: MessageSegment[];
  previewAttachments: ChatAttachment[];
  unreferencedAttachments: ChatAttachment[];
};

type CachedChatBubbleDisplay = {
  attachmentSignature: string;
  content: string;
  conversationSignature: string;
  display: ChatBubbleDisplay;
};

const referencePattern = /\[(文件引用|目录引用|图片引用): ([^\]\n|]+?) \| ([^\]\n]+?)\]/g;
const chatBubbleDisplayCache = new WeakMap<object, CachedChatBubbleDisplay>();

export function readChatBubbleDisplay(message: ChatBubbleMessage): ChatBubbleDisplay {
  const conversationReferences = message.conversationReferences ?? [];
  const attachmentSignature = createAttachmentSignature(message.attachments);
  const conversationSignature = createConversationReferenceSignature(conversationReferences);
  const cached = chatBubbleDisplayCache.get(message);
  if (
    cached &&
    cached.content === message.content &&
    cached.attachmentSignature === attachmentSignature &&
    cached.conversationSignature === conversationSignature
  ) {
    return cached.display;
  }

  const display = measurePerfSync(
    "chat-bubble.derive",
    () => deriveChatBubbleDisplay(message.content, message.attachments, conversationReferences),
    { detail: `${message.taskId}:${message.id}` },
  );

  chatBubbleDisplayCache.set(message, {
    content: message.content,
    attachmentSignature,
    conversationSignature,
    display,
  });
  return display;
}

function deriveChatBubbleDisplay(
  content: string,
  attachments: ChatAttachment[],
  conversationReferences: ChatConversationReference[],
): ChatBubbleDisplay {
  const previewAttachments: ChatAttachment[] = [];
  const attachmentByPath = new Map<string, ChatAttachment>();
  for (const attachment of attachments) {
    attachmentByPath.set(attachment.path, attachment);
    if (isImageAttachment(attachment)) previewAttachments.push(attachment);
  }

  const conversationReferenceByTaskId = new Map<string, ChatConversationReference>();
  for (const reference of conversationReferences) {
    conversationReferenceByTaskId.set(reference.taskId, reference);
  }

  const segments: MessageSegment[] = [];
  const referencedAttachmentPaths = new Set<string>();
  let cursor = 0;
  const matches = [
    ...Array.from(content.matchAll(referencePattern), (match) => ({
      kind: "attachment" as const,
      match,
    })),
    ...Array.from(content.matchAll(conversationReferencePattern()), (match) => ({
      kind: "conversationReference" as const,
      match,
    })),
  ].sort((a, b) => (a.match.index ?? 0) - (b.match.index ?? 0));

  for (const entry of matches) {
    const start = entry.match.index ?? 0;
    if (start > cursor) segments.push({ type: "text", text: content.slice(cursor, start) });
    if (entry.kind === "attachment") {
      const [, label, rawName, rawPath] = entry.match;
      const name = rawName.trim();
      const path = rawPath.trim();
      referencedAttachmentPaths.add(path);
      segments.push({
        type: "attachment",
        attachment: attachmentByPath.get(path) ?? fallbackAttachment(label, name, path),
      });
    } else {
      const [, rawTitle, rawTaskId] = entry.match;
      const title = rawTitle.trim();
      const taskId = rawTaskId.trim();
      segments.push({
        type: "conversationReference",
        reference: conversationReferenceByTaskId.get(taskId) ?? {
          taskId,
          title: title || taskId,
          route: "",
        },
      });
    }
    cursor = start + entry.match[0].length;
  }

  if (cursor < content.length) segments.push({ type: "text", text: content.slice(cursor) });
  const normalizedSegments = segments.length ? segments : [{ type: "text" as const, text: content }];
  const unreferencedAttachments = attachments.filter((attachment) =>
    !referencedAttachmentPaths.has(attachment.path) && !isImageAttachment(attachment)
  );

  return {
    segments: normalizedSegments,
    previewAttachments,
    unreferencedAttachments,
  };
}

function fallbackAttachment(label: string, name: string, path: string): ChatAttachment {
  return {
    id: `inline-${path}`,
    name: name || path,
    path,
    kind: label === "目录引用" ? "directory" : "file",
    size: null,
    exists: true,
    mime: label === "图片引用" ? "image/*" : null,
    directory: null,
  };
}

function createAttachmentSignature(attachments: ChatAttachment[]): string {
  return attachments
    .map((attachment) =>
      [
        attachment.id,
        attachment.name,
        attachment.path,
        attachment.kind,
        attachment.mime ?? "",
        attachment.exists === false ? "0" : "1",
      ].join("\u0001"))
    .join("\u0002");
}

function createConversationReferenceSignature(conversationReferences: ChatConversationReference[]): string {
  return conversationReferences
    .map((reference) =>
      [
        reference.taskId,
        reference.title,
        reference.route,
        reference.projectId ?? "",
        reference.projectName ?? "",
      ].join("\u0001"))
    .join("\u0002");
}
