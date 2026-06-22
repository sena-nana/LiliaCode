import memoryCommandContract from "./memory-command-contract.json" with { type: "json" };

const manifest = Object.freeze(memoryCommandContract);

export const MEMORY_COMMANDS_CONTRACT = manifest;
export const MEMORY_LIST_COMMAND = manifest.commands.list;
export const MEMORY_UPSERT_COMMAND = manifest.commands.upsert;
export const MEMORY_SET_ENABLED_COMMAND = manifest.commands.setEnabled;
export const MEMORY_DELETE_COMMAND = manifest.commands.delete;
export const MEMORY_GET_SETTINGS_COMMAND = manifest.commands.getSettings;
export const MEMORY_SET_SETTINGS_COMMAND = manifest.commands.setSettings;
export const MEMORY_GET_INJECTION_STATE_COMMAND =
  manifest.commands.getInjectionState;
export const MEMORY_SET_TASK_ENABLED_COMMAND = manifest.commands.setTaskEnabled;
export const MEMORY_RESET_TASK_COOLDOWN_COMMAND =
  manifest.commands.resetTaskCooldown;
