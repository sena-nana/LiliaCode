export type CodexJsonObject = Record<string, unknown>;

export interface CodexProfileSettings {
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

export const CODEX_REASONING_EFFORTS: readonly string[];
export const REASONING_EFFORTS: readonly string[];
export const BACKEND_REASONING_EFFORTS: Record<string, readonly string[]>;
export const CODEX_SETTINGS_PROFILES: readonly string[];
export const DEFAULT_CODEX_PROFILE_SETTINGS: CodexProfileSettings;

export function isReasoningEffort(value: unknown): boolean;
export function normalizeReasoningEffort(value: unknown): string | null;
export function reasoningEffortsForBackend(backend: string): readonly string[];
export function normalizeReasoningEffortForBackend(
  backend: string,
  value: unknown,
): string | null;
export function isCodexReasoningEffort(value: unknown): boolean;
export function normalizeCodexReasoningEffort(value: unknown): string | null;
export function isCodexSettingsProfile(value: unknown): boolean;
export function normalizeCodexSettingsProfile(value: unknown): string;
export function normalizeUniqueTrimmedStrings(value: unknown): string[];
export function normalizeCodexJsonObject(value: unknown): CodexJsonObject | null;
export function normalizeCodexProfileSettings(
  input: Partial<CodexProfileSettings> | null | undefined,
  base?: CodexProfileSettings,
): CodexProfileSettings;
