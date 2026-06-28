import { computed, type ComputedRef } from "vue";
import type { AgentTimelineEvent } from "@lilia/contracts";
import type { PendingAsk } from "./useAskUser";
import type { PendingAgentInteraction } from "./useAgentPendingInteractions";
import type { PendingArchitectureChange } from "./useProjectArchitectureInteractions";
import type { ToolConsentRequest } from "../services/chat";
import {
  pendingAgentActionsForTask,
  type PendingAgentAction,
} from "./pendingAgentActions";

export function usePendingAgentActionsForTask(
  asks: ComputedRef<PendingAsk[]>,
  toolConsents: ComputedRef<ToolConsentRequest[]>,
  agentInteractions: ComputedRef<PendingAgentInteraction[]> = computed(() => []),
  timelineEvents: ComputedRef<AgentTimelineEvent[]> = computed(() => []),
  architectureChanges: ComputedRef<PendingArchitectureChange[]> = computed(() => []),
): ComputedRef<PendingAgentAction[]> {
  return computed(() =>
    pendingAgentActionsForTask({
      asks: asks.value,
      toolConsents: toolConsents.value,
      agentInteractions: agentInteractions.value,
      timelineEvents: timelineEvents.value,
      architectureChanges: architectureChanges.value,
    })
  );
}

