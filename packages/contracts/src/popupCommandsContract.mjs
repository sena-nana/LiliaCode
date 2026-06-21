import popupCommandContract from "./popup-command-contract.json" with { type: "json" };

const manifest = Object.freeze(popupCommandContract);

export const POPUP_COMMANDS_CONTRACT = manifest;
export const POPUP_GET_WINDOW_SETTINGS_COMMAND = manifest.commands.getWindowSettings;
export const POPUP_SET_WINDOW_SETTINGS_COMMAND = manifest.commands.setWindowSettings;
export const POPUP_REMEMBER_LAST_PROJECT_COMMAND =
  manifest.commands.rememberLastProject;
export const POPUP_OPEN_NEW_CHAT_COMMAND = manifest.commands.openNewChat;
export const POPUP_OPEN_TASK_COMMAND = manifest.commands.openTask;
export const POPUP_OPEN_CHILD_QUESTION_COMMAND = manifest.commands.openChildQuestion;
export const POPUP_OPEN_CONVERSATION_STATUS_COMMAND =
  manifest.commands.openConversationStatus;
export const POPUP_TOGGLE_CONVERSATION_STATUS_COMMAND =
  manifest.commands.toggleConversationStatus;
export const POPUP_FOCUS_MAIN_COMMAND = manifest.commands.focusMain;
