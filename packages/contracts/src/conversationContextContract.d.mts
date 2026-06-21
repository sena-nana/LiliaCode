export type ConversationContextToolName =
  | "QueryConversationContext"
  | "query_conversation_context"
  | "mcp__lilia__query_conversation_context";

export const CONVERSATION_CONTEXT_CONTRACT: Record<string, unknown>;
export const MAX_CONVERSATION_CONTEXT_MESSAGES: number;
export const MAX_CONVERSATION_CONTEXT_TEXT: number;
export const DEFAULT_CONVERSATION_CONTEXT_INCLUDE_MESSAGES: boolean;
export const CONVERSATION_CONTEXT_TOOL_NAMES: readonly ConversationContextToolName[];
export const CONVERSATION_CONTEXT_TOOL_NAME: "QueryConversationContext";
export const CONVERSATION_CONTEXT_CLAUDE_TOOL_NAME: "query_conversation_context";
export const CONVERSATION_CONTEXT_MCP_TOOL_NAME: "mcp__lilia__query_conversation_context";
export const QUERY_CONVERSATION_CONTEXT_INPUT_SCHEMA: Record<string, unknown>;
export function isLiliaConversationContextTool(
  toolName: unknown,
): toolName is ConversationContextToolName;
