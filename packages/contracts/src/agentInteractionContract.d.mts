export type AgentInteractionKind =
  | "plan_approval"
  | "tool_consent"
  | "ask_user"
  | "architecture_change"
  | "mcp_elicitation"
  | "permission_approval";

export type RuntimeInteractionKind = "mcp_elicitation" | "permission_approval";

export type AskUserInteractionKind = "ask_user" | "plan_approval";

export type PendingAutoDecisionKind =
  | "ask_user"
  | "plan_approval"
  | "tool_consent"
  | "title_update"
  | "mcp_elicitation"
  | "architecture_change"
  | "permission_approval";

export interface PlanApprovalSpecDefaults {
  questionId: string;
  questionHeader: string;
  confirmLabel: string;
  cancelLabel: string;
  dismissable: boolean;
  titleTemplate: string;
  sourceTemplate: string;
}

export interface McpElicitationResult {
  action: "accept" | "decline" | "cancel";
  content?: Record<string, unknown> | null;
  _meta?: unknown;
}

export interface PermissionApprovalResult {
  action: "approve" | "decline" | "cancel";
  grantedAccess?: unknown;
  scope?: unknown;
  strictAutoReview?: boolean;
  providerContext?: unknown;
}

export const AGENT_INTERACTION_CONTRACT: {
  agentInteractionKinds: readonly AgentInteractionKind[];
  askUserInteractionKind: "ask_user";
  planApprovalInteractionKind: "plan_approval";
  toolConsentInteractionKind: "tool_consent";
  architectureInteractionKind: "architecture_change";
  mcpElicitationInteractionKind: "mcp_elicitation";
  permissionApprovalInteractionKind: "permission_approval";
  commands: {
    getSettings: "agent_interaction_get_settings";
    setSettings: "agent_interaction_set_settings";
    listSubagents: "agent_interaction_list_subagents";
    upsertSubagent: "agent_interaction_upsert_subagent";
    deleteSubagent: "agent_interaction_delete_subagent";
  };
  runtimeInteractionKinds: readonly RuntimeInteractionKind[];
  agentTimelinePendingInteractions: readonly ["tool_consent", "mcp_elicitation", "permission_approval"];
  pendingAutoDecisionKinds: readonly PendingAutoDecisionKind[];
  pendingAutoDecisionLabels: Readonly<Record<PendingAutoDecisionKind, string>>;
  planApprovalSpecDefaults: Readonly<PlanApprovalSpecDefaults>;
};

export const AGENT_INTERACTION_KINDS: readonly AgentInteractionKind[];
export const ASK_USER_INTERACTION_KIND: "ask_user";
export const PLAN_APPROVAL_INTERACTION_KIND: "plan_approval";
export const ASK_USER_INTERACTION_KINDS: readonly [
  "ask_user",
  "plan_approval",
];
export const TOOL_CONSENT_INTERACTION_KIND: "tool_consent";
export const ARCHITECTURE_INTERACTION_KIND: "architecture_change";
export const MCP_ELICITATION_INTERACTION_KIND: "mcp_elicitation";
export const PERMISSION_APPROVAL_INTERACTION_KIND: "permission_approval";
export const AGENT_INTERACTION_GET_SETTINGS_COMMAND: "agent_interaction_get_settings";
export const AGENT_INTERACTION_SET_SETTINGS_COMMAND: "agent_interaction_set_settings";
export const AGENT_INTERACTION_LIST_SUBAGENTS_COMMAND: "agent_interaction_list_subagents";
export const AGENT_INTERACTION_UPSERT_SUBAGENT_COMMAND: "agent_interaction_upsert_subagent";
export const AGENT_INTERACTION_DELETE_SUBAGENT_COMMAND: "agent_interaction_delete_subagent";
export const RUNTIME_INTERACTION_KINDS: readonly RuntimeInteractionKind[];
export const AGENT_TIMELINE_PENDING_INTERACTIONS: readonly ["tool_consent", "mcp_elicitation", "permission_approval"];
export const PENDING_AUTO_DECISION_KINDS: readonly PendingAutoDecisionKind[];
export const PENDING_AUTO_DECISION_LABELS: Readonly<Record<PendingAutoDecisionKind, string>>;
export const PLAN_APPROVAL_SPEC_DEFAULTS: Readonly<PlanApprovalSpecDefaults>;

export function isAgentInteractionKind(value: unknown): value is AgentInteractionKind;

export function isAskUserInteractionKind(value: unknown): value is AskUserInteractionKind;

export function isRuntimeInteractionKind(value: unknown): value is RuntimeInteractionKind;

export function isPendingAutoDecisionKind(value: unknown): value is PendingAutoDecisionKind;

export function pendingAutoDecisionLabel(kind: PendingAutoDecisionKind): string;

export function normalizeMcpElicitationResult(value: unknown): McpElicitationResult;

export function normalizePermissionApprovalResult(value: unknown): PermissionApprovalResult;

export function normalizeRuntimeInteractionResult(
  kind: RuntimeInteractionKind,
  result: unknown,
): McpElicitationResult | PermissionApprovalResult;
