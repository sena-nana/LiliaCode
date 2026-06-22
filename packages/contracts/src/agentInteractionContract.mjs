import agentInteractionContract from "./agent-interaction-contract.json" with { type: "json" };

const manifest = deepFreeze(agentInteractionContract);

export const AGENT_INTERACTION_CONTRACT = manifest;
export const AGENT_INTERACTION_KINDS = manifest.agentInteractionKinds;
export const ASK_USER_INTERACTION_KIND = manifest.askUserInteractionKind;
export const PLAN_APPROVAL_INTERACTION_KIND = manifest.planApprovalInteractionKind;
export const ASK_USER_INTERACTION_KINDS = [
  ASK_USER_INTERACTION_KIND,
  PLAN_APPROVAL_INTERACTION_KIND,
];
export const TOOL_CONSENT_INTERACTION_KIND = manifest.toolConsentInteractionKind;
export const ARCHITECTURE_INTERACTION_KIND = manifest.architectureInteractionKind;
export const MCP_ELICITATION_INTERACTION_KIND = manifest.mcpElicitationInteractionKind;
export const PERMISSION_APPROVAL_INTERACTION_KIND = manifest.permissionApprovalInteractionKind;
export const AGENT_INTERACTION_GET_SETTINGS_COMMAND = manifest.commands.getSettings;
export const AGENT_INTERACTION_SET_SETTINGS_COMMAND = manifest.commands.setSettings;
export const AGENT_INTERACTION_LIST_SUBAGENTS_COMMAND = manifest.commands.listSubagents;
export const AGENT_INTERACTION_UPSERT_SUBAGENT_COMMAND = manifest.commands.upsertSubagent;
export const AGENT_INTERACTION_DELETE_SUBAGENT_COMMAND = manifest.commands.deleteSubagent;
export const RUNTIME_INTERACTION_KINDS = manifest.runtimeInteractionKinds;
export const AGENT_TIMELINE_PENDING_INTERACTIONS = manifest.agentTimelinePendingInteractions;
export const PENDING_AUTO_DECISION_KINDS = manifest.pendingAutoDecisionKinds;
export const PENDING_AUTO_DECISION_LABELS = manifest.pendingAutoDecisionLabels;
export const PLAN_APPROVAL_SPEC_DEFAULTS = manifest.planApprovalSpecDefaults;

const interactionKindSet = new Set(AGENT_INTERACTION_KINDS);
const askUserInteractionKindSet = new Set(ASK_USER_INTERACTION_KINDS);
const runtimeInteractionKindSet = new Set(RUNTIME_INTERACTION_KINDS);
const pendingAutoDecisionKindSet = new Set(PENDING_AUTO_DECISION_KINDS);

export function isAgentInteractionKind(value) {
  return typeof value === "string" && interactionKindSet.has(value);
}

export function isAskUserInteractionKind(value) {
  return typeof value === "string" && askUserInteractionKindSet.has(value);
}

export function isRuntimeInteractionKind(value) {
  return typeof value === "string" && runtimeInteractionKindSet.has(value);
}

export function isPendingAutoDecisionKind(value) {
  return typeof value === "string" && pendingAutoDecisionKindSet.has(value);
}

export function pendingAutoDecisionLabel(kind) {
  return PENDING_AUTO_DECISION_LABELS[kind];
}

export function normalizeMcpElicitationResult(value) {
  const row = recordOrEmpty(value);
  const action = row.action === "accept" || row.action === "decline" ? row.action : "cancel";
  const content = isRecord(row.content) ? row.content : null;
  return {
    action,
    content,
    _meta: row._meta ?? null,
  };
}

export function normalizePermissionApprovalResult(value) {
  const row = recordOrEmpty(value);
  const grantedAccess = isRecord(row.grantedAccess) ? row.grantedAccess : {};
  const action =
    row.action === "approve" || row.action === "decline" || row.action === "cancel"
      ? row.action
      : row.strictAutoReview === true
        ? "cancel"
        : Object.keys(grantedAccess).length > 0
          ? "approve"
          : "decline";
  return {
    action,
    grantedAccess,
    scope: row.scope || "turn",
    ...(typeof row.strictAutoReview === "boolean"
      ? { strictAutoReview: row.strictAutoReview }
      : {}),
  };
}

export function normalizeRuntimeInteractionResult(kind, result) {
  return kind === MCP_ELICITATION_INTERACTION_KIND
    ? normalizeMcpElicitationResult(result)
    : normalizePermissionApprovalResult(result);
}

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}

function isRecord(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function recordOrEmpty(value) {
  return isRecord(value) ? value : {};
}
