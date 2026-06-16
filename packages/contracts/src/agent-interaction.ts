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

export type McpElicitationAction = "accept" | "decline" | "cancel";

export interface McpElicitationPayload {
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

export interface McpElicitationResult {
  action: McpElicitationAction;
  content?: Record<string, unknown>;
}

export interface PermissionApprovalPayload {
  reason: string | null;
  requestedAccess: unknown;
  scopeSuggestion?: unknown;
  providerContext?: {
    codex?: {
      threadId: string;
      turnId: string;
      itemId: string;
      startedAtMs: number;
      cwd: string;
      permissions: unknown;
    };
    [provider: string]: unknown;
  };
}

export interface PermissionApprovalResult {
  action: "approve" | "decline" | "cancel";
  grantedAccess?: unknown;
  scope?: unknown;
  strictAutoReview?: boolean;
  providerContext?: unknown;
}

export type AgentInteractionPayloadByKind = {
  plan_approval: AskUserSpec;
  ask_user: AskUserSpec;
  architecture_change: ProjectArchitectureInteractionPayload;
  tool_consent: ToolConsentInteractionPayload;
  mcp_elicitation: McpElicitationPayload;
  permission_approval: PermissionApprovalPayload;
};

export type AgentInteractionResultByKind = {
  plan_approval: AskUserResult;
  ask_user: AskUserResult;
  architecture_change: ProjectArchitectureInteractionResult;
  tool_consent: ToolConsentResponsePayload;
  mcp_elicitation: McpElicitationResult;
  permission_approval: PermissionApprovalResult;
};

export type AgentInteractionRequest = {
  [K in AgentInteractionKind]: AgentInteractionRequestOf<K>;
}[AgentInteractionKind];

export type AgentInteractionResponse = {
  [K in AgentInteractionKind]: AgentInteractionResponseOf<K>;
}[AgentInteractionKind];

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
