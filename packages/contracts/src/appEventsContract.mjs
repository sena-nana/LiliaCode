import appEventsContract from "./app-events-contract.json" with { type: "json" };

const manifest = Object.freeze(appEventsContract);

export const APP_EVENTS_CONTRACT = manifest;
export const MAIN_NAVIGATE_EVENT_NAME = manifest.mainNavigateEventName;
export const POPUP_NAVIGATE_EVENT_NAME = manifest.popupNavigateEventName;
export const CLI_PROJECT_OPEN_EVENT_NAME = manifest.cliProjectOpenEventName;
export const CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND =
  manifest.cliProjectOpenConsumePendingCommand;
