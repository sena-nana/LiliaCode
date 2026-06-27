import { CONNECTION_MODES } from "./chatBackendsContract.mjs";
import {
  DEFAULT_CODEX_PROFILE_SETTINGS,
  normalizeCodexProfileSettings,
} from "./providerRuntime.mjs";

const CONNECTION_MODE_SET = new Set(CONNECTION_MODES);

export { DEFAULT_CODEX_PROFILE_SETTINGS, normalizeCodexProfileSettings };

export const LiliaCodeCore = Object.freeze({
  normalizeCodexProfileSettings,
  normalizeCodexAccountQuotaStatus,
  createCodexQuotaUnavailableStatus,
});

export function normalizeCodexAccountQuotaStatus(input) {
  const row = recordValue(input) ?? {};
  return {
    available: booleanValue(row.available, false),
    connectionMode: normalizeConnectionMode(row.connectionMode),
    limitId: nullableText(row.limitId),
    limitName: nullableText(row.limitName),
    planType: nullableText(row.planType),
    rateLimitReachedType: nullableText(row.rateLimitReachedType),
    fiveHour: normalizeQuotaWindow(row.fiveHour),
    weekly: normalizeQuotaWindow(row.weekly),
    sparkFiveHour: normalizeQuotaWindow(row.sparkFiveHour),
    sparkWeekly: normalizeQuotaWindow(row.sparkWeekly),
    credits: normalizeQuotaCredits(row.credits),
    sparkCredits: normalizeQuotaCredits(row.sparkCredits),
    rateLimitResetCredits: normalizeResetCredits(row.rateLimitResetCredits),
    accountUsage: normalizeAccountUsage(row.accountUsage),
    usageError: nullableText(row.usageError),
    fetchedAt: nonNegativeNumber(row.fetchedAt) ?? Date.now(),
    error: nullableError(row.error),
  };
}

export function createCodexQuotaUnavailableStatus({
  error = null,
  connectionMode = "codex-account",
  fetchedAt = Date.now(),
} = {}) {
  return normalizeCodexAccountQuotaStatus({
    available: false,
    connectionMode,
    fetchedAt,
    error,
  });
}

function normalizeQuotaWindow(value) {
  const row = recordValue(value);
  if (!row) return null;
  const usedPercent = numberValue(row.usedPercent);
  if (usedPercent === null) return null;
  return {
    usedPercent: clampPercent(usedPercent),
    windowDurationMins: positiveInteger(row.windowDurationMins),
    resetsAt: positiveInteger(row.resetsAt),
  };
}

function normalizeQuotaCredits(value) {
  const row = recordValue(value);
  if (!row) return null;
  return {
    hasCredits: booleanValue(row.hasCredits, false),
    unlimited: booleanValue(row.unlimited, false),
    balance: nullableText(row.balance),
  };
}

function normalizeResetCredits(value) {
  const row = recordValue(value);
  if (!row) return null;
  return {
    availableCount: nonNegativeInteger(row.availableCount) ?? 0,
  };
}

function normalizeAccountUsage(value) {
  const row = recordValue(value);
  if (!row) return null;
  return {
    summary: normalizeAccountUsageSummary(row.summary),
    dailyUsageBuckets: normalizeDailyUsageBuckets(row.dailyUsageBuckets),
  };
}

function normalizeAccountUsageSummary(value) {
  const row = recordValue(value) ?? {};
  return {
    lifetimeTokens: nonNegativeInteger(row.lifetimeTokens),
    peakDailyTokens: nonNegativeInteger(row.peakDailyTokens),
    longestRunningTurnSec: nonNegativeInteger(row.longestRunningTurnSec),
    currentStreakDays: nonNegativeInteger(row.currentStreakDays),
    longestStreakDays: nonNegativeInteger(row.longestStreakDays),
  };
}

function normalizeDailyUsageBuckets(value) {
  if (!Array.isArray(value)) return null;
  const buckets = value
    .map((item) => {
      const row = recordValue(item);
      const startDate = nullableText(row?.startDate);
      if (!row || !startDate) return null;
      return {
        startDate,
        tokens: nonNegativeInteger(row.tokens) ?? 0,
      };
    })
    .filter(Boolean);
  return buckets.length > 0 ? buckets : null;
}

function normalizeConnectionMode(value) {
  return typeof value === "string" && CONNECTION_MODE_SET.has(value)
    ? value
    : "codex-account";
}

function recordValue(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : null;
}

function nullableText(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function nullableError(value) {
  if (value === null || value === undefined) return null;
  const text = String(value).trim();
  return text ? text : null;
}

function booleanValue(value, fallback) {
  return typeof value === "boolean" ? value : fallback;
}

function numberValue(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function nonNegativeNumber(value) {
  const number = numberValue(value);
  return number !== null && number >= 0 ? number : null;
}

function positiveInteger(value) {
  const number = numberValue(value);
  return number !== null && number > 0 ? Math.floor(number) : null;
}

function nonNegativeInteger(value) {
  const number = nonNegativeNumber(value);
  return number === null ? null : Math.floor(number);
}

function clampPercent(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}
