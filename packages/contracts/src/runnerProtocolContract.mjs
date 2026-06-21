import runnerProtocolContract from "./runner-protocol-contract.json" with { type: "json" };

const manifest = deepFreeze(runnerProtocolContract);

export const RUNNER_PROTOCOL_CONTRACT = manifest;
export const RUNNER_RUNTIME_EVENT_TYPES = manifest.runtimeEventTypes;
export const RUNNER_CONTROL_MESSAGE_TYPES = manifest.controlMessageTypes;
export const RUNNER_STDIN_PAYLOAD_KEYS = manifest.stdinPayloadKeys;
export const RUNNER_STDIN_TURN_KEYS = manifest.stdinTurnKeys;
export const RUNNER_TOOL_USE_EVENT_TYPE = manifest.runtimeEventTypes.toolUse;
export const RUNNER_TODO_LIST_EVENT_TYPE = manifest.runtimeEventTypes.todoList;
export const RUNNER_TIMELINE_EVENT_TYPE = manifest.runtimeEventTypes.timeline;
export const RUNNER_INTERACTION_REQUEST_EVENT_TYPE = manifest.runtimeEventTypes.interactionRequest;
export const RUNNER_QUOTA_USAGE_REQUEST_EVENT_TYPE = manifest.runtimeEventTypes.quotaUsageRequest;
export const RUNNER_CONTEXT_USAGE_EVENT_TYPE = manifest.runtimeEventTypes.contextUsage;
export const RUNNER_DONE_EVENT_TYPE = manifest.runtimeEventTypes.done;
export const RUNNER_PROMPT_SUGGESTION_EVENT_TYPE = manifest.runtimeEventTypes.promptSuggestion;
export const RUNNER_ERROR_EVENT_TYPE = manifest.runtimeEventTypes.error;
export const RUNNER_INTERACTION_RESPONSE_CONTROL_TYPE = manifest.controlMessageTypes.interactionResponse;
export const RUNNER_SETTINGS_UPDATE_CONTROL_TYPE = manifest.controlMessageTypes.settingsUpdate;
export const RUNNER_INTERRUPT_TURN_CONTROL_TYPE = manifest.controlMessageTypes.interruptTurn;
export const RUNNER_QUOTA_USAGE_RESULT_CONTROL_TYPE = manifest.controlMessageTypes.quotaUsageResult;
export const RUNNER_LILIA_IAB_RESULT_CONTROL_TYPE = manifest.controlMessageTypes.liliaIabResult;

function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  for (const child of Object.values(value)) {
    deepFreeze(child);
  }
  return Object.freeze(value);
}
