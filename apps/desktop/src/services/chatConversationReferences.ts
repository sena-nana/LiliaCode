import type { AgentTimelinePayload, ChatConversationReference } from "@lilia/contracts";

const CONVERSATION_REFERENCE_LABEL_PATTERN = /\[对话引用: ([^\]\n|]+?) \| ([^\]\n]+?)\]/g;

export function conversationReferencePattern(): RegExp {
  return new RegExp(CONVERSATION_REFERENCE_LABEL_PATTERN);
}

export function serializeConversationReference(reference: ChatConversationReference): string {
  return `[对话引用: ${reference.title} | ${reference.taskId}]`;
}

export function conversationReferenceToPayload(
  reference: ChatConversationReference,
): Record<string, AgentTimelinePayload> {
  return {
    taskId: reference.taskId,
    title: reference.title,
    route: reference.route,
    projectId: reference.projectId ?? null,
    projectName: reference.projectName ?? null,
  };
}

export function conversationReferencesToPayload(
  conversationReferences: ChatConversationReference[],
): AgentTimelinePayload[] {
  return conversationReferences.map(conversationReferenceToPayload);
}

export function isChatConversationReference(value: unknown): value is ChatConversationReference {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const row = value as Record<string, unknown>;
  return typeof row.taskId === "string" &&
    typeof row.title === "string" &&
    typeof row.route === "string" &&
    (typeof row.projectId === "string" || typeof row.projectId === "undefined" || row.projectId === null) &&
    (typeof row.projectName === "string" || typeof row.projectName === "undefined" || row.projectName === null);
}

export function readConversationReferences(value: unknown): ChatConversationReference[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter(isChatConversationReference)
    .map((reference) => ({
      taskId: reference.taskId,
      title: reference.title,
      route: reference.route,
      projectId: reference.projectId ?? undefined,
      projectName: reference.projectName ?? undefined,
    }));
}

export function resolveConversationReferenceMatch(
  match: RegExpMatchArray,
  conversationReferences: ChatConversationReference[],
): ChatConversationReference {
  const [, rawTitle, rawTaskId] = match;
  const title = rawTitle.trim();
  const taskId = rawTaskId.trim();
  return conversationReferences.find((reference) => reference.taskId === taskId) ??
    {
      taskId,
      title: title || taskId,
      route: "",
    };
}

export function stripSerializedConversationReferences(
  content: string,
  conversationReferences: ChatConversationReference[],
): string {
  let next = content;
  for (const reference of conversationReferences) {
    next = next.split(serializeConversationReference(reference)).join("");
  }
  return next;
}
