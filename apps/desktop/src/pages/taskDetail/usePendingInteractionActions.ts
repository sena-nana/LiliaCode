import type {
  AgentTimelineEvent,
  AskUserSpec,
  ProjectArchitectureInteractionPayload,
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
import type { ToolConsentRequest as ContractToolConsentRequest } from "@lilia/contracts";

export function clearPendingInteractionsForTask(
  taskId: string,
  options: { turnId?: string | null; keepRequestIds?: Set<string> } = {},
) {
  clearAskUsersForTask(taskId, options);
  clearToolConsentForTask(taskId, options);
  clearAgentPendingInteractionsForTask(taskId, options);
  clearProjectArchitectureInteractionsForTask(taskId, options);
  if (!options.keepRequestIds && options.turnId === undefined) {
    clearConversationRequiresAction(taskId);
  }
}

export function hydratePendingInteractions(events: AgentTimelineEvent[], taskId: string): Set<string> {
  const activeRequestIds = new Set<string>();
  for (const event of events) {
    if (event.taskId !== taskId) continue;
    if (event.status !== "requires_action") continue;
    const payload = payloadRecord(event);
    if (event.kind === "ask_user") {
      const requestId = payloadString(event, "requestId");
      if (requestId) activeRequestIds.add(requestId);
      const spec = ((payload?.spec as AskUserSpec | undefined) ?? null) ?? {
        title: event.title,
        questions: Array.isArray(payload?.questions) ? payload.questions as AskUserSpec["questions"] : [],
      };
      hydrateAgentAskUserRequest({
        taskId: event.taskId,
        turnId: event.turnId ?? "",
        backend: event.backend,
        requestId: requestId ?? "",
        spec,
      }, "ask_user");
      continue;
    }
    if (event.kind === "plan") {
      const requestId = payloadString(event, "requestId");
      if (requestId) activeRequestIds.add(requestId);
      const spec: AskUserSpec = {
        title: event.title,
        source: "Agent",
        intent: "plan_approval",
        dismissable: true,
        questions: [
          {
            id: "approve-plan",
            header: "计划确认",
            question: "",
            mode: "confirm",
            confirmLabel: "按计划执行",
            cancelLabel: "先不执行",
          },
        ],
      };
      hydrateAgentAskUserRequest({
        taskId: event.taskId,
        turnId: event.turnId ?? "",
        backend: event.backend,
        requestId: requestId ?? "",
        spec,
      }, "plan_approval");
      continue;
    }
    if (event.kind === "title_update") continue;
    const interaction = payloadString(event, "interaction");
    const requestId = payloadString(event, "requestId");
    if (event.kind === "architecture_change") {
      if (!requestId || !payload) continue;
      activeRequestIds.add(requestId);
      hydrateProjectArchitectureInteraction({
        taskId: event.taskId,
        turnId: event.turnId,
        backend: event.backend,
        requestId,
        payload: payload as unknown as ProjectArchitectureInteractionPayload,
      });
      continue;
    }
    if (!interaction || !requestId || !payload) continue;
    activeRequestIds.add(requestId);
    if (interaction === "tool_consent") {
      const request: ContractToolConsentRequest = {
        taskId: event.taskId,
        turnId: event.turnId ?? "",
        backend: event.backend,
        requestId,
        toolName: typeof payload.toolName === "string" ? payload.toolName : "tool",
        input: payload.input && typeof payload.input === "object" && !Array.isArray(payload.input)
          ? payload.input as Record<string, unknown>
          : {},
        title: typeof payload.title === "string" ? payload.title : null,
        displayName: typeof payload.displayName === "string" ? payload.displayName : null,
        description: typeof payload.description === "string" ? payload.description : null,
        blockedPath: typeof payload.blockedPath === "string" ? payload.blockedPath : null,
        decisionReason: typeof payload.decisionReason === "string" ? payload.decisionReason : null,
        toolUseId: typeof payload.toolUseId === "string" ? payload.toolUseId : null,
        cwd: typeof payload.cwd === "string" ? payload.cwd : null,
        reason: typeof payload.reason === "string" ? payload.reason : null,
        commandActions: payload.commandActions,
      };
      hydrateToolConsentRequest(request);
      continue;
    }
    if (interaction === "mcp_elicitation") {
      const requestedSchema = payload.requestedSchema;
      hydrateAgentPendingInteraction({
        kind: "mcp_elicitation",
        taskId: event.taskId,
        turnId: event.turnId,
        requestId,
        payload: {
          threadId: typeof payload.threadId === "string" ? payload.threadId : "",
          turnId: typeof payload.turnId === "string" ? payload.turnId : event.turnId,
          serverName: typeof payload.serverName === "string" ? payload.serverName : "",
          mode: payload.mode === "url" ? "url" : "form",
          message: typeof payload.message === "string" ? payload.message : "",
          requestedSchema,
          url: typeof payload.url === "string" ? payload.url : undefined,
          elicitationId: typeof payload.elicitationId === "string" ? payload.elicitationId : undefined,
          _meta: payload._meta,
        },
      });
      continue;
    }
    if (interaction === "permission_approval") {
      hydrateAgentPendingInteraction({
        kind: "permission_approval",
        taskId: event.taskId,
        turnId: event.turnId,
        requestId,
        payload: {
          reason: typeof payload.reason === "string" ? payload.reason : null,
          requestedAccess: payload.requestedAccess ?? {},
          scopeSuggestion: payload.scopeSuggestion,
          providerContext: payload.providerContext && typeof payload.providerContext === "object" && !Array.isArray(payload.providerContext)
            ? payload.providerContext as Record<string, unknown>
            : undefined,
        },
      });
    }
  }
  return activeRequestIds;
}

function payloadRecord(event: AgentTimelineEvent): Record<string, unknown> | null {
  return event.payload && typeof event.payload === "object" && !Array.isArray(event.payload)
    ? event.payload as Record<string, unknown>
    : null;
}

function payloadString(event: AgentTimelineEvent, key: string): string | null {
  const payload = payloadRecord(event);
  const value = payload?.[key];
  return typeof value === "string" ? value : null;
}
