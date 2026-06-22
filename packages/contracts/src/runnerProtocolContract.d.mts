export interface RunnerRuntimeEventTypes {
  toolUse: "tool_use";
  todoList: "todo_list";
  timeline: "timeline";
  interactionRequest: "interaction_request";
  quotaUsageRequest: "quota_usage_request";
  contextUsage: "context_usage";
  done: "done";
  promptSuggestion: "prompt_suggestion";
  error: "error";
}

export interface RunnerControlMessageTypes {
  interactionResponse: "interaction_response";
  settingsUpdate: "settings_update";
  interruptTurn: "interrupt_turn";
  quotaUsageResult: "quota_usage_result";
  liliaIabResult: "lilia_iab_result";
}

export interface RunnerStdinPayloadKeys {
  backend: "backend";
  turn: "turn";
  workflow: "workflow";
  runtimeCommand: "runtimeCommand";
  runtimeOptions: "runtimeOptions";
  extensions: "extensions";
}

export interface RunnerStdinTurnKeys {
  cwd: "cwd";
  prompt: "prompt";
  attachments: "attachments";
  conversationReferences: "conversationReferences";
  model: "model";
  resumeSessionId: "resumeSessionId";
  planMode: "planMode";
  goalMode: "goalMode";
  permission: "permission";
}

export interface RunnerProtocolContract {
  runtimeEventTypes: RunnerRuntimeEventTypes;
  controlMessageTypes: RunnerControlMessageTypes;
  stdinPayloadKeys: RunnerStdinPayloadKeys;
  stdinTurnKeys: RunnerStdinTurnKeys;
}

export const RUNNER_PROTOCOL_CONTRACT: Readonly<RunnerProtocolContract>;
export const RUNNER_RUNTIME_EVENT_TYPES: Readonly<RunnerRuntimeEventTypes>;
export const RUNNER_CONTROL_MESSAGE_TYPES: Readonly<RunnerControlMessageTypes>;
export const RUNNER_STDIN_PAYLOAD_KEYS: Readonly<RunnerStdinPayloadKeys>;
export const RUNNER_STDIN_TURN_KEYS: Readonly<RunnerStdinTurnKeys>;
export const RUNNER_TOOL_USE_EVENT_TYPE: RunnerRuntimeEventTypes["toolUse"];
export const RUNNER_TODO_LIST_EVENT_TYPE: RunnerRuntimeEventTypes["todoList"];
export const RUNNER_TIMELINE_EVENT_TYPE: RunnerRuntimeEventTypes["timeline"];
export const RUNNER_INTERACTION_REQUEST_EVENT_TYPE: RunnerRuntimeEventTypes["interactionRequest"];
export const RUNNER_QUOTA_USAGE_REQUEST_EVENT_TYPE: RunnerRuntimeEventTypes["quotaUsageRequest"];
export const RUNNER_CONTEXT_USAGE_EVENT_TYPE: RunnerRuntimeEventTypes["contextUsage"];
export const RUNNER_DONE_EVENT_TYPE: RunnerRuntimeEventTypes["done"];
export const RUNNER_PROMPT_SUGGESTION_EVENT_TYPE: RunnerRuntimeEventTypes["promptSuggestion"];
export const RUNNER_ERROR_EVENT_TYPE: RunnerRuntimeEventTypes["error"];
export const RUNNER_INTERACTION_RESPONSE_CONTROL_TYPE: RunnerControlMessageTypes["interactionResponse"];
export const RUNNER_SETTINGS_UPDATE_CONTROL_TYPE: RunnerControlMessageTypes["settingsUpdate"];
export const RUNNER_INTERRUPT_TURN_CONTROL_TYPE: RunnerControlMessageTypes["interruptTurn"];
export const RUNNER_QUOTA_USAGE_RESULT_CONTROL_TYPE: RunnerControlMessageTypes["quotaUsageResult"];
export const RUNNER_LILIA_IAB_RESULT_CONTROL_TYPE: RunnerControlMessageTypes["liliaIabResult"];
