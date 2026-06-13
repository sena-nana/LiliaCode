import type { AskUserResult, AskUserSpec } from "./ask-user";
import type {
  ProjectArchitectureInteractionPayload,
  ProjectArchitectureInteractionResult,
} from "./architecture";
import type {
  ChatBackendKind,
  CodexToolConsentDecision,
  ToolConsentResponsePayload,
  ToolConsentUpdatedInput,
} from "./chat";

export type AgentInteractionKind =
  | "plan_approval"
  | "tool_consent"
  | "ask_user"
  | "architecture_change"
  | "mcp_elicitation"
  | "permission_approval";

export interface ToolConsentInteractionPayload {
  toolName: string;
  input: ToolConsentUpdatedInput;
  title?: string | null;
  displayName?: string | null;
  description?: string | null;
  blockedPath?: string | null;
  decisionReason?: string | null;
  toolUseId?: string | null;
  toolUseID?: string | null;
  backend?: ChatBackendKind;
  additionalPermissions?: unknown;
  availableDecisions?: CodexToolConsentDecision[];
  proposedExecpolicyAmendment?: unknown;
  proposedNetworkPolicyAmendments?: unknown;
  networkApprovalContext?: unknown;
  cwd?: string | null;
  reason?: string | null;
  commandActions?: unknown;
}

export type CodexMcpElicitationAction = "accept" | "decline" | "cancel";

export interface CodexMcpElicitationPayload {
  threadId: string;
  turnId: string | null;
  serverName: string;
  mode: "form" | "url";
  message: string;
  requestedSchema?: unknown;
  url?: string;
  elicitationId?: string;
  _meta?: unknown;
}

export interface CodexMcpElicitationResult {
  action: CodexMcpElicitationAction;
  content?: Record<string, unknown>;
}

export interface CodexPermissionApprovalPayload {
  threadId: string;
  turnId: string;
  itemId: string;
  startedAtMs: number;
  cwd: string;
  reason: string | null;
  permissions: unknown;
}

export interface CodexPermissionApprovalResult {
  permissions: unknown;
  scope: unknown;
  strictAutoReview?: boolean;
}

export type AgentInteractionPayloadByKind = {
  plan_approval: AskUserSpec;
  ask_user: AskUserSpec;
  architecture_change: ProjectArchitectureInteractionPayload;
  tool_consent: ToolConsentInteractionPayload;
  mcp_elicitation: CodexMcpElicitationPayload;
  permission_approval: CodexPermissionApprovalPayload;
};

export type AgentInteractionResultByKind = {
  plan_approval: AskUserResult;
  ask_user: AskUserResult;
  architecture_change: ProjectArchitectureInteractionResult;
  tool_consent: ToolConsentResponsePayload;
  mcp_elicitation: CodexMcpElicitationResult;
  permission_approval: CodexPermissionApprovalResult;
};

export type AgentInteractionRequest =
  | AgentInteractionRequestOf<"plan_approval">
  | AgentInteractionRequestOf<"tool_consent">
  | AgentInteractionRequestOf<"ask_user">
  | AgentInteractionRequestOf<"architecture_change">
  | AgentInteractionRequestOf<"mcp_elicitation">
  | AgentInteractionRequestOf<"permission_approval">;

export type AgentInteractionResponse =
  | AgentInteractionResponseOf<"plan_approval">
  | AgentInteractionResponseOf<"tool_consent">
  | AgentInteractionResponseOf<"ask_user">
  | AgentInteractionResponseOf<"architecture_change">
  | AgentInteractionResponseOf<"mcp_elicitation">
  | AgentInteractionResponseOf<"permission_approval">;

export interface AgentInteractionRequestOf<K extends AgentInteractionKind> {
  taskId: string;
  turnId: string;
  backend: ChatBackendKind;
  requestId: string;
  kind: K;
  payload: AgentInteractionPayloadByKind[K];
}

export interface AgentInteractionResponseOf<K extends AgentInteractionKind> {
  taskId: string;
  requestId: string;
  kind: K;
  result: AgentInteractionResultByKind[K];
}
