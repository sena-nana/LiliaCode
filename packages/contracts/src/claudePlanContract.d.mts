export const CLAUDE_PLAN_CONTRACT: {
  planToolNames: readonly string[];
  readonlyDeniedTools: readonly string[];
  readonlyAllowedTools: readonly string[];
};

export const CLAUDE_PLAN_TOOL_NAMES: readonly string[];
export const CLAUDE_READONLY_DENIED_TOOLS: readonly string[];
export const CLAUDE_READONLY_ALLOWED_TOOLS: readonly string[];

export function isClaudePlanTool(toolName: unknown): boolean;

export function isReadonlyDeniedClaudeTool(toolName: unknown): boolean;
