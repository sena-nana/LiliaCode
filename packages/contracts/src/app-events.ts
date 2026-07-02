export {
  CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND,
  CLI_PROJECT_OPEN_EVENT_NAME,
  MAIN_NAVIGATE_EVENT_NAME,
  POPUP_NAVIGATE_EVENT_NAME,
} from "./appEventsContract.mjs";
export {
  POPUP_FOCUS_MAIN_COMMAND,
  POPUP_GET_WINDOW_SETTINGS_COMMAND,
  POPUP_OPEN_CHILD_QUESTION_COMMAND,
  POPUP_OPEN_CONVERSATION_STATUS_COMMAND,
  POPUP_OPEN_NEW_CHAT_COMMAND,
  POPUP_OPEN_TASK_COMMAND,
  POPUP_REMEMBER_LAST_PROJECT_COMMAND,
  POPUP_SET_WINDOW_SETTINGS_COMMAND,
  POPUP_TOGGLE_CONVERSATION_STATUS_COMMAND,
} from "./popupCommandsContract.mjs";

export interface AppNavigateEvent {
  route: string;
}

export interface CliProjectOpenEvent {
  projectId: string;
  cwd: string;
}

export function createAppNavigateEvent(route: string): AppNavigateEvent {
  return { route };
}

export function createCliProjectOpenEvent(
  projectId: string,
  cwd: string,
): CliProjectOpenEvent {
  return { projectId, cwd };
}
