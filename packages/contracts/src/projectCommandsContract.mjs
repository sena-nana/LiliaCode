import projectCommandContract from "./project-command-contract.json" with { type: "json" };

const manifest = Object.freeze(projectCommandContract);

export const PROJECT_COMMANDS_CONTRACT = manifest;
export const PROJECT_LIST_COMMAND = manifest.commands.list;
export const PROJECT_DASHBOARD_LIST_COMMAND = manifest.commands.dashboardList;
export const PROJECT_GET_COMMAND = manifest.commands.get;
export const PROJECT_CREATE_COMMAND = manifest.commands.create;
export const PROJECT_RENAME_COMMAND = manifest.commands.rename;
export const PROJECT_REMOVE_COMMAND = manifest.commands.remove;
export const PROJECT_TOGGLE_PIN_COMMAND = manifest.commands.togglePin;
export const PROJECT_REORDER_COMMAND = manifest.commands.reorder;
export const PROJECT_GET_SETTINGS_COMMAND = manifest.commands.getSettings;
export const PROJECT_SET_SETTINGS_COMMAND = manifest.commands.setSettings;
