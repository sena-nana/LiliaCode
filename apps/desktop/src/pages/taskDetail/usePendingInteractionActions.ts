import type { Ref } from "vue";
import type {
  AgentTimelineEvent,
  AskUserSpec,
  ProjectArchitectureInteractionPayload,
} from "@lilia/contracts";
import {
  clearAskUsersForTask,
  resolveAskUserById,
  type PendingAsk,
} from "../../composables/useAskUser";
import { hydrateAgentAskUserRequest } from "../../composables/useAgentAskUserBridge";
import {
  clearAgentPendingInteractionsForTask,
  hydrateAgentPendingInteraction,
  respondMcpElicitation,
  respondPermissionApproval,
  type PendingAgentInteraction,
} from "../../composables/useAgentPendingInteractions";
import {
  clearProjectArchitectureInteractionsForTask,
  hydrateProjectArchitectureInteraction,
  respondProjectArchitectureChange,
  type PendingArchitectureChange,
} from "../../composables/useProjectArchitectureInteractions";
import { clearConversationRequiresAction } from "../../composables/useConversationActivity";
import type { PendingAgentActionResolution } from "../../composables/usePendingAgentActions";
import {
  clearToolConsentForTask,
  hydrateToolConsentRequest,
  respondConsent,
} from "../../composables/useToolConsentBridge";
import {
  respondTitleUpdate,
  type ToolConsentDecision,
  type ToolConsentRequest,
  type ToolConsentUpdatedInput,
} from "../../services/chat";
import type { ToolConsentRequest as ContractToolConsentRequest } from "@lilia/contracts";

export function usePendingInteractionActions(options: {
  taskId: () => string;
  pendingAskUser: Ref<PendingAsk | null>;
  pendingToolConsent: Ref<ToolConsentRequest | null>;
  pendingToolConsents: Ref<ToolConsentRequest[]>;
  pendingAgentInteractions: Ref<PendingAgentInteraction[]>;
  pendingArchitectureChanges: Ref<PendingArchitectureChange[]>;
}) {
  function onResolveAskUser(result: Parameters<typeof resolveAskUserById>[1]) {
    const ask = options.pendingAskUser.value;
    if (!ask) return;
    resolveAskUserById(ask.id, result);
  }

  async function onResolveToolConsent(
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ) {
    const request = options.pendingToolConsent.value;
    if (!request) return;
    try {
      await respondConsent(request.taskId, request.requestId, decision, message, updatedInput);
    } catch (err) {
      console.error("[tool-consent] respond failed", err);
    }
  }

  async function onResolvePendingAgentAction(resolution: PendingAgentActionResolution) {
    if (resolution.kind === "title_update") {
      try {
        await respondTitleUpdate(options.taskId(), resolution.requestId, resolution.decision);
      } catch (err) {
        console.error("[title-update] respond failed", err);
      }
      return;
    }
    if (resolution.kind === "tool_consent") {
      const request = options.pendingToolConsents.value.find(
        (item) => item.requestId === resolution.requestId,
      );
      if (!request) return;
      try {
        await respondConsent(
          request.taskId,
          request.requestId,
          resolution.decision,
          resolution.message,
          resolution.updatedInput,
        );
      } catch (err) {
        console.error("[tool-consent] respond failed", err);
      }
      return;
    }
    if (resolution.kind === "mcp_elicitation") {
      try {
        await respondMcpElicitation(
          options.taskId(),
          resolution.requestId,
          {
            action: resolution.action,
            ...(resolution.content ? { content: resolution.content } : {}),
          },
        );
      } catch (err) {
        console.error("[mcp-elicitation] respond failed", err);
      }
      return;
    }
    if (resolution.kind === "permission_approval") {
      const request = options.pendingAgentInteractions.value.find(
        (item) => item.kind === "permission_approval" && item.requestId === resolution.requestId,
      );
      if (!request || request.kind !== "permission_approval") return;
      try {
        await respondPermissionApproval(
          options.taskId(),
          resolution.requestId,
          resolution.decision === "allow"
            ? {
                action: "approve",
                grantedAccess: request.payload.requestedAccess,
                scope: request.payload.scopeSuggestion ?? "turn",
                providerContext: request.payload.providerContext,
              }
            : {
                action: "decline",
                grantedAccess: {},
                scope: request.payload.scopeSuggestion ?? "turn",
                strictAutoReview: true,
                providerContext: request.payload.providerContext,
              },
        );
      } catch (err) {
        console.error("[permission-approval] respond failed", err);
      }
      return;
    }
    if (resolution.kind === "architecture_change") {
      const request = options.pendingArchitectureChanges.value.find(
        (item) => item.requestId === resolution.requestId,
      );
      if (!request) return;
      try {
        await respondProjectArchitectureChange(request, resolution.decision);
      } catch (err) {
        console.error("[project-architecture] respond failed", err);
      }
      return;
    }
    resolveAskUserById(resolution.askId, resolution.result);
  }

  return {
    onResolveAskUser,
    onResolveToolConsent,
    onResolvePendingAgentAction,
  };
}

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
          requestedAccess: payload.requestedAccess ?? payload.permissions,
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
