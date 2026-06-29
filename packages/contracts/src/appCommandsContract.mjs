import appCommandContract from "./app-command-contract.json" with { type: "json" };

const manifest = Object.freeze(appCommandContract);

export const APP_COMMANDS_CONTRACT = manifest;
export const APP_RESTART_COMMAND = manifest.appRestartCommand;
