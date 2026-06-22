export type CodexJsonObject = Record<string, unknown>;

export interface CodexProfileSettingsCore {
  profile: string;
  model: string | null;
  reasoningEffort: string | null;
  runtimeWorkspaceRoots: string[];
  responsesApiClientMetadata: CodexJsonObject | null;
  additionalContext: string | null;
  persistExtendedHistory: boolean | null;
  initialTurnsPage: CodexJsonObject | null;
  excludeTurns: string[];
}

export interface ProviderHelpers {
  CODEX_REASONING_EFFORTS: readonly string[];
  CODEX_SETTINGS_PROFILES: readonly string[];
  DEFAULT_CODEX_PROFILE_SETTINGS: CodexProfileSettingsCore;
  isCodexReasoningEffort(value: unknown): boolean;
  normalizeCodexReasoningEffort(value: unknown): string | null;
  isCodexSettingsProfile(value: unknown): boolean;
  normalizeCodexSettingsProfile(value: unknown): string;
  normalizeUniqueTrimmedStrings(value: unknown): string[];
  normalizeCodexJsonObject(value: unknown): CodexJsonObject | null;
  normalizeCodexProfileSettings(
    input: Partial<CodexProfileSettingsCore> | null | undefined,
    base?: CodexProfileSettingsCore,
  ): CodexProfileSettingsCore;
}

export function createProviderHelpers(
  providerCodexJson: unknown,
  chatBackendsJson: unknown,
): ProviderHelpers;

export function normalizeUniqueTrimmedStrings(value: unknown): string[];
export function normalizeCodexJsonObject(value: unknown): CodexJsonObject | null;
