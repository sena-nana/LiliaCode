import type { AskUserResult, AskUserSpec } from "./ask-user";
import type {
  ChatBackendKind,
  ToolConsentResponsePayload,
  ToolConsentUpdatedInput,
} from "./chat";

export type AgentInteractionKind = "plan_approval" | "tool_consent" | "ask_user";

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
}

export type AgentInteractionPayloadByKind = {
  plan_approval: AskUserSpec;
  ask_user: AskUserSpec;
  tool_consent: ToolConsentInteractionPayload;
};

export type AgentInteractionResultByKind = {
  plan_approval: AskUserResult;
  ask_user: AskUserResult;
  tool_consent: ToolConsentResponsePayload;
};

export type AgentInteractionRequest =
  | AgentInteractionRequestOf<"plan_approval">
  | AgentInteractionRequestOf<"tool_consent">
  | AgentInteractionRequestOf<"ask_user">;

export type AgentInteractionResponse =
  | AgentInteractionResponseOf<"plan_approval">
  | AgentInteractionResponseOf<"tool_consent">
  | AgentInteractionResponseOf<"ask_user">;

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
