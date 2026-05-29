export const PLAN_APPROVAL_QUESTION_ID = "approve-plan";

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

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function compactLine(value, max = 600) {
  if (typeof value !== "string") return "";
  const text = value.replace(/\s+/g, " ").trim();
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function readFirstString(record, keys, max = 6000) {
  if (!isRecord(record)) return "";
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) {
      const text = value.trim();
      return text.length > max ? `${text.slice(0, max)}...` : text;
    }
  }
  return "";
}

function parseJsonLike(value) {
  if (isRecord(value)) return value;
  if (typeof value !== "string" || !value.trim()) return {};
  try {
    const parsed = JSON.parse(value);
    return isRecord(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

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
  const prompts = Array.isArray(input?.allowedPrompts) ? input.allowedPrompts : [];
  return prompts
    .filter(isRecord)
    .map((item) => ({
      tool: compactLine(item.tool, 80) || "tool",
      prompt: compactLine(item.prompt, 400),
    }))
    .filter((item) => item.prompt);
}

export function extractPlanTextFromInput(input) {
  return readFirstString(input, ["plan", "content", "text", "markdown"], 12000);
}

export function extractPlanResult(output) {
  const parsed = parseJsonLike(output);
  const plan =
    readFirstString(parsed, ["plan", "content", "text", "markdown"], 12000) ||
    (typeof output === "string" ? output.trim() : "");
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
  const plan = result.plan || inputPlan || fallbackPlan || "";
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

export function buildPlanApprovalSpec() {
  return {
    title: "确认 Claude 计划",
    source: "Claude Plan",
    intent: "plan_approval",
    dismissable: true,
    questions: [
      {
        id: PLAN_APPROVAL_QUESTION_ID,
        header: "计划确认",
        question: "",
        mode: "confirm",
        confirmLabel: "按计划执行",
        cancelLabel: "先不执行",
      },
    ],
  };
}

export function isPlanApprovalAccepted(result) {
  if (!isRecord(result) || result.cancelled === true) return false;
  const answers = isRecord(result.answers) ? result.answers : {};
  const answer = answers[PLAN_APPROVAL_QUESTION_ID];
  return isRecord(answer) && answer.value === "yes";
}

export function readPlanRevisionRequest(result) {
  if (!isRecord(result) || result.cancelled === true) return "";
  const answers = isRecord(result.answers) ? result.answers : {};
  const answer = answers[PLAN_APPROVAL_QUESTION_ID];
  if (!isRecord(answer)) return "";
  if (answer.value === "yes" || answer.value === "no") return "";
  return compactLine(answer.notes, 6000);
}

export function buildPlanRevisionDenyMessage(revisionRequest) {
  const request = compactLine(revisionRequest, 6000);
  return [
    "用户要求修改计划，暂不执行当前计划。",
    request ? `修改要求：${request}` : "",
    "请根据这条修改要求调整计划，然后再次调用 ExitPlanMode 请求确认。",
  ].filter(Boolean).join("\n");
}
