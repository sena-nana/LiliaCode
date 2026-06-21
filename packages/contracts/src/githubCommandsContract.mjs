import githubCommandContract from "./github-command-contract.json" with { type: "json" };

const manifest = Object.freeze(githubCommandContract);

export const GITHUB_COMMANDS_CONTRACT = manifest;
export const GIT_CLONE_REPO_COMMAND = manifest.commands.gitCloneRepo;
export const GITHUB_GET_BINDING_STATUS_COMMAND = manifest.commands.getBindingStatus;
export const GITHUB_START_DEVICE_FLOW_COMMAND = manifest.commands.startDeviceFlow;
export const GITHUB_POLL_DEVICE_FLOW_COMMAND = manifest.commands.pollDeviceFlow;
export const GITHUB_UNBIND_COMMAND = manifest.commands.unbind;
export const GITHUB_LIST_REPOS_COMMAND = manifest.commands.listRepos;
export const GITHUB_CLONE_REPO_COMMAND = manifest.commands.cloneRepo;
