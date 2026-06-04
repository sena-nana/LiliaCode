import type {
  AgentInteractionRequest,
  AskUserSpec,
  ToolConsentRequest,
} from "@lilia/contracts";
import {
  onAgentInteractionRequest,
  onAskUserRequest,
  onToolConsentRequest,
  type AgentAskUserRequest,
} from "../services/chat";
import { handleAgentAskUserRequest } from "./useAgentAskUserBridge";
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
  };
}

function handleInteraction(req: AgentInteractionRequest) {
  if (req.kind === "tool_consent") {
    handleToolConsentRequest(toolRequestFromInteraction(req), { unified: true });
    return;
  }
  void handleAgentAskUserRequest(askRequestFromInteraction(req), {
    unified: true,
    kind: req.kind,
  });
}

export async function installAgentInteractionBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  unlistenAll = await Promise.all([
    onAgentInteractionRequest(handleInteraction),
    onToolConsentRequest((req) => handleToolConsentRequest(req, { unified: false })),
    onAskUserRequest((req) => {
      void handleAgentAskUserRequest(req);
    }),
  ]);
  return () => {
    for (const unlisten of unlistenAll) unlisten();
    unlistenAll = [];
    installed = false;
  };
}
