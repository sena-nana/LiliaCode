import systemCommandContract from "./system-command-contract.json" with { type: "json" };

const manifest = Object.freeze(systemCommandContract);

export const SYSTEM_COMMANDS_CONTRACT = manifest;
export const SYSTEM_OPEN_PATH_COMMAND = manifest.commands.openPath;
export const SYSTEM_OPEN_URL_COMMAND = manifest.commands.openUrl;
export const SYSTEM_OPEN_IN_VSCODE_COMMAND = manifest.commands.openInVSCode;
