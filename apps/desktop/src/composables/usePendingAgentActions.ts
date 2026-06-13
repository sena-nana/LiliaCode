import { computed, type ComputedRef } from "vue";
import type { AgentTimelineEvent, AskUserResult } from "@lilia/contracts";
import type { PendingAsk } from "./useAskUser";
import type {
  PendingCodexInteraction,
  PendingCodexMcpElicitation,
  PendingCodexPermissionApproval,
} from "./useCodexPendingInteractions";
import type { PendingArchitectureChange } from "./useProjectArchitectureInteractions";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../services/chat";

export type PendingAgentActionKind =
  | "tool_consent"
  | "ask_user"
  | "plan_approval"
  | "mcp_elicitation"
  | "permission_approval"
  | "architecture_change"
  | "title_update";

export type PendingAgentAction =
  | {
      kind: "tool_consent";
      taskId: string;
      turnId: string | null;
      requestId: string;
      request: ToolConsentRequest;
    }
  | {
      kind: "ask_user" | "plan_approval";
      taskId: string | null;
      turnId: string | null;
      requestId: string | null;
      ask: PendingAsk;
    }
  | PendingCodexMcpElicitation
  | PendingCodexPermissionApproval
  | PendingArchitectureChange
  | {
      kind: "title_update";
      taskId: string;
      turnId: string | null;
      requestId: string;
      proposedTitle: string;
      previousTitle: string | null;
    };

export type PendingAgentActionResolution =
  | {
      kind: "tool_consent";
      requestId: string;
      decision: ToolConsentDecision;
      message?: string;
      updatedInput?: ToolConsentUpdatedInput;
    }
  | {
      kind: "ask_user" | "plan_approval";
      requestId: string | null;
      askId: number;
      result: AskUserResult;
    }
  | {
      kind: "mcp_elicitation";
      requestId: string;
      action: "accept" | "decline" | "cancel";
      content?: Record<string, unknown>;
    }
  | {
      kind: "permission_approval";
      requestId: string;
      decision: "allow" | "deny";
    }
  | {
      kind: "architecture_change";
      requestId: string;
      decision: "allow" | "deny";
    }
  | {
      kind: "title_update";
      requestId: string;
      decision: "accept" | "decline";
    };

export function usePendingAgentActionsForTask(
  asks: ComputedRef<PendingAsk[]>,
  toolConsents: ComputedRef<ToolConsentRequest[]>,
  codexInteractions: ComputedRef<PendingCodexInteraction[]> = computed(() => []),
  timelineEvents: ComputedRef<AgentTimelineEvent[]> = computed(() => []),
  architectureChanges: ComputedRef<PendingArchitectureChange[]> = computed(() => []),
): ComputedRef<PendingAgentAction[]> {
  return computed(() => [
    ...asks.value.map((ask): PendingAgentAction => ({
      kind: ask.spec.intent === "plan_approval" ? "plan_approval" : "ask_user",
      taskId: ask.taskId,
      turnId: ask.turnId,
      requestId: ask.requestId ?? null,
      ask,
    })),
    ...toolConsents.value.map((request): PendingAgentAction => ({
      kind: "tool_consent",
      taskId: request.taskId,
      turnId: request.turnId,
      requestId: request.requestId,
      request,
    })),
    ...codexInteractions.value,
    ...architectureChanges.value,
    ...timelineEvents.value.flatMap(titleUpdateActionForEvent),
  ]);
}

function titleUpdateActionForEvent(event: AgentTimelineEvent): PendingAgentAction[] {
  if (event.kind !== "title_update" || event.status !== "requires_action") return [];
  const requestId = readPayloadString(event, "requestId");
  const proposedTitle = readPayloadString(event, "proposedTitle");
  if (!requestId || !proposedTitle) return [];
  return [{
    kind: "title_update",
    taskId: event.taskId,
    turnId: event.turnId,
    requestId,
    proposedTitle,
    previousTitle: readPayloadString(event, "previousTitle"),
  }];
}

export function pendingActionForTimelineEvent(
  event: AgentTimelineEvent,
  actions: readonly PendingAgentAction[],
): PendingAgentAction | null {
  if (event.status !== "requires_action") return null;
  for (const action of actions) {
    if (action.kind === "title_update" && event.kind === "title_update") {
      if (readPayloadString(event, "requestId") === action.requestId) return action;
      continue;
    }
    if (action.kind === "plan_approval" && event.kind === "plan") {
      if (action.turnId && event.turnId === action.turnId) return action;
      continue;
    }
    if (action.kind === "ask_user" && event.kind === "ask_user") {
      if (action.requestId && readPayloadString(event, "requestId") === action.requestId) return action;
      continue;
    }
    if (action.kind === "tool_consent") {
      if (readPayloadString(event, "requestId") === action.requestId) return action;
      continue;
    }
    if (action.kind === "mcp_elicitation" && event.kind === "mcp") {
      if (readPayloadString(event, "requestId") === action.requestId) return action;
      continue;
    }
    if (action.kind === "permission_approval" && event.kind === "diagnostic") {
      if (readPayloadString(event, "requestId") === action.requestId) return action;
      continue;
    }
    if (action.kind === "architecture_change" && event.kind === "architecture_change") {
      if (readPayloadString(event, "requestId") === action.requestId) return action;
    }
  }
  return null;
}

export function timelineEventRequiresAgentAction(event: AgentTimelineEvent): boolean {
  if (event.status !== "requires_action") return false;
  if (event.kind === "title_update") return true;
  if (event.kind === "plan") return true;
  if (event.kind === "ask_user") return true;
  if (event.kind === "architecture_change") return true;
  if (readPayloadString(event, "interaction") === "mcp_elicitation") return true;
  if (readPayloadString(event, "interaction") === "permission_approval") return true;
  return readPayloadString(event, "interaction") === "tool_consent";
}

function readPayloadString(event: AgentTimelineEvent, key: string): string | null {
  const payload = event.payload;
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = (payload as Record<string, unknown>)[key];
  return typeof value === "string" ? value : null;
}
