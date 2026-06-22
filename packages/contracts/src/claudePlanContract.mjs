import claudePlanContract from "./claude-plan-contract.json" with { type: "json" };

const manifest = deepFreeze(claudePlanContract);

export const CLAUDE_PLAN_CONTRACT = manifest;
export const CLAUDE_PLAN_TOOL_NAMES = manifest.planToolNames;
export const CLAUDE_READONLY_DENIED_TOOLS = manifest.readonlyDeniedTools;
export const CLAUDE_READONLY_ALLOWED_TOOLS = manifest.readonlyAllowedTools;

const planToolNameSet = new Set(CLAUDE_PLAN_TOOL_NAMES);
const readonlyDeniedToolSet = new Set(CLAUDE_READONLY_DENIED_TOOLS);
const readonlyAllowedToolSet = new Set(CLAUDE_READONLY_ALLOWED_TOOLS);

export function isClaudePlanTool(toolName) {
  return planToolNameSet.has(String(toolName || ""));
}

export function isReadonlyDeniedClaudeTool(toolName) {
  const name = String(toolName || "");
  if (readonlyDeniedToolSet.has(name)) return true;
  return !readonlyAllowedToolSet.has(name);
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
