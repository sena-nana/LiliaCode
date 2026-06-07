import { z } from "zod/v4";
import { isRecord, shortText, stringOrNull } from "./utils.mjs";

const MAX_MESSAGES = 12;
const MAX_TEXT = 1200;

export const LILIA_CONVERSATION_CONTEXT_TOOL_NAMES = new Set([
  "QueryConversationContext",
  "query_conversation_context",
  "mcp__lilia__query_conversation_context",
]);

export function isLiliaConversationContextTool(toolName) {
  return LILIA_CONVERSATION_CONTEXT_TOOL_NAMES.has(String(toolName || ""));
}

export const queryConversationContextInputSchema = {
  taskId: z.string().optional(),
  includeMessages: z.boolean().optional().default(true),
};

export const queryConversationContextJsonSchema = {
  type: "object",
  properties: {
    taskId: {
      type: "string",
      description: "Conversation task id to query. Defaults to the parent conversation.",
    },
    includeMessages: {
      type: "boolean",
      default: true,
      description: "Whether to include clipped readable timeline messages.",
    },
  },
  additionalProperties: false,
};

export const codexQueryConversationContextDynamicTool = {
  name: "QueryConversationContext",
  description:
    "Query Lilia conversation context available to this child conversation, including the parent conversation and readable message summaries.",
  inputSchema: queryConversationContextJsonSchema,
};

export function conversationContextEnabled(conversationContext) {
  return isRecord(conversationContext) &&
    stringOrNull(conversationContext.currentTaskId) &&
    stringOrNull(conversationContext.parentTaskId);
}

export function buildConversationContextToolDescription() {
  return [
    "Query Lilia conversation context available to this child conversation.",
    "Use it when the user asks about prior conversation details or asks you to continue from the parent conversation.",
    "If taskId is omitted, the parent conversation is returned.",
  ].join(" ");
}

function normalizeMessage(message) {
  if (!isRecord(message)) return null;
  const role = stringOrNull(message.role);
  const content = shortText(message.content, MAX_TEXT);
  if (!role || !content) return null;
  return {
    role,
    content,
    createdAt: typeof message.createdAt === "number" ? message.createdAt : null,
  };
}

function normalizeTask(task, includeMessages) {
  if (!isRecord(task)) return null;
  const taskId = stringOrNull(task.taskId);
  if (!taskId) return null;
  const messages = includeMessages && Array.isArray(task.messages)
    ? task.messages.map(normalizeMessage).filter(Boolean).slice(0, MAX_MESSAGES)
    : [];
  return {
    taskId,
    projectId: stringOrNull(task.projectId),
    title: stringOrNull(task.title) || "未命名对话",
    status: stringOrNull(task.status),
    parentId: stringOrNull(task.parentId),
    createdAt: typeof task.createdAt === "number" ? task.createdAt : null,
    messages,
    truncated: task.truncated === true || (Array.isArray(task.messages) && task.messages.length > messages.length),
  };
}

export function createConversationContextHandler(conversationContext) {
  return async function handleQueryConversationContext(input = {}) {
    if (!isRecord(conversationContext)) {
      return {
        ok: false,
        error: "Conversation context is not available for this turn.",
      };
    }
    const includeMessages = input.includeMessages !== false;
    const parentTaskId = stringOrNull(conversationContext.parentTaskId);
    const requestedTaskId = stringOrNull(input.taskId) || parentTaskId;
    const tasks = Array.isArray(conversationContext.tasks) ? conversationContext.tasks : [];
    const task = tasks.find((item) =>
      isRecord(item) && stringOrNull(item.taskId) === requestedTaskId
    );
    if (!task) {
      return {
        ok: false,
        currentTaskId: stringOrNull(conversationContext.currentTaskId),
        parentTaskId,
        requestedTaskId,
        availableTaskIds: tasks
          .map((item) => isRecord(item) ? stringOrNull(item.taskId) : null)
          .filter(Boolean),
        error: "Requested conversation is not in the available context set.",
      };
    }
    return {
      ok: true,
      currentTaskId: stringOrNull(conversationContext.currentTaskId),
      parentTaskId,
      requestedTaskId,
      task: normalizeTask(task, includeMessages),
    };
  };
}
