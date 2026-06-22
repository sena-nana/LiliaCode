import {
  agentTimelineActionDescriptor,
  type AgentTimelineEvent,
} from "@lilia/contracts";
import {
  pendingActionForTimelineEvent,
  type PendingAgentAction,
} from "../../composables/pendingAgentActions";

export interface TimelinePendingActionState {
  action: PendingAgentAction | null;
  expired: boolean;
}

export type TimelinePendingActionStateReader = (
  event: AgentTimelineEvent,
) => TimelinePendingActionState;

export function timelinePendingActionState(
  event: AgentTimelineEvent,
  actions: readonly PendingAgentAction[],
  showExpired: boolean | undefined,
): TimelinePendingActionState {
  const action = pendingActionForTimelineEvent(event, actions);
  return {
    action,
    expired: !action && showExpired === true && agentTimelineActionDescriptor(event) !== null,
  };
}

export function hasTimelinePendingActionState(state: TimelinePendingActionState): boolean {
  return state.action !== null || state.expired;
}
