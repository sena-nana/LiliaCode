import promptContract from "./prompt-contract.json" with { type: "json" };

const manifest = Object.freeze(promptContract);

export const PROMPT_CONTRACT = manifest;
export const PROMPT_RUNNER = manifest.runner;
export const PROMPT_CLAUDE = manifest.claude;
export const PROMPT_CODEX = manifest.codex;
export const PROMPT_ASSISTANT = manifest.assistant;
export const PROMPT_SUGGESTION = manifest.suggestion;
export const PROMPT_TITLE = manifest.title;
export const PROMPT_AUTOMATION = manifest.automation;
export const DEFAULT_AUTOMATION_AGENT_PROMPT = manifest.automation.defaultAgentPrompt;
export const DEFAULT_AUTOMATION_HUMAN_PROMPT = manifest.automation.defaultHumanPrompt;
