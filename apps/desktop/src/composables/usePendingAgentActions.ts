import { computed, type ComputedRef } from "vue";
import type { AgentTimelineEvent, AskUserResult } from "@lilia/contracts";
import type { PendingAsk } from "./useAskUser";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../services/chat";

export type PendingAgentActionKind = "tool_consent" | "ask_user" | "plan_approval";

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
    };

export function usePendingAgentActionsForTask(
  asks: ComputedRef<PendingAsk[]>,
  toolConsents: ComputedRef<ToolConsentRequest[]>,
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
  ]);
}

export function pendingActionForTimelineEvent(
  event: AgentTimelineEvent,
  actions: readonly PendingAgentAction[],
): PendingAgentAction | null {
  for (const action of actions) {
    if (action.kind === "plan_approval" && event.kind === "plan") {
      if (action.turnId && event.turnId === action.turnId) return action;
      continue;
    }
    if (action.kind === "ask_user" && event.kind === "ask_user") {
      if (matchesRequestId(event, action.requestId)) return action;
      if (!readEventRequestId(event) && action.turnId && event.turnId === action.turnId) return action;
      continue;
    }
    if (action.kind === "tool_consent") {
      if (matchesRequestId(event, action.requestId)) return action;
      if (
        !readEventRequestId(event) &&
        action.turnId &&
        event.turnId === action.turnId &&
        event.kind !== "plan" &&
        event.kind !== "ask_user" &&
        readPayloadString(event, "toolName") === action.request.toolName
      ) {
        return action;
      }
    }
  }
  return null;
}

export function timelineEventRequiresAgentAction(event: AgentTimelineEvent): boolean {
  if (event.status !== "requires_action") return false;
  if (event.kind === "plan") return true;
  if (event.kind === "ask_user") return true;
  return readPayloadString(event, "interaction") === "tool_consent" ||
    readPayloadBoolean(event, "permissionRequest") === true;
}

function matchesRequestId(event: AgentTimelineEvent, requestId: string | null): boolean {
  if (!requestId) return false;
  return readEventRequestId(event) === requestId;
}

function readEventRequestId(event: AgentTimelineEvent): string | null {
  return readPayloadString(event, "requestId") ||
    readPayloadString(event, "request_id");
}

function readPayloadString(event: AgentTimelineEvent, key: string): string | null {
  const payload = event.payload;
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = (payload as Record<string, unknown>)[key];
  return typeof value === "string" ? value : null;
}

function readPayloadBoolean(event: AgentTimelineEvent, key: string): boolean | null {
  const payload = event.payload;
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = (payload as Record<string, unknown>)[key];
  return typeof value === "boolean" ? value : null;
}
