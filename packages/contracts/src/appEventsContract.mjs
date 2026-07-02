const manifest = Object.freeze({
  mainNavigateEventName: "lilia:main:navigate",
  popupNavigateEventName: "lilia:popup:navigate",
  cliProjectOpenEventName: "lilia:cli-project-open",
  cliProjectOpenConsumePendingCommand: "cli_project_open_consume_pending",
});

export const APP_EVENTS_CONTRACT = manifest;
export const MAIN_NAVIGATE_EVENT_NAME = manifest.mainNavigateEventName;
export const POPUP_NAVIGATE_EVENT_NAME = manifest.popupNavigateEventName;
export const CLI_PROJECT_OPEN_EVENT_NAME = manifest.cliProjectOpenEventName;
export const CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND =
  manifest.cliProjectOpenConsumePendingCommand;
