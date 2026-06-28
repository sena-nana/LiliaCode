import type { Ref } from "vue";
import {
  ARCHITECTURE_INTERACTION_KIND,
  createMcpElicitationResult,
  createPermissionApprovalResult,
  isAskUserInteractionKind,
  MCP_ELICITATION_INTERACTION_KIND,
  PERMISSION_APPROVAL_INTERACTION_KIND,
  TITLE_UPDATE_ACTION_KIND,
  TOOL_CONSENT_INTERACTION_KIND,
  type AskUserInteractionKind,
} from "@lilia/contracts";
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
import {
  pendingAgentActionResolution,
  pendingAskAgentAction,
  type PendingAgentActionResolution,
} from "../../composables/pendingAgentActions";
import {
  respondTitleUpdate,
  type ToolConsentDecision,
  type ToolConsentRequest,
  type ToolConsentUpdatedInput,
} from "../../services/chat";

interface UsePendingInteractionResolversOptions {
  taskId: () => string;
  pendingAskUser: Ref<PendingAsk | null>;
  pendingAskUsers: Ref<PendingAsk[]>;
  pendingTitleUpdateRequestIds: Ref<ReadonlySet<string>>;
  pendingToolConsent: Ref<ToolConsentRequest | null>;
  pendingToolConsents: Ref<ToolConsentRequest[]>;
  pendingAgentInteractions: Ref<PendingAgentInteraction[]>;
  pendingArchitectureChanges: Ref<PendingArchitectureChange[]>;
}

interface PendingResolutionPlanContext {
  taskId: string;
  pendingAsks: readonly PendingAsk[];
  pendingAgentInteractions: readonly PendingAgentInteraction[];
  pendingArchitectureChanges: readonly PendingArchitectureChange[];
  pendingTitleUpdateRequestIds: ReadonlySet<string>;
  pendingToolConsents: readonly ToolConsentRequest[];
}

export type PendingResolutionPlan =
  | {
      label: "ask-user";
      target: "ask_user";
      askId: number;
      result: Parameters<typeof resolveAskUserById>[1];
    }
  | {
      label: "title-update";
      target: "title_update";
      taskId: string;
      requestId: string;
      decision: "accept" | "decline";
    }
  | {
      label: "tool-consent";
      target: "tool_consent";
      request: ToolConsentRequest;
      decision: ToolConsentDecision;
      message?: string;
      updatedInput?: ToolConsentUpdatedInput;
    }
  | {
      label: "mcp-elicitation";
      target: "mcp_elicitation";
      taskId: string;
      requestId: string;
      result: Parameters<typeof respondMcpElicitation>[2];
    }
  | {
      label: "permission-approval";
      target: "permission_approval";
      taskId: string;
      requestId: string;
      result: Parameters<typeof respondPermissionApproval>[2];
    }
  | {
      label: "project-architecture";
      target: "architecture_change";
      request: PendingArchitectureChange;
      decision: "allow" | "deny";
    };

function isAskUserActionResolution(
  resolution: PendingAgentActionResolution,
): resolution is Extract<PendingAgentActionResolution, { kind: AskUserInteractionKind }> {
  return isAskUserInteractionKind(resolution.kind);
}

function findByRequestId<T extends { requestId: string }>(
  items: readonly T[],
  requestId: string,
): T | null {
  return items.find((item) => item.requestId === requestId) ?? null;
}

function findAskResolutionTarget(
  asks: readonly PendingAsk[],
  resolution: Extract<PendingAgentActionResolution, { kind: AskUserInteractionKind }>,
): PendingAsk | null {
  if (resolution.requestId) {
    return asks.find((item) => item.requestId === resolution.requestId) ?? null;
  }
  const ask = asks.find((item) => item.id === resolution.askId) ?? null;
  if (!ask) return null;
  if (ask.requestId) return null;
  return ask;
}

function findPermissionApproval(
  interactions: readonly PendingAgentInteraction[],
  requestId: string,
): Extract<PendingAgentInteraction, { kind: typeof PERMISSION_APPROVAL_INTERACTION_KIND }> | null {
  const request = findByRequestId(interactions, requestId);
  return request?.kind === PERMISSION_APPROVAL_INTERACTION_KIND ? request : null;
}

function findMcpElicitation(
  interactions: readonly PendingAgentInteraction[],
  requestId: string,
): Extract<PendingAgentInteraction, { kind: typeof MCP_ELICITATION_INTERACTION_KIND }> | null {
  const request = findByRequestId(interactions, requestId);
  return request?.kind === MCP_ELICITATION_INTERACTION_KIND ? request : null;
}

export function pendingToolConsentsForResolution(
  active: ToolConsentRequest | null,
  requests: readonly ToolConsentRequest[],
): readonly ToolConsentRequest[] {
  if (!active) return requests;
  return requests.some((request) => request.requestId === active.requestId)
    ? requests
    : [active, ...requests];
}

async function runPendingResolution(
  label: string,
  respond: () => Promise<void>,
) {
  try {
    await respond();
  } catch (err) {
    console.error(`[${label}] respond failed`, err);
  }
}

export function pendingResolutionPlan(
  context: PendingResolutionPlanContext,
  resolution: PendingAgentActionResolution,
): PendingResolutionPlan | null {
  switch (resolution.kind) {
    case TITLE_UPDATE_ACTION_KIND:
      return context.pendingTitleUpdateRequestIds.has(resolution.requestId)
        ? {
            label: "title-update",
            target: "title_update",
            taskId: context.taskId,
            requestId: resolution.requestId,
            decision: resolution.decision,
          }
        : null;
    case TOOL_CONSENT_INTERACTION_KIND: {
      const request = findByRequestId(context.pendingToolConsents, resolution.requestId);
      return request
        ? {
            label: "tool-consent",
            target: "tool_consent",
            request,
            decision: resolution.decision,
            message: resolution.message,
            updatedInput: resolution.updatedInput,
          }
        : null;
    }
    case MCP_ELICITATION_INTERACTION_KIND: {
      const request = findMcpElicitation(context.pendingAgentInteractions, resolution.requestId);
      return request
        ? {
            label: "mcp-elicitation",
            target: "mcp_elicitation",
            taskId: context.taskId,
            requestId: resolution.requestId,
            result: createMcpElicitationResult(resolution.action, resolution.content),
          }
        : null;
    }
    case PERMISSION_APPROVAL_INTERACTION_KIND: {
      const request = findPermissionApproval(
        context.pendingAgentInteractions,
        resolution.requestId,
      );
      return request
        ? {
            label: "permission-approval",
            target: "permission_approval",
            taskId: context.taskId,
            requestId: resolution.requestId,
            result: createPermissionApprovalResult(request.payload, resolution.decision),
          }
        : null;
    }
    case ARCHITECTURE_INTERACTION_KIND: {
      const request = findByRequestId(
        context.pendingArchitectureChanges,
        resolution.requestId,
      );
      return request
        ? {
            label: "project-architecture",
            target: "architecture_change",
            request,
            decision: resolution.decision,
          }
        : null;
    }
    default:
      if (isAskUserActionResolution(resolution)) {
        const ask = findAskResolutionTarget(context.pendingAsks, resolution);
        return ask
          ? {
              label: "ask-user",
              target: "ask_user",
              askId: ask.id,
              result: resolution.result,
            }
          : null;
      }
      const exhaustive: never = resolution;
      return exhaustive;
  }
}

function currentPendingResolutionContext(
  options: UsePendingInteractionResolversOptions,
): PendingResolutionPlanContext {
  return {
    taskId: options.taskId(),
    pendingAsks: options.pendingAskUsers.value,
    pendingAgentInteractions: options.pendingAgentInteractions.value,
    pendingArchitectureChanges: options.pendingArchitectureChanges.value,
    pendingTitleUpdateRequestIds: options.pendingTitleUpdateRequestIds.value,
    pendingToolConsents: pendingToolConsentsForResolution(
      options.pendingToolConsent.value,
      options.pendingToolConsents.value,
    ),
  };
}

async function executePendingResolutionPlan(plan: PendingResolutionPlan) {
  switch (plan.target) {
    case "ask_user":
      resolveAskUserById(plan.askId, plan.result);
      return;
    case "title_update":
      await respondTitleUpdate(plan.taskId, plan.requestId, plan.decision);
      return;
    case "tool_consent":
      await respondConsent(
        plan.request.taskId,
        plan.request.requestId,
        plan.decision,
        plan.message,
        plan.updatedInput,
      );
      return;
    case "mcp_elicitation":
      await respondMcpElicitation(plan.taskId, plan.requestId, plan.result);
      return;
    case "permission_approval":
      await respondPermissionApproval(plan.taskId, plan.requestId, plan.result);
      return;
    case "architecture_change":
      await respondProjectArchitectureChange(plan.request, plan.decision);
      return;
  }
}

async function resolvePendingAgentActionResolution(
  options: UsePendingInteractionResolversOptions,
  resolution: PendingAgentActionResolution,
) {
  const plan = pendingResolutionPlan(currentPendingResolutionContext(options), resolution);
  if (!plan) return;
  await runPendingResolution(plan.label, () => executePendingResolutionPlan(plan));
}

export function usePendingInteractionResolvers(options: UsePendingInteractionResolversOptions) {
  async function onResolveAskUser(result: Parameters<typeof resolveAskUserById>[1]) {
    const ask = options.pendingAskUser.value;
    if (!ask) return;
    const resolution = pendingAgentActionResolution(pendingAskAgentAction(ask), {
      askResult: result,
    });
    if (!resolution) return;
    await resolvePendingAgentActionResolution(options, resolution);
  }

  async function onResolveToolConsent(
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ) {
    const request = options.pendingToolConsent.value;
    if (!request) return;
    await resolvePendingAgentActionResolution(options, {
      kind: TOOL_CONSENT_INTERACTION_KIND,
      requestId: request.requestId,
      decision,
      message,
      updatedInput,
    });
  }

  async function onResolvePendingAgentAction(resolution: PendingAgentActionResolution) {
    await resolvePendingAgentActionResolution(options, resolution);
  }

  return {
    onResolveAskUser,
    onResolveToolConsent,
    onResolvePendingAgentAction,
  };
}

