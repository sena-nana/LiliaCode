import { createCodexAppServer } from "./appServer.mjs";
import { initializeCodexAppServer } from "./runCodex.mjs";
import { isRecord, stringOrNull } from "../utils.mjs";

const FIVE_HOUR_MINUTES = 300;
const WEEKLY_MINUTES = 10080;
const FIVE_HOUR_TOLERANCE = 90;
const WEEKLY_TOLERANCE = 1440;
const RATE_LIMIT_READ_RETRIES = 2;
const RATE_LIMIT_READ_RETRY_DELAY_MS = 500;

function numberOrNull(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function normalizeCredits(value) {
  if (!isRecord(value)) return null;
  return {
    hasCredits: Boolean(value.hasCredits),
    unlimited: Boolean(value.unlimited),
    balance: stringOrNull(value.balance),
  };
}

function normalizeWindow(value) {
  if (!isRecord(value)) return null;
  const usedPercent = numberOrNull(value.usedPercent);
  if (usedPercent === null) return null;
  return {
    usedPercent,
    windowDurationMins: numberOrNull(value.windowDurationMins),
    resetsAt: numberOrNull(value.resetsAt),
  };
}

function durationNear(window, target, tolerance) {
  return typeof window?.windowDurationMins === "number" &&
    Math.abs(window.windowDurationMins - target) <= tolerance;
}

function normalizeLimitSnapshot(value) {
  if (!isRecord(value)) return null;
  const primary = normalizeWindow(value.primary);
  const secondary = normalizeWindow(value.secondary);
  const windows = [primary, secondary].filter(Boolean);
  const fiveHour = windows.find((window) =>
    durationNear(window, FIVE_HOUR_MINUTES, FIVE_HOUR_TOLERANCE)
  ) || primary;
  const weekly = windows.find((window) =>
    window !== fiveHour && durationNear(window, WEEKLY_MINUTES, WEEKLY_TOLERANCE)
  ) || (secondary !== fiveHour ? secondary : null);
  return {
    available: Boolean(fiveHour || weekly),
    connectionMode: "codex-account",
    limitId: stringOrNull(value.limitId),
    limitName: stringOrNull(value.limitName),
    planType: stringOrNull(value.planType),
    rateLimitReachedType: stringOrNull(value.rateLimitReachedType),
    fiveHour,
    weekly,
    credits: normalizeCredits(value.credits),
  };
}

function readLimitSnapshot(result, limitId) {
  if (!isRecord(result)) return null;
  const byLimitId = isRecord(result.rateLimitsByLimitId) ? result.rateLimitsByLimitId : null;
  if (byLimitId && isRecord(byLimitId[limitId])) return byLimitId[limitId];
  if (limitId === "codex" && isRecord(result.rateLimits)) return result.rateLimits;
  return null;
}

function isSparkLimit(key, value) {
  if (!isRecord(value)) return false;
  const text = [
    key,
    stringOrNull(value.limitId),
    stringOrNull(value.limitName),
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
  return text.includes("spark");
}

function readSparkLimitSnapshot(result) {
  if (!isRecord(result)) return null;
  const byLimitId = isRecord(result.rateLimitsByLimitId) ? result.rateLimitsByLimitId : null;
  if (!byLimitId) return null;
  if (isRecord(byLimitId.spark)) return byLimitId.spark;
  return Object.entries(byLimitId)
    .find(([key, value]) => isSparkLimit(key, value))?.[1] ?? null;
}

function normalizeRateLimitSnapshot(result) {
  const codex = normalizeLimitSnapshot(readLimitSnapshot(result, "codex"));
  const spark = normalizeLimitSnapshot(readSparkLimitSnapshot(result));
  if (!codex && !spark) return null;
  return {
    available: Boolean(codex?.available || spark?.available),
    connectionMode: "codex-account",
    limitId: codex?.limitId ?? null,
    limitName: codex?.limitName ?? null,
    planType: codex?.planType ?? spark?.planType ?? null,
    rateLimitReachedType: codex?.rateLimitReachedType ?? spark?.rateLimitReachedType ?? null,
    fiveHour: codex?.fiveHour ?? null,
    weekly: codex?.weekly ?? null,
    sparkFiveHour: spark?.fiveHour ?? null,
    sparkWeekly: spark?.weekly ?? null,
    credits: codex?.credits ?? null,
    sparkCredits: spark?.credits ?? null,
    fetchedAt: Date.now(),
    error: null,
  };
}

function sleep(ms) {
  return ms > 0 ? new Promise((resolve) => setTimeout(resolve, ms)) : Promise.resolve();
}

async function requestRateLimits(server, { retries = RATE_LIMIT_READ_RETRIES, retryDelayMs = RATE_LIMIT_READ_RETRY_DELAY_MS } = {}) {
  let lastError = null;
  for (let attempt = 0; attempt <= retries; attempt += 1) {
    try {
      return await server.request("account/rateLimits/read");
    } catch (err) {
      lastError = err;
      if (attempt >= retries) break;
      await sleep(retryDelayMs);
    }
  }
  throw lastError;
}

export async function readCodexAccountQuotaStatus(
  {
    createServer = createCodexAppServer,
    retries = RATE_LIMIT_READ_RETRIES,
    retryDelayMs = RATE_LIMIT_READ_RETRY_DELAY_MS,
  } = {},
) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const result = await requestRateLimits(server, { retries, retryDelayMs });
    const snapshot = normalizeRateLimitSnapshot(result);
    if (snapshot) return snapshot;
    return {
      available: false,
      connectionMode: "codex-account",
      limitId: null,
      limitName: null,
      planType: null,
      rateLimitReachedType: null,
      fiveHour: null,
      weekly: null,
      sparkFiveHour: null,
      sparkWeekly: null,
      credits: null,
      sparkCredits: null,
      fetchedAt: Date.now(),
      error: "Codex 官方额度接口未返回可识别的额度数据。",
    };
  } finally {
    server.close();
  }
}
