import {
  DEFAULT_CODEX_PROFILE_SETTINGS as DEFAULT_CODEX_PROFILE_SETTINGS_IMPL,
  LiliaCodeCore as LiliaCodeCoreImpl,
  createCodexQuotaUnavailableStatus as createCodexQuotaUnavailableStatusImpl,
  normalizeCodexAccountQuotaStatus as normalizeCodexAccountQuotaStatusImpl,
  normalizeCodexProfileSettings as normalizeCodexProfileSettingsImpl,
} from "./liliaCodeCore.mjs";
import type { ConnectionMode, CodexProfileSettings } from "./provider";
import type { CodexAccountQuotaStatus } from "./quota";

export type LiliaCodeCoreCodexProfileSettings = CodexProfileSettings;
export type LiliaCodeCoreCodexQuotaStatus = CodexAccountQuotaStatus;

export interface LiliaCodeCoreCodexQuotaUnavailableInput {
  error?: unknown;
  connectionMode?: ConnectionMode;
  fetchedAt?: number;
}

export const LILIA_CODE_CORE_DEFAULT_CODEX_PROFILE_SETTINGS =
  DEFAULT_CODEX_PROFILE_SETTINGS_IMPL as CodexProfileSettings;

export const normalizeLiliaCodeCoreCodexProfileSettings =
  normalizeCodexProfileSettingsImpl as (
    input: Partial<CodexProfileSettings> | null | undefined,
    base?: CodexProfileSettings,
  ) => CodexProfileSettings;

export const normalizeLiliaCodeCoreCodexQuotaStatus =
  normalizeCodexAccountQuotaStatusImpl as (
    input: Partial<CodexAccountQuotaStatus> | null | undefined,
  ) => CodexAccountQuotaStatus;

export const createLiliaCodeCoreCodexQuotaUnavailableStatus =
  createCodexQuotaUnavailableStatusImpl as (
    input?: LiliaCodeCoreCodexQuotaUnavailableInput,
  ) => CodexAccountQuotaStatus;

export const LiliaCodeCore = LiliaCodeCoreImpl as {
  normalizeCodexProfileSettings: typeof normalizeLiliaCodeCoreCodexProfileSettings;
  normalizeCodexAccountQuotaStatus: typeof normalizeLiliaCodeCoreCodexQuotaStatus;
  createCodexQuotaUnavailableStatus: typeof createLiliaCodeCoreCodexQuotaUnavailableStatus;
};
