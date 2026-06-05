import type {
  AgentInteractionRequest,
  AskUserSpec,
  ToolConsentRequest,
} from "@lilia/contracts";
import { onAgentInteractionRequest } from "../services/chat";
import { handleAgentAskUserRequest, type AgentAskUserRequest } from "./useAgentAskUserBridge";
import { handleToolConsentRequest } from "./useToolConsentBridge";

let installed = false;
let unlistenAll: Array<() => void> = [];

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function recordOrEmpty(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function askRequestFromInteraction(req: AgentInteractionRequest): AgentAskUserRequest {
  return {
    taskId: req.taskId,
    turnId: req.turnId,
    backend: req.backend,
    requestId: req.requestId,
    spec: req.payload as AskUserSpec,
  };
}

function toolRequestFromInteraction(req: AgentInteractionRequest): ToolConsentRequest {
  const payload = recordOrEmpty(req.payload);
  return {
    taskId: req.taskId,
    turnId: req.turnId,
    backend: req.backend,
    requestId: req.requestId,
    toolName: stringOrNull(payload.toolName) || "tool",
    input: recordOrEmpty(payload.input),
    title: stringOrNull(payload.title),
    displayName: stringOrNull(payload.displayName),
    description: stringOrNull(payload.description),
    blockedPath: stringOrNull(payload.blockedPath),
    decisionReason: stringOrNull(payload.decisionReason),
    toolUseId: stringOrNull(payload.toolUseId) || stringOrNull(payload.toolUseID),
    additionalPermissions: payload.additionalPermissions,
    availableDecisions: Array.isArray(payload.availableDecisions)
      ? payload.availableDecisions.filter((item): item is string => typeof item === "string")
      : undefined,
    proposedExecpolicyAmendment: payload.proposedExecpolicyAmendment,
    proposedNetworkPolicyAmendments: payload.proposedNetworkPolicyAmendments,
    networkApprovalContext: payload.networkApprovalContext,
    cwd: stringOrNull(payload.cwd),
    reason: stringOrNull(payload.reason),
    commandActions: payload.commandActions,
  };
}

function handleInteraction(req: AgentInteractionRequest) {
  if (req.kind === "tool_consent") {
    handleToolConsentRequest(toolRequestFromInteraction(req));
    return;
  }
  void handleAgentAskUserRequest(askRequestFromInteraction(req), req.kind);
}

export async function installAgentInteractionBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  unlistenAll = [await onAgentInteractionRequest(handleInteraction)];
  return () => {
    for (const unlisten of unlistenAll) unlisten();
    unlistenAll = [];
    installed = false;
  };
}
