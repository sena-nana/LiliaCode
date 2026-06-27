import type { CodexProfileSettings } from "./provider";
import type { CodexAccountQuotaStatus } from "./quota";

export type { CodexProfileSettings, CodexAccountQuotaStatus };

export interface CodexQuotaUnavailableInput {
  error?: unknown;
  connectionMode?: string;
  fetchedAt?: number;
}

export const DEFAULT_CODEX_PROFILE_SETTINGS: CodexProfileSettings;

export function normalizeCodexProfileSettings(
  input: Partial<CodexProfileSettings> | null | undefined,
  base?: CodexProfileSettings,
): CodexProfileSettings;

export function normalizeCodexAccountQuotaStatus(
  input: Partial<CodexAccountQuotaStatus> | null | undefined,
): CodexAccountQuotaStatus;

export function createCodexQuotaUnavailableStatus(
  input?: CodexQuotaUnavailableInput,
): CodexAccountQuotaStatus;

export const LiliaCodeCore: {
  normalizeCodexProfileSettings: typeof normalizeCodexProfileSettings;
  normalizeCodexAccountQuotaStatus: typeof normalizeCodexAccountQuotaStatus;
  createCodexQuotaUnavailableStatus: typeof createCodexQuotaUnavailableStatus;
};
