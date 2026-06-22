import type {
  AgentTimelineEvent,
  AgentTimelinePendingInteraction,
  AskUserInteractionKind,
  AskUserSpec,
} from "@lilia/contracts";
import {
  ARCHITECTURE_INTERACTION_KIND,
  ASK_USER_INTERACTION_KIND,
  agentTimelineActionDescriptor,
  agentTimelinePayloadRecord,
  createPlanApprovalAskUserSpec,
  MCP_ELICITATION_INTERACTION_KIND,
  readAgentTimelinePayloadString,
  normalizeAgentInteractionRequest,
  normalizeAskUserSpec,
  normalizeMcpElicitationPayload,
  normalizePermissionApprovalPayload,
  normalizeProjectArchitectureInteractionPayload,
  normalizeToolConsentRequestFromInteraction,
  PERMISSION_APPROVAL_INTERACTION_KIND,
  PLAN_APPROVAL_INTERACTION_KIND,
  TITLE_UPDATE_ACTION_KIND,
  TOOL_CONSENT_INTERACTION_KIND,
  timelineEventScopedKey,
} from "@lilia/contracts";
import {
  clearAskUsersForTask,
} from "../../composables/useAskUser";
import { hydrateAgentAskUserRequest } from "../../composables/useAgentAskUserBridge";
import {
  clearAgentPendingInteractionsForTask,
  hydrateAgentPendingInteraction,
} from "../../composables/useAgentPendingInteractions";
import {
  clearProjectArchitectureInteractionsForTask,
  hydrateProjectArchitectureInteraction,
} from "../../composables/useProjectArchitectureInteractions";
import { clearConversationRequiresAction } from "../../composables/useConversationActivity";
import {
  clearToolConsentForTask,
  hydrateToolConsentRequest,
} from "../../composables/useToolConsentBridge";
import {
  shouldClearPendingInteraction,
  type ClearPendingInteractionsOptions,
} from "../../composables/pendingInteractionClearOptions";

type AskUserHydrationRequest = Parameters<typeof hydrateAgentAskUserRequest>[0];
type ToolConsentHydrationRequest = Parameters<typeof hydrateToolConsentRequest>[0];
type AgentPendingInteractionHydrationRequest = Parameters<typeof hydrateAgentPendingInteraction>[0];
type ProjectArchitectureHydrationRequest = Parameters<typeof hydrateProjectArchitectureInteraction>[0];

export type PendingInteractionHydration =
  | {
      target: "ask_user";
      activeRequestId: string;
      interactionKind: AskUserInteractionKind;
      request: AskUserHydrationRequest;
    }
  | {
      target: "tool_consent";
      activeRequestId: string;
      request: ToolConsentHydrationRequest;
    }
  | {
      target: "agent_interaction";
      activeRequestId: string;
      request: AgentPendingInteractionHydrationRequest;
    }
  | {
      target: "architecture_change";
      activeRequestId: string;
      request: ProjectArchitectureHydrationRequest;
    };

interface RememberedTimelinePendingRequest {
  taskId: string;
  turnId: string | null;
  requestId: string;
}

const pendingInteractionRequestIdsByTimelineEventId = new Map<
  string,
  RememberedTimelinePendingRequest
>();

interface PendingInteractionRequestIdSource {
  requestId?: string | null;
}

export function pendingInteractionRequestIdsForSources(input: {
  asks: Iterable<PendingInteractionRequestIdSource>;
  toolConsents: Iterable<PendingInteractionRequestIdSource>;
  agentInteractions: Iterable<PendingInteractionRequestIdSource>;
  architectureChanges: Iterable<PendingInteractionRequestIdSource>;
}): Set<string> {
  const requestIds = new Set<string>();
  for (const items of [
    input.asks,
    input.toolConsents,
    input.agentInteractions,
    input.architectureChanges,
  ]) {
    for (const item of items) {
      if (item.requestId) requestIds.add(item.requestId);
    }
  }
  return requestIds;
}

export function pendingInteractionRequestIdsToKeepAfterLoad(input: {
  hydratedRequestIds: Iterable<string>;
  currentRequestIds: Iterable<string>;
  pendingBeforeLoadRequestIds: ReadonlySet<string>;
}): Set<string> {
  const keepRequestIds = new Set(input.hydratedRequestIds);
  for (const requestId of input.currentRequestIds) {
    if (!input.pendingBeforeLoadRequestIds.has(requestId)) {
      keepRequestIds.add(requestId);
    }
  }
  return keepRequestIds;
}

export function shouldClearConversationRequiresAction(
  options: ClearPendingInteractionsOptions,
): boolean {
  return !options.keepRequestIds && options.turnId === undefined && options.requestId === undefined;
}

export function clearPendingInteractionsForTask(
  taskId: string,
  options: ClearPendingInteractionsOptions = {},
) {
  clearAskUsersForTask(taskId, options);
  clearToolConsentForTask(taskId, options);
  clearAgentPendingInteractionsForTask(taskId, options);
  clearProjectArchitectureInteractionsForTask(taskId, options);
  clearRememberedPendingInteractionRequestIds(taskId, options);
  if (shouldClearConversationRequiresAction(options)) {
    clearConversationRequiresAction(taskId);
  }
}

function clearRememberedPendingInteractionRequestIds(
  taskId: string,
  options: ClearPendingInteractionsOptions = {},
) {
  for (const [eventId, remembered] of pendingInteractionRequestIdsByTimelineEventId) {
    if (!shouldClearPendingInteraction(remembered, taskId, options)) continue;
    pendingInteractionRequestIdsByTimelineEventId.delete(eventId);
  }
}

function askUserHydrationForTimelineEvent(
  event: AgentTimelineEvent,
  requestId: string,
): PendingInteractionHydration | null {
  const payload = agentTimelinePayloadRecord(event);
  const spec = normalizeAskUserSpec(payload?.spec) ?? normalizeAskUserSpec({
    title: event.title,
    questions: Array.isArray(payload?.questions)
      ? payload.questions as unknown as AskUserSpec["questions"]
      : [],
  });
  if (!spec) return null;
  return {
    target: "ask_user",
    activeRequestId: requestId,
    interactionKind: ASK_USER_INTERACTION_KIND,
    request: {
      taskId: event.taskId,
      turnId: event.turnId ?? "",
      backend: event.backend,
      requestId,
      spec,
    },
  };
}

function planApprovalHydrationForTimelineEvent(
  event: AgentTimelineEvent,
  requestId: string,
): PendingInteractionHydration | null {
  return {
    target: "ask_user",
    activeRequestId: requestId,
    interactionKind: PLAN_APPROVAL_INTERACTION_KIND,
    request: {
      taskId: event.taskId,
      turnId: event.turnId ?? "",
      backend: event.backend,
      requestId,
      spec: createPlanApprovalAskUserSpec({
        title: event.title,
        source: "Agent",
      }),
    },
  };
}

function architectureHydrationForTimelineEvent(
  event: AgentTimelineEvent,
  requestId: string,
): PendingInteractionHydration | null {
  const payload = agentTimelinePayloadRecord(event);
  if (!payload) return null;
  const architecturePayload = normalizeProjectArchitectureInteractionPayload(payload);
  if (!architecturePayload) return null;
  return {
    target: "architecture_change",
    activeRequestId: requestId,
    request: {
      taskId: event.taskId,
      turnId: event.turnId,
      backend: event.backend,
      requestId,
      payload: architecturePayload,
    },
  };
}

function toolConsentHydrationForTimelineEvent(
  event: AgentTimelineEvent,
  requestId: string,
  payload: Record<string, unknown>,
): PendingInteractionHydration | null {
  const interactionRequest = normalizeAgentInteractionRequest({
    taskId: event.taskId,
    turnId: event.turnId ?? "",
    backend: event.backend,
    requestId,
    kind: TOOL_CONSENT_INTERACTION_KIND,
    payload,
  });
  const request = interactionRequest
    ? normalizeToolConsentRequestFromInteraction(interactionRequest)
    : null;
  return request
    ? {
        target: "tool_consent",
        activeRequestId: requestId,
        request,
      }
    : null;
}

function runtimeInteractionHydrationForTimelineEvent(
  event: AgentTimelineEvent,
  interaction: AgentTimelinePendingInteraction,
  requestId: string,
): PendingInteractionHydration | null {
  const payload = agentTimelinePayloadRecord(event);
  if (!interaction || !payload) return null;
  if (interaction === TOOL_CONSENT_INTERACTION_KIND) {
    return toolConsentHydrationForTimelineEvent(event, requestId, payload);
  }
  if (interaction === MCP_ELICITATION_INTERACTION_KIND) {
    const mcpPayload = normalizeMcpElicitationPayload(payload);
    return mcpPayload
      ? {
          target: "agent_interaction",
          activeRequestId: requestId,
          request: {
            kind: MCP_ELICITATION_INTERACTION_KIND,
            taskId: event.taskId,
            turnId: event.turnId,
            requestId,
            payload: mcpPayload,
          },
        }
      : null;
  }
  if (interaction === PERMISSION_APPROVAL_INTERACTION_KIND) {
    const permissionPayload = normalizePermissionApprovalPayload(payload);
    return permissionPayload
      ? {
          target: "agent_interaction",
          activeRequestId: requestId,
          request: {
            kind: PERMISSION_APPROVAL_INTERACTION_KIND,
            taskId: event.taskId,
            turnId: event.turnId,
            requestId,
            payload: permissionPayload,
          },
        }
      : null;
  }
  return null;
}

export function pendingInteractionHydrationForTimelineEvent(
  event: AgentTimelineEvent,
  taskId: string,
): PendingInteractionHydration | null {
  if (event.taskId !== taskId) return null;
  const action = agentTimelineActionDescriptor(event);
  if (!action) return null;
  if (action.kind === ASK_USER_INTERACTION_KIND) {
    return askUserHydrationForTimelineEvent(event, action.requestId);
  }
  if (action.kind === PLAN_APPROVAL_INTERACTION_KIND) {
    return planApprovalHydrationForTimelineEvent(event, action.requestId);
  }
  if (action.kind === TITLE_UPDATE_ACTION_KIND) return null;
  if (action.kind === ARCHITECTURE_INTERACTION_KIND) {
    return architectureHydrationForTimelineEvent(event, action.requestId);
  }
  return runtimeInteractionHydrationForTimelineEvent(event, action.kind, action.requestId);
}

function applyPendingInteractionHydration(hydration: PendingInteractionHydration) {
  switch (hydration.target) {
    case "ask_user":
      hydrateAgentAskUserRequest(hydration.request, hydration.interactionKind);
      return;
    case "tool_consent":
      hydrateToolConsentRequest(hydration.request);
      return;
    case "agent_interaction":
      hydrateAgentPendingInteraction(hydration.request);
      return;
    case "architecture_change":
      hydrateProjectArchitectureInteraction(hydration.request);
      return;
  }
}

function rememberPendingInteractionHydration(
  event: AgentTimelineEvent,
  hydration: PendingInteractionHydration,
  options: { clearReplacedRequest?: boolean } = {},
) {
  const memoryKey = timelineEventScopedKey(event);
  const previous = pendingInteractionRequestIdsByTimelineEventId.get(memoryKey);
  if (
    options.clearReplacedRequest === true &&
    previous &&
    previous.taskId === event.taskId &&
    previous.requestId !== hydration.activeRequestId
  ) {
    clearPendingInteractionsForTask(event.taskId, { requestId: previous.requestId });
  }
  pendingInteractionRequestIdsByTimelineEventId.set(memoryKey, {
    taskId: event.taskId,
    turnId: event.turnId,
    requestId: hydration.activeRequestId,
  });
}

function clearInactivePendingInteractionForTimelineEvent(event: AgentTimelineEvent, taskId: string) {
  if (event.taskId !== taskId) return;
  const memoryKey = timelineEventScopedKey(event);
  const remembered = pendingInteractionRequestIdsByTimelineEventId.get(memoryKey);
  const requestId = readAgentTimelinePayloadString(event, "requestId") ??
    remembered?.requestId;
  pendingInteractionRequestIdsByTimelineEventId.delete(memoryKey);
  if (!requestId) return;
  clearPendingInteractionsForTask(taskId, { requestId });
}

export function hydratePendingInteractions(events: AgentTimelineEvent[], taskId: string): Set<string> {
  const activeRequestIds = new Set<string>();
  for (const event of events) {
    const hydration = pendingInteractionHydrationForTimelineEvent(event, taskId);
    if (!hydration) continue;
    activeRequestIds.add(hydration.activeRequestId);
    rememberPendingInteractionHydration(event, hydration);
    applyPendingInteractionHydration(hydration);
  }
  return activeRequestIds;
}

export function syncPendingInteractionsForTimelineEvents(
  events: readonly AgentTimelineEvent[],
  taskId: string,
): Set<string> {
  const activeRequestIds = new Set<string>();
  for (const event of events) {
    const hydration = pendingInteractionHydrationForTimelineEvent(event, taskId);
    if (!hydration) {
      clearInactivePendingInteractionForTimelineEvent(event, taskId);
      continue;
    }
    activeRequestIds.add(hydration.activeRequestId);
    rememberPendingInteractionHydration(event, hydration, { clearReplacedRequest: true });
    applyPendingInteractionHydration(hydration);
  }
  return activeRequestIds;
}
