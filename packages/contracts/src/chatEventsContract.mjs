import chatEventsContract from "./chat-events-contract.json" with { type: "json" };

const manifest = Object.freeze(chatEventsContract);

export const CHAT_EVENTS_CONTRACT = manifest;
export const CHAT_TURN_STARTED_EVENT_NAME = manifest.chatTurnStartedEventName;
export const CHAT_DONE_EVENT_NAME = manifest.chatDoneEventName;
export const CHAT_CONTEXT_USAGE_EVENT_NAME = manifest.chatContextUsageEventName;
export const CHAT_AGENT_INTERACTION_REQUEST_EVENT_NAME =
  manifest.chatAgentInteractionRequestEventName;
export const AGENT_TIMELINE_EVENT_NAME = manifest.agentTimelineEventName;
export const AGENT_TIMELINE_BATCH_EVENT_NAME = manifest.agentTimelineBatchEventName;
