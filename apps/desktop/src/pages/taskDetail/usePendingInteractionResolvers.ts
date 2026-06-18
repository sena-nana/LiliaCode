import type { Ref } from "vue";
import {
  resolveAskUserById,
  type PendingAsk,
} from "../../composables/useAskUser";
import {
  respondConsent,
} from "../../composables/useToolConsentBridge";
import {
  respondMcpElicitation,
  respondPermissionApproval,
  type PendingAgentInteraction,
} from "../../composables/useAgentPendingInteractions";
import {
  respondProjectArchitectureChange,
  type PendingArchitectureChange,
} from "../../composables/useProjectArchitectureInteractions";
import type { PendingAgentActionResolution } from "../../composables/usePendingAgentActions";
import {
  respondTitleUpdate,
  type ToolConsentDecision,
  type ToolConsentRequest,
  type ToolConsentUpdatedInput,
} from "../../services/chat";

export function usePendingInteractionResolvers(options: {
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
