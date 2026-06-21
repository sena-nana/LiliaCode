import type { CodexProfileSettings } from "./providerRuntime.mjs";
import type { RuntimePermissionMode } from "./permissionModes.mjs";

export interface AgentSubagentBackendSettings {
  enabled: boolean;
}

export interface ClaudeSubagentModeSettings extends AgentSubagentBackendSettings {
  forwardSubagentText: boolean;
  agentProgressSummaries: boolean;
}

export interface AgentSubagentModeSettings {
  enabled: boolean;
  codex: AgentSubagentBackendSettings;
  claude: ClaudeSubagentModeSettings;
}

export interface AutoTurnDecisionSettings {
  enabled: boolean;
  allowModelTier: boolean;
  allowReasoningEffort: boolean;
  allowPlanMode: boolean;
  allowGoalMode: boolean;
  allowSessionFork: boolean;
}

export type AutoTurnDecisionPermissionKey = Exclude<
  keyof AutoTurnDecisionSettings,
  "enabled"
>;

export interface AutoTurnDecisionPermissionOption {
  key: AutoTurnDecisionPermissionKey;
  label: string;
  ariaLabel: string;
}

export interface AgentInteractionSettings {
  nonInterruptMode: boolean;
  debug: boolean;
  permissionMode: RuntimePermissionMode;
  codexProfile: CodexProfileSettings;
  subagentMode: AgentSubagentModeSettings;
  autoTurnDecision: AutoTurnDecisionSettings;
}

export const AUTO_TURN_DECISION_PERMISSION_OPTIONS: readonly AutoTurnDecisionPermissionOption[];
export const AUTO_TURN_DECISION_PERMISSION_KEYS: readonly AutoTurnDecisionPermissionKey[];
export const DEFAULT_AGENT_SUBAGENT_MODE_SETTINGS: Readonly<AgentSubagentModeSettings>;
export const DEFAULT_AUTO_TURN_DECISION_SETTINGS: Readonly<AutoTurnDecisionSettings>;
export const DEFAULT_AGENT_INTERACTION_SETTINGS: Readonly<AgentInteractionSettings>;

export function normalizeAgentSubagentModeSettings(
  input: Partial<AgentSubagentModeSettings> | null | undefined,
  base?: AgentSubagentModeSettings,
): AgentSubagentModeSettings;

export function normalizeAutoTurnDecisionSettings(
  input: Partial<AutoTurnDecisionSettings> | null | undefined,
  base?: AutoTurnDecisionSettings,
): AutoTurnDecisionSettings;

export function normalizeAgentInteractionSettings(
  input: Partial<AgentInteractionSettings> | null | undefined,
  base?: AgentInteractionSettings,
): AgentInteractionSettings;
