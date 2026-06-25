import {
  deriveChatMessageDisplay,
  type ChatAttachment,
  type ChatConversationReference,
  type ChatMessage,
  type ChatMessageDisplay,
} from "@lilia/contracts";
import { measurePerfSync } from "../../utils/perf";

export type ChatBubbleMessage = ChatMessage & { streaming?: boolean; queued?: boolean };
export type ChatBubbleDisplay = ChatMessageDisplay;

type CachedChatBubbleDisplay = {
  attachmentSignature: string;
  content: string;
  conversationSignature: string;
  display: ChatBubbleDisplay;
};

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
    () => deriveChatMessageDisplay(message.content, message.attachments, conversationReferences),
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
