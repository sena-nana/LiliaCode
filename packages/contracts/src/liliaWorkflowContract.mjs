import liliaWorkflowContract from "./lilia-workflow-contract.json" with { type: "json" };

const manifest = deepFreeze(liliaWorkflowContract);

export const LILIA_WORKFLOW_CONTRACT = manifest;
export const LILIA_QUERY_WORKFLOW_TYPES = manifest.queryWorkflowTypes;
export const LILIA_TASK_WORKFLOW_TYPE = manifest.taskWorkflow.type;
export const LILIA_TASK_WORKFLOW_KINDS = manifest.taskWorkflow.kinds;
export const LILIA_REVIEW_WORKFLOW_TYPE = manifest.review.type;
export const LILIA_REVIEW_TARGET_TYPES = manifest.review.targetTypes;
export const LILIA_REVIEW_DELIVERIES = manifest.review.deliveries;
export const DEFAULT_LILIA_REVIEW_DELIVERY = manifest.review.defaultDelivery;
export const LILIA_FIX_SUGGESTION_WORKFLOW_TYPE = manifest.fixSuggestion.type;
export const LILIA_FIX_SUGGESTION_MODES = manifest.fixSuggestion.modes;
export const DEFAULT_LILIA_FIX_SUGGESTION_MODE = manifest.fixSuggestion.defaultMode;
export const LILIA_BATCH_APPLY_WORKFLOW_TYPE = manifest.batchApply.type;
export const LILIA_BATCH_APPLY_SOURCE_KINDS = manifest.batchApply.sourceKinds;
export const LILIA_GOAL_WORKFLOW_TYPE = manifest.goal.type;
export const LILIA_GOAL_ACTIONS = manifest.goal.actions;
export const LILIA_GOAL_STATUSES = manifest.goal.statuses;
export const DEFAULT_LILIA_GOAL_STATUS = manifest.goal.defaultStatus;
export const LILIA_MEMORY_MODE_WORKFLOW_TYPE = manifest.memoryMode.type;
export const LILIA_MEMORY_MODES = manifest.memoryMode.modes;
export const LILIA_MEMORY_RESET_WORKFLOW_TYPE = manifest.memoryReset.type;
export const LILIA_COMPACT_WORKFLOW_TYPE = manifest.compact.type;
export const LILIA_BACKGROUND_TERMINALS_CLEAN_WORKFLOW_TYPE =
  manifest.backgroundTerminalsClean.type;
export const LILIA_CONFIG_DIAGNOSTICS_WORKFLOW_TYPE = manifest.configDiagnostics.type;
export const DEFAULT_LILIA_CONFIG_DIAGNOSTICS_INCLUDE_LAYERS =
  manifest.configDiagnostics.defaultIncludeLayers;
export const AUTOMATION_WORKFLOW_TYPE = manifest.automation.type;
export const CHAT_SLASH_COMMAND_WORKFLOW_TYPE = manifest.slashCommand.type;

const reviewTargetTypeSet = new Set(LILIA_REVIEW_TARGET_TYPES);
const queryWorkflowTypeSet = new Set(LILIA_QUERY_WORKFLOW_TYPES);
const taskWorkflowKindSet = new Set(LILIA_TASK_WORKFLOW_KINDS);
const reviewDeliverySet = new Set(LILIA_REVIEW_DELIVERIES);
const fixSuggestionModeSet = new Set(LILIA_FIX_SUGGESTION_MODES);
const batchApplySourceKindSet = new Set(LILIA_BATCH_APPLY_SOURCE_KINDS);
const goalActionSet = new Set(LILIA_GOAL_ACTIONS);
const goalStatusSet = new Set(LILIA_GOAL_STATUSES);
const goalRequiresObjectiveSet = new Set(manifest.goal.requiresObjective);
const memoryModeSet = new Set(LILIA_MEMORY_MODES);

export function isLiliaReviewTargetType(value) {
  return typeof value === "string" && reviewTargetTypeSet.has(value);
}

export function isLiliaQueryWorkflowType(value) {
  return typeof value === "string" && queryWorkflowTypeSet.has(value);
}

export function isLiliaTaskWorkflowKind(value) {
  return typeof value === "string" && taskWorkflowKindSet.has(value);
}

export function isLiliaReviewDelivery(value) {
  return typeof value === "string" && reviewDeliverySet.has(value);
}

export function isLiliaFixSuggestionMode(value) {
  return typeof value === "string" && fixSuggestionModeSet.has(value);
}

export function isLiliaBatchApplySourceKind(value) {
  return typeof value === "string" && batchApplySourceKindSet.has(value);
}

export function isLiliaGoalAction(value) {
  return typeof value === "string" && goalActionSet.has(value);
}

export function isLiliaGoalStatus(value) {
  return typeof value === "string" && goalStatusSet.has(value);
}

export function isLiliaMemoryMode(value) {
  return typeof value === "string" && memoryModeSet.has(value);
}

export function normalizeLiliaGoalStatus(value) {
  return isLiliaGoalStatus(value) ? value : DEFAULT_LILIA_GOAL_STATUS;
}

export function normalizeLiliaReviewTarget(value) {
  const target = isRecord(value) ? value : null;
  const type = stringOrNull(target?.type);
  if (!isLiliaReviewTargetType(type)) return null;
  if (type === "uncommittedChanges") return { type };
  if (type === "baseBranch") {
    const branch = stringOrNull(target?.branch)?.trim() || "";
    return branch ? { type, branch } : null;
  }
  const sha = stringOrNull(target?.sha)?.trim() || "";
  return sha ? { type, sha } : null;
}

export function normalizeLiliaReviewDelivery(value) {
  return isLiliaReviewDelivery(value) ? value : DEFAULT_LILIA_REVIEW_DELIVERY;
}

export function normalizeLiliaTaskWorkflow(value) {
  const workflow = isRecord(value) && value.type === LILIA_TASK_WORKFLOW_TYPE
    ? value
    : null;
  if (!workflow) return null;
  const kind = stringOrNull(workflow.kind);
  if (!isLiliaTaskWorkflowKind(kind)) {
    throw new Error("Lilia task workflow missing a valid kind");
  }
  return {
    kind,
    instructions: stringOrNull(workflow.instructions)?.trim() || "",
  };
}

export function normalizeLiliaFixSuggestionMode(value) {
  return isLiliaFixSuggestionMode(value) ? value : DEFAULT_LILIA_FIX_SUGGESTION_MODE;
}

export function normalizeLiliaReviewWorkflow(value) {
  const workflow = isRecord(value) && value.type === LILIA_REVIEW_WORKFLOW_TYPE
    ? value
    : null;
  if (!workflow) return null;
  const target = normalizeLiliaReviewTarget(workflow.target);
  if (!target) throw new Error("Lilia review workflow missing a valid target");
  return {
    target,
    instructions: stringOrNull(workflow.instructions)?.trim() || "",
    delivery: normalizeLiliaReviewDelivery(workflow.delivery),
  };
}

export function normalizeLiliaFixSuggestionWorkflow(value) {
  const workflow = isRecord(value) && value.type === LILIA_FIX_SUGGESTION_WORKFLOW_TYPE
    ? value
    : null;
  if (!workflow) return null;
  const target = normalizeLiliaReviewTarget(workflow.target);
  if (!target) throw new Error("Lilia fix suggestion workflow missing a valid target");
  return {
    target,
    instructions: stringOrNull(workflow.instructions)?.trim() || "",
    mode: normalizeLiliaFixSuggestionMode(workflow.mode),
  };
}

export function normalizeLiliaBatchApplyWorkflow(value) {
  const workflow = isRecord(value) && value.type === LILIA_BATCH_APPLY_WORKFLOW_TYPE
    ? value
    : null;
  if (!workflow) return null;
  const sourceTurnId = stringOrNull(workflow.sourceTurnId)?.trim() || "";
  const sourceKind = stringOrNull(workflow.sourceKind);
  const sourceSummary = stringOrNull(workflow.sourceSummary)?.trim() || "";
  if (!sourceTurnId) throw new Error("Lilia batch apply workflow missing sourceTurnId");
  if (!isLiliaBatchApplySourceKind(sourceKind)) {
    throw new Error("Lilia batch apply workflow missing a valid sourceKind");
  }
  if (!sourceSummary) throw new Error("Lilia batch apply workflow missing sourceSummary");
  return {
    sourceTurnId,
    sourceKind,
    sourceSummary,
    instructions: stringOrNull(workflow.instructions)?.trim() || "",
  };
}

export function createLiliaReviewWorkflow(target, options = {}) {
  const normalizedTarget = normalizeLiliaReviewTarget(target);
  if (!normalizedTarget) throw new Error("Lilia review workflow missing a valid target");
  const workflow = {
    type: LILIA_REVIEW_WORKFLOW_TYPE,
    target: normalizedTarget,
    delivery: normalizeLiliaReviewDelivery(options.delivery),
  };
  const instructions = stringOrNull(options.instructions)?.trim() || "";
  if (instructions) workflow.instructions = instructions;
  return workflow;
}

export function createLiliaTaskWorkflow(kind, options = {}) {
  const normalized = normalizeLiliaTaskWorkflow({
    type: LILIA_TASK_WORKFLOW_TYPE,
    kind,
    instructions: options.instructions,
  });
  if (!normalized) throw new Error("Lilia task workflow missing a valid kind");
  const workflow = {
    type: LILIA_TASK_WORKFLOW_TYPE,
    kind: normalized.kind,
  };
  if (normalized.instructions) workflow.instructions = normalized.instructions;
  return workflow;
}

export function createLiliaFixSuggestionWorkflow(target, options = {}) {
  const normalizedTarget = normalizeLiliaReviewTarget(target);
  if (!normalizedTarget) throw new Error("Lilia fix suggestion workflow missing a valid target");
  const workflow = {
    type: LILIA_FIX_SUGGESTION_WORKFLOW_TYPE,
    target: normalizedTarget,
    mode: normalizeLiliaFixSuggestionMode(options.mode),
  };
  const instructions = stringOrNull(options.instructions)?.trim() || "";
  if (instructions) workflow.instructions = instructions;
  return workflow;
}

export function createLiliaBatchApplyWorkflow(input, options = {}) {
  const normalized = normalizeLiliaBatchApplyWorkflow({
    type: LILIA_BATCH_APPLY_WORKFLOW_TYPE,
    ...input,
    instructions: options.instructions,
  });
  if (!normalized) throw new Error("Lilia batch apply workflow missing sourceSummary");
  const workflow = {
    type: LILIA_BATCH_APPLY_WORKFLOW_TYPE,
    sourceTurnId: normalized.sourceTurnId,
    sourceKind: normalized.sourceKind,
    sourceSummary: normalized.sourceSummary,
  };
  if (normalized.instructions) workflow.instructions = normalized.instructions;
  return workflow;
}

export function normalizeLiliaGoalWorkflow(value) {
  const workflow = isRecord(value) && value.type === LILIA_GOAL_WORKFLOW_TYPE
    ? value
    : null;
  if (!workflow) return null;
  const action = stringOrNull(workflow.action);
  if (!isLiliaGoalAction(action)) {
    throw new Error("Lilia goal workflow missing a valid action");
  }
  const objective = stringOrNull(workflow.objective)?.trim() || "";
  if (goalRequiresObjectiveSet.has(action) && !objective) {
    throw new Error("Lilia goal workflow missing objective");
  }
  return {
    action,
    objective,
    status: normalizeLiliaGoalStatus(workflow.status),
    tokenBudget: numberOrNull(workflow.tokenBudget),
  };
}

export function createLiliaGoalWorkflow(action, options = {}) {
  if (!isLiliaGoalAction(action)) {
    throw new Error("Lilia goal workflow missing a valid action");
  }
  const objective = stringOrNull(options.objective)?.trim() || "";
  if (goalRequiresObjectiveSet.has(action) && !objective) {
    throw new Error("Lilia goal workflow missing objective");
  }
  const workflow = {
    type: LILIA_GOAL_WORKFLOW_TYPE,
    action,
  };
  if (objective) workflow.objective = objective;
  if (action === "set" || hasOwn(options, "status")) {
    workflow.status = normalizeLiliaGoalStatus(options.status);
  }
  if (action === "set" || hasOwn(options, "tokenBudget")) {
    workflow.tokenBudget = numberOrNull(options.tokenBudget);
  }
  return workflow;
}

export function createLiliaCompactWorkflow() {
  return { type: LILIA_COMPACT_WORKFLOW_TYPE };
}

export function normalizeLiliaMemoryModeWorkflow(value) {
  const workflow = isRecord(value) && value.type === LILIA_MEMORY_MODE_WORKFLOW_TYPE
    ? value
    : null;
  if (!workflow) return null;
  const mode = stringOrNull(workflow.mode);
  if (!isLiliaMemoryMode(mode)) {
    throw new Error("Lilia memory mode workflow missing a valid mode");
  }
  return { mode };
}

export function isLiliaMemoryResetWorkflow(value) {
  return isRecord(value) && value.type === LILIA_MEMORY_RESET_WORKFLOW_TYPE;
}

export function isLiliaCompactWorkflow(value) {
  return isRecord(value) && value.type === LILIA_COMPACT_WORKFLOW_TYPE;
}

export function isLiliaBackgroundTerminalsCleanWorkflow(value) {
  return isRecord(value) && value.type === LILIA_BACKGROUND_TERMINALS_CLEAN_WORKFLOW_TYPE;
}

export function normalizeLiliaConfigDiagnosticsWorkflow(value) {
  const workflow = isRecord(value) && value.type === LILIA_CONFIG_DIAGNOSTICS_WORKFLOW_TYPE
    ? value
    : null;
  if (!workflow) return null;
  return {
    includeLayers: typeof workflow.includeLayers === "boolean"
      ? workflow.includeLayers
      : DEFAULT_LILIA_CONFIG_DIAGNOSTICS_INCLUDE_LAYERS,
  };
}

function numberOrNull(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function stringOrNull(value) {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

function hasOwn(value, key) {
  return isRecord(value) && Object.prototype.hasOwnProperty.call(value, key);
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
