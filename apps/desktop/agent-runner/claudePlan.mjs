export const PLAN_APPROVAL_QUESTION_ID = "approve-plan";

export {
  buildPlanApprovalSpec,
  buildPlanRevisionDenyMessage,
  isPlanApprovalAccepted,
  readPlanRevisionRequest,
} from "./planApproval.mjs";

import {
  compactLine,
  parseRecordJson,
  readArrayRecords,
  readFirstString,
  readFirstText,
} from "../../../packages/contracts/src/toolUtils.mjs";

const PLAN_TOOL_NAMES = new Set(["ExitPlanMode", "exit_plan_mode"]);
const READONLY_DENIED_TOOLS = new Set([
  "Bash",
  "Write",
  "Edit",
  "MultiEdit",
  "NotebookEdit",
]);
const READONLY_ALLOWED_TOOLS = new Set([
  "Read",
  "LS",
  "Glob",
  "Grep",
  "WebFetch",
  "WebSearch",
  "NotebookRead",
  "TodoWrite",
]);

export function isClaudePlanTool(toolName) {
  return PLAN_TOOL_NAMES.has(String(toolName || ""));
}

export function isReadonlyDeniedClaudeTool(toolName) {
  const name = String(toolName || "");
  if (READONLY_DENIED_TOOLS.has(name)) return true;
  return !READONLY_ALLOWED_TOOLS.has(name);
}

export function normalizeClaudePermissionMode(permission) {
  switch (permission) {
    case "full":
      return "bypassPermissions";
    case "ask":
    case "readonly":
    default:
      return "default";
  }
}

export function readPlanAllowedPrompts(input) {
  return readArrayRecords(input?.allowedPrompts)
    .map((item) => ({
      tool: compactLine(item.tool, 80) || "tool",
      prompt: compactLine(item.prompt, 400),
    }))
    .filter((item) => item.prompt);
}

export function extractPlanTextFromInput(input) {
  return readFirstText(input, ["plan", "content", "text", "markdown"], 12000);
}

export function extractPlanResult(output) {
  const parsed = parseRecordJson(output) ?? {};
  const plan = readFirstText(parsed, ["plan", "content", "text", "markdown"], 12000);
  return {
    plan,
    filePath: readFirstString(parsed, ["filePath", "file_path"], 1200) || undefined,
    isAgent: parsed.isAgent === true,
    hasTaskTool: parsed.hasTaskTool === true,
    planWasEdited: parsed.planWasEdited === true,
    awaitingLeaderApproval: parsed.awaitingLeaderApproval === true,
    revisionRequest: readFirstString(parsed, ["revisionRequest", "revision_request"], 6000),
  };
}
export function buildPlanPayload({
  input,
  output,
  fallbackPlan = "",
  approved = null,
  executionPermission,
  source = "ExitPlanMode",
}) {
  const result = extractPlanResult(output);
  const inputPlan = extractPlanTextFromInput(input);
  const revisionRequest =
    result.revisionRequest ||
    readFirstString(input, ["revisionRequest", "revision_request"], 6000);
  const preserveInputPlan = Boolean(revisionRequest) || approved === false;
  const plan = preserveInputPlan
    ? inputPlan || fallbackPlan || result.plan || ""
    : result.plan || inputPlan || fallbackPlan || "";
  return {
    source,
    plan,
    allowedPrompts: readPlanAllowedPrompts(input),
    approved,
    executionPermission,
    ...(revisionRequest ? { revisionRequest } : {}),
    ...(result.filePath ? { filePath: result.filePath } : {}),
    ...(result.planWasEdited ? { planWasEdited: true } : {}),
    ...(result.awaitingLeaderApproval ? { awaitingLeaderApproval: true } : {}),
    ...(result.isAgent ? { isAgent: true } : {}),
    ...(result.hasTaskTool ? { hasTaskTool: true } : {}),
  };
}
