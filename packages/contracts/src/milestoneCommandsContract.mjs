import milestoneCommandContract from "./milestone-command-contract.json" with { type: "json" };

const manifest = Object.freeze(milestoneCommandContract);

export const MILESTONE_COMMANDS_CONTRACT = manifest;
export const MILESTONE_LIST_COMMAND = manifest.commands.list;
export const MILESTONE_CREATE_COMMAND = manifest.commands.create;
export const MILESTONE_UPDATE_COMMAND = manifest.commands.update;
export const MILESTONE_DELETE_COMMAND = manifest.commands.delete;
export const MILESTONE_REORDER_COMMAND = manifest.commands.reorder;
export const MILESTONE_SET_TASKS_COMMAND = manifest.commands.setTasks;
