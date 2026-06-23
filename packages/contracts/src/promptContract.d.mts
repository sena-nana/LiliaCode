export interface PromptRunnerContract {
  attachmentIntro: string;
  attachmentUserMessageLabel: string;
}

export interface PromptClaudeContract {
  systemPromptPreset: string;
  windowsPlatformAppend: readonly string[];
  compactPrompt: string;
  workflows: Record<string, Record<string, string>>;
}

export interface PromptCodexContract {
  sparkDefaultInstruction: string;
  planRevision: Record<string, string>;
  planExecution: Record<string, string>;
  workflows: Record<string, Record<string, string>>;
}

export interface PromptAssistantContract {
  promptOptimize: {
    systemInstruction: string;
    requestInstruction: string;
    requirements: readonly string[];
  };
  autoTurnDecision: {
    systemInstruction: string;
    requestInstruction: string;
    tierPolicy: Record<"light" | "normal" | "deep", string>;
  };
}

export interface PromptSuggestionContract {
  systemInstruction: string;
  generationRules: readonly string[];
}

export interface PromptTitleContract {
  systemInstruction: string;
}

export interface PromptAutomationContract {
  defaultAgentPrompt: string;
  defaultHumanPrompt: string;
}

export interface PromptContract {
  runner: PromptRunnerContract;
  claude: PromptClaudeContract;
  codex: PromptCodexContract;
  assistant: PromptAssistantContract;
  suggestion: PromptSuggestionContract;
  title: PromptTitleContract;
  automation: PromptAutomationContract;
}

export const PROMPT_CONTRACT: Readonly<PromptContract>;
export const PROMPT_RUNNER: Readonly<PromptRunnerContract>;
export const PROMPT_CLAUDE: Readonly<PromptClaudeContract>;
export const PROMPT_CODEX: Readonly<PromptCodexContract>;
export const PROMPT_ASSISTANT: Readonly<PromptAssistantContract>;
export const PROMPT_SUGGESTION: Readonly<PromptSuggestionContract>;
export const PROMPT_TITLE: Readonly<PromptTitleContract>;
export const PROMPT_AUTOMATION: Readonly<PromptAutomationContract>;
export const DEFAULT_AUTOMATION_AGENT_PROMPT: string;
export const DEFAULT_AUTOMATION_HUMAN_PROMPT: string;
