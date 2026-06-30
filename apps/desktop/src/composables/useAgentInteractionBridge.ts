import {
  ARCHITECTURE_INTERACTION_KIND,
  isAskUserInteractionKind,
  normalizeToolConsentRequestFromInteraction,
  TOOL_CONSENT_INTERACTION_KIND,
  type AgentInteractionRequest,
  type AskUserInteractionKind,
} from "@lilia/contracts";
import { onAgentInteractionRequest } from "../services/chat";
import { installUnlistenFns, runUnlistenFns } from "@lilia/ui";
import { handleAgentAskUserRequest, type AgentAskUserRequest } from "./useAgentAskUserBridge";
import { handleAgentPendingInteractionRequest } from "./useAgentPendingInteractions";
import { handleProjectArchitectureInteractionRequest } from "./useProjectArchitectureInteractions";
import { handleToolConsentRequest } from "./useToolConsentBridge";

let installed = false;
let unlistenAll: Array<() => void> = [];

function askRequestFromInteraction(
  req: Extract<
    AgentInteractionRequest,
    { kind: AskUserInteractionKind }
  >,
): AgentAskUserRequest {
  return {
    taskId: req.taskId,
    turnId: req.turnId,
    backend: req.backend,
    requestId: req.requestId,
    spec: req.payload,
  };
}

function isAskUserInteractionRequest(
  req: AgentInteractionRequest,
): req is Extract<AgentInteractionRequest, { kind: AskUserInteractionKind }> {
  return isAskUserInteractionKind(req.kind);
}

function handleInteraction(req: AgentInteractionRequest) {
  if (handleAgentPendingInteractionRequest(req)) return;
  if (req.kind === ARCHITECTURE_INTERACTION_KIND) {
    handleProjectArchitectureInteractionRequest({
      taskId: req.taskId,
      turnId: req.turnId || null,
      backend: req.backend,
      requestId: req.requestId,
      payload: req.payload,
    });
    return;
  }
  if (req.kind === TOOL_CONSENT_INTERACTION_KIND) {
    const toolRequest = normalizeToolConsentRequestFromInteraction(req);
    if (toolRequest) handleToolConsentRequest(toolRequest);
    return;
  }
  if (isAskUserInteractionRequest(req)) {
    void handleAgentAskUserRequest(askRequestFromInteraction(req), req.kind);
  }
}

export async function installAgentInteractionBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  try {
    unlistenAll = await installUnlistenFns([
      () => onAgentInteractionRequest(handleInteraction),
    ]);
  } catch (err) {
    unlistenAll = [];
    installed = false;
    throw err;
  }
  return () => {
    runUnlistenFns(unlistenAll.splice(0).reverse());
    unlistenAll = [];
    installed = false;
  };
}

