import conversationContextContract from "./conversation-context-contract.json" with { type: "json" };

const manifest = deepFreeze(conversationContextContract);

export const CONVERSATION_CONTEXT_CONTRACT = manifest;
export const MAX_CONVERSATION_CONTEXT_MESSAGES = manifest.maxConversationContextMessages;
export const MAX_CONVERSATION_CONTEXT_TEXT = manifest.maxConversationContextText;
export const DEFAULT_CONVERSATION_CONTEXT_INCLUDE_MESSAGES =
  manifest.defaultConversationContextIncludeMessages;
export const CONVERSATION_CONTEXT_TOOL_NAMES = manifest.conversationContextToolNames;
export const CONVERSATION_CONTEXT_TOOL_NAME = CONVERSATION_CONTEXT_TOOL_NAMES[0];
export const CONVERSATION_CONTEXT_CLAUDE_TOOL_NAME = CONVERSATION_CONTEXT_TOOL_NAMES[1];
export const CONVERSATION_CONTEXT_MCP_TOOL_NAME = CONVERSATION_CONTEXT_TOOL_NAMES[2];
export const QUERY_CONVERSATION_CONTEXT_INPUT_SCHEMA =
  manifest.queryConversationContextInputSchema;

const conversationContextToolNameSet = new Set(CONVERSATION_CONTEXT_TOOL_NAMES);

export function isLiliaConversationContextTool(toolName) {
  return conversationContextToolNameSet.has(String(toolName || ""));
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
