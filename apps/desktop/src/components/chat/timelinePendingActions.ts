import type { AgentTimelineEvent } from "@lilia/contracts";
import {
  pendingActionForTimelineEvent,
  timelineEventRequiresAgentAction,
  type PendingAgentAction,
} from "../../composables/usePendingAgentActions";

export interface TimelinePendingActionState {
  action: PendingAgentAction | null;
  expired: boolean;
}

export function timelinePendingActionState(
  event: AgentTimelineEvent,
  actions: readonly PendingAgentAction[],
  showExpired: boolean | undefined,
): TimelinePendingActionState {
  const action = pendingActionForTimelineEvent(event, actions);
  return {
    action,
    expired: !action && showExpired === true && timelineEventRequiresAgentAction(event),
  };
}

export function hasTimelinePendingActionState(state: TimelinePendingActionState): boolean {
  return state.action !== null || state.expired;
}
